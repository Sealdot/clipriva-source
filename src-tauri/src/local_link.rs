//! Local Link native security and state boundary.
//!
//! Explicit opt-in starts a memory-only Bonjour/TCP runtime. Every pairing is
//! Noise-authenticated and requires bilateral SAS confirmation before a peer
//! key can become trusted; WebView input is never a trust anchor by itself.

mod discovery;
mod identity;
mod lifecycle;
#[cfg(target_os = "macos")]
mod lifecycle_macos;
mod protocol;
mod state_machine;

use std::collections::{HashMap, VecDeque};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, OptionalExtension, Row};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::db::{
    Database, LocalLinkCopyClaimOutcome, LocalLinkCopyEffectState, LocalLinkSaveFinalization,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    BeginLocalLinkPairingRequest, LocalLinkDevice, LocalLinkDeviceRePairingReason,
    LocalLinkDeviceTrustStatus, LocalLinkDiagnosticArchitecture, LocalLinkDiagnosticAvailability,
    LocalLinkDiagnosticOsFamily, LocalLinkDiagnosticPeer, LocalLinkDiagnosticTransfer,
    LocalLinkDiagnostics, LocalLinkPairingDetails, LocalLinkPreferences, LocalLinkReadinessAction,
    LocalLinkReadinessBlocker, LocalLinkReadinessCheck, LocalLinkReadinessSnapshot,
    LocalLinkReadinessStage, LocalLinkReadinessStageState, LocalLinkReadinessState,
    LocalLinkReceiveAction, LocalLinkTransfer, LocalLinkTransferDirection,
    LocalLinkTransferFailure, LocalLinkTransferStatus, LocalLinkTrustDuration,
};

use self::identity::{fingerprint, LocalIdentity};
use self::lifecycle::{
    LifecycleAction, LifecycleEvent, LifecycleSupervisor, NetworkPathState, SecureSuspensionReason,
    TransportHealth,
};
use self::protocol::{
    pairing_code, transcript_fingerprint, OfferDecision, PairingCommit, PairingConfirm,
    PairingProposal, PayloadOrCancel, SecureChannel, StatusResponseState, TransferOffer,
    WireMessage,
};
use self::state_machine::{ReceiptOutcome, ReceiptStateMachine, TerminalReceipt, TransferPhase};

pub(crate) const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
// v3 makes post-delivery status reconciliation explicit. Older peers fail
// version validation before any offer, query, or body is sent; there is no
// plaintext or downgrade fallback.
pub(crate) const PROTOCOL_VERSION: u16 = 3;
pub(crate) const MAX_TEXT_PAYLOAD_BYTES: usize = 256 * 1024;
pub(crate) const TRANSFER_TTL_SECONDS: i64 = 60;
/// A trusted peer must be verified again after this period. The expiry is
/// derived from `trusted_at`, so no new key, endpoint, or user-content data is
/// stored to support the policy.
pub(crate) const TRUST_DURATION_DAYS: i64 = 30;
const MAX_DEVICE_ID_LENGTH: usize = 128;
const MAX_DEVICE_NAME_LENGTH: usize = 80;
const MAX_TRANSFER_LIST_LIMIT: u32 = 20;
const TRANSFER_RETENTION_HOURS: i64 = 24;
const REPLAY_RETENTION_HOURS: i64 = 24;
const MAX_PENDING_PER_PEER: usize = 1;
const MAX_PENDING_GLOBAL: usize = 3;
const MAX_REQUESTS_PER_MINUTE: usize = 5;
const MAX_DIAGNOSTIC_RECORDS: usize = 50;
const LOCAL_LINK_DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;
// Bonjour records intentionally cannot identify a trusted peer. Project an
// online state only after an authenticated native session has identified it,
// and let that projection age out without adding a durable reachability log.
const RUNTIME_PRESENCE_TTL_SECONDS: i64 = 90;
const TRANSFER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DELIVERY_LIFECYCLE_IO_TIMEOUT: Duration = Duration::from_millis(500);
const STATUS_RECONCILIATION_DELAYS: [Duration; 5] = [
    Duration::from_millis(0),
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(10),
];
const STATUS_RECONCILIATION_BUDGET_MS: i64 = 20_000;
const STATUS_QUERY_IO_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_RECONCILIATION_ENDPOINTS: usize = 4;
const TERMINAL_EVIDENCE_GRACE_MS: i64 = 60_000;

/// Local Link plaintext is absent from this structure's public/debug surface.
/// `Zeroizing<String>` clears the owned process copy on every terminal path.
struct PendingInbound {
    peer_device_id: String,
    offer: TransferOffer,
    content: Option<Zeroizing<String>>,
    /// Set after the receiving person explicitly opens the request. This is
    /// in-memory coordination for a content-free wire acknowledgement; it is
    /// never derived from or coupled to the plaintext body.
    viewed: bool,
}

struct PairingSession {
    pairing_id: String,
    session_id: String,
    transcript_fingerprint: String,
    peer_fingerprint: String,
    sas: String,
    expires_at_ms: i64,
    confirm: Sender<PairingSignal>,
    local_confirmed: bool,
    trust_duration: Option<LocalLinkTrustDuration>,
}

/// A real session-only trust grant. The peer public key is held only in this
/// process and disappears when Local Link is disabled or the app exits; the
/// database retains no usable key for this option.
#[derive(Clone)]
struct SessionTrustedDevice {
    public_key: Vec<u8>,
    public_key_fingerprint: String,
    paired_at: String,
}

/// Native-only reachability evidence from a completed pairing or an
/// authenticated session. It contains neither an endpoint nor user content
/// and is deliberately cleared with the Local Link runtime.
#[derive(Clone)]
struct RuntimeDevicePresence {
    last_seen_at: String,
    observed_at_ms: i64,
}

enum PairingSignal {
    Confirm,
    Cancel,
}

/// Signals from the native receiver UI to the authenticated transport worker.
/// The payload is deliberately limited to a lifecycle event or a terminal
/// receipt; plaintext never crosses this channel.
enum InboundTransferSignal {
    Viewed,
    Terminal(TerminalReceipt),
}

/// A sender-side cancellation request. It carries no plaintext and only
/// exists while the authenticated delivery worker owns a live session.
enum OutgoingTransferSignal {
    Cancel,
}

/// Delivery failures before this boundary are proven not-delivered and may
/// try another discovered endpoint. Once PayloadComplete may have reached the
/// receiver, the sender must stop replaying plaintext and reconcile metadata.
#[derive(Debug, Clone, Copy)]
struct DeliveryFailure {
    reason: LocalLinkTransferFailure,
    receiver_may_have_payload: bool,
    reconciliation_endpoint: Option<SocketAddr>,
}

impl DeliveryFailure {
    fn before_payload(reason: LocalLinkTransferFailure) -> Self {
        Self {
            reason,
            receiver_may_have_payload: false,
            reconciliation_endpoint: None,
        }
    }

    fn after_payload(reason: LocalLinkTransferFailure) -> Self {
        Self {
            reason,
            receiver_may_have_payload: true,
            reconciliation_endpoint: None,
        }
    }

    fn at_endpoint(mut self, endpoint: SocketAddr) -> Self {
        self.reconciliation_endpoint = Some(endpoint);
        self
    }
}

/// Evidence returned by the native pasteboard write. The random marker is
/// supplied separately and the count contains no clipboard text.
pub(crate) struct ClipboardWriteEvidence {
    pub(crate) change_count: i64,
}

struct SessionHello {
    session_id: String,
    device_id: String,
    display_name: String,
}

#[derive(Default)]
struct RuntimeState {
    pending_inbound: HashMap<String, PendingInbound>,
    request_times: HashMap<String, VecDeque<i64>>,
    pairing_sessions: HashMap<String, PairingSession>,
    session_trusted_devices: HashMap<String, SessionTrustedDevice>,
    device_presence: HashMap<String, RuntimeDevicePresence>,
    receipt_waiters: HashMap<String, Sender<InboundTransferSignal>>,
    outgoing_cancel_waiters: HashMap<String, Sender<OutgoingTransferSignal>>,
    /// Transfer IDs currently owned by a metadata-only reconciliation worker.
    /// This is process-local coordination and never contains payload data.
    reconciliation_workers: HashMap<String, u64>,
    /// Known lifecycle suspensions pause the persisted active-time deadline.
    /// A generation change invalidates old workers before transport resumes.
    reconciliation_paused_at_ms: Option<i64>,
    receiver_available: bool,
    #[cfg(test)]
    drop_next_terminal_receipt: bool,
}

#[derive(Default)]
struct ReaperSignalState {
    stopped: bool,
    generation: u64,
}

#[derive(Default)]
struct ReaperSignal {
    state: Mutex<ReaperSignalState>,
    changed: Condvar,
}

struct DeadlineReaper {
    signal: Arc<ReaperSignal>,
    worker: Option<JoinHandle<()>>,
}

/// Enforces one absolute wall-clock bound across all blocking operations on a
/// status-query socket. Per-I/O timeouts alone are insufficient because Noise,
/// Hello and StatusResponse each perform separate reads/writes.
struct SocketDeadlineGuard {
    cancel: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl SocketDeadlineGuard {
    fn start(stream: &TcpStream, timeout: Duration) -> std::io::Result<Self> {
        let deadline_stream = stream.try_clone()?;
        let (cancel, cancelled) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("clipriva-local-link-socket-deadline".to_owned())
            .spawn(move || {
                if matches!(
                    cancelled.recv_timeout(timeout),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    let _ = deadline_stream.shutdown(Shutdown::Both);
                }
            })?;
        Ok(Self {
            cancel: Some(cancel),
            worker: Some(worker),
        })
    }
}

impl Drop for SocketDeadlineGuard {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl DeadlineReaper {
    fn start(
        database: Arc<Database>,
        runtime: Arc<Mutex<RuntimeState>>,
        receipts: Arc<Mutex<ReceiptStateMachine>>,
    ) -> AppResult<Self> {
        let signal = Arc::new(ReaperSignal::default());
        let worker_signal = Arc::clone(&signal);
        let worker = thread::Builder::new()
            .name("clipriva-local-link-deadline".to_owned())
            .spawn(move || deadline_reaper_loop(database, runtime, receipts, worker_signal))?;
        Ok(Self {
            signal,
            worker: Some(worker),
        })
    }

    fn notify(&self) {
        let mut state = self
            .signal
            .state
            .lock()
            .expect("Local Link reaper signal mutex poisoned");
        state.generation = state.generation.wrapping_add(1);
        self.signal.changed.notify_one();
    }

    fn stop(&mut self) {
        {
            let mut state = self
                .signal
                .state
                .lock()
                .expect("Local Link reaper signal mutex poisoned");
            state.stopped = true;
            state.generation = state.generation.wrapping_add(1);
            self.signal.changed.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for DeadlineReaper {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Runtime-only transport ownership. The listener and discovery handle are
/// created only after the persisted user opt-in succeeds and are both dropped
/// before Local Link is disabled/reset. No endpoint crosses this boundary into
/// SQLite or Tauri IPC.
struct TransportRuntime {
    stop: Arc<AtomicBool>,
    healthy: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    discovery: Option<discovery::DiscoveryRuntime>,
    #[cfg(test)]
    listener_addr: SocketAddr,
}

impl TransportRuntime {
    fn start(
        service: Weak<LocalLinkService>,
        pairing_visible: bool,
        generation: u64,
    ) -> AppResult<Self> {
        Self::start_inner(service, true, pairing_visible, 0, generation)
    }

    fn start_inner(
        service: Weak<LocalLinkService>,
        advertise: bool,
        pairing_visible: bool,
        port: u16,
        generation: u64,
    ) -> AppResult<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port)).map_err(transport_error)?;
        listener.set_nonblocking(true).map_err(transport_error)?;
        let listener_addr = listener.local_addr().map_err(transport_error)?;
        let discovery = if advertise {
            let mut discovery = discovery::DiscoveryRuntime::start(listener_addr.port())?;
            discovery.set_pairing_visible(pairing_visible)?;
            Some(discovery)
        } else {
            None
        };
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let healthy = Arc::new(AtomicBool::new(true));
        let worker_healthy = Arc::clone(&healthy);
        let worker = thread::Builder::new()
            .name("clipriva-local-link-listener".to_owned())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let Some(service) = service.upgrade() else {
                                break;
                            };
                            let _ = thread::Builder::new()
                                .name("clipriva-local-link-session".to_owned())
                                .spawn(move || service.handle_inbound_connection(stream));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => break,
                    }
                }
                worker_healthy.store(false, Ordering::Release);
                if !worker_stop.load(Ordering::Acquire) {
                    if let Some(service) = service.upgrade() {
                        service.handle_lifecycle_event(LifecycleEvent::TransportWorkerDied {
                            generation,
                        });
                    }
                }
            })
            .map_err(transport_error)?;
        Ok(Self {
            stop,
            healthy,
            worker: Some(worker),
            discovery,
            #[cfg(test)]
            listener_addr,
        })
    }

    fn candidates(&self) -> Vec<discovery::DiscoveryCandidate> {
        self.discovery
            .as_ref()
            .map(discovery::DiscoveryRuntime::candidates)
            .unwrap_or_default()
    }

    fn endpoints(&self) -> Vec<SocketAddr> {
        self.discovery
            .as_ref()
            .map(discovery::DiscoveryRuntime::endpoints)
            .unwrap_or_default()
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }

    fn set_pairing_visible(&mut self, visible: bool) -> AppResult<()> {
        if let Some(discovery) = self.discovery.as_mut() {
            discovery.set_pairing_visible(visible)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn listener_addr(&self) -> SocketAddr {
        self.listener_addr
    }
}

impl Drop for TransportRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // DiscoveryRuntime's Drop unregisters before its worker is joined.
        self.discovery.take();
        self.healthy.store(false, Ordering::Release);
    }
}

fn transport_error(_error: impl std::fmt::Display) -> AppError {
    AppError::InvalidInput(
        "Local Link could not start its private listener; recovery will retry while the feature remains enabled."
            .to_owned(),
    )
}

/// Native Local Link service. Network endpoints and pending plaintext are
/// process-only. The long-term private identity is lazy-loaded after the user
/// explicitly enables Local Link and remains in macOS Keychain.
pub struct LocalLinkService {
    database: Arc<Database>,
    identity: Arc<Mutex<Option<Arc<LocalIdentity>>>>,
    runtime: Arc<Mutex<RuntimeState>>,
    receipts: Arc<Mutex<ReceiptStateMachine>>,
    deadline_reaper: DeadlineReaper,
    transport: Mutex<Option<TransportRuntime>>,
    lifecycle: Mutex<LifecycleSupervisor>,
    /// Serializes reducer actions with the events they produce. Keeping this
    /// separate from the reducer mutex lets transport startup call back into
    /// the reducer without allowing an older StartTransport action to run
    /// after a newer sleep/disable event.
    lifecycle_execution: Mutex<()>,
    #[cfg(target_os = "macos")]
    lifecycle_monitor: Mutex<Option<lifecycle_macos::MacOsLifecycleMonitor>>,
    self_weak: Weak<LocalLinkService>,
}

impl LocalLinkService {
    pub fn new(database: Arc<Database>) -> AppResult<Arc<Self>> {
        database.with_local_link_connection(|connection| {
            let timestamp = now();
            let reconciliation_deadline = timestamp_from_millis(
                Utc::now()
                    .timestamp_millis()
                    .saturating_add(STATUS_RECONCILIATION_BUDGET_MS),
            );
            expire_trusted_devices_with_connection(connection, Utc::now())?;
            connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'failed', failure_reason = 'payloadUnavailable',
                     updated_at = ?1, completed_at = ?1, expires_at = NULL
                 WHERE (direction = 'incoming'
                        AND status IN ('encrypted', 'awaitingReceiver', 'viewed')
                        AND NOT EXISTS (
                            SELECT 1 FROM local_link_effect_claims claims
                            WHERE claims.transfer_id = local_link_transfers.id
                              AND claims.state = 'prepared'
                        ))
                    OR (direction = 'outgoing'
                        AND status IN ('connecting', 'encrypted'))",
                params![timestamp],
            )?;
            // An outgoing request that reached the delivery boundary has no
            // recoverable body after restart. Preserve only enough metadata to
            // query the authenticated receiver; never downgrade this to an
            // automatic payload retry.
            connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'reconciling', failure_reason = NULL,
                     updated_at = ?1, completed_at = NULL,
                     expires_at = COALESCE(expires_at, ?2)
                 WHERE direction = 'outgoing'
                   AND status IN ('awaitingReceiver', 'viewed')",
                params![timestamp, reconciliation_deadline],
            )?;
            connection.execute(
                "UPDATE local_link_transfers
                 SET expires_at = ?1
                 WHERE direction = 'outgoing' AND status = 'reconciling'
                   AND expires_at IS NULL",
                [reconciliation_deadline],
            )?;
            prune_transfer_summaries(connection)?;
            prune_replay_tombstones(connection)?;
            Ok(())
        })?;
        let runtime = Arc::new(Mutex::new(RuntimeState {
            receiver_available: true,
            ..RuntimeState::default()
        }));
        let receipts = Arc::new(Mutex::new(ReceiptStateMachine::default()));
        let deadline_reaper = DeadlineReaper::start(
            Arc::clone(&database),
            Arc::clone(&runtime),
            Arc::clone(&receipts),
        )?;
        let service = Arc::new_cyclic(|self_weak| Self {
            database,
            identity: Arc::new(Mutex::new(None)),
            runtime,
            receipts,
            deadline_reaper,
            transport: Mutex::new(None),
            lifecycle: Mutex::new(LifecycleSupervisor::new(false)),
            lifecycle_execution: Mutex::new(()),
            #[cfg(target_os = "macos")]
            lifecycle_monitor: Mutex::new(None),
            self_weak: self_weak.clone(),
        });
        // A saved enabled preference is prior explicit consent. Lifecycle
        // startup keeps that preference intact across a transient bind/path
        // failure and moves transport through its bounded recovery schedule.
        let restored_preferences = service.preferences()?;
        if restored_preferences.enabled {
            service.handle_lifecycle_event(LifecycleEvent::PreferenceChanged { enabled: true });
        }
        Ok(service)
    }

    pub fn preferences(&self) -> AppResult<LocalLinkPreferences> {
        self.database
            .with_local_link_connection(local_link_preferences_with_connection)
    }

    /// Return a finite, read-only projection of the first Local Link blocker.
    /// In particular, this method never starts or restarts transport. When the
    /// feature is off, local-network access remains explicitly `notChecked`.
    pub fn readiness_snapshot(&self) -> AppResult<LocalLinkReadinessSnapshot> {
        let preferences = self.preferences()?;
        let transport_running = self
            .transport
            .lock()
            .expect("Local Link transport mutex poisoned")
            .as_ref()
            .is_some_and(TransportRuntime::is_healthy);
        if !preferences.enabled || !transport_running || !preferences.discovery_enabled {
            return Ok(derive_readiness_snapshot(
                &preferences,
                transport_running,
                &[],
            ));
        }
        let devices = self.readiness_devices()?;
        Ok(derive_readiness_snapshot(
            &preferences,
            transport_running,
            &devices,
        ))
    }

    /// Build an ephemeral, allow-listed support bundle. Existing models are
    /// projected into new values rather than serialized directly so peer
    /// labels, stable IDs, fingerprints and other prohibited fields cannot be
    /// added accidentally through a future DTO change.
    pub fn build_diagnostics(&self) -> AppResult<LocalLinkDiagnostics> {
        let preferences = self.preferences()?;
        let mut readiness = self.readiness_snapshot()?;
        // A readiness action may target the opaque device ID used by another
        // named command. Diagnostics use per-export pseudonyms instead.
        readiness.action_device_id = None;
        let devices = self.readiness_devices()?;
        let transfers = self.list_transfers(MAX_TRANSFER_LIST_LIMIT)?;
        let export_salt = uuid::Uuid::new_v4().to_string();
        let peer_ref = |device_id: &str| diagnostic_peer_ref(&export_salt, device_id);

        let mut diagnostic_transfers = transfers
            .into_iter()
            .take(MAX_DIAGNOSTIC_RECORDS)
            .map(|transfer| LocalLinkDiagnosticTransfer {
                peer_ref: peer_ref(&transfer.device_id),
                direction: transfer.direction,
                status: transfer.status,
                failure_reason: transfer.failure_reason,
                recovery_action: transfer.recovery_action,
                duration_ms: diagnostic_duration_ms(&transfer.created_at, &transfer.updated_at),
                created_at: transfer.created_at,
                updated_at: transfer.updated_at,
                completed_at: transfer.completed_at,
            })
            .collect::<Vec<_>>();
        diagnostic_transfers.truncate(MAX_DIAGNOSTIC_RECORDS);

        let remaining = MAX_DIAGNOSTIC_RECORDS.saturating_sub(diagnostic_transfers.len());
        let mut diagnostic_peers = devices
            .into_iter()
            .map(|device| LocalLinkDiagnosticPeer {
                peer_ref: peer_ref(&device.device_id),
                trust_status: device.trust_status,
                trust_duration: device.trust_duration,
                availability: if device.online {
                    LocalLinkDiagnosticAvailability::Online
                } else {
                    LocalLinkDiagnosticAvailability::Offline
                },
            })
            .collect::<Vec<_>>();
        // Never let a peer display name influence the exported order.
        diagnostic_peers.sort_by(|left, right| left.peer_ref.cmp(&right.peer_ref));
        diagnostic_peers.truncate(remaining);

        let record_count = diagnostic_peers.len() + diagnostic_transfers.len();
        Ok(LocalLinkDiagnostics {
            schema_version: LOCAL_LINK_DIAGNOSTIC_SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            os_family: diagnostic_os_family(),
            architecture: diagnostic_architecture(),
            generated_at: now(),
            local_link_enabled: preferences.enabled,
            discovery_enabled: preferences.discovery_enabled,
            readiness,
            record_count: u32::try_from(record_count).unwrap_or(u32::MAX),
            peers: diagnostic_peers,
            transfers: diagnostic_transfers,
        })
    }

    pub fn update_preferences(
        &self,
        preferences: LocalLinkPreferences,
    ) -> AppResult<LocalLinkPreferences> {
        let device_name = validate_device_name(&preferences.device_name)?;
        if preferences.enabled {
            self.ensure_identity()?;
            self.database.with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_preferences
                     SET enabled = 1, discovery_enabled = ?1, device_name = ?2, updated_at = ?3
                     WHERE singleton = 1",
                    params![preferences.discovery_enabled, device_name, now()],
                )?;
                Ok(())
            })?;
            self.handle_lifecycle_event(LifecycleEvent::PreferenceChanged { enabled: true });
            // A duplicate enable can still change Bonjour visibility without
            // restarting a healthy listener.
            if self
                .transport
                .lock()
                .expect("Local Link transport mutex poisoned")
                .as_ref()
                .is_some_and(TransportRuntime::is_healthy)
            {
                if let Some(transport) = self
                    .transport
                    .lock()
                    .expect("Local Link transport mutex poisoned")
                    .as_mut()
                {
                    transport.set_pairing_visible(preferences.discovery_enabled)?;
                }
            }
        } else {
            self.database.with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_preferences
                     SET enabled = 0, discovery_enabled = 0, device_name = ?1, updated_at = ?2
                     WHERE singleton = 1",
                    params![device_name, now()],
                )?;
                Ok(())
            })?;
            self.handle_lifecycle_event(LifecycleEvent::PreferenceChanged { enabled: false });
            self.cancel_all_active(LocalLinkTransferFailure::PayloadUnavailable, true)?;
            self.identity
                .lock()
                .expect("Local Link identity mutex poisoned")
                .take();
        }
        self.preferences()
    }

    pub fn local_identity_fingerprint(&self) -> AppResult<String> {
        let preferences = self.preferences()?;
        ensure_link_enabled(&preferences)?;
        Ok(self.ensure_identity()?.fingerprint().to_owned())
    }

    pub fn reset_identity(&self) -> AppResult<()> {
        self.reset_identity_state(LocalIdentity::delete_stored)
    }

    fn reset_identity_state(
        &self,
        delete_stored_identity: impl FnOnce() -> AppResult<()>,
    ) -> AppResult<()> {
        // Persist the off switch before any fallible cleanup. If cancelling a
        // transfer, deleting Keychain material, or invalidating trust fails,
        // subsequent commands still observe Local Link as disabled.
        self.database.with_local_link_connection(|connection| {
            connection.execute(
                "UPDATE local_link_preferences
                 SET enabled = 0, discovery_enabled = 0, updated_at = ?1
                 WHERE singleton = 1",
                [now()],
            )?;
            Ok(())
        })?;
        self.handle_lifecycle_event(LifecycleEvent::PreferenceChanged { enabled: false });
        self.identity
            .lock()
            .expect("Local Link identity mutex poisoned")
            .take();
        self.cancel_all_active(LocalLinkTransferFailure::PayloadUnavailable, false)?;
        delete_stored_identity()?;
        self.database.with_local_link_connection(|connection| {
            connection.execute(
                "UPDATE local_link_devices
                 SET trust_status = 'needsRePairing', peer_public_key = X'',
                     public_key_fingerprint = '', trusted_at = NULL,
                     last_seen_at = NULL
                 WHERE trust_status IN ('pairing', 'trusted', 'needsRePairing')",
                [],
            )?;
            Ok(())
        })?;
        self.receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .clear();
        self.deadline_reaper.notify();
        Ok(())
    }

    #[cfg(test)]
    fn reset_identity_for_test(&self) -> AppResult<()> {
        self.reset_identity_state(|| Ok(()))
    }

    fn ensure_identity(&self) -> AppResult<Arc<LocalIdentity>> {
        self.expire_trusted_devices()?;
        let mut identity = self
            .identity
            .lock()
            .expect("Local Link identity mutex poisoned");
        if let Some(identity) = identity.as_ref() {
            return Ok(Arc::clone(identity));
        }
        let has_trusted_peers = self.database.with_local_link_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM local_link_devices WHERE trust_status = 'trusted'
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value != 0)
                .map_err(AppError::from)
        })?;
        let loaded = Arc::new(LocalIdentity::load_or_create(!has_trusted_peers)?);
        *identity = Some(Arc::clone(&loaded));
        Ok(loaded)
    }

    fn start_transport(
        &self,
        pairing_visible: bool,
        generation: u64,
        retired: &mut Vec<TransportRuntime>,
    ) -> AppResult<()> {
        let snapshot = self
            .lifecycle
            .lock()
            .expect("Local Link lifecycle mutex poisoned")
            .snapshot();
        if snapshot.generation != generation
            || !snapshot.enabled
            || snapshot.sleeping
            || !snapshot.session_active
            || snapshot.terminating
            || snapshot.network_path == NetworkPathState::Unsatisfied
            || snapshot.transport_health != TransportHealth::Starting
        {
            return Err(AppError::InvalidInput(
                "Local Link ignored a stale transport start request.".to_owned(),
            ));
        }
        // A restored explicit preference must not make Core startup depend on
        // Keychain availability. Treat a temporarily locked/inaccessible
        // identity as a supervisor start failure so session activation/wake
        // can retry it without accepting a connection under an ad-hoc key.
        self.ensure_identity()?;
        self.resume_reconciliation_deadlines()?;
        let mut transport = self
            .transport
            .lock()
            .expect("Local Link transport mutex poisoned");
        if transport
            .as_ref()
            .is_some_and(|runtime| !runtime.is_healthy())
        {
            if let Some(runtime) = transport.take() {
                retired.push(runtime);
            }
        }
        if transport.is_none() {
            *transport = Some(TransportRuntime::start(
                self.self_weak.clone(),
                pairing_visible,
                generation,
            )?);
        } else if let Some(transport) = transport.as_mut() {
            transport.set_pairing_visible(pairing_visible)?;
        }
        drop(transport);
        self.resume_status_reconciliations()?;
        Ok(())
    }

    pub(crate) fn handle_lifecycle_event(&self, event: LifecycleEvent) {
        let retired = {
            let _execution = self
                .lifecycle_execution
                .lock()
                .expect("Local Link lifecycle execution mutex poisoned");
            let mut retired = Vec::new();
            let mut events = VecDeque::from([event]);
            while let Some(event) = events.pop_front() {
                let actions = self
                    .lifecycle
                    .lock()
                    .expect("Local Link lifecycle mutex poisoned")
                    .handle(event);
                for action in actions {
                    if let Some(event) = self.execute_lifecycle_action(action, &mut retired) {
                        events.push_back(event);
                    }
                }
            }
            retired
        };
        // TransportRuntime::drop joins its listener. Do that only after the
        // executor lock is released because a dying listener may already be
        // waiting to report TransportWorkerDied through the same executor.
        drop(retired);
    }

    fn execute_lifecycle_action(
        &self,
        action: LifecycleAction,
        retired: &mut Vec<TransportRuntime>,
    ) -> Option<LifecycleEvent> {
        match action {
            LifecycleAction::CancelScheduledWork { .. }
            | LifecycleAction::MarkTransportDegraded { .. } => {}
            LifecycleAction::SetReceiverAvailable(available) => {
                let _ = self.set_receiver_available(available);
            }
            LifecycleAction::SuspendSensitiveRuntime { reason } => match reason {
                SecureSuspensionReason::Disabled
                | SecureSuspensionReason::Sleep
                | SecureSuspensionReason::SessionInactive
                | SecureSuspensionReason::Termination => {
                    crate::commands::close_local_link_reveals_natively();
                }
            },
            LifecycleAction::StopTransport { .. } => {
                self.pause_status_reconciliations();
                self.retire_transport(retired);
            }
            LifecycleAction::ScheduleRestartDebounce { generation, delay } => {
                let service = self.self_weak.clone();
                let _ = thread::Builder::new()
                    .name("clipriva-local-link-wake-debounce".to_owned())
                    .spawn(move || {
                        thread::sleep(delay);
                        if let Some(service) = service.upgrade() {
                            service.handle_lifecycle_event(
                                LifecycleEvent::RestartDebounceElapsed { generation },
                            );
                        }
                    });
            }
            LifecycleAction::ScheduleRetry {
                generation,
                attempt,
                delay,
            } => {
                let service = self.self_weak.clone();
                let _ = thread::Builder::new()
                    .name("clipriva-local-link-restart".to_owned())
                    .spawn(move || {
                        thread::sleep(delay);
                        if let Some(service) = service.upgrade() {
                            service.handle_lifecycle_event(LifecycleEvent::RetryElapsed {
                                generation,
                                attempt,
                            });
                        }
                    });
            }
            LifecycleAction::StartTransport { generation } => {
                let pairing_visible = self
                    .preferences()
                    .map(|preferences| preferences.discovery_enabled)
                    .unwrap_or(false);
                let event = if self
                    .start_transport(pairing_visible, generation, retired)
                    .is_ok()
                {
                    LifecycleEvent::TransportStarted { generation }
                } else {
                    LifecycleEvent::TransportStartFailed { generation }
                };
                return Some(event);
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn install_platform_lifecycle_monitor(self: &Arc<Self>) {
        let service = Arc::downgrade(self);
        let sink: lifecycle::LifecycleEventSink = Arc::new(move |event| {
            if let Some(service) = service.upgrade() {
                service.handle_lifecycle_event(event);
            }
        });
        let monitor = lifecycle_macos::MacOsLifecycleMonitor::start(sink);
        *self
            .lifecycle_monitor
            .lock()
            .expect("Local Link lifecycle monitor mutex poisoned") = Some(monitor);
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn install_platform_lifecycle_monitor(self: &Arc<Self>) {}

    fn retire_transport(&self, retired: &mut Vec<TransportRuntime>) {
        if let Some(runtime) = self
            .transport
            .lock()
            .expect("Local Link transport mutex poisoned")
            .take()
        {
            retired.push(runtime);
        }
    }

    fn pause_status_reconciliations(&self) {
        let mut runtime = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned");
        runtime
            .reconciliation_paused_at_ms
            .get_or_insert_with(|| Utc::now().timestamp_millis());
    }

    /// Extend every persisted reconciliation deadline by a known lifecycle
    /// suspension. Process crashes do not reset the deadline; only a matching
    /// sleep/session/path stop and later supervisor restart pauses active time.
    fn resume_reconciliation_deadlines(&self) -> AppResult<()> {
        let paused_at_ms = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .reconciliation_paused_at_ms;
        let Some(paused_at_ms) = paused_at_ms else {
            return Ok(());
        };
        let paused_ms = Utc::now()
            .timestamp_millis()
            .saturating_sub(paused_at_ms)
            .max(0);
        if paused_ms > 0 {
            self.database.with_local_link_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let mut statement = transaction.prepare(
                    "SELECT id, expires_at FROM local_link_transfers
                     WHERE direction = 'outgoing' AND status = 'reconciling'
                       AND expires_at IS NOT NULL",
                )?;
                let deadlines = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                drop(statement);
                for (transfer_id, expires_at) in deadlines {
                    let deadline = DateTime::parse_from_rfc3339(&expires_at)
                        .map_err(|_| {
                            AppError::InvalidInput(
                                "Local Link reconciliation deadline is invalid.".to_owned(),
                            )
                        })?
                        .with_timezone(&Utc)
                        + chrono::Duration::milliseconds(paused_ms);
                    transaction.execute(
                        "UPDATE local_link_transfers SET expires_at = ?1
                         WHERE id = ?2 AND direction = 'outgoing'
                           AND status = 'reconciling'",
                        params![
                            deadline.to_rfc3339_opts(SecondsFormat::Millis, true),
                            transfer_id
                        ],
                    )?;
                }
                transaction.commit()?;
                Ok(())
            })?;
        }
        let mut runtime = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned");
        if runtime.reconciliation_paused_at_ms == Some(paused_at_ms) {
            runtime.reconciliation_paused_at_ms = None;
        }
        Ok(())
    }

    /// Apply the fixed trust lifetime before an operation can inspect or use a
    /// peer identity. Expiry deliberately keeps `trusted_at` as audit metadata
    /// but clears usable key material and moves the device into the existing
    /// explicit re-pairing state.
    fn expire_trusted_devices(&self) -> AppResult<usize> {
        self.database.with_local_link_connection(|connection| {
            expire_trusted_devices_with_connection(connection, Utc::now())
        })
    }

    fn is_session_trusted_device(&self, device_id: &str) -> bool {
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .session_trusted_devices
            .contains_key(device_id)
    }

    /// Record only a verified native observation. Discovery is anonymous by
    /// design, so an mDNS record alone must never make a trusted device appear
    /// online or refresh its last-seen display value.
    fn record_authenticated_device_presence(&self, device_id: &str) {
        let observed_at_ms = Utc::now().timestamp_millis();
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .device_presence
            .insert(
                device_id.to_owned(),
                RuntimeDevicePresence {
                    last_seen_at: timestamp_from_millis(observed_at_ms),
                    observed_at_ms,
                },
            );
    }

    fn discovery_candidates(&self) -> Vec<discovery::DiscoveryCandidate> {
        if !self
            .preferences()
            .map(|preferences| preferences.enabled && preferences.discovery_enabled)
            .unwrap_or(false)
        {
            return Vec::new();
        }
        self.transport
            .lock()
            .expect("Local Link transport mutex poisoned")
            .as_ref()
            .map(TransportRuntime::candidates)
            .unwrap_or_default()
    }

    fn transport_endpoints(&self) -> Vec<SocketAddr> {
        self.transport
            .lock()
            .expect("Local Link transport mutex poisoned")
            .as_ref()
            .map(TransportRuntime::endpoints)
            .unwrap_or_default()
    }

    pub fn list_devices(&self) -> AppResult<Vec<LocalLinkDevice>> {
        self.expire_trusted_devices()?;
        let mut devices = self.database.with_local_link_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT device_id, display_name, public_key_fingerprint, trust_status, paired_at,
                        trusted_at, revoked_at, last_seen_at, last_transfer_at,
                        protocol_min, protocol_max, trust_duration
                 FROM local_link_devices
                 ORDER BY CASE trust_status
                     WHEN 'trusted' THEN 0 WHEN 'needsRePairing' THEN 1
                     WHEN 'pairing' THEN 2 ELSE 3 END,
                     display_name COLLATE NOCASE ASC, device_id ASC",
            )?;
            let devices = statement.query_map([], map_local_link_device)?;
            devices
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)
        })?;
        let (session_trusted_devices, device_presence) = {
            let runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            (
                runtime.session_trusted_devices.clone(),
                runtime.device_presence.clone(),
            )
        };
        let now_ms = Utc::now().timestamp_millis();
        for device in &mut devices {
            if device.trust_status == LocalLinkDeviceTrustStatus::Trusted {
                if let Some(presence) = device_presence
                    .get(&device.device_id)
                    .filter(|presence| runtime_presence_is_current(presence, now_ms))
                {
                    device.online = true;
                    // This runtime projection lets the Devices surface show
                    // the latest authenticated observation without turning
                    // routine reachability into a persisted tracking record.
                    device.last_seen_at = Some(presence.last_seen_at.clone());
                }
            }
        }
        for (device_id, session) in session_trusted_devices {
            if let Some(device) = devices
                .iter_mut()
                .find(|device| device.device_id == device_id)
            {
                device.public_key_fingerprint = session.public_key_fingerprint;
                device.trust_status = LocalLinkDeviceTrustStatus::Trusted;
                device.paired_at = session.paired_at.clone();
                device.trusted_at = Some(session.paired_at);
                device.trust_duration = LocalLinkTrustDuration::ThisSession;
                device.trust_expires_at = None;
                device.re_pairing_reason = None;
                device.revoked_at = None;
                device.online = true;
            }
        }
        // Bonjour records are intentionally returned only as anonymous,
        // runtime-only candidates. They never create a trust row or expose an
        // endpoint to the UI.
        for candidate in self.discovery_candidates() {
            if devices
                .iter()
                .any(|device| device.device_id == candidate.id)
            {
                continue;
            }
            devices.push(LocalLinkDevice {
                device_id: candidate.id,
                display_name: "Nearby ClipRiva device".to_owned(),
                public_key_fingerprint: String::new(),
                trust_status: LocalLinkDeviceTrustStatus::Revoked,
                paired_at: now(),
                trusted_at: None,
                trust_duration: LocalLinkTrustDuration::ThirtyDays,
                trust_expires_at: None,
                re_pairing_reason: None,
                revoked_at: None,
                last_seen_at: Some(now()),
                last_transfer_at: None,
                online: true,
                protocol_min: PROTOCOL_VERSION,
                protocol_max: PROTOCOL_VERSION,
            });
        }
        Ok(devices)
    }

    /// Read the device state needed by readiness and diagnostics without
    /// applying expiry migrations or exposing anonymous discovery candidates.
    /// Expired thirty-day trust is projected as `needsRePairing` in memory so
    /// this path remains truthful without changing SQLite.
    fn readiness_devices(&self) -> AppResult<Vec<LocalLinkDevice>> {
        let mut devices = self.database.with_local_link_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT device_id, display_name, public_key_fingerprint, trust_status, paired_at,
                        trusted_at, revoked_at, last_seen_at, last_transfer_at,
                        protocol_min, protocol_max, trust_duration
                 FROM local_link_devices
                 ORDER BY CASE trust_status
                     WHEN 'trusted' THEN 0 WHEN 'needsRePairing' THEN 1
                     WHEN 'pairing' THEN 2 ELSE 3 END,
                     display_name COLLATE NOCASE ASC, device_id ASC",
            )?;
            let devices = statement.query_map([], map_local_link_device)?;
            devices
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)
        })?;
        let current_time = Utc::now();
        for device in &mut devices {
            if device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                && trust_has_expired(
                    device.trusted_at.as_deref(),
                    device.trust_duration,
                    current_time,
                )
            {
                device.trust_status = LocalLinkDeviceTrustStatus::NeedsRePairing;
                device.re_pairing_reason = Some(LocalLinkDeviceRePairingReason::TrustExpired);
                device.online = false;
            }
        }

        let (session_trusted_devices, device_presence) = {
            let runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            (
                runtime.session_trusted_devices.clone(),
                runtime.device_presence.clone(),
            )
        };
        let now_ms = current_time.timestamp_millis();
        for device in &mut devices {
            if device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                && device_presence
                    .get(&device.device_id)
                    .is_some_and(|presence| runtime_presence_is_current(presence, now_ms))
            {
                device.online = true;
            }
        }
        for (device_id, session) in session_trusted_devices {
            if let Some(device) = devices
                .iter_mut()
                .find(|device| device.device_id == device_id)
            {
                device.trust_status = LocalLinkDeviceTrustStatus::Trusted;
                device.trust_duration = LocalLinkTrustDuration::ThisSession;
                device.trusted_at = Some(session.paired_at);
                device.trust_expires_at = None;
                device.re_pairing_reason = None;
                device.revoked_at = None;
                device.online = true;
            }
        }
        Ok(devices)
    }

    pub fn begin_pairing(
        &self,
        request: BeginLocalLinkPairingRequest,
    ) -> AppResult<LocalLinkDevice> {
        validate_device_id(&request.device_id)?;
        validate_device_name(&request.display_name)?;
        self.expire_trusted_devices()?;
        ensure_link_enabled(&self.preferences()?)?;
        let identity = self.ensure_identity()?;
        let candidates = self.discovery_candidates();
        let candidate = candidates
            .into_iter()
            .find(|candidate| candidate.id == request.device_id)
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "The selected Local Link discovery candidate is no longer available."
                        .to_owned(),
                )
            })?;
        if candidate.endpoints.is_empty() {
            return Err(AppError::InvalidInput(
                "No Local Link device is currently discoverable. No trust record was created."
                    .to_owned(),
            ));
        }
        let mut last_error = None;
        for endpoint in candidate.endpoints {
            match self.open_outbound_pairing(endpoint, Arc::clone(&identity)) {
                Ok(device) => return Ok(device),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            AppError::InvalidInput(
                "Could not establish an authenticated Local Link pairing session.".to_owned(),
            )
        }))
    }

    pub fn active_pairing_details(&self) -> AppResult<Option<LocalLinkPairingDetails>> {
        let device_id = {
            let runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            match runtime.pairing_sessions.len() {
                0 => return Ok(None),
                1 => runtime.pairing_sessions.keys().next().cloned(),
                _ => {
                    return Err(AppError::InvalidInput(
                        "More than one Local Link pairing session is active.".to_owned(),
                    ));
                }
            }
        };
        device_id
            .map(|device_id| self.pairing_details(&device_id))
            .transpose()
    }

    fn pairing_details(&self, device_id: &str) -> AppResult<LocalLinkPairingDetails> {
        let device_id = validate_device_id(device_id)?;
        let (verification_code, expires_at_ms, peer_fingerprint, local_confirmed) = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .get(&device_id)
            .map(|session| {
                (
                    session.sas.clone(),
                    session.expires_at_ms,
                    session.peer_fingerprint.clone(),
                    session.local_confirmed,
                )
            })
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "This Local Link pairing session is no longer available.".to_owned(),
                )
            })?;
        let device = self.database.with_local_link_connection(|connection| {
            local_link_device_with_connection(connection, &device_id)?.ok_or(AppError::NotFound)
        })?;
        Ok(LocalLinkPairingDetails {
            device,
            verification_code,
            expires_at: timestamp_from_millis(expires_at_ms),
            local_fingerprint: self.ensure_identity()?.fingerprint().to_owned(),
            peer_fingerprint,
            local_confirmed,
            // The remote confirmation is deliberately not inferred until its
            // encrypted transcript confirmation arrives and trust is committed.
            peer_confirmed: false,
        })
    }

    pub fn confirm_pairing(
        &self,
        device_id: &str,
        trust_duration: LocalLinkTrustDuration,
    ) -> AppResult<LocalLinkDevice> {
        let device_id = validate_device_id(device_id)?;
        let signal = {
            let mut runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            let session = runtime
                .pairing_sessions
                .get_mut(&device_id)
                .ok_or_else(|| {
                    AppError::InvalidInput(
                        "This Local Link pairing session is no longer available.".to_owned(),
                    )
                })?;
            if session.expires_at_ms <= Utc::now().timestamp_millis() {
                return Err(AppError::InvalidInput(
                    "This Local Link pairing session expired. No trust record was created."
                        .to_owned(),
                ));
            }
            if session.local_confirmed {
                return Err(AppError::InvalidInput(
                    "This Mac already confirmed the Local Link pairing code.".to_owned(),
                ));
            }
            session.local_confirmed = true;
            session.trust_duration = Some(trust_duration);
            session.confirm.clone()
        };
        signal.send(PairingSignal::Confirm).map_err(|_| {
            AppError::InvalidInput(
                "The Local Link pairing session ended before this confirmation.".to_owned(),
            )
        })?;
        self.database.with_local_link_connection(|connection| {
            local_link_device_with_connection(connection, &device_id)?.ok_or(AppError::NotFound)
        })
    }

    pub fn cancel_pairing(&self, device_id: &str) -> AppResult<()> {
        let device_id = validate_device_id(device_id)?;
        let pairing = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .remove(&device_id);
        if let Some(pairing) = pairing {
            let _ = pairing.confirm.send(PairingSignal::Cancel);
            self.database.with_local_link_connection(|connection| {
                connection.execute(
                    "DELETE FROM local_link_devices
                     WHERE device_id = ?1 AND trust_status = 'pairing'",
                    [&device_id],
                )?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Revocation and receiver decisions share the runtime mutex. This makes
    /// their order linearizable: if revocation wins, no later clipboard/History
    /// side effect can claim the payload; if an action wins, it completes before
    /// revocation returns.
    pub fn revoke_device(&self, device_id: &str) -> AppResult<LocalLinkDevice> {
        let device_id = validate_device_id(device_id)?;
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let (device, updates) = self.database.with_local_link_connection(|connection| {
            local_link_device_with_connection(connection, &device_id)?
                .ok_or(AppError::NotFound)?;
            let transaction = connection.unchecked_transaction()?;
            let mut statement = transaction.prepare(
                "SELECT transfers.id, transfers.direction, transfers.status,
                        EXISTS (
                            SELECT 1 FROM local_link_effect_claims claims
                            WHERE claims.transfer_id = transfers.id
                              AND claims.state = 'prepared'
                        )
                 FROM local_link_transfers transfers
                 WHERE device_id = ?1
                   AND status IN ('connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'reconciling')",
            )?;
            let active = statement
                .query_map([&device_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)? != 0,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            let timestamp = now();
            transaction.execute(
                "UPDATE local_link_devices
                 SET trust_status = 'revoked', peer_public_key = X'',
                     revoked_at = ?1, trusted_at = NULL
                 WHERE device_id = ?2",
                params![timestamp, device_id],
            )?;
            let mut updates = Vec::with_capacity(active.len());
            for (transfer_id, direction, status, prepared) in active {
                let uncertain = prepared
                    || (direction == "outgoing"
                        && matches!(status.as_str(), "awaitingReceiver" | "viewed" | "reconciling"));
                let receipt = if uncertain {
                    TerminalReceipt::outcome_unknown()
                } else {
                    TerminalReceipt::not_delivered(LocalLinkTransferFailure::DeviceRevoked)
                };
                transaction.execute(
                    "UPDATE local_link_transfers
                     SET status = 'failed', failure_reason = ?1, receiver_action = NULL,
                         updated_at = ?2, completed_at = ?2, expires_at = NULL
                     WHERE id = ?3",
                    params![
                        if uncertain { "outcomeUnknown" } else { "deviceRevoked" },
                        timestamp,
                        transfer_id
                    ],
                )?;
                if prepared {
                    transaction.execute(
                        "UPDATE local_link_effect_claims
                         SET state = 'uncertain', updated_at = ?1
                         WHERE transfer_id = ?2 AND state = 'prepared'",
                        params![timestamp, transfer_id],
                    )?;
                }
                updates.push((transfer_id, receipt));
            }
            transaction.commit()?;
            let device = local_link_device_with_connection(connection, &device_id)?
                .ok_or(AppError::NotFound)?;
            Ok((device, updates))
        })?;

        // Persist the trust and transfer decision before discarding runtime
        // evidence, so a database error leaves the revocation retryable.
        runtime.session_trusted_devices.remove(&device_id);
        runtime.device_presence.remove(&device_id);
        runtime.pending_inbound.retain(|transfer_id, pending| {
            pending.peer_device_id != device_id && !updates.iter().any(|(id, _)| id == transfer_id)
        });
        let mut inbound_waiters = Vec::new();
        let mut outgoing_signals = Vec::new();
        for (transfer_id, receipt) in &updates {
            if let Some(waiter) = runtime.receipt_waiters.remove(transfer_id) {
                inbound_waiters.push((waiter, *receipt));
            }
            if let Some(signal) = runtime.outgoing_cancel_waiters.remove(transfer_id) {
                outgoing_signals.push(signal);
            }
        }
        drop(runtime);
        let mut receipts = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned");
        for (transfer_id, receipt) in &updates {
            let _ = receipts.apply_receipt(transfer_id, *receipt);
        }
        drop(receipts);
        for (waiter, receipt) in inbound_waiters {
            let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
        }
        for signal in outgoing_signals {
            let _ = signal.send(OutgoingTransferSignal::Cancel);
        }
        if updates.iter().any(|(_, receipt)| {
            receipt.outcome == ReceiptOutcome::OutcomeUnknown
                || receipt.outcome == ReceiptOutcome::NotDelivered
        }) {
            crate::commands::close_local_link_reveals_natively();
        }
        self.deadline_reaper.notify();
        Ok(device)
    }

    pub fn send_clipboard_item(
        &self,
        clipboard_item_id: &str,
        device_id: &str,
    ) -> AppResult<LocalLinkTransfer> {
        let device_id = validate_device_id(device_id)?;
        self.expire_trusted_devices()?;
        let trust_failure = if self.is_session_trusted_device(&device_id) {
            self.database.with_local_link_connection(|connection| {
                ensure_link_enabled(&local_link_preferences_with_connection(connection)?)?;
                Ok(None)
            })?
        } else {
            self.database.with_local_link_connection(|connection| {
                ensure_link_enabled(&local_link_preferences_with_connection(connection)?)?;
                let (status, key): (String, Vec<u8>) = connection
                    .query_row(
                        "SELECT trust_status, peer_public_key FROM local_link_devices WHERE device_id = ?1",
                        [&device_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?
                    .ok_or(AppError::NotFound)?;
                Ok(match LocalLinkDeviceTrustStatus::from_storage_value(&status) {
                    Some(LocalLinkDeviceTrustStatus::Trusted) if key.len() == 32 => None,
                    Some(LocalLinkDeviceTrustStatus::Revoked) => {
                        Some(LocalLinkTransferFailure::DeviceRevoked)
                    }
                    _ => Some(LocalLinkTransferFailure::DeviceNotTrusted),
                })
            })?
        };
        if let Some(failure) = trust_failure {
            // A stale device card can race this command. Preserve a
            // content-free terminal outcome rather than returning an opaque
            // command error, so Transfers can show the exact recovery step.
            return self.persist_failed_outgoing(&device_id, 0, failure);
        }
        let payload = match self
            .database
            .local_link_export_clipboard_item(clipboard_item_id)
        {
            Ok(payload) => payload,
            Err(error) => {
                return self.persist_failed_outgoing(
                    &device_id,
                    0,
                    failure_for_export_error(&error),
                )
            }
        };
        validate_text_payload(&payload.content)?;
        let endpoints = self.transport_endpoints();
        if endpoints.is_empty() {
            return self.persist_failed_outgoing(
                &device_id,
                payload.content.len(),
                LocalLinkTransferFailure::DeviceUnavailable,
            );
        }
        let transfer = self.persist_outgoing_connecting(&device_id, payload.content.len())?;
        let transfer_id = transfer.id.clone();
        let content = Zeroizing::new(payload.content);
        let Some(generation) = self.active_delivery_generation() else {
            self.mark_outgoing_failed(&transfer_id, LocalLinkTransferFailure::PayloadUnavailable)?;
            return self.database.with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, &transfer_id)?
                    .ok_or(AppError::NotFound)
            });
        };
        let service = self.self_weak.clone();
        let failure_transfer_id = transfer_id.clone();
        let spawned = thread::Builder::new()
            .name("clipriva-local-link-send".to_owned())
            .spawn(move || {
                if let Some(service) = service.upgrade() {
                    service.deliver_outgoing(
                        endpoints,
                        device_id,
                        transfer_id,
                        content,
                        generation,
                    );
                }
            });
        if spawned.is_err() {
            self.mark_outgoing_failed(
                &failure_transfer_id,
                LocalLinkTransferFailure::PayloadUnavailable,
            )?;
            return self.database.with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, &failure_transfer_id)?
                    .ok_or(AppError::NotFound)
            });
        }
        Ok(transfer)
    }

    pub fn cancel_transfer(&self, transfer_id: &str) -> AppResult<LocalLinkTransfer> {
        let result = self.database.with_local_link_connection(|connection| {
            let transfer = local_link_transfer_with_connection(connection, transfer_id)?
                .ok_or(AppError::NotFound)?;
            if transfer.direction != LocalLinkTransferDirection::Outgoing {
                return Err(AppError::InvalidInput(
                    "Only the sender can cancel a Local Link transfer; use Reject for an incoming request."
                        .to_owned(),
                ));
            }
            if transfer.status == LocalLinkTransferStatus::Cancelled
                || transfer.status == LocalLinkTransferStatus::Reconciling
            {
                // A repeated click or delayed sender-side retry is harmless.
                // The receiver still receives at most one Cancel frame from
                // the live delivery worker.
                return Ok(transfer);
            }
            if !is_active_status(transfer.status) {
                return Err(AppError::InvalidInput(
                    "This Local Link transfer is already final.".to_owned(),
                ));
            }
            let timestamp = now();
            let changed = if matches!(
                transfer.status,
                LocalLinkTransferStatus::Connecting | LocalLinkTransferStatus::Encrypted
            ) {
                connection.execute(
                    "UPDATE local_link_transfers
                     SET status = 'cancelled', failure_reason = NULL, receiver_action = NULL,
                         updated_at = ?1, completed_at = ?1, expires_at = NULL
                     WHERE id = ?2 AND direction = 'outgoing'
                       AND status IN ('connecting', 'encrypted')",
                    params![timestamp, transfer_id],
                )?
            } else {
                // Once the complete encrypted payload has been written, Cancel
                // is provisional until the receiver's terminal state is known.
                // Preserve only a metadata-query deadline; never retain or
                // replay the body from this state.
                let deadline = timestamp_from_millis(
                    Utc::now()
                        .timestamp_millis()
                        .saturating_add(STATUS_RECONCILIATION_BUDGET_MS),
                );
                connection.execute(
                    "UPDATE local_link_transfers
                     SET status = 'reconciling', failure_reason = NULL,
                         receiver_action = NULL, updated_at = ?1,
                         completed_at = NULL, expires_at = ?2
                     WHERE id = ?3 AND direction = 'outgoing'
                       AND status IN ('awaitingReceiver', 'viewed')",
                    params![timestamp, deadline, transfer_id],
                )?
            };
            if changed != 1 {
                return Err(AppError::InvalidInput(
                    "This Local Link transfer is already final.".to_owned(),
                ));
            }
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        });
        if let Ok(transfer) = &result {
            if transfer.status == LocalLinkTransferStatus::Cancelled {
                let _ = self
                    .receipts
                    .lock()
                    .expect("Local Link receipt mutex poisoned")
                    .apply_receipt(
                        transfer_id,
                        TerminalReceipt::completed(ReceiptOutcome::Cancelled),
                    );
            }
            if let Some(waiter) = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned")
                .outgoing_cancel_waiters
                .get(transfer_id)
            {
                let _ = waiter.send(OutgoingTransferSignal::Cancel);
            }
        }
        result
    }

    pub fn list_transfers(&self, limit: u32) -> AppResult<Vec<LocalLinkTransfer>> {
        self.expire_pending(Utc::now().timestamp_millis())?;
        let limit = i64::from(limit.clamp(1, MAX_TRANSFER_LIST_LIMIT));
        let transfers = self.database.with_local_link_connection(|connection| {
            prune_transfer_summaries(connection)?;
            let mut statement = connection.prepare(
                "SELECT t.id, t.device_id, d.display_name, t.direction, t.status,
                        t.item_kind, t.byte_size, t.created_at, t.updated_at,
                        t.completed_at, t.expires_at, t.failure_reason, t.receiver_action
                 FROM local_link_transfers t
                 JOIN local_link_devices d ON d.device_id = t.device_id
                 ORDER BY t.updated_at DESC, t.id DESC
                 LIMIT ?1",
            )?;
            let transfers = statement.query_map([limit], map_local_link_transfer)?;
            transfers
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)
        })?;
        let mut receipts = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned");
        for transfer in &transfers {
            sync_receipt_state(&mut receipts, transfer);
        }
        Ok(transfers)
    }

    pub fn clear_transfers(&self) -> AppResult<u64> {
        self.expire_pending(Utc::now().timestamp_millis())?;
        self.database.with_local_link_connection(|connection| {
            let deleted = connection.execute(
                "DELETE FROM local_link_transfers
                 WHERE status IN ('copied', 'saved', 'rejected', 'cancelled', 'expired', 'failed')",
                [],
            )?;
            Ok(deleted as u64)
        })
    }

    /// Resolve crash-left Copy claims from the current native pasteboard
    /// marker. Only the matching random token can prove the one completed
    /// write; every other prepared claim becomes outcomeUnknown and is never
    /// replayed. The reader is not invoked when no recovery claim exists.
    pub(crate) fn recover_copy_effects(
        &self,
        read_native_marker: impl FnOnce() -> AppResult<(Option<String>, i64)>,
    ) -> AppResult<usize> {
        let claims = self
            .database
            .local_link_copy_effect_claims_for_recovery(1_000)?;
        if claims.is_empty() {
            return Ok(0);
        }
        // If the OS no longer permits reading the marker, uncertainty is the
        // only truthful terminal result. Recovery must still never replay the
        // clipboard body or keep an unrecoverable pending claim forever.
        let (current_marker, change_count) = read_native_marker().unwrap_or((None, 0));
        for claim in &claims {
            if current_marker.as_deref() == Some(claim.effect_token.as_str()) {
                self.database.complete_local_link_copy_effect(
                    &claim.transfer_id,
                    &claim.effect_token,
                    change_count,
                )?;
            } else {
                self.database.mark_local_link_copy_effect_uncertain(
                    &claim.transfer_id,
                    &claim.effect_token,
                )?;
            }
        }
        Ok(claims.len())
    }

    /// Reveal pending plaintext only through a native-process callback. Exactly
    /// one short-lived clone is transferred into the native view and zeroized
    /// when that view closes; it is never serializable through Tauri IPC.
    pub fn reveal_transfer(
        &self,
        transfer_id: &str,
        reveal_native: impl FnOnce(Zeroizing<String>) -> AppResult<()>,
    ) -> AppResult<()> {
        // Revealing is an explicit native-only user action, so it is also a
        // safe lifecycle boundary for recording `viewed`. The transition has
        // no plaintext DTO and repeated calls are idempotent.
        self.mark_transfer_viewed(transfer_id)?;
        let payload = {
            let runtime = self
                .runtime
                .lock()
                .expect("pending Local Link payload mutex poisoned");
            let pending = runtime.pending_inbound.get(transfer_id).ok_or_else(|| {
                AppError::InvalidInput(
                    "This Local Link text is no longer available to reveal.".to_owned(),
                )
            })?;
            let content = pending.content.as_ref().ok_or_else(|| {
                AppError::InvalidInput("This Local Link text has not finished arriving.".to_owned())
            })?;
            Zeroizing::new(content.to_string())
        };
        reveal_native(payload)
    }

    /// Record the receiver's explicit open action without exposing or
    /// persisting the transfer plaintext. The first transition is forwarded to
    /// the authenticated transport worker when one is still connected;
    /// identical later calls return the existing metadata-only transfer.
    pub fn mark_transfer_viewed(&self, transfer_id: &str) -> AppResult<LocalLinkTransfer> {
        self.expire_pending(Utc::now().timestamp_millis())?;
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let pending = runtime
            .pending_inbound
            .get_mut(transfer_id)
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "This Local Link transfer is no longer waiting for a decision.".to_owned(),
                )
            })?;
        if pending.content.is_none() {
            return Err(AppError::InvalidInput(
                "This Local Link text has not finished arriving.".to_owned(),
            ));
        }

        let transfer = self.database.with_local_link_connection(|connection| {
            if pending.viewed {
                return local_link_transfer_with_connection(connection, transfer_id)?
                    .filter(|transfer| transfer.status == LocalLinkTransferStatus::Viewed)
                    .ok_or_else(|| {
                        AppError::InvalidInput(
                            "This Local Link transfer is no longer waiting for a decision."
                                .to_owned(),
                        )
                    });
            }
            let timestamp = now();
            let changed = connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'viewed', updated_at = ?1
                 WHERE id = ?2 AND direction = 'incoming' AND status = 'awaitingReceiver'",
                params![timestamp, transfer_id],
            )?;
            if changed != 1 {
                return Err(AppError::InvalidInput(
                    "This Local Link transfer is no longer waiting for a decision.".to_owned(),
                ));
            }
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        })?;

        if !pending.viewed {
            pending.viewed = true;
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .advance(transfer_id, TransferPhase::Viewed);
            if let Some(waiter) = runtime.receipt_waiters.get(transfer_id) {
                let _ = waiter.send(InboundTransferSignal::Viewed);
            }
        }
        Ok(transfer)
    }

    /// Claim a pending payload exactly once. Save and its transfer terminal are
    /// one SQLite transaction. Copy first persists a content-free random
    /// marker claim, then performs one native pasteboard write, and finally
    /// commits the terminal state. A prepared claim is never written again.
    pub fn accept_transfer(
        &self,
        transfer_id: &str,
        action: LocalLinkReceiveAction,
        write_to_clipboard: impl FnOnce(&str, &str) -> AppResult<ClipboardWriteEvidence>,
    ) -> AppResult<LocalLinkTransfer> {
        if action == LocalLinkReceiveAction::Reject {
            return self.reject_transfer(transfer_id);
        }
        self.expire_trusted_devices()?;
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let Some(pending) = runtime.pending_inbound.get(transfer_id) else {
            return self.database.with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, transfer_id)?
                    .filter(|transfer| {
                        matches!(
                            (action, transfer.status, transfer.receiver_action),
                            (
                                LocalLinkReceiveAction::Copy,
                                LocalLinkTransferStatus::Copied,
                                Some(LocalLinkReceiveAction::Copy)
                            ) | (
                                LocalLinkReceiveAction::Save,
                                LocalLinkTransferStatus::Saved,
                                Some(LocalLinkReceiveAction::Save)
                            )
                        )
                    })
                    .ok_or_else(|| {
                        AppError::InvalidInput(
                            "This Local Link transfer is no longer waiting for a decision."
                                .to_owned(),
                        )
                    })
            });
        };
        if pending.offer.created_at_ms + i64::from(pending.offer.ttl_seconds) * 1_000
            <= Utc::now().timestamp_millis()
        {
            runtime.pending_inbound.remove(transfer_id);
            return self.mark_expired_while_locked(transfer_id);
        }
        let session_trusted = runtime
            .session_trusted_devices
            .contains_key(&pending.peer_device_id);
        let peer_device_id = pending.peer_device_id.clone();
        if !session_trusted
            && self
                .database
                .with_local_link_connection(|connection| {
                    ensure_trusted_device(connection, &peer_device_id)
                })
                .is_err()
        {
            runtime.pending_inbound.remove(transfer_id);
            return self
                .mark_failed_while_locked(transfer_id, LocalLinkTransferFailure::DeviceNotTrusted);
        }
        let payload = pending.content.as_ref().ok_or_else(|| {
            AppError::InvalidInput("The pending Local Link text was cleared.".to_owned())
        })?;

        let result = match action {
            LocalLinkReceiveAction::Copy => {
                let proposed_token = uuid::Uuid::new_v4().to_string();
                let claim = self
                    .database
                    .prepare_local_link_copy_effect(transfer_id, &proposed_token)?;
                match claim {
                    LocalLinkCopyClaimOutcome::Claimed(claim) => {
                        let evidence =
                            match write_to_clipboard(payload.as_str(), &claim.effect_token) {
                                Ok(evidence) => evidence,
                                Err(_) => {
                                    self.database.mark_local_link_copy_effect_uncertain(
                                        transfer_id,
                                        &claim.effect_token,
                                    )?;
                                    runtime.pending_inbound.remove(transfer_id);
                                    let transfer =
                                        self.database.with_local_link_connection(|connection| {
                                            local_link_transfer_with_connection(
                                                connection,
                                                transfer_id,
                                            )?
                                            .ok_or(AppError::NotFound)
                                        })?;
                                    self.notify_inbound_terminal(&mut runtime, &transfer)?;
                                    return Ok(transfer);
                                }
                            };
                        if let Err(error) = self.database.complete_local_link_copy_effect(
                            transfer_id,
                            &claim.effect_token,
                            evidence.change_count,
                        ) {
                            // The native write may already be visible. Never
                            // repeat it merely because the terminal DB commit
                            // failed; resolve conservatively if possible.
                            if self
                                .database
                                .mark_local_link_copy_effect_uncertain(
                                    transfer_id,
                                    &claim.effect_token,
                                )
                                .is_ok()
                            {
                                runtime.pending_inbound.remove(transfer_id);
                                let transfer =
                                    self.database.with_local_link_connection(|connection| {
                                        local_link_transfer_with_connection(
                                            connection,
                                            transfer_id,
                                        )?
                                        .ok_or(AppError::NotFound)
                                    })?;
                                self.notify_inbound_terminal(&mut runtime, &transfer)?;
                                return Ok(transfer);
                            }
                            runtime.pending_inbound.remove(transfer_id);
                            return Err(error);
                        }
                    }
                    LocalLinkCopyClaimOutcome::Existing(claim) => match claim.state {
                        LocalLinkCopyEffectState::Prepared => {
                            return Err(AppError::InvalidInput(
                                "This Copy is awaiting native pasteboard recovery; it will not be repeated."
                                    .to_owned(),
                            ));
                        }
                        LocalLinkCopyEffectState::Completed
                        | LocalLinkCopyEffectState::Uncertain => {}
                    },
                }
                self.database.with_local_link_connection(|connection| {
                    local_link_transfer_with_connection(connection, transfer_id)?
                        .ok_or(AppError::NotFound)
                })
            }
            LocalLinkReceiveAction::Save => {
                match self.database.finalize_local_link_save(
                    transfer_id,
                    &peer_device_id,
                    payload.as_str(),
                )? {
                    LocalLinkSaveFinalization::Saved(item) => drop(item),
                    LocalLinkSaveFinalization::AlreadySaved => {}
                }
                self.database.with_local_link_connection(|connection| {
                    local_link_transfer_with_connection(connection, transfer_id)?
                        .ok_or(AppError::NotFound)
                })
            }
            LocalLinkReceiveAction::Reject => unreachable!(),
        };
        runtime.pending_inbound.remove(transfer_id);
        if let Ok(transfer) = result.as_ref() {
            self.notify_inbound_terminal(&mut runtime, transfer)?;
        }
        drop(runtime);
        result
    }

    fn notify_inbound_terminal(
        &self,
        runtime: &mut RuntimeState,
        transfer: &LocalLinkTransfer,
    ) -> AppResult<()> {
        let terminal = terminal_receipt_for_transfer(transfer).ok_or_else(|| {
            AppError::InvalidInput(
                "Local Link receiver action did not produce a terminal result.".to_owned(),
            )
        })?;
        let _ = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .apply_receipt(&transfer.id, terminal);
        if let Some(waiter) = runtime.receipt_waiters.remove(&transfer.id) {
            let _ = waiter.send(InboundTransferSignal::Terminal(terminal));
        }
        crate::commands::close_local_link_reveals_natively();
        Ok(())
    }

    pub fn reject_transfer(&self, transfer_id: &str) -> AppResult<LocalLinkTransfer> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        if !runtime.pending_inbound.contains_key(transfer_id) {
            return Err(AppError::InvalidInput(
                "This Local Link transfer is no longer waiting for a decision.".to_owned(),
            ));
        }
        // Persist the winning decision before dropping the only in-memory
        // body. A database error therefore leaves the request retryable.
        self.database.finalize_local_link_reject(transfer_id)?;
        let result = self.database.with_local_link_connection(|connection| {
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        });
        runtime.pending_inbound.remove(transfer_id);
        if result.is_ok() {
            let terminal = TerminalReceipt::completed(ReceiptOutcome::Rejected);
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .apply_receipt(transfer_id, terminal);
            if let Some(waiter) = runtime.receipt_waiters.remove(transfer_id) {
                let _ = waiter.send(InboundTransferSignal::Terminal(terminal));
            }
        }
        drop(runtime);
        if result.is_ok() {
            crate::commands::close_local_link_reveals_natively();
        }
        result
    }

    /// Old transfers never retain a payload or item reference for retry.
    pub fn retry_transfer(&self, _transfer_id: &str) -> AppResult<LocalLinkTransfer> {
        Err(AppError::InvalidInput(
            "Select the current clipboard item and start a new Local Link transfer.".to_owned(),
        ))
    }

    fn persist_failed_outgoing(
        &self,
        device_id: &str,
        byte_size: usize,
        failure: LocalLinkTransferFailure,
    ) -> AppResult<LocalLinkTransfer> {
        let transfer_id = uuid::Uuid::new_v4().to_string();
        self.database.with_local_link_connection(|connection| {
            let timestamp = now();
            connection.execute(
                "INSERT INTO local_link_transfers
                 (id, device_id, direction, status, item_kind, byte_size,
                  created_at, updated_at, completed_at, expires_at,
                  failure_reason, receiver_action)
                 VALUES (?1, ?2, 'outgoing', 'failed', 'text', ?3,
                         ?4, ?4, ?4, NULL, ?5, NULL)",
                params![
                    transfer_id,
                    device_id,
                    i64::try_from(byte_size).unwrap_or(i64::MAX),
                    timestamp,
                    failure.as_storage_value()
                ],
            )?;
            prune_transfer_summaries(connection)?;
            local_link_transfer_with_connection(connection, &transfer_id)?.ok_or(AppError::NotFound)
        })
    }

    fn persist_outgoing_connecting(
        &self,
        device_id: &str,
        byte_size: usize,
    ) -> AppResult<LocalLinkTransfer> {
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let transfer = self.database.with_local_link_connection(|connection| {
            let timestamp = now();
            connection.execute(
                "INSERT INTO local_link_transfers
                 (id, device_id, direction, status, item_kind, byte_size,
                  created_at, updated_at, completed_at, expires_at,
                  failure_reason, receiver_action)
                 VALUES (?1, ?2, 'outgoing', 'connecting', 'text', ?3,
                         ?4, ?4, NULL, NULL, NULL, NULL)",
                params![
                    transfer_id,
                    device_id,
                    i64::try_from(byte_size).unwrap_or(i64::MAX),
                    timestamp,
                ],
            )?;
            prune_transfer_summaries(connection)?;
            local_link_transfer_with_connection(connection, &transfer_id)?.ok_or(AppError::NotFound)
        })?;
        let _ = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .begin(transfer.id.clone());
        Ok(transfer)
    }

    fn cancel_all_active(
        &self,
        failure: LocalLinkTransferFailure,
        cancelled: bool,
    ) -> AppResult<()> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let persisted = self.database.with_local_link_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut statement = transaction.prepare(
                "SELECT transfers.id, transfers.direction, transfers.status,
                        EXISTS (
                            SELECT 1 FROM local_link_effect_claims claims
                            WHERE claims.transfer_id = transfers.id
                              AND claims.state = 'prepared'
                        )
                 FROM local_link_transfers transfers
                 WHERE transfers.status IN
                       ('connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'reconciling')",
            )?;
            let active = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)? != 0,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            let timestamp = now();
            let mut updates = Vec::with_capacity(active.len());
            for (transfer_id, direction, status, prepared) in active {
                let uncertain = prepared
                    || (direction == "outgoing"
                        && matches!(
                            status.as_str(),
                            "awaitingReceiver" | "viewed" | "reconciling"
                        ));
                let receipt = if uncertain {
                    TerminalReceipt::outcome_unknown()
                } else if cancelled {
                    TerminalReceipt::completed(ReceiptOutcome::Cancelled)
                } else {
                    TerminalReceipt::not_delivered(failure)
                };
                let (terminal_status, failure_reason) = if uncertain {
                    ("failed", Some("outcomeUnknown"))
                } else if cancelled {
                    ("cancelled", None)
                } else {
                    ("failed", Some(failure.as_storage_value()))
                };
                transaction.execute(
                    "UPDATE local_link_transfers
                     SET status = ?1, failure_reason = ?2, receiver_action = NULL,
                         updated_at = ?3, completed_at = ?3, expires_at = NULL
                     WHERE id = ?4",
                    params![terminal_status, failure_reason, timestamp, transfer_id],
                )?;
                if prepared {
                    transaction.execute(
                        "UPDATE local_link_effect_claims
                         SET state = 'uncertain', updated_at = ?1
                         WHERE transfer_id = ?2 AND state = 'prepared'",
                        params![timestamp, transfer_id],
                    )?;
                }
                updates.push((transfer_id, receipt));
            }
            transaction.commit()?;
            Ok(updates)
        });

        // Sensitive process state is always cleared, even if SQLite is
        // unavailable. In that failure case the only truthful live receipt is
        // outcomeUnknown; the command still returns the database error.
        runtime.pending_inbound.clear();
        runtime.request_times.clear();
        runtime.session_trusted_devices.clear();
        runtime.device_presence.clear();
        let waiters = std::mem::take(&mut runtime.receipt_waiters);
        let outgoing = std::mem::take(&mut runtime.outgoing_cancel_waiters);
        drop(runtime);
        for signal in outgoing.into_values() {
            let _ = signal.send(OutgoingTransferSignal::Cancel);
        }
        crate::commands::close_local_link_reveals_natively();

        let updates = match persisted {
            Ok(updates) => updates,
            Err(error) => {
                let receipt = TerminalReceipt::outcome_unknown();
                let mut receipts = self
                    .receipts
                    .lock()
                    .expect("Local Link receipt mutex poisoned");
                for (transfer_id, waiter) in waiters {
                    let _ = receipts.apply_remote_receipt(&transfer_id, receipt);
                    let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
                }
                self.deadline_reaper.notify();
                return Err(error);
            }
        };
        let mut receipts = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned");
        for (transfer_id, receipt) in &updates {
            let _ = receipts.apply_receipt(transfer_id, *receipt);
        }
        drop(receipts);
        for (transfer_id, waiter) in waiters {
            let receipt = updates
                .iter()
                .find_map(|(id, receipt)| (id == &transfer_id).then_some(*receipt))
                .unwrap_or_else(TerminalReceipt::outcome_unknown);
            let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
        }
        self.deadline_reaper.notify();
        Ok(())
    }

    fn expire_pending(&self, now_ms: i64) -> AppResult<()> {
        expire_pending_runtime(&self.database, &self.runtime, &self.receipts, now_ms).map(|_| ())
    }

    fn mark_expired_while_locked(&self, transfer_id: &str) -> AppResult<LocalLinkTransfer> {
        let result = self.database.with_local_link_connection(|connection| {
            let timestamp = now();
            connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'expired', failure_reason = NULL, updated_at = ?1,
                     completed_at = ?1, expires_at = NULL
                 WHERE id = ?2 AND status IN ('awaitingReceiver', 'viewed')",
                params![timestamp, transfer_id],
            )?;
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        });
        if result.is_ok() {
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .apply_receipt(
                    transfer_id,
                    TerminalReceipt::completed(ReceiptOutcome::Expired),
                );
            crate::commands::close_local_link_reveals_natively();
        }
        result
    }

    fn mark_failed_while_locked(
        &self,
        transfer_id: &str,
        failure: LocalLinkTransferFailure,
    ) -> AppResult<LocalLinkTransfer> {
        let result = self.database.with_local_link_connection(|connection| {
            let timestamp = now();
            connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'failed', failure_reason = ?1, updated_at = ?2,
                     completed_at = ?2, expires_at = NULL
                 WHERE id = ?3 AND status IN ('awaitingReceiver', 'viewed')",
                params![failure.as_storage_value(), timestamp, transfer_id],
            )?;
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        });
        if result.is_ok() {
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .apply_receipt(transfer_id, TerminalReceipt::not_delivered(failure));
            crate::commands::close_local_link_reveals_natively();
        }
        result
    }

    /// Apply an authenticated sender cancellation while holding the same
    /// runtime lock as receiver decisions. The SQLite transition and removal
    /// of `PendingInbound` are therefore serialized with Copy, Save and
    /// Reject: whichever acquires this boundary first is the one terminal
    /// outcome seen by both devices. `PendingInbound` owns the only receiver
    /// body, so removing it drops and zeroizes that text before the receipt is
    /// returned to the sender.
    fn cancel_inbound_transfer(&self, transfer_id: &str) -> AppResult<TerminalReceipt> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let transfer = self.database.with_local_link_connection(|connection| {
            let current = local_link_transfer_with_connection(connection, transfer_id)?
                .ok_or(AppError::NotFound)?;
            if current.direction != LocalLinkTransferDirection::Incoming {
                return Err(AppError::InvalidInput(
                    "Local Link cancellation did not target an incoming transfer.".to_owned(),
                ));
            }
            if terminal_receipt_for_transfer(&current).is_some() {
                return Ok(current);
            }
            let timestamp = now();
            let changed = connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'cancelled', failure_reason = NULL, receiver_action = NULL,
                     updated_at = ?1, completed_at = ?1, expires_at = NULL
                 WHERE id = ?2 AND direction = 'incoming'
                   AND status IN ('encrypted', 'awaitingReceiver', 'viewed')",
                params![timestamp, transfer_id],
            )?;
            if changed != 1 {
                return Err(AppError::InvalidInput(
                    "This Local Link transfer is no longer active.".to_owned(),
                ));
            }
            local_link_transfer_with_connection(connection, transfer_id)?.ok_or(AppError::NotFound)
        })?;
        let receipt = terminal_receipt_for_transfer(&transfer).ok_or_else(|| {
            AppError::InvalidInput("This Local Link transfer is no longer active.".to_owned())
        })?;

        // Do this while the receiver-decision lock is still held. A decision
        // that begins later sees no pending body; a decision that began first
        // left a terminal summary above and is returned idempotently instead.
        runtime.pending_inbound.remove(transfer_id);
        runtime.receipt_waiters.remove(transfer_id);
        let _ = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .apply_receipt(transfer_id, receipt);
        drop(runtime);
        crate::commands::close_local_link_reveals_natively();
        self.deadline_reaper.notify();
        Ok(receipt)
    }

    /// Protocol adapter seam used by the encrypted transport after Noise has
    /// authenticated `peer_public_key`. The offer is evaluated before plaintext
    /// is accepted, so busy/rate/replay failures never receive a body.
    #[allow(dead_code)]
    pub(crate) fn reserve_inbound_offer(
        &self,
        peer_public_key: &[u8],
        offer: TransferOffer,
    ) -> Result<String, LocalLinkTransferFailure> {
        let now_ms = Utc::now().timestamp_millis();
        offer.validate(now_ms)?;
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        if !runtime.receiver_available {
            return Err(LocalLinkTransferFailure::ReceiverLocked);
        }
        let peer_device_id = runtime
            .session_trusted_devices
            .iter()
            .find(|(_, device)| device.public_key.as_slice() == peer_public_key)
            .map(|(device_id, _)| device_id.clone())
            .map(Ok)
            .unwrap_or_else(|| {
                self.database
                    .with_local_link_connection(|connection| {
                        expire_trusted_devices_with_connection(connection, Utc::now())?;
                        ensure_link_enabled(&local_link_preferences_with_connection(connection)?)?;
                        trusted_device_id_for_key(connection, peer_public_key)
                    })
                    .map_err(|_| LocalLinkTransferFailure::AuthFailed)
            })?;

        let cutoff = now_ms - 60_000;
        let times = runtime
            .request_times
            .entry(peer_device_id.clone())
            .or_default();
        while times.front().is_some_and(|time| *time <= cutoff) {
            times.pop_front();
        }
        if times.len() >= MAX_REQUESTS_PER_MINUTE {
            return Err(LocalLinkTransferFailure::RateLimited);
        }
        times.push_back(now_ms);
        if runtime
            .pending_inbound
            .values()
            .filter(|pending| pending.peer_device_id == peer_device_id)
            .count()
            >= MAX_PENDING_PER_PEER
        {
            return Err(LocalLinkTransferFailure::ReceiverBusy);
        }
        if runtime.pending_inbound.len() >= MAX_PENDING_GLOBAL {
            return Err(LocalLinkTransferFailure::ReceiverBusy);
        }

        let offer_deadline_ms = offer
            .created_at_ms
            .saturating_add(i64::from(offer.ttl_seconds) * 1_000);
        let expires_at = timestamp_from_millis(offer_deadline_ms);
        let replay_expires_at = (Utc::now() + chrono::Duration::hours(REPLAY_RETENTION_HOURS))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let database_result = self.database.with_local_link_connection(|connection| {
            prune_replay_tombstones(connection)?;
            let inserted = connection.execute(
                "INSERT OR IGNORE INTO local_link_replay_tombstones
                 (peer_device_id, transfer_id, session_id, nonce, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    peer_device_id,
                    offer.transfer_id,
                    offer.session_id,
                    offer.nonce,
                    replay_expires_at
                ],
            )?;
            if inserted != 1 {
                return Err(AppError::InvalidInput("duplicate".to_owned()));
            }
            let timestamp = now();
            connection.execute(
                "INSERT INTO local_link_transfers
                 (id, device_id, direction, status, item_kind, byte_size,
                  created_at, updated_at, completed_at, expires_at,
                  failure_reason, receiver_action)
                 VALUES (?1, ?2, 'incoming', 'encrypted', 'text', ?3,
                         ?4, ?4, NULL, ?5, NULL, NULL)",
                params![
                    offer.transfer_id,
                    peer_device_id,
                    i64::from(offer.byte_size),
                    timestamp,
                    expires_at
                ],
            )?;
            prune_transfer_summaries(connection)?;
            Ok(())
        });
        if database_result.is_err() {
            return Err(LocalLinkTransferFailure::Duplicate);
        }
        let transfer_id = offer.transfer_id.clone();
        runtime.pending_inbound.insert(
            transfer_id.clone(),
            PendingInbound {
                peer_device_id,
                offer,
                content: None,
                viewed: false,
            },
        );
        {
            let mut receipts = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned");
            let _ = receipts.begin(transfer_id.clone());
            let _ = receipts.advance(&transfer_id, TransferPhase::Encrypted);
        }
        self.deadline_reaper.notify();
        Ok(transfer_id)
    }

    #[allow(dead_code)]
    pub(crate) fn commit_inbound_payload(
        &self,
        transfer_id: &str,
        payload: Vec<u8>,
    ) -> Result<LocalLinkTransfer, LocalLinkTransferFailure> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        let pending = runtime
            .pending_inbound
            .get_mut(transfer_id)
            .ok_or(LocalLinkTransferFailure::PayloadUnavailable)?;
        pending.offer.verify_payload(&payload)?;
        if pending.content.is_some() {
            return Err(LocalLinkTransferFailure::Duplicate);
        }
        let content =
            String::from_utf8(payload).map_err(|_| LocalLinkTransferFailure::ProtocolViolation)?;
        let content = Zeroizing::new(content);
        let transfer = self
            .database
            .with_local_link_connection(|connection| {
                let timestamp = now();
                let changed = connection.execute(
                    "UPDATE local_link_transfers
                     SET status = 'awaitingReceiver', updated_at = ?1
                     WHERE id = ?2 AND status = 'encrypted'",
                    params![timestamp, transfer_id],
                )?;
                if changed != 1 {
                    return Err(AppError::InvalidInput("duplicate".to_owned()));
                }
                local_link_transfer_with_connection(connection, transfer_id)?
                    .ok_or(AppError::NotFound)
            })
            .map_err(|_| LocalLinkTransferFailure::Duplicate)?;
        pending.content = Some(content);
        let _ = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .advance(transfer_id, TransferPhase::AwaitingReceiver);
        Ok(transfer)
    }

    fn set_receiver_available(&self, available: bool) -> AppResult<()> {
        let mut runtime = self
            .runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned");
        runtime.receiver_available = available;
        if !available {
            let persisted = self.database.with_local_link_connection(|connection| {
                let transaction = connection.unchecked_transaction()?;
                let mut statement = transaction.prepare(
                    "SELECT transfers.id, transfers.direction, transfers.status,
                            EXISTS (
                                SELECT 1 FROM local_link_effect_claims claims
                                WHERE claims.transfer_id = transfers.id
                                  AND claims.state = 'prepared'
                            )
                     FROM local_link_transfers transfers
                     WHERE transfers.status IN
                           ('connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'reconciling')",
                )?;
                let active = statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)? != 0,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                drop(statement);
                let timestamp = now();
                let deadline = timestamp_from_millis(
                    Utc::now()
                        .timestamp_millis()
                        .saturating_add(STATUS_RECONCILIATION_BUDGET_MS),
                );
                let mut updates = Vec::with_capacity(active.len());
                for (transfer_id, direction, status, prepared) in active {
                    let receipt = if direction == "incoming" {
                        let receipt = if prepared {
                            TerminalReceipt::outcome_unknown()
                        } else {
                            TerminalReceipt::not_delivered(
                                LocalLinkTransferFailure::ReceiverLocked,
                            )
                        };
                        transaction.execute(
                            "UPDATE local_link_transfers
                             SET status = 'failed', failure_reason = ?1,
                                 receiver_action = NULL, updated_at = ?2,
                                 completed_at = ?2, expires_at = NULL
                             WHERE id = ?3",
                            params![
                                if prepared { "outcomeUnknown" } else { "receiverLocked" },
                                timestamp,
                                transfer_id
                            ],
                        )?;
                        Some(receipt)
                    } else if matches!(status.as_str(), "awaitingReceiver" | "viewed" | "reconciling") {
                        transaction.execute(
                            "UPDATE local_link_transfers
                             SET status = 'reconciling', failure_reason = NULL,
                                 receiver_action = NULL, updated_at = ?1,
                                 completed_at = NULL, expires_at = COALESCE(expires_at, ?2)
                             WHERE id = ?3",
                            params![timestamp, deadline, transfer_id],
                        )?;
                        None
                    } else {
                        let receipt = TerminalReceipt::not_delivered(
                            LocalLinkTransferFailure::PayloadUnavailable,
                        );
                        transaction.execute(
                            "UPDATE local_link_transfers
                             SET status = 'failed', failure_reason = 'payloadUnavailable',
                                 receiver_action = NULL, updated_at = ?1,
                                 completed_at = ?1, expires_at = NULL
                             WHERE id = ?2",
                            params![timestamp, transfer_id],
                        )?;
                        Some(receipt)
                    };
                    if prepared {
                        transaction.execute(
                            "UPDATE local_link_effect_claims
                             SET state = 'uncertain', updated_at = ?1
                             WHERE transfer_id = ?2 AND state = 'prepared'",
                            params![timestamp, transfer_id],
                        )?;
                    }
                    updates.push((transfer_id, receipt));
                }
                transaction.commit()?;
                Ok(updates)
            });

            runtime.pending_inbound.clear();
            let waiters = std::mem::take(&mut runtime.receipt_waiters);
            let pairing_signals = runtime
                .pairing_sessions
                .drain()
                .map(|(_, session)| session.confirm)
                .collect::<Vec<_>>();
            let outgoing_signals = runtime
                .outgoing_cancel_waiters
                .drain()
                .map(|(_, signal)| signal)
                .collect::<Vec<_>>();
            drop(runtime);
            for pairing in pairing_signals {
                let _ = pairing.send(PairingSignal::Cancel);
            }
            for outgoing in outgoing_signals {
                let _ = outgoing.send(OutgoingTransferSignal::Cancel);
            }
            let updates = match persisted {
                Ok(updates) => updates,
                Err(error) => {
                    let receipt = TerminalReceipt::outcome_unknown();
                    let mut receipts = self
                        .receipts
                        .lock()
                        .expect("Local Link receipt mutex poisoned");
                    for (transfer_id, waiter) in waiters {
                        let _ = receipts.apply_remote_receipt(&transfer_id, receipt);
                        let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
                    }
                    self.deadline_reaper.notify();
                    return Err(error);
                }
            };
            let mut receipts = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned");
            for (transfer_id, receipt) in &updates {
                if let Some(receipt) = receipt {
                    let _ = receipts.apply_receipt(transfer_id, *receipt);
                }
            }
            drop(receipts);
            for (transfer_id, waiter) in waiters {
                let receipt = updates
                    .iter()
                    .find_map(|(id, receipt)| (id == &transfer_id).then_some(*receipt))
                    .flatten()
                    .unwrap_or_else(TerminalReceipt::outcome_unknown);
                let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
            }
            self.deadline_reaper.notify();
        }
        Ok(())
    }

    fn local_session_hello(&self, session_id: String) -> AppResult<WireMessage> {
        let identity = self.ensure_identity()?;
        let preferences = self.preferences()?;
        ensure_link_enabled(&preferences)?;
        Ok(WireMessage::SessionHello {
            version: PROTOCOL_VERSION,
            session_id,
            device_id: local_device_id(&identity),
            display_name: preferences.device_name,
        })
    }

    fn open_outbound_pairing(
        &self,
        endpoint: SocketAddr,
        identity: Arc<LocalIdentity>,
    ) -> AppResult<LocalLinkDevice> {
        let stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(4))
            .map_err(transport_error)?;
        configure_stream(&stream)?;
        let mut channel = SecureChannel::initiator(stream, &identity)?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let local_hello = self.local_session_hello(session_id.clone())?;
        let (local_device_id, local_display_name) = match &local_hello {
            WireMessage::SessionHello {
                device_id,
                display_name,
                ..
            } => (device_id.clone(), display_name.clone()),
            _ => unreachable!("local session helper always creates SessionHello"),
        };
        channel.send_message(&local_hello)?;
        let remote_hello = receive_hello(&mut channel)?;
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let transcript = transcript_fingerprint(channel.handshake_hash());
        let proposal = PairingProposal {
            version: PROTOCOL_VERSION,
            pairing_id: pairing_id.clone(),
            session_id: session_id.clone(),
            device_id: local_device_id,
            display_name: local_display_name,
            public_key_fingerprint: identity.fingerprint().to_owned(),
            expires_at_ms: Utc::now().timestamp_millis() + TRANSFER_TTL_SECONDS * 1_000,
        };
        channel.send_message(&WireMessage::PairingProposal(proposal))?;
        let (device, receiver) = self.register_pairing_session(
            &remote_hello,
            channel.remote_static(),
            pairing_id.clone(),
            session_id.clone(),
            transcript,
            pairing_code(channel.handshake_hash()),
        )?;
        let service = self.self_weak.clone();
        let _ = thread::Builder::new()
            .name("clipriva-local-link-pairing".to_owned())
            .spawn(move || {
                if let Some(service) = service.upgrade() {
                    let _ = service.await_pairing_confirmation(
                        channel,
                        remote_hello,
                        pairing_id,
                        session_id,
                        receiver,
                    );
                }
            })
            .map_err(transport_error)?;
        Ok(device)
    }

    fn handle_inbound_connection(&self, stream: TcpStream) {
        let _ = configure_stream(&stream);
        let result = (|| -> AppResult<()> {
            let identity = self.ensure_identity()?;
            let mut channel = SecureChannel::responder(stream, &identity)?;
            let remote_hello = receive_hello(&mut channel)?;
            let local_session_id = uuid::Uuid::new_v4().to_string();
            channel.send_message(&self.local_session_hello(local_session_id)?)?;
            match channel.receive_message()? {
                WireMessage::PairingProposal(proposal) => {
                    if proposal.device_id != remote_hello.device_id
                        || proposal.display_name != remote_hello.display_name
                        || proposal.public_key_fingerprint != fingerprint(channel.remote_static())
                        || proposal.session_id != remote_hello.session_id
                    {
                        return Err(AppError::InvalidInput(
                            "Local Link pairing identity did not match the authenticated session."
                                .to_owned(),
                        ));
                    }
                    let pairing_id = proposal.pairing_id.clone();
                    let session_id = proposal.session_id.clone();
                    let (.., receiver) = self.register_pairing_session(
                        &remote_hello,
                        channel.remote_static(),
                        pairing_id.clone(),
                        session_id.clone(),
                        transcript_fingerprint(channel.handshake_hash()),
                        pairing_code(channel.handshake_hash()),
                    )?;
                    self.await_pairing_confirmation(
                        channel,
                        remote_hello,
                        pairing_id,
                        session_id,
                        receiver,
                    )
                }
                WireMessage::Offer(offer) => {
                    self.receive_inbound_offer(channel, remote_hello, offer)
                }
                WireMessage::StatusQuery { transfer_id } => {
                    self.respond_to_status_query(channel, remote_hello, transfer_id)
                }
                _ => Err(AppError::InvalidInput(
                    "Local Link received an unexpected session operation.".to_owned(),
                )),
            }
        })();
        if let Err(error) = result {
            // Deliberately content-free: transport failures are diagnosable
            // without logging peer payloads, endpoints, or plaintext.
            eprintln!("Local Link session closed: {error}");
        }
    }

    /// Resolve one authenticated metadata-only query. A caller can inspect
    /// only an incoming transfer that belongs to its Noise-authenticated
    /// device ID; every mismatch is deliberately indistinguishable from an
    /// unknown transfer.
    fn respond_to_status_query(
        &self,
        mut channel: SecureChannel<TcpStream>,
        remote: SessionHello,
        transfer_id: String,
    ) -> AppResult<()> {
        if self
            .authenticate_peer(&remote.device_id, channel.remote_static())
            .is_err()
        {
            return Err(AppError::InvalidInput(
                "Local Link status query identity was not trusted.".to_owned(),
            ));
        }
        let transfer = self.database.with_local_link_connection(|connection| {
            let transfer = local_link_transfer_with_connection(connection, &transfer_id)?;
            Ok(transfer.filter(|transfer| {
                transfer.direction == LocalLinkTransferDirection::Incoming
                    && transfer.device_id == remote.device_id
            }))
        })?;
        let status = match transfer {
            None => StatusResponseState::Unknown,
            Some(transfer) => terminal_receipt_for_transfer(&transfer)
                .map(StatusResponseState::Terminal)
                .unwrap_or_else(|| {
                    if self
                        .runtime
                        .lock()
                        .expect("Local Link runtime mutex poisoned")
                        .pending_inbound
                        .contains_key(&transfer_id)
                    {
                        StatusResponseState::Pending
                    } else {
                        // SQLite may have failed while a lifecycle boundary
                        // still cleared the only in-memory body. Never report
                        // that stale row as pending or invite a body replay.
                        StatusResponseState::Terminal(TerminalReceipt::outcome_unknown())
                    }
                }),
        };
        channel.send_message(&WireMessage::StatusResponse {
            transfer_id,
            status,
        })
    }

    fn register_pairing_session(
        &self,
        remote: &SessionHello,
        peer_public_key: &[u8],
        pairing_id: String,
        session_id: String,
        transcript: String,
        sas: String,
    ) -> AppResult<(LocalLinkDevice, Receiver<PairingSignal>)> {
        if peer_public_key.len() != 32 {
            return Err(AppError::InvalidInput(
                "The Local Link peer identity is invalid. No trust record was created.".to_owned(),
            ));
        }
        self.expire_trusted_devices()?;
        let expires_at_ms = Utc::now().timestamp_millis() + TRANSFER_TTL_SECONDS * 1_000;
        let peer_fingerprint = fingerprint(peer_public_key);
        if !self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .is_empty()
        {
            return Err(AppError::InvalidInput(
                "A Local Link pairing session is already active on this Mac.".to_owned(),
            ));
        }
        let device = self.database.with_local_link_connection(|connection| {
            if let Some(existing) =
                local_link_device_with_connection(connection, &remote.device_id)?
            {
                if existing.trust_status == LocalLinkDeviceTrustStatus::Trusted {
                    let key: Vec<u8> = connection.query_row(
                        "SELECT peer_public_key FROM local_link_devices WHERE device_id = ?1",
                        [&remote.device_id],
                        |row| row.get(0),
                    )?;
                    if key.as_slice() != peer_public_key {
                        return Err(AppError::InvalidInput(
                            "The Local Link peer identity changed. Reset or revoke and pair again."
                                .to_owned(),
                        ));
                    }
                    return Err(AppError::InvalidInput(
                        "This Local Link device is already trusted.".to_owned(),
                    ));
                }
            }
            let timestamp = now();
            connection.execute(
                "INSERT INTO local_link_devices
                 (device_id, display_name, peer_public_key, public_key_fingerprint,
                  trust_status, paired_at, trusted_at, revoked_at, last_seen_at,
                  protocol_min, protocol_max)
                 VALUES (?1, ?2, X'', '', 'pairing', ?3, NULL, NULL, ?3, ?4, ?4)
                 ON CONFLICT(device_id) DO UPDATE SET
                   display_name = excluded.display_name, peer_public_key = X'',
                   public_key_fingerprint = '', trust_status = 'pairing',
                   paired_at = excluded.paired_at, trusted_at = NULL, revoked_at = NULL,
                   last_seen_at = excluded.last_seen_at, protocol_min = excluded.protocol_min,
                   protocol_max = excluded.protocol_max",
                params![
                    remote.device_id,
                    remote.display_name,
                    timestamp,
                    i64::from(PROTOCOL_VERSION)
                ],
            )?;
            local_link_device_with_connection(connection, &remote.device_id)?
                .ok_or(AppError::NotFound)
        })?;
        let (sender, receiver) = mpsc::channel();
        let mut runtime = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned");
        if !runtime.pairing_sessions.is_empty() {
            return Err(AppError::InvalidInput(
                "A Local Link pairing session is already active on this Mac.".to_owned(),
            ));
        }
        // Starting a new ceremony replaces any prior session-only grant for
        // this device. The old in-memory key must not remain usable while the
        // person verifies a new identity.
        runtime.session_trusted_devices.remove(&remote.device_id);
        runtime.pairing_sessions.insert(
            remote.device_id.clone(),
            PairingSession {
                pairing_id,
                session_id,
                transcript_fingerprint: transcript,
                peer_fingerprint,
                sas,
                expires_at_ms,
                confirm: sender,
                local_confirmed: false,
                trust_duration: None,
            },
        );
        Ok((device, receiver))
    }

    fn await_pairing_confirmation(
        &self,
        mut channel: SecureChannel<TcpStream>,
        remote: SessionHello,
        pairing_id: String,
        session_id: String,
        receiver: Receiver<PairingSignal>,
    ) -> AppResult<()> {
        let (transcript, deadline) = {
            let runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            let session = runtime
                .pairing_sessions
                .get(&remote.device_id)
                .ok_or_else(|| {
                    AppError::InvalidInput("Local Link pairing was cancelled.".to_owned())
                })?;
            if session.pairing_id != pairing_id || session.session_id != session_id {
                return Err(AppError::InvalidInput(
                    "Local Link pairing session identifiers do not match.".to_owned(),
                ));
            }
            (
                session.transcript_fingerprint.clone(),
                session.expires_at_ms,
            )
        };
        let wait = Duration::from_millis(
            deadline
                .saturating_sub(Utc::now().timestamp_millis())
                .max(0) as u64,
        );
        match receiver.recv_timeout(wait) {
            Ok(PairingSignal::Confirm) => {}
            Ok(PairingSignal::Cancel) | Err(_) => {
                self.clear_pairing_candidate(&remote.device_id, &pairing_id);
                return Err(AppError::InvalidInput(
                    "Local Link pairing expired or was cancelled. No trust record was created."
                        .to_owned(),
                ));
            }
        }
        channel.send_message(&WireMessage::PairingConfirm(PairingConfirm {
            version: PROTOCOL_VERSION,
            pairing_id: pairing_id.clone(),
            session_id: session_id.clone(),
            transcript_fingerprint: transcript.clone(),
            confirmed: true,
        }))?;
        let remote_confirm = match channel.receive_message()? {
            WireMessage::PairingConfirm(confirm) => confirm,
            _ => {
                self.clear_pairing_candidate(&remote.device_id, &pairing_id);
                return Err(AppError::InvalidInput(
                    "Local Link pairing confirmation is missing.".to_owned(),
                ));
            }
        };
        if !remote_confirm.confirmed
            || remote_confirm.pairing_id != pairing_id
            || remote_confirm.session_id != session_id
            || remote_confirm.transcript_fingerprint != transcript
        {
            self.clear_pairing_candidate(&remote.device_id, &pairing_id);
            return Err(AppError::InvalidInput(
                "Local Link pairing confirmation did not match the encrypted transcript."
                    .to_owned(),
            ));
        }
        let peer_fingerprint = fingerprint(channel.remote_static());
        let local_identity = self.ensure_identity()?;
        let local_device_id = local_device_id(local_identity.as_ref());
        channel.send_message(&WireMessage::PairingCommit(PairingCommit {
            version: PROTOCOL_VERSION,
            pairing_id: pairing_id.clone(),
            session_id: session_id.clone(),
            device_id: local_device_id,
            public_key_fingerprint: self.ensure_identity()?.fingerprint().to_owned(),
        }))?;
        let remote_commit = match channel.receive_message()? {
            WireMessage::PairingCommit(commit) => commit,
            _ => {
                self.clear_pairing_candidate(&remote.device_id, &pairing_id);
                return Err(AppError::InvalidInput(
                    "Local Link pairing commit is missing.".to_owned(),
                ));
            }
        };
        if remote_commit.pairing_id != pairing_id
            || remote_commit.session_id != session_id
            || remote_commit.device_id != remote.device_id
            || remote_commit.public_key_fingerprint != peer_fingerprint
        {
            self.clear_pairing_candidate(&remote.device_id, &pairing_id);
            return Err(AppError::InvalidInput(
                "Local Link pairing commit did not match the authenticated identity.".to_owned(),
            ));
        }
        self.complete_pairing(&remote.device_id, &pairing_id, channel.remote_static())
    }

    fn clear_pairing_candidate(&self, device_id: &str, pairing_id: &str) {
        let removed = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .get(device_id)
            .is_some_and(|session| session.pairing_id == pairing_id);
        if removed {
            self.runtime
                .lock()
                .expect("Local Link runtime mutex poisoned")
                .pairing_sessions
                .remove(device_id);
            let _ = self.database.with_local_link_connection(|connection| {
                connection.execute(
                    "DELETE FROM local_link_devices WHERE device_id = ?1 AND trust_status = 'pairing'",
                    [device_id],
                )?;
                Ok(())
            });
        }
    }

    fn complete_pairing(
        &self,
        device_id: &str,
        pairing_id: &str,
        peer_public_key: &[u8],
    ) -> AppResult<()> {
        let session = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .remove(device_id)
            .ok_or_else(|| {
                AppError::InvalidInput("Local Link pairing was cancelled.".to_owned())
            })?;
        if session.pairing_id != pairing_id
            || !session.local_confirmed
            || peer_public_key.len() != 32
        {
            return Err(AppError::InvalidInput(
                "Local Link pairing did not complete a verified bilateral confirmation.".to_owned(),
            ));
        }
        let trust_duration = session.trust_duration.unwrap_or_default();
        let timestamp = now();
        let fingerprint = fingerprint(peer_public_key);
        match trust_duration {
            LocalLinkTrustDuration::ThisSession => {
                // Keep a metadata row for the transfer FK and device history,
                // but never persist a session peer key or a trusted state.
                self.database.with_local_link_connection(|connection| {
                    let changed = connection.execute(
                        "UPDATE local_link_devices
                         SET peer_public_key = X'', public_key_fingerprint = '',
                             trust_status = 'needsRePairing', trusted_at = NULL,
                             revoked_at = NULL, last_seen_at = ?1,
                             trust_duration = 'thirtyDays'
                         WHERE device_id = ?2 AND trust_status = 'pairing'",
                        params![timestamp, device_id],
                    )?;
                    if changed != 1 {
                        return Err(AppError::InvalidInput(
                            "Local Link pairing trust state changed before commit.".to_owned(),
                        ));
                    }
                    Ok(())
                })?;
                self.runtime
                    .lock()
                    .expect("Local Link runtime mutex poisoned")
                    .session_trusted_devices
                    .insert(
                        device_id.to_owned(),
                        SessionTrustedDevice {
                            public_key: peer_public_key.to_vec(),
                            public_key_fingerprint: fingerprint,
                            paired_at: timestamp,
                        },
                    );
            }
            LocalLinkTrustDuration::ThirtyDays | LocalLinkTrustDuration::Always => {
                self.database.with_local_link_connection(|connection| {
                    let changed = connection.execute(
                        "UPDATE local_link_devices
                         SET peer_public_key = ?1, public_key_fingerprint = ?2,
                             trust_status = 'trusted', trusted_at = ?3, last_seen_at = ?3,
                             trust_duration = ?4
                         WHERE device_id = ?5 AND trust_status = 'pairing'",
                        params![
                            peer_public_key,
                            fingerprint,
                            timestamp,
                            trust_duration.as_storage_value(),
                            device_id
                        ],
                    )?;
                    if changed != 1 {
                        return Err(AppError::InvalidInput(
                            "Local Link pairing trust state changed before commit.".to_owned(),
                        ));
                    }
                    Ok(())
                })?;
            }
        }
        self.record_authenticated_device_presence(device_id);
        Ok(())
    }

    fn receive_inbound_offer(
        &self,
        mut channel: SecureChannel<TcpStream>,
        remote: SessionHello,
        offer: TransferOffer,
    ) -> AppResult<()> {
        if offer.session_id != remote.session_id {
            return Err(AppError::InvalidInput(
                "Local Link offer did not match its authenticated session.".to_owned(),
            ));
        }
        let transfer_id = offer.transfer_id.clone();
        if let Err(failure) = self.authenticate_peer(&remote.device_id, channel.remote_static()) {
            channel.send_message(&WireMessage::OfferDecision {
                transfer_id,
                decision: OfferDecision::Rejected(failure),
            })?;
            return Err(failure_to_error(failure));
        }
        match self.reserve_inbound_offer(channel.remote_static(), offer) {
            Ok(_) => channel.send_message(&WireMessage::OfferDecision {
                transfer_id: transfer_id.clone(),
                decision: OfferDecision::Ready,
            })?,
            Err(failure) => {
                channel.send_message(&WireMessage::OfferDecision {
                    transfer_id,
                    decision: OfferDecision::Rejected(failure),
                })?;
                return Ok(());
            }
        }
        let byte_size = self.database.with_local_link_connection(|connection| {
            connection
                .query_row(
                    "SELECT byte_size FROM local_link_transfers WHERE id = ?1",
                    [&transfer_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(AppError::from)
        })?;
        let payload = match channel.receive_payload_or_cancel(
            &transfer_id,
            usize::try_from(byte_size).unwrap_or(MAX_TEXT_PAYLOAD_BYTES + 1),
        )? {
            PayloadOrCancel::Payload(payload) => payload,
            PayloadOrCancel::Cancelled => {
                let receipt = self.cancel_inbound_transfer(&transfer_id)?;
                channel.send_message(&WireMessage::Receipt {
                    transfer_id: transfer_id.clone(),
                    receipt,
                })?;
                self.await_receipt_ack(&mut channel, &transfer_id, receipt)?;
                return Ok(());
            }
        };
        match channel.receive_message()? {
            WireMessage::PayloadComplete {
                transfer_id: completed_id,
            } if completed_id == transfer_id => {}
            _ => {
                self.mark_inbound_network_failure(
                    &transfer_id,
                    LocalLinkTransferFailure::ProtocolViolation,
                )?;
                return Err(AppError::InvalidInput(
                    "Local Link payload completion did not match the offer.".to_owned(),
                ));
            }
        }
        self.commit_inbound_payload(&transfer_id, payload)
            .map_err(failure_to_error)?;
        let (sender, receiver) = mpsc::channel();
        let already_viewed = {
            let mut runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            let Some(pending) = runtime.pending_inbound.get(&transfer_id) else {
                return Err(AppError::InvalidInput(
                    "Local Link incoming payload is no longer available.".to_owned(),
                ));
            };
            let already_viewed = pending.viewed;
            runtime.receipt_waiters.insert(transfer_id.clone(), sender);
            already_viewed
        };
        if already_viewed {
            channel.send_message(&WireMessage::Viewed {
                transfer_id: transfer_id.clone(),
            })?;
        }
        channel.set_poll_timeout(TRANSFER_POLL_INTERVAL)?;
        let deadline_ms = self.inbound_deadline(&transfer_id)?;
        let receipt = loop {
            match channel.poll_message()? {
                Some(WireMessage::Cancel {
                    transfer_id: cancelled_id,
                }) if cancelled_id == transfer_id => {
                    break self.cancel_inbound_transfer(&transfer_id)?;
                }
                Some(_) => {
                    self.mark_inbound_network_failure(
                        &transfer_id,
                        LocalLinkTransferFailure::ProtocolViolation,
                    )?;
                    return Err(AppError::InvalidInput(
                        "Local Link received an unexpected transfer message.".to_owned(),
                    ));
                }
                None => {}
            }
            let wait = Duration::from_millis(
                deadline_ms
                    .saturating_sub(Utc::now().timestamp_millis())
                    .min(TRANSFER_POLL_INTERVAL.as_millis() as i64)
                    .max(0) as u64,
            );
            match receiver.recv_timeout(wait) {
                Ok(InboundTransferSignal::Viewed) => {
                    channel.send_message(&WireMessage::Viewed {
                        transfer_id: transfer_id.clone(),
                    })?;
                }
                Ok(InboundTransferSignal::Terminal(receipt)) => break receipt,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if Utc::now().timestamp_millis() < deadline_ms {
                        continue;
                    }
                    self.expire_pending(Utc::now().timestamp_millis())?;
                    break TerminalReceipt::completed(ReceiptOutcome::Expired);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.expire_pending(Utc::now().timestamp_millis())?;
                    break TerminalReceipt::completed(ReceiptOutcome::Expired);
                }
            }
        };
        #[cfg(test)]
        {
            let mut runtime = self
                .runtime
                .lock()
                .expect("Local Link runtime mutex poisoned");
            if runtime.drop_next_terminal_receipt {
                runtime.drop_next_terminal_receipt = false;
                return Ok(());
            }
        }
        channel.send_message(&WireMessage::Receipt {
            transfer_id: transfer_id.clone(),
            receipt,
        })?;
        self.await_receipt_ack(&mut channel, &transfer_id, receipt)?;
        Ok(())
    }

    /// Acknowledge the receiver's terminal receipt. A duplicate Cancel after
    /// the receiver is terminal simply replays the same content-free receipt;
    /// it cannot reopen or replace that terminal state.
    fn await_receipt_ack(
        &self,
        channel: &mut SecureChannel<TcpStream>,
        transfer_id: &str,
        receipt: TerminalReceipt,
    ) -> AppResult<()> {
        channel.set_poll_timeout(TRANSFER_POLL_INTERVAL)?;
        // Keep the session finite if the peer vanishes after a receipt. The
        // terminal summary remains available for a future status convergence;
        // this bounded live connection retains the existing no-retry transport
        // policy.
        for _ in 0..200 {
            match channel.poll_message()? {
                Some(WireMessage::ReceiptAck {
                    transfer_id: ack_id,
                    receipt: acknowledged,
                }) if ack_id == transfer_id && acknowledged == receipt => return Ok(()),
                Some(WireMessage::Cancel {
                    transfer_id: cancelled_id,
                }) if cancelled_id == transfer_id => {
                    channel.send_message(&WireMessage::Receipt {
                        transfer_id: transfer_id.to_owned(),
                        receipt,
                    })?;
                }
                Some(_) => {
                    return Err(AppError::InvalidInput(
                        "Local Link receipt acknowledgement did not converge.".to_owned(),
                    ));
                }
                None => continue,
            }
        }
        Ok(())
    }

    fn deliver_outgoing(
        &self,
        endpoints: Vec<SocketAddr>,
        device_id: String,
        transfer_id: String,
        content: Zeroizing<String>,
        generation: u64,
    ) {
        let (cancel_sender, cancel_receiver) = mpsc::channel();
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .outgoing_cancel_waiters
            .insert(transfer_id.clone(), cancel_sender);

        let result = (|| -> Result<(), DeliveryFailure> {
            if !self.delivery_generation_is_active(generation) {
                return Err(DeliveryFailure::before_payload(
                    LocalLinkTransferFailure::PayloadUnavailable,
                ));
            }
            if self.outgoing_cancellation_requested(&transfer_id, &cancel_receiver) {
                return Ok(());
            }
            let identity = self.ensure_identity().map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::AuthFailed)
            })?;
            let mut last_failure =
                DeliveryFailure::before_payload(LocalLinkTransferFailure::DeviceUnavailable);
            for endpoint in endpoints {
                if !self.delivery_generation_is_active(generation) {
                    return Err(DeliveryFailure::before_payload(
                        LocalLinkTransferFailure::PayloadUnavailable,
                    ));
                }
                if self.outgoing_cancellation_requested(&transfer_id, &cancel_receiver) {
                    return Ok(());
                }
                match self.deliver_to_endpoint(
                    endpoint,
                    &identity,
                    &device_id,
                    &transfer_id,
                    content.as_bytes(),
                    &cancel_receiver,
                    generation,
                ) {
                    Ok(()) => return Ok(()),
                    Err(failure) if failure.receiver_may_have_payload => {
                        return Err(failure.at_endpoint(endpoint))
                    }
                    Err(failure) => last_failure = failure,
                }
            }
            Err(last_failure)
        })();

        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .outgoing_cancel_waiters
            .remove(&transfer_id);
        if let Err(failure) = result {
            // A local cancellation is terminal and must never be rewritten as
            // a transport failure if the peer session closed while Cancel was
            // in flight.
            if !self.outgoing_is_cancelled(&transfer_id) {
                if failure.receiver_may_have_payload {
                    if self.mark_outgoing_reconciling(&transfer_id).is_ok() {
                        self.start_status_reconciliation(
                            transfer_id,
                            failure.reconciliation_endpoint,
                        );
                    }
                } else {
                    let _ = self.mark_outgoing_failed(&transfer_id, failure.reason);
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the endpoint attempt keeps explicit borrowed security and lifecycle boundaries"
    )]
    fn deliver_to_endpoint(
        &self,
        endpoint: SocketAddr,
        identity: &LocalIdentity,
        device_id: &str,
        transfer_id: &str,
        content: &[u8],
        cancel_receiver: &Receiver<OutgoingTransferSignal>,
        generation: u64,
    ) -> Result<(), DeliveryFailure> {
        let stream =
            TcpStream::connect_timeout(&endpoint, DELIVERY_LIFECYCLE_IO_TIMEOUT).map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::DeviceUnavailable)
            })?;
        if !self.delivery_generation_is_active(generation) {
            return Err(DeliveryFailure::before_payload(
                LocalLinkTransferFailure::PayloadUnavailable,
            ));
        }
        configure_stream(&stream).map_err(|_| {
            DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
        })?;
        stream
            .set_read_timeout(Some(DELIVERY_LIFECYCLE_IO_TIMEOUT))
            .and_then(|_| stream.set_write_timeout(Some(DELIVERY_LIFECYCLE_IO_TIMEOUT)))
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        let mut channel = SecureChannel::initiator(stream, identity)
            .map_err(|_| DeliveryFailure::before_payload(LocalLinkTransferFailure::AuthFailed))?;
        let session_id = uuid::Uuid::new_v4().to_string();
        channel
            .send_message(&self.local_session_hello(session_id.clone()).map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::AuthFailed)
            })?)
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        let remote = receive_hello(&mut channel).map_err(|_| {
            DeliveryFailure::before_payload(LocalLinkTransferFailure::ProtocolViolation)
        })?;
        if !self.delivery_generation_is_active(generation) {
            return Err(DeliveryFailure::before_payload(
                LocalLinkTransferFailure::PayloadUnavailable,
            ));
        }
        if remote.device_id != device_id {
            return Err(DeliveryFailure::before_payload(
                LocalLinkTransferFailure::AuthFailed,
            ));
        }
        self.authenticate_peer(device_id, channel.remote_static())
            .map_err(DeliveryFailure::before_payload)?;
        self.advance_outgoing(transfer_id, LocalLinkTransferStatus::Encrypted)
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::PayloadUnavailable)
            })?;
        if self.outgoing_cancellation_requested(transfer_id, cancel_receiver) {
            return Ok(());
        }
        channel
            .set_poll_timeout(TRANSFER_POLL_INTERVAL)
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        let offer = TransferOffer::new(session_id, transfer_id.to_owned(), content);
        channel
            .send_message(&WireMessage::Offer(offer))
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        loop {
            if !self.delivery_generation_is_active(generation) {
                return Err(DeliveryFailure::before_payload(
                    LocalLinkTransferFailure::PayloadUnavailable,
                ));
            }
            if self.outgoing_cancellation_requested(transfer_id, cancel_receiver) {
                return self.send_cancel_and_wait_for_receipt(
                    &mut channel,
                    transfer_id,
                    cancel_receiver,
                    generation,
                );
            }
            match channel.poll_message().map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })? {
                Some(WireMessage::OfferDecision {
                    transfer_id: response_id,
                    decision: OfferDecision::Ready,
                }) if response_id == transfer_id => break,
                Some(WireMessage::OfferDecision {
                    transfer_id: response_id,
                    decision: OfferDecision::Rejected(failure),
                }) if response_id == transfer_id => {
                    return Err(DeliveryFailure::before_payload(failure))
                }
                Some(_) => {
                    return Err(DeliveryFailure::before_payload(
                        LocalLinkTransferFailure::ProtocolViolation,
                    ))
                }
                None => continue,
            }
        }
        if self.outgoing_cancellation_requested(transfer_id, cancel_receiver) {
            return self.send_cancel_and_wait_for_receipt(
                &mut channel,
                transfer_id,
                cancel_receiver,
                generation,
            );
        }
        if !self.delivery_generation_is_active(generation) {
            return Err(DeliveryFailure::before_payload(
                LocalLinkTransferFailure::PayloadUnavailable,
            ));
        }
        channel.send_payload(transfer_id, content).map_err(|_| {
            DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
        })?;
        // This is the durable no-replay boundary. Before it, a crash is proven
        // not delivered because the receiver cannot act without a complete
        // payload and PayloadComplete. After it, persist only reconciliation
        // metadata before attempting the commit frame.
        self.advance_outgoing(transfer_id, LocalLinkTransferStatus::AwaitingReceiver)
            .map_err(|_| {
                DeliveryFailure::after_payload(LocalLinkTransferFailure::PayloadUnavailable)
            })?;
        if !self.delivery_generation_is_active(generation) {
            return Err(DeliveryFailure::after_payload(
                LocalLinkTransferFailure::PayloadUnavailable,
            ));
        }
        channel
            .send_message(&WireMessage::PayloadComplete {
                transfer_id: transfer_id.to_owned(),
            })
            .map_err(|_| {
                DeliveryFailure::after_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        self.wait_for_outgoing_receipt(
            &mut channel,
            transfer_id,
            cancel_receiver,
            false,
            true,
            generation,
        )
    }

    fn send_cancel_and_wait_for_receipt(
        &self,
        channel: &mut SecureChannel<TcpStream>,
        transfer_id: &str,
        cancel_receiver: &Receiver<OutgoingTransferSignal>,
        generation: u64,
    ) -> Result<(), DeliveryFailure> {
        channel
            .send_message(&WireMessage::Cancel {
                transfer_id: transfer_id.to_owned(),
            })
            .map_err(|_| {
                DeliveryFailure::before_payload(LocalLinkTransferFailure::TransportUnavailable)
            })?;
        self.wait_for_outgoing_receipt(
            channel,
            transfer_id,
            cancel_receiver,
            true,
            false,
            generation,
        )
    }

    /// Wait for the receiver's terminal receipt without blocking cancellation.
    /// A receipt can legitimately replace the sender's provisional cancelled
    /// summary only when the receiver's decision had already won before it
    /// observed the Cancel frame.
    fn wait_for_outgoing_receipt(
        &self,
        channel: &mut SecureChannel<TcpStream>,
        transfer_id: &str,
        cancel_receiver: &Receiver<OutgoingTransferSignal>,
        mut cancel_sent: bool,
        receiver_may_have_payload: bool,
        generation: u64,
    ) -> Result<(), DeliveryFailure> {
        let delivery_failure = |reason| {
            if receiver_may_have_payload {
                DeliveryFailure::after_payload(reason)
            } else {
                DeliveryFailure::before_payload(reason)
            }
        };
        loop {
            if !self.delivery_generation_is_active(generation) {
                return Err(delivery_failure(
                    LocalLinkTransferFailure::PayloadUnavailable,
                ));
            }
            if !cancel_sent && self.outgoing_cancellation_requested(transfer_id, cancel_receiver) {
                channel
                    .send_message(&WireMessage::Cancel {
                        transfer_id: transfer_id.to_owned(),
                    })
                    .map_err(|_| {
                        delivery_failure(LocalLinkTransferFailure::TransportUnavailable)
                    })?;
                cancel_sent = true;
            }
            match channel
                .poll_message()
                .map_err(|_| delivery_failure(LocalLinkTransferFailure::PayloadUnavailable))?
            {
                Some(WireMessage::Viewed {
                    transfer_id: viewed_id,
                }) if viewed_id == transfer_id => {
                    if !cancel_sent {
                        if self.outgoing_cancellation_requested(transfer_id, cancel_receiver) {
                            channel
                                .send_message(&WireMessage::Cancel {
                                    transfer_id: transfer_id.to_owned(),
                                })
                                .map_err(|_| {
                                    delivery_failure(LocalLinkTransferFailure::TransportUnavailable)
                                })?;
                            cancel_sent = true;
                        } else {
                            self.mark_outgoing_viewed(transfer_id).map_err(|_| {
                                delivery_failure(LocalLinkTransferFailure::PayloadUnavailable)
                            })?;
                        }
                    }
                }
                Some(WireMessage::Receipt {
                    transfer_id: receipt_id,
                    receipt,
                }) if receipt_id == transfer_id => {
                    self.apply_outgoing_receipt(transfer_id, receipt)
                        .map_err(|_| {
                            delivery_failure(LocalLinkTransferFailure::PayloadUnavailable)
                        })?;
                    // The sender result is already durable. Losing this ack
                    // cannot roll it back; the receiver keeps the same
                    // metadata-only terminal summary for later queries.
                    let _ = channel.send_message(&WireMessage::ReceiptAck {
                        transfer_id: transfer_id.to_owned(),
                        receipt,
                    });
                    return Ok(());
                }
                Some(_) => {
                    return Err(delivery_failure(
                        LocalLinkTransferFailure::ProtocolViolation,
                    ))
                }
                None => continue,
            }
        }
    }

    fn outgoing_cancellation_requested(
        &self,
        transfer_id: &str,
        cancel_receiver: &Receiver<OutgoingTransferSignal>,
    ) -> bool {
        matches!(
            cancel_receiver.try_recv(),
            Ok(OutgoingTransferSignal::Cancel)
        ) || self.outgoing_is_cancelled(transfer_id)
    }

    fn active_delivery_generation(&self) -> Option<u64> {
        let snapshot = self
            .lifecycle
            .lock()
            .expect("Local Link lifecycle mutex poisoned")
            .snapshot();
        (snapshot.enabled
            && !snapshot.sleeping
            && snapshot.session_active
            && !snapshot.terminating
            && snapshot.network_path != NetworkPathState::Unsatisfied)
            .then_some(snapshot.generation)
    }

    fn delivery_generation_is_active(&self, generation: u64) -> bool {
        self.active_delivery_generation() == Some(generation)
    }

    fn outgoing_is_cancelled(&self, transfer_id: &str) -> bool {
        self.database
            .with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, transfer_id)
            })
            .ok()
            .flatten()
            .is_some_and(|transfer| {
                transfer.direction == LocalLinkTransferDirection::Outgoing
                    && transfer.status == LocalLinkTransferStatus::Cancelled
            })
    }

    /// Authenticated transport still needs a current local trust decision.
    /// Distinguish an expired/revoked record from a key mismatch so the sender
    /// receives a truthful, content-free recovery reason before any payload is
    /// offered.
    fn authenticate_peer(
        &self,
        device_id: &str,
        peer_public_key: &[u8],
    ) -> Result<(), LocalLinkTransferFailure> {
        self.expire_trusted_devices()
            .map_err(|_| LocalLinkTransferFailure::AuthFailed)?;
        let session = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .session_trusted_devices
            .get(device_id)
            .cloned();
        if let Some(session) = session {
            if session.public_key.as_slice() == peer_public_key {
                self.record_authenticated_device_presence(device_id);
                return Ok(());
            }
            return Err(LocalLinkTransferFailure::AuthFailed);
        }
        let peer = self
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT trust_status, peer_public_key FROM local_link_devices WHERE device_id = ?1",
                        [device_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
                    )
                    .optional()
                    .map_err(AppError::from)
            })
            .map_err(|_| LocalLinkTransferFailure::AuthFailed)?;
        let Some((status, stored)) = peer else {
            return Err(LocalLinkTransferFailure::DeviceNotTrusted);
        };
        if LocalLinkDeviceTrustStatus::from_storage_value(&status)
            != Some(LocalLinkDeviceTrustStatus::Trusted)
            || stored.len() != 32
        {
            return Err(LocalLinkTransferFailure::DeviceNotTrusted);
        }
        if stored.as_slice() != peer_public_key {
            return Err(LocalLinkTransferFailure::AuthFailed);
        }
        self.record_authenticated_device_presence(device_id);
        Ok(())
    }

    fn advance_outgoing(
        &self,
        transfer_id: &str,
        status: LocalLinkTransferStatus,
    ) -> AppResult<()> {
        self.database.with_local_link_connection(|connection| {
            let changed = connection.execute(
                "UPDATE local_link_transfers SET status = ?1, updated_at = ?2
                 WHERE id = ?3 AND direction = 'outgoing'
                   AND status IN ('connecting', 'encrypted', 'awaitingReceiver', 'viewed')",
                params![status.as_storage_value(), now(), transfer_id],
            )?;
            if changed != 1 {
                return Err(AppError::InvalidInput(
                    "Local Link transfer is no longer active.".to_owned(),
                ));
            }
            Ok(())
        })?;
        let phase = match status {
            LocalLinkTransferStatus::Encrypted => Some(TransferPhase::Encrypted),
            LocalLinkTransferStatus::AwaitingReceiver => Some(TransferPhase::AwaitingReceiver),
            _ => None,
        };
        if let Some(phase) = phase {
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .advance(transfer_id, phase);
        }
        Ok(())
    }

    /// Synchronize the sender's metadata-only lifecycle after an authenticated
    /// `Viewed` acknowledgement. Duplicate acknowledgements are intentionally
    /// harmless; terminal states remain immutable.
    fn mark_outgoing_viewed(&self, transfer_id: &str) -> AppResult<()> {
        let transitioned = self.database.with_local_link_connection(|connection| {
            let transfer = local_link_transfer_with_connection(connection, transfer_id)?
                .ok_or(AppError::NotFound)?;
            match transfer.status {
                LocalLinkTransferStatus::Viewed => Ok(false),
                LocalLinkTransferStatus::AwaitingReceiver => {
                    let changed = connection.execute(
                        "UPDATE local_link_transfers SET status = 'viewed', updated_at = ?1
                         WHERE id = ?2 AND direction = 'outgoing' AND status = 'awaitingReceiver'",
                        params![now(), transfer_id],
                    )?;
                    if changed != 1 {
                        return Err(AppError::InvalidInput(
                            "Local Link transfer is no longer active.".to_owned(),
                        ));
                    }
                    Ok(true)
                }
                _ => Err(AppError::InvalidInput(
                    "Local Link transfer is no longer active.".to_owned(),
                )),
            }
        })?;
        if transitioned {
            let _ = self
                .receipts
                .lock()
                .expect("Local Link receipt mutex poisoned")
                .advance(transfer_id, TransferPhase::Viewed);
        }
        Ok(())
    }

    fn mark_outgoing_reconciling(&self, transfer_id: &str) -> AppResult<()> {
        let deadline = timestamp_from_millis(
            Utc::now()
                .timestamp_millis()
                .saturating_add(STATUS_RECONCILIATION_BUDGET_MS),
        );
        self.database
            .mark_local_link_transfer_reconciling(transfer_id, &deadline)
            .map(|_| ())
    }

    /// Start exactly one process-local worker for a persisted ambiguous
    /// transfer. The worker owns no plaintext and may therefore be resumed
    /// after wake, a path change, or process restart.
    fn start_status_reconciliation(&self, transfer_id: String, first_endpoint: Option<SocketAddr>) {
        let snapshot = self
            .lifecycle
            .lock()
            .expect("Local Link lifecycle mutex poisoned")
            .snapshot();
        if !snapshot.enabled
            || snapshot.sleeping
            || !snapshot.session_active
            || snapshot.terminating
            || snapshot.network_path == NetworkPathState::Unsatisfied
        {
            return;
        }
        let generation = snapshot.generation;
        let mut runtime = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned");
        if runtime.reconciliation_workers.get(&transfer_id) == Some(&generation) {
            return;
        }
        runtime
            .reconciliation_workers
            .insert(transfer_id.clone(), generation);
        drop(runtime);
        let service = self.self_weak.clone();
        let worker_transfer_id = transfer_id.clone();
        if thread::Builder::new()
            .name("clipriva-local-link-status".to_owned())
            .spawn(move || {
                let first_endpoint = first_endpoint;
                for delay in STATUS_RECONCILIATION_DELAYS {
                    if !delay.is_zero() {
                        thread::sleep(delay);
                    }
                    let Some(service) = service.upgrade() else {
                        return;
                    };
                    if !service.reconciliation_generation_is_active(generation) {
                        service.finish_reconciliation_worker(&worker_transfer_id, generation);
                        return;
                    }
                    if !service.outgoing_is_reconciling(&worker_transfer_id) {
                        service.finish_reconciliation_worker(&worker_transfer_id, generation);
                        return;
                    }
                    let Some(deadline_ms) = service.reconciliation_deadline_ms(&worker_transfer_id)
                    else {
                        let _ = service.mark_reconciliation_outcome_unknown(&worker_transfer_id);
                        service.finish_reconciliation_worker(&worker_transfer_id, generation);
                        return;
                    };
                    if Utc::now().timestamp_millis() >= deadline_ms {
                        let _ = service.mark_reconciliation_outcome_unknown(&worker_transfer_id);
                        service.finish_reconciliation_worker(&worker_transfer_id, generation);
                        return;
                    }
                    let Some(device_id) = service.reconciliation_device_id(&worker_transfer_id)
                    else {
                        service.finish_reconciliation_worker(&worker_transfer_id, generation);
                        return;
                    };
                    let mut endpoints = Vec::new();
                    if let Some(endpoint) = first_endpoint {
                        endpoints.push(endpoint);
                    }
                    for endpoint in service.transport_endpoints() {
                        if !endpoints.contains(&endpoint) {
                            endpoints.push(endpoint);
                        }
                    }
                    endpoints.truncate(MAX_RECONCILIATION_ENDPOINTS);
                    for endpoint in endpoints {
                        if !service.reconciliation_generation_is_active(generation)
                            || Utc::now().timestamp_millis() >= deadline_ms
                        {
                            break;
                        }
                        match service.query_transfer_status_at_endpoint(
                            endpoint,
                            &device_id,
                            &worker_transfer_id,
                            deadline_ms,
                        ) {
                            Ok(StatusResponseState::Terminal(receipt)) => {
                                if service
                                    .apply_outgoing_receipt(&worker_transfer_id, receipt)
                                    .is_ok()
                                    || !service.outgoing_is_reconciling(&worker_transfer_id)
                                {
                                    service.finish_reconciliation_worker(
                                        &worker_transfer_id,
                                        generation,
                                    );
                                    return;
                                }
                            }
                            Ok(StatusResponseState::Pending | StatusResponseState::Unknown)
                            | Err(_) => {}
                        }
                    }
                }
                if let Some(service) = service.upgrade() {
                    if service.reconciliation_generation_is_active(generation) {
                        let remaining_ms = service
                            .reconciliation_deadline_ms(&worker_transfer_id)
                            .unwrap_or_else(|| Utc::now().timestamp_millis())
                            .saturating_sub(Utc::now().timestamp_millis());
                        if remaining_ms > 0 {
                            thread::sleep(Duration::from_millis(remaining_ms as u64));
                        }
                        if service.reconciliation_generation_is_active(generation) {
                            let _ =
                                service.mark_reconciliation_outcome_unknown(&worker_transfer_id);
                        }
                    }
                    service.finish_reconciliation_worker(&worker_transfer_id, generation);
                }
            })
            .is_err()
        {
            self.finish_reconciliation_worker(&transfer_id, generation);
            let _ = self.mark_reconciliation_outcome_unknown(&transfer_id);
        }
    }

    fn finish_reconciliation_worker(&self, transfer_id: &str, generation: u64) {
        let mut runtime = self
            .runtime
            .lock()
            .expect("Local Link runtime mutex poisoned");
        if runtime.reconciliation_workers.get(transfer_id) == Some(&generation) {
            runtime.reconciliation_workers.remove(transfer_id);
        }
    }

    fn reconciliation_generation_is_active(&self, generation: u64) -> bool {
        let snapshot = self
            .lifecycle
            .lock()
            .expect("Local Link lifecycle mutex poisoned")
            .snapshot();
        snapshot.generation == generation
            && snapshot.enabled
            && !snapshot.sleeping
            && snapshot.session_active
            && !snapshot.terminating
            && snapshot.network_path != NetworkPathState::Unsatisfied
    }

    fn reconciliation_deadline_ms(&self, transfer_id: &str) -> Option<i64> {
        self.database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT expires_at FROM local_link_transfers
                         WHERE id = ?1 AND direction = 'outgoing'
                           AND status = 'reconciling'",
                        [transfer_id],
                        |row| row.get::<_, Option<String>>(0),
                    )
                    .optional()
                    .map_err(AppError::from)
            })
            .ok()
            .flatten()
            .flatten()
            .and_then(|deadline| DateTime::parse_from_rfc3339(&deadline).ok())
            .map(|deadline| deadline.timestamp_millis())
    }

    fn outgoing_is_reconciling(&self, transfer_id: &str) -> bool {
        self.database
            .with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, transfer_id)
            })
            .ok()
            .flatten()
            .is_some_and(|transfer| {
                transfer.direction == LocalLinkTransferDirection::Outgoing
                    && transfer.status == LocalLinkTransferStatus::Reconciling
            })
    }

    fn reconciliation_device_id(&self, transfer_id: &str) -> Option<String> {
        self.database
            .with_local_link_connection(|connection| {
                local_link_transfer_with_connection(connection, transfer_id)
            })
            .ok()
            .flatten()
            .filter(|transfer| {
                transfer.direction == LocalLinkTransferDirection::Outgoing
                    && transfer.status == LocalLinkTransferStatus::Reconciling
            })
            .map(|transfer| transfer.device_id)
    }

    fn query_transfer_status_at_endpoint(
        &self,
        endpoint: SocketAddr,
        device_id: &str,
        transfer_id: &str,
        reconciliation_deadline_ms: i64,
    ) -> Result<StatusResponseState, LocalLinkTransferFailure> {
        let identity = self
            .ensure_identity()
            .map_err(|_| LocalLinkTransferFailure::AuthFailed)?;
        let query_deadline_ms =
            reconciliation_deadline_ms.min(Utc::now().timestamp_millis().saturating_add(1_500));
        let remaining_ms = query_deadline_ms
            .saturating_sub(Utc::now().timestamp_millis())
            .max(1);
        let connect_timeout = Duration::from_millis(
            u64::try_from(remaining_ms)
                .unwrap_or(1)
                .min(STATUS_QUERY_IO_TIMEOUT.as_millis() as u64),
        );
        let io_timeout = Duration::from_millis(
            u64::try_from(remaining_ms)
                .unwrap_or(1)
                .min(STATUS_QUERY_IO_TIMEOUT.as_millis() as u64),
        );
        let stream = TcpStream::connect_timeout(&endpoint, connect_timeout)
            .map_err(|_| LocalLinkTransferFailure::DeviceUnavailable)?;
        stream
            .set_nonblocking(false)
            .and_then(|_| stream.set_nodelay(true))
            .and_then(|_| stream.set_read_timeout(Some(io_timeout)))
            .and_then(|_| stream.set_write_timeout(Some(io_timeout)))
            .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?;
        let socket_remaining_ms = query_deadline_ms
            .saturating_sub(Utc::now().timestamp_millis())
            .max(1);
        let _deadline_guard = SocketDeadlineGuard::start(
            &stream,
            Duration::from_millis(u64::try_from(socket_remaining_ms).unwrap_or(1)),
        )
        .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?;
        let mut channel = SecureChannel::initiator(stream, &identity)
            .map_err(|_| LocalLinkTransferFailure::AuthFailed)?;
        channel
            .send_message(
                &self
                    .local_session_hello(uuid::Uuid::new_v4().to_string())
                    .map_err(|_| LocalLinkTransferFailure::AuthFailed)?,
            )
            .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?;
        let remote =
            receive_hello(&mut channel).map_err(|_| LocalLinkTransferFailure::ProtocolViolation)?;
        if remote.device_id != device_id {
            return Err(LocalLinkTransferFailure::AuthFailed);
        }
        self.authenticate_peer(device_id, channel.remote_static())?;
        channel
            .send_message(&WireMessage::StatusQuery {
                transfer_id: transfer_id.to_owned(),
            })
            .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?;
        channel
            .set_poll_timeout(TRANSFER_POLL_INTERVAL)
            .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?;
        while Utc::now().timestamp_millis() < query_deadline_ms {
            match channel
                .poll_message()
                .map_err(|_| LocalLinkTransferFailure::TransportUnavailable)?
            {
                Some(WireMessage::StatusResponse {
                    transfer_id: response_id,
                    status,
                }) if response_id == transfer_id => return Ok(status),
                Some(_) => return Err(LocalLinkTransferFailure::ProtocolViolation),
                None => continue,
            }
        }
        Err(LocalLinkTransferFailure::TransportUnavailable)
    }

    fn mark_reconciliation_outcome_unknown(&self, transfer_id: &str) -> AppResult<()> {
        self.database
            .finalize_local_link_outcome_unknown(transfer_id)
            .map(|_| ())
    }

    fn resume_status_reconciliations(&self) -> AppResult<()> {
        let transfer_ids = self.database.with_local_link_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id FROM local_link_transfers
                 WHERE direction = 'outgoing' AND status = 'reconciling'",
            )?;
            let ids = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ids)
        })?;
        for transfer_id in transfer_ids {
            self.start_status_reconciliation(transfer_id, None);
        }
        Ok(())
    }

    fn mark_outgoing_failed(
        &self,
        transfer_id: &str,
        failure: LocalLinkTransferFailure,
    ) -> AppResult<()> {
        self.database.with_local_link_connection(|connection| {
            connection.execute(
                "UPDATE local_link_transfers
                 SET status = 'failed', failure_reason = ?1, updated_at = ?2,
                     completed_at = ?2, expires_at = NULL
                 WHERE id = ?3 AND direction = 'outgoing'
                   AND status IN ('connecting', 'encrypted', 'awaitingReceiver', 'viewed')",
                params![failure.as_storage_value(), now(), transfer_id],
            )?;
            Ok(())
        })
    }

    fn apply_outgoing_receipt(&self, transfer_id: &str, receipt: TerminalReceipt) -> AppResult<()> {
        if !receipt.is_valid() {
            return Err(AppError::InvalidInput(
                "Local Link receipt outcome and failure reason are inconsistent.".to_owned(),
            ));
        }
        let (status, failure, action, completed_delivery) = match receipt.outcome {
            ReceiptOutcome::Copied => ("copied", None, Some("copy"), true),
            ReceiptOutcome::Saved => ("saved", None, Some("save"), true),
            ReceiptOutcome::Rejected => ("rejected", None, Some("reject"), false),
            ReceiptOutcome::Cancelled => ("cancelled", None, None, false),
            ReceiptOutcome::Expired => ("expired", None, None, false),
            ReceiptOutcome::OutcomeUnknown => (
                "failed",
                Some(LocalLinkTransferFailure::OutcomeUnknown.as_storage_value()),
                None,
                false,
            ),
            ReceiptOutcome::NotDelivered => (
                "failed",
                receipt
                    .failure
                    .map(LocalLinkTransferFailure::as_storage_value),
                None,
                false,
            ),
        };
        self.database.with_local_link_connection(|connection| {
            let current = local_link_transfer_with_connection(connection, transfer_id)?
                .ok_or(AppError::NotFound)?;
            if current.direction != LocalLinkTransferDirection::Outgoing {
                return Err(AppError::InvalidInput(
                    "Local Link receipt did not target an outgoing transfer.".to_owned(),
                ));
            }
            if let Some(current_receipt) = terminal_receipt_for_transfer(&current) {
                if current_receipt == receipt {
                    return Ok(());
                }
                if current.status != LocalLinkTransferStatus::Cancelled
                    && current.failure_reason != Some(LocalLinkTransferFailure::OutcomeUnknown)
                {
                    return Err(AppError::InvalidInput(
                        "Local Link transfer is already final.".to_owned(),
                    ));
                }
                // The receiver decided before it observed our Cancel or before
                // local reconciliation lost proof. Its authenticated terminal
                // receipt may replace only provisional cancellation or
                // outcomeUnknown, never another proven terminal result.
            }
            let timestamp = now();
            let eligible_statuses = if current.status == LocalLinkTransferStatus::Cancelled {
                "'cancelled'"
            } else if current.failure_reason == Some(LocalLinkTransferFailure::OutcomeUnknown) {
                "'failed'"
            } else {
                "'connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'reconciling'"
            };
            let update = format!(
                "UPDATE local_link_transfers
                 SET status = ?1, failure_reason = ?2, receiver_action = ?3,
                     updated_at = ?4, completed_at = ?4, expires_at = NULL
                 WHERE id = ?5 AND direction = 'outgoing'
                   AND status IN ({eligible_statuses})"
            );
            let changed = connection.execute(
                &update,
                params![status, failure, action, timestamp, transfer_id],
            )?;
            if changed != 1 {
                return Err(AppError::InvalidInput(
                    "Local Link transfer is no longer active.".to_owned(),
                ));
            }
            if completed_delivery {
                // This is success metadata only. Never use a failed,
                // cancelled, expired, or rejected attempt as evidence that a
                // peer received the body.
                let device_changed = connection.execute(
                    "UPDATE local_link_devices
                     SET last_transfer_at = ?1
                     WHERE device_id = (
                         SELECT device_id FROM local_link_transfers
                         WHERE id = ?2 AND direction = 'outgoing'
                           AND status IN ('copied', 'saved')
                     )",
                    params![timestamp, transfer_id],
                )?;
                if device_changed != 1 {
                    return Err(AppError::InvalidInput(
                        "Local Link transfer device is unavailable for the delivery receipt."
                            .to_owned(),
                    ));
                }
            }
            Ok(())
        })?;
        let _ = self
            .receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .apply_remote_receipt(transfer_id, receipt);
        Ok(())
    }

    fn mark_inbound_network_failure(
        &self,
        transfer_id: &str,
        failure: LocalLinkTransferFailure,
    ) -> AppResult<()> {
        self.mark_failed_while_locked(transfer_id, failure)
            .map(|_| ())
    }

    fn inbound_deadline(&self, transfer_id: &str) -> AppResult<i64> {
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pending_inbound
            .get(transfer_id)
            .map(pending_deadline_ms)
            .ok_or_else(|| AppError::InvalidInput("Local Link payload is unavailable.".to_owned()))
    }

    #[cfg(test)]
    fn start_transport_for_test(&self, port: u16) -> AppResult<SocketAddr> {
        self.database.with_local_link_connection(|connection| {
            connection.execute(
                "UPDATE local_link_preferences SET enabled = 1, discovery_enabled = 0",
                [],
            )?;
            Ok(())
        })?;
        *self
            .identity
            .lock()
            .expect("Local Link identity mutex poisoned") =
            Some(Arc::new(LocalIdentity::generate_for_test()?));
        *self
            .lifecycle
            .lock()
            .expect("Local Link lifecycle mutex poisoned") = LifecycleSupervisor::new(true);
        let mut transport = self
            .transport
            .lock()
            .expect("Local Link transport mutex poisoned");
        *transport = Some(TransportRuntime::start_inner(
            self.self_weak.clone(),
            false,
            false,
            port,
            1,
        )?);
        Ok(transport
            .as_ref()
            .expect("transport just started")
            .listener_addr())
    }

    #[cfg(test)]
    fn pairing_code_for_test(&self, device_id: &str) -> Option<String> {
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .pairing_sessions
            .get(device_id)
            .map(|session| session.sas.clone())
    }

    #[cfg(test)]
    fn drop_next_terminal_receipt_for_test(&self) {
        self.runtime
            .lock()
            .expect("Local Link runtime mutex poisoned")
            .drop_next_terminal_receipt = true;
    }

    #[cfg(test)]
    fn begin_pairing_at_for_test(&self, endpoint: SocketAddr) -> AppResult<LocalLinkDevice> {
        let identity = self.ensure_identity()?;
        self.open_outbound_pairing(endpoint, identity)
    }

    #[cfg(test)]
    fn send_clipboard_item_to_endpoint_for_test(
        &self,
        clipboard_item_id: &str,
        device_id: &str,
        endpoint: SocketAddr,
    ) -> AppResult<LocalLinkTransfer> {
        let device_id = validate_device_id(device_id)?;
        self.expire_trusted_devices()?;
        self.database.with_local_link_connection(|connection| {
            ensure_link_enabled(&local_link_preferences_with_connection(connection)?)?;
            if self.is_session_trusted_device(&device_id) {
                Ok(())
            } else {
                ensure_trusted_device(connection, &device_id)
            }
        })?;
        let payload = self
            .database
            .local_link_export_clipboard_item(clipboard_item_id)?;
        validate_text_payload(&payload.content)?;
        let transfer = self.persist_outgoing_connecting(&device_id, payload.content.len())?;
        let generation = self.active_delivery_generation().ok_or_else(|| {
            AppError::InvalidInput("Local Link transport is suspended.".to_owned())
        })?;
        let service = self.self_weak.clone();
        let transfer_id = transfer.id.clone();
        let _ = thread::Builder::new()
            .name("clipriva-local-link-test-send".to_owned())
            .spawn(move || {
                if let Some(service) = service.upgrade() {
                    service.deliver_outgoing(
                        vec![endpoint],
                        device_id,
                        transfer_id,
                        Zeroizing::new(payload.content),
                        generation,
                    );
                }
            })
            .map_err(transport_error)?;
        Ok(transfer)
    }
}

impl Drop for LocalLinkService {
    fn drop(&mut self) {
        self.transport
            .lock()
            .expect("Local Link transport mutex poisoned")
            .take();
        self.deadline_reaper.stop();
        self.runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned")
            .pending_inbound
            .clear();
        self.receipts
            .lock()
            .expect("Local Link receipt mutex poisoned")
            .clear();
        self.identity
            .lock()
            .expect("Local Link identity mutex poisoned")
            .take();
    }
}

fn configure_stream(stream: &TcpStream) -> AppResult<()> {
    // Accepted sockets inherit the listener's non-blocking mode on macOS.
    // Noise framing uses bounded blocking reads with explicit timeouts.
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(65)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    Ok(())
}

fn receive_hello(channel: &mut SecureChannel<TcpStream>) -> AppResult<SessionHello> {
    match channel.receive_message()? {
        WireMessage::SessionHello {
            session_id,
            device_id,
            display_name,
            ..
        } => Ok(SessionHello {
            session_id,
            device_id,
            display_name,
        }),
        _ => Err(AppError::InvalidInput(
            "Local Link session did not begin with an authenticated hello.".to_owned(),
        )),
    }
}

fn local_device_id(identity: &LocalIdentity) -> String {
    format!(
        "ll-{}",
        identity
            .fingerprint()
            .chars()
            .filter(char::is_ascii_hexdigit)
            .collect::<String>()
            .to_ascii_lowercase()
    )
}

fn failure_to_error(failure: LocalLinkTransferFailure) -> AppError {
    AppError::InvalidInput(format!(
        "Local Link transfer was rejected before plaintext was accepted ({}) .",
        failure.as_storage_value()
    ))
}

fn deadline_reaper_loop(
    database: Arc<Database>,
    runtime: Arc<Mutex<RuntimeState>>,
    receipts: Arc<Mutex<ReceiptStateMachine>>,
    signal: Arc<ReaperSignal>,
) {
    loop {
        let observed_generation = {
            let state = signal
                .state
                .lock()
                .expect("Local Link reaper signal mutex poisoned");
            if state.stopped {
                return;
            }
            state.generation
        };

        let now_ms = Utc::now().timestamp_millis();
        let _ = expire_pending_runtime(&database, &runtime, &receipts, now_ms);
        let next_deadline = runtime
            .lock()
            .expect("pending Local Link payload mutex poisoned")
            .pending_inbound
            .values()
            .map(pending_deadline_ms)
            .min();
        let wait = next_deadline
            .map(|deadline| Duration::from_millis(deadline.saturating_sub(now_ms).max(0) as u64))
            .unwrap_or_else(|| Duration::from_secs(60));

        let state = signal
            .state
            .lock()
            .expect("Local Link reaper signal mutex poisoned");
        if state.stopped {
            return;
        }
        if state.generation != observed_generation {
            continue;
        }
        let _ = signal
            .changed
            .wait_timeout_while(state, wait, |state| {
                !state.stopped && state.generation == observed_generation
            })
            .expect("Local Link reaper signal mutex poisoned");
    }
}

fn expire_pending_runtime(
    database: &Database,
    runtime: &Mutex<RuntimeState>,
    receipts: &Mutex<ReceiptStateMachine>,
    now_ms: i64,
) -> AppResult<usize> {
    let mut runtime = runtime
        .lock()
        .expect("pending Local Link payload mutex poisoned");
    let expired = runtime
        .pending_inbound
        .iter()
        .filter(|(_, pending)| pending_deadline_ms(pending) <= now_ms)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    if expired.is_empty() {
        return Ok(0);
    }
    let persisted = database.with_local_link_connection(|connection| {
        let transaction = connection.unchecked_transaction()?;
        let timestamp = now();
        let mut updates = Vec::with_capacity(expired.len());
        for transfer_id in &expired {
            let prepared = transaction.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM local_link_effect_claims
                     WHERE transfer_id = ?1 AND state = 'prepared'
                 )",
                [transfer_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            let receipt = if prepared {
                transaction.execute(
                    "UPDATE local_link_transfers
                     SET status = 'failed', failure_reason = 'outcomeUnknown',
                         receiver_action = NULL, updated_at = ?1,
                         completed_at = ?1, expires_at = NULL
                     WHERE id = ?2 AND status IN ('encrypted', 'awaitingReceiver', 'viewed')",
                    params![timestamp, transfer_id],
                )?;
                transaction.execute(
                    "UPDATE local_link_effect_claims
                     SET state = 'uncertain', updated_at = ?1
                     WHERE transfer_id = ?2 AND state = 'prepared'",
                    params![timestamp, transfer_id],
                )?;
                TerminalReceipt::outcome_unknown()
            } else {
                transaction.execute(
                    "UPDATE local_link_transfers
                     SET status = 'expired', failure_reason = NULL,
                         receiver_action = NULL, updated_at = ?1,
                         completed_at = ?1, expires_at = NULL
                     WHERE id = ?2 AND status IN ('encrypted', 'awaitingReceiver', 'viewed')",
                    params![timestamp, transfer_id],
                )?;
                TerminalReceipt::completed(ReceiptOutcome::Expired)
            };
            updates.push((transfer_id.clone(), receipt));
        }
        transaction.commit()?;
        Ok(updates)
    });
    for transfer_id in &expired {
        runtime.pending_inbound.remove(transfer_id);
    }
    let expired_waiters = expired
        .iter()
        .filter_map(|transfer_id| {
            runtime
                .receipt_waiters
                .remove(transfer_id)
                .map(|waiter| (transfer_id.clone(), waiter))
        })
        .collect::<Vec<_>>();
    drop(runtime);
    crate::commands::close_local_link_reveals_natively();
    let updates = match persisted {
        Ok(updates) => updates,
        Err(error) => {
            let receipt = TerminalReceipt::outcome_unknown();
            let mut receipts = receipts.lock().expect("Local Link receipt mutex poisoned");
            for (transfer_id, waiter) in expired_waiters {
                let _ = receipts.apply_remote_receipt(&transfer_id, receipt);
                let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
            }
            return Err(error);
        }
    };
    let mut receipt_state = receipts.lock().expect("Local Link receipt mutex poisoned");
    for (transfer_id, receipt) in &updates {
        let _ = receipt_state.apply_receipt(transfer_id, *receipt);
    }
    drop(receipt_state);
    for (transfer_id, waiter) in expired_waiters {
        let receipt = updates
            .iter()
            .find_map(|(id, receipt)| (id == &transfer_id).then_some(*receipt))
            .unwrap_or_else(TerminalReceipt::outcome_unknown);
        let _ = waiter.send(InboundTransferSignal::Terminal(receipt));
    }
    Ok(expired.len())
}

fn pending_deadline_ms(pending: &PendingInbound) -> i64 {
    pending
        .offer
        .created_at_ms
        .saturating_add(i64::from(pending.offer.ttl_seconds) * 1_000)
}

fn runtime_presence_is_current(presence: &RuntimeDevicePresence, now_ms: i64) -> bool {
    now_ms.saturating_sub(presence.observed_at_ms) <= RUNTIME_PRESENCE_TTL_SECONDS * 1_000
}

fn local_link_preferences_with_connection(
    connection: &rusqlite::Connection,
) -> AppResult<LocalLinkPreferences> {
    connection
        .query_row(
            "SELECT enabled, discovery_enabled, device_name
             FROM local_link_preferences WHERE singleton = 1",
            [],
            |row| {
                Ok(LocalLinkPreferences {
                    enabled: row.get::<_, i64>(0)? != 0,
                    discovery_enabled: row.get::<_, i64>(1)? != 0,
                    device_name: row.get(2)?,
                })
            },
        )
        .map_err(AppError::from)
}

fn local_link_device_with_connection(
    connection: &rusqlite::Connection,
    device_id: &str,
) -> AppResult<Option<LocalLinkDevice>> {
    connection
        .query_row(
            "SELECT device_id, display_name, public_key_fingerprint, trust_status, paired_at,
                    trusted_at, revoked_at, last_seen_at, last_transfer_at,
                    protocol_min, protocol_max, trust_duration
             FROM local_link_devices WHERE device_id = ?1",
            [device_id],
            map_local_link_device,
        )
        .optional()
        .map_err(AppError::from)
}

fn local_link_transfer_with_connection(
    connection: &rusqlite::Connection,
    transfer_id: &str,
) -> AppResult<Option<LocalLinkTransfer>> {
    connection
        .query_row(
            "SELECT t.id, t.device_id, d.display_name, t.direction, t.status,
                    t.item_kind, t.byte_size, t.created_at, t.updated_at,
                    t.completed_at, t.expires_at, t.failure_reason, t.receiver_action
             FROM local_link_transfers t
             JOIN local_link_devices d ON d.device_id = t.device_id
             WHERE t.id = ?1",
            [transfer_id],
            map_local_link_transfer,
        )
        .optional()
        .map_err(AppError::from)
}

fn ensure_link_enabled(preferences: &LocalLinkPreferences) -> AppResult<()> {
    if preferences.enabled {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "Local Link is disabled. No text was delivered.".to_owned(),
        ))
    }
}

fn ensure_trusted_device(connection: &rusqlite::Connection, device_id: &str) -> AppResult<()> {
    let (status, key): (String, Vec<u8>) = connection
        .query_row(
            "SELECT trust_status, peer_public_key FROM local_link_devices WHERE device_id = ?1",
            [device_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    match LocalLinkDeviceTrustStatus::from_storage_value(&status) {
        Some(LocalLinkDeviceTrustStatus::Trusted) if key.len() == 32 => Ok(()),
        Some(LocalLinkDeviceTrustStatus::Revoked) => Err(AppError::InvalidInput(
            "This Local Link device was revoked. No text was delivered.".to_owned(),
        )),
        Some(LocalLinkDeviceTrustStatus::NeedsRePairing) => Err(AppError::InvalidInput(
            "This device must be securely paired again. No text was delivered.".to_owned(),
        )),
        _ => Err(AppError::InvalidInput(
            "This device does not have a verified identity. No text was delivered.".to_owned(),
        )),
    }
}

fn trusted_device_id_for_key(
    connection: &rusqlite::Connection,
    peer_public_key: &[u8],
) -> AppResult<String> {
    connection
        .query_row(
            "SELECT device_id FROM local_link_devices
             WHERE trust_status = 'trusted' AND peer_public_key = ?1
               AND public_key_fingerprint = ?2",
            params![peer_public_key, fingerprint(peer_public_key)],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            AppError::InvalidInput("Could not verify this Local Link device.".to_owned())
        })
}

fn expire_trusted_devices_with_connection(
    connection: &rusqlite::Connection,
    current_time: DateTime<Utc>,
) -> AppResult<usize> {
    let cutoff = (current_time - chrono::Duration::days(TRUST_DURATION_DAYS))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let expired = connection.execute(
        "UPDATE local_link_devices
         SET trust_status = 'needsRePairing', peer_public_key = X'',
             public_key_fingerprint = '', last_seen_at = NULL
         WHERE trust_status = 'trusted'
           AND trust_duration = 'thirtyDays'
           AND (
               trusted_at IS NULL
               OR julianday(trusted_at) IS NULL
               OR julianday(trusted_at) <= julianday(?1)
           )",
        [cutoff],
    )?;
    Ok(expired)
}

fn trust_expiry_from_timestamp(
    trusted_at: &str,
    trust_duration: LocalLinkTrustDuration,
) -> Option<DateTime<Utc>> {
    if trust_duration != LocalLinkTrustDuration::ThirtyDays {
        return None;
    }
    DateTime::parse_from_rfc3339(trusted_at)
        .ok()
        .map(|timestamp| {
            timestamp.with_timezone(&Utc) + chrono::Duration::days(TRUST_DURATION_DAYS)
        })
}

fn trust_expires_at(
    trusted_at: Option<&str>,
    trust_duration: LocalLinkTrustDuration,
) -> Option<String> {
    trusted_at
        .and_then(|trusted_at| trust_expiry_from_timestamp(trusted_at, trust_duration))
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn trust_has_expired(
    trusted_at: Option<&str>,
    trust_duration: LocalLinkTrustDuration,
    current_time: DateTime<Utc>,
) -> bool {
    if trust_duration != LocalLinkTrustDuration::ThirtyDays {
        return false;
    }
    trusted_at
        .and_then(|trusted_at| trust_expiry_from_timestamp(trusted_at, trust_duration))
        .is_none_or(|expires_at| expires_at <= current_time)
}

fn map_local_link_device(row: &Row<'_>) -> rusqlite::Result<LocalLinkDevice> {
    let trust_status: String = row.get(3)?;
    let trust_status =
        LocalLinkDeviceTrustStatus::from_storage_value(&trust_status).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(
                3,
                "trust_status".to_owned(),
                rusqlite::types::Type::Text,
            )
        })?;
    let protocol_min: i64 = row.get(9)?;
    let protocol_max: i64 = row.get(10)?;
    let trust_duration: String = row.get(11)?;
    let trust_duration =
        LocalLinkTrustDuration::from_storage_value(&trust_duration).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(
                11,
                "trust_duration".to_owned(),
                rusqlite::types::Type::Text,
            )
        })?;
    let trusted_at: Option<String> = row.get(5)?;
    let trust_expires_at = trust_expires_at(trusted_at.as_deref(), trust_duration);
    let re_pairing_reason = match trust_status {
        LocalLinkDeviceTrustStatus::NeedsRePairing
            if trusted_at.as_deref().is_some_and(|trusted_at| {
                trust_has_expired(Some(trusted_at), trust_duration, Utc::now())
            }) =>
        {
            Some(LocalLinkDeviceRePairingReason::TrustExpired)
        }
        LocalLinkDeviceTrustStatus::NeedsRePairing => {
            Some(LocalLinkDeviceRePairingReason::IdentityInvalidated)
        }
        _ => None,
    };
    Ok(LocalLinkDevice {
        device_id: row.get(0)?,
        display_name: row.get(1)?,
        public_key_fingerprint: row.get(2)?,
        trust_status,
        paired_at: row.get(4)?,
        trusted_at,
        trust_duration,
        trust_expires_at,
        re_pairing_reason,
        revoked_at: row.get(6)?,
        last_seen_at: row.get(7)?,
        last_transfer_at: row.get(8)?,
        online: false,
        protocol_min: u16::try_from(protocol_min).unwrap_or(PROTOCOL_VERSION),
        protocol_max: u16::try_from(protocol_max).unwrap_or(PROTOCOL_VERSION),
    })
}

fn map_local_link_transfer(row: &Row<'_>) -> rusqlite::Result<LocalLinkTransfer> {
    let direction_value: String = row.get(3)?;
    let direction =
        LocalLinkTransferDirection::from_storage_value(&direction_value).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(
                3,
                "direction".to_owned(),
                rusqlite::types::Type::Text,
            )
        })?;
    let status_value: String = row.get(4)?;
    let status = LocalLinkTransferStatus::from_storage_value(&status_value).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(4, "status".to_owned(), rusqlite::types::Type::Text)
    })?;
    let failure_reason: Option<String> = row.get(11)?;
    let failure_reason = failure_reason
        .map(|value| {
            LocalLinkTransferFailure::from_storage_value(&value).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(
                    11,
                    "failure_reason".to_owned(),
                    rusqlite::types::Type::Text,
                )
            })
        })
        .transpose()?;
    let receiver_action: Option<String> = row.get(12)?;
    let receiver_action = receiver_action
        .map(|value| {
            LocalLinkReceiveAction::from_storage_value(&value).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(
                    12,
                    "receiver_action".to_owned(),
                    rusqlite::types::Type::Text,
                )
            })
        })
        .transpose()?;
    let byte_size: i64 = row.get(6)?;
    let mut transfer = LocalLinkTransfer {
        id: row.get(0)?,
        device_id: row.get(1)?,
        peer_display_name: row.get(2)?,
        direction,
        status,
        item_kind: row.get(5)?,
        byte_size: u32::try_from(byte_size).unwrap_or(u32::MAX),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        completed_at: row.get(9)?,
        expires_at: row.get(10)?,
        failure_reason,
        recovery_action: None,
        receiver_action,
    };
    transfer.recovery_action = transfer.recovery_action();
    Ok(transfer)
}

impl LocalLinkTransferStatus {
    fn as_storage_value(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Encrypted => "encrypted",
            Self::AwaitingReceiver => "awaitingReceiver",
            Self::Viewed => "viewed",
            Self::Reconciling => "reconciling",
            Self::Copied => "copied",
            Self::Saved => "saved",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Failed => "failed",
        }
    }
}

fn is_active_status(status: LocalLinkTransferStatus) -> bool {
    matches!(
        status,
        LocalLinkTransferStatus::Connecting
            | LocalLinkTransferStatus::Encrypted
            | LocalLinkTransferStatus::AwaitingReceiver
            | LocalLinkTransferStatus::Viewed
    )
}

/// Convert only persisted terminal metadata into its wire receipt form. This
/// deliberately has no access to `PendingInbound`, so neither body nor preview
/// can enter a receipt, retry, or status response.
fn terminal_receipt_for_transfer(transfer: &LocalLinkTransfer) -> Option<TerminalReceipt> {
    let outcome = match transfer.status {
        LocalLinkTransferStatus::Copied => ReceiptOutcome::Copied,
        LocalLinkTransferStatus::Saved => ReceiptOutcome::Saved,
        LocalLinkTransferStatus::Rejected => ReceiptOutcome::Rejected,
        LocalLinkTransferStatus::Cancelled => ReceiptOutcome::Cancelled,
        LocalLinkTransferStatus::Expired => ReceiptOutcome::Expired,
        LocalLinkTransferStatus::Failed => {
            if transfer.failure_reason == Some(LocalLinkTransferFailure::OutcomeUnknown) {
                return Some(TerminalReceipt::outcome_unknown());
            }
            return Some(TerminalReceipt::not_delivered(
                transfer
                    .failure_reason
                    .unwrap_or(LocalLinkTransferFailure::PayloadUnavailable),
            ));
        }
        LocalLinkTransferStatus::Connecting
        | LocalLinkTransferStatus::Encrypted
        | LocalLinkTransferStatus::AwaitingReceiver
        | LocalLinkTransferStatus::Viewed
        | LocalLinkTransferStatus::Reconciling => return None,
    };
    Some(TerminalReceipt::completed(outcome))
}

fn sync_receipt_state(machine: &mut ReceiptStateMachine, transfer: &LocalLinkTransfer) {
    let _ = machine.begin(transfer.id.clone());
    match transfer.status {
        LocalLinkTransferStatus::Connecting => {}
        LocalLinkTransferStatus::Encrypted => {
            let _ = machine.advance(&transfer.id, TransferPhase::Encrypted);
        }
        LocalLinkTransferStatus::AwaitingReceiver => {
            let _ = machine.advance(&transfer.id, TransferPhase::Encrypted);
            let _ = machine.advance(&transfer.id, TransferPhase::AwaitingReceiver);
        }
        LocalLinkTransferStatus::Viewed => {
            let _ = machine.advance(&transfer.id, TransferPhase::Encrypted);
            let _ = machine.advance(&transfer.id, TransferPhase::AwaitingReceiver);
            let _ = machine.advance(&transfer.id, TransferPhase::Viewed);
        }
        LocalLinkTransferStatus::Reconciling => {
            let _ = machine.advance(&transfer.id, TransferPhase::Encrypted);
            let _ = machine.advance(&transfer.id, TransferPhase::AwaitingReceiver);
        }
        LocalLinkTransferStatus::Copied => {
            let _ = machine.resolve_status(
                &transfer.id,
                Some(TerminalReceipt::completed(ReceiptOutcome::Copied)),
            );
        }
        LocalLinkTransferStatus::Saved => {
            let _ = machine.resolve_status(
                &transfer.id,
                Some(TerminalReceipt::completed(ReceiptOutcome::Saved)),
            );
        }
        LocalLinkTransferStatus::Rejected => {
            let _ = machine.resolve_status(
                &transfer.id,
                Some(TerminalReceipt::completed(ReceiptOutcome::Rejected)),
            );
        }
        LocalLinkTransferStatus::Cancelled => {
            let _ = machine.resolve_status(
                &transfer.id,
                Some(TerminalReceipt::completed(ReceiptOutcome::Cancelled)),
            );
        }
        LocalLinkTransferStatus::Expired => {
            let _ = machine.resolve_status(
                &transfer.id,
                Some(TerminalReceipt::completed(ReceiptOutcome::Expired)),
            );
        }
        LocalLinkTransferStatus::Failed => {
            let failure = transfer
                .failure_reason
                .unwrap_or(LocalLinkTransferFailure::PayloadUnavailable);
            let receipt = if failure == LocalLinkTransferFailure::OutcomeUnknown {
                TerminalReceipt::outcome_unknown()
            } else {
                TerminalReceipt::not_delivered(failure)
            };
            let _ = machine.resolve_status(&transfer.id, Some(receipt));
        }
    }
}

fn validate_device_id(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_DEVICE_ID_LENGTH
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::InvalidInput(
            "Local Link device identifiers must be 1 to 128 non-whitespace characters.".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_device_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_DEVICE_NAME_LENGTH
        || value.chars().any(char::is_control)
    {
        return Err(AppError::InvalidInput(
            "Local Link device names must contain 1 to 80 visible characters.".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn validate_text_payload(content: &str) -> AppResult<()> {
    if content.is_empty() {
        return Err(AppError::InvalidInput(
            "Local Link can send one non-empty text clip at a time.".to_owned(),
        ));
    }
    if content.len() > MAX_TEXT_PAYLOAD_BYTES {
        return Err(AppError::InvalidInput(
            "This text clip exceeds Local Link's 256 KiB beta limit.".to_owned(),
        ));
    }
    Ok(())
}

fn prune_transfer_summaries(connection: &rusqlite::Connection) -> AppResult<()> {
    let cutoff = (Utc::now() - chrono::Duration::hours(TRANSFER_RETENTION_HOURS))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    connection.execute(
        "DELETE FROM local_link_transfers
         WHERE updated_at < ?1
           AND status IN ('copied', 'saved', 'rejected', 'cancelled', 'expired', 'failed')
           AND NOT EXISTS (
               SELECT 1 FROM local_link_effect_claims claims
               WHERE claims.transfer_id = local_link_transfers.id
                 AND claims.state = 'prepared'
           )",
        [cutoff],
    )?;
    let evidence_cutoff = timestamp_from_millis(
        Utc::now()
            .timestamp_millis()
            .saturating_sub(TERMINAL_EVIDENCE_GRACE_MS),
    );
    connection.execute(
        "DELETE FROM local_link_transfers
         WHERE status IN ('copied', 'saved', 'rejected', 'cancelled', 'expired', 'failed')
           AND updated_at < ?2
           AND NOT EXISTS (
               SELECT 1 FROM local_link_effect_claims claims
               WHERE claims.transfer_id = local_link_transfers.id
                 AND claims.state = 'prepared'
           )
           AND id NOT IN (
             SELECT id FROM local_link_transfers
             ORDER BY updated_at DESC, id DESC
             LIMIT ?1
         )",
        params![i64::from(MAX_TRANSFER_LIST_LIMIT), evidence_cutoff],
    )?;
    Ok(())
}

fn prune_replay_tombstones(connection: &rusqlite::Connection) -> AppResult<()> {
    connection.execute(
        "DELETE FROM local_link_replay_tombstones WHERE expires_at <= ?1",
        [now()],
    )?;
    Ok(())
}

fn failure_for_export_error(error: &AppError) -> LocalLinkTransferFailure {
    match error {
        AppError::NotFound => LocalLinkTransferFailure::ItemUnavailable,
        AppError::InvalidInput(message) if message.contains("type is not available") => {
            LocalLinkTransferFailure::UnsupportedItem
        }
        _ => LocalLinkTransferFailure::CaptureBlocked,
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn timestamp_from_millis(timestamp_ms: i64) -> String {
    chrono::DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn derive_readiness_snapshot(
    preferences: &LocalLinkPreferences,
    transport_running: bool,
    devices: &[LocalLinkDevice],
) -> LocalLinkReadinessSnapshot {
    let mut stages = [
        LocalLinkReadinessStage::LocalNetwork,
        LocalLinkReadinessStage::Enabled,
        LocalLinkReadinessStage::Discovery,
        LocalLinkReadinessStage::Trust,
        LocalLinkReadinessStage::AuthenticatedPresence,
    ]
    .into_iter()
    .map(|stage| LocalLinkReadinessCheck {
        stage,
        state: LocalLinkReadinessStageState::NotChecked,
    })
    .collect::<Vec<_>>();
    let pending_request_capacity =
        u32::try_from(MAX_PENDING_GLOBAL).expect("fixed pending capacity fits in u32");

    let result = |state,
                  stages: Vec<LocalLinkReadinessCheck>,
                  first_blocker,
                  primary_action,
                  action_device_id,
                  ready_device_count| LocalLinkReadinessSnapshot {
        state,
        stages,
        first_blocker,
        primary_action,
        action_device_id,
        ready_device_count,
        pending_request_capacity,
    };

    // Consent comes before a permission/network probe. The local-network
    // stage stays unobserved while Local Link is off, even though it is the
    // first stage in an enabled connection check.
    if !preferences.enabled {
        stages[1].state = LocalLinkReadinessStageState::Blocked;
        return result(
            LocalLinkReadinessState::ActionRequired,
            stages,
            Some(LocalLinkReadinessBlocker::LocalLinkDisabled),
            Some(LocalLinkReadinessAction::EnableLocalLink),
            None,
            0,
        );
    }

    if !transport_running {
        stages[0].state = LocalLinkReadinessStageState::Blocked;
        return result(
            LocalLinkReadinessState::ActionRequired,
            stages,
            Some(LocalLinkReadinessBlocker::LocalNetworkUnavailable),
            Some(LocalLinkReadinessAction::RestartLocalLink),
            None,
            0,
        );
    }
    stages[0].state = LocalLinkReadinessStageState::Passed;
    stages[1].state = LocalLinkReadinessStageState::Passed;

    if !preferences.discovery_enabled {
        stages[2].state = LocalLinkReadinessStageState::Blocked;
        return result(
            LocalLinkReadinessState::ActionRequired,
            stages,
            Some(LocalLinkReadinessBlocker::DiscoveryDisabled),
            Some(LocalLinkReadinessAction::EnableDiscovery),
            None,
            0,
        );
    }
    stages[2].state = LocalLinkReadinessStageState::Passed;

    let trusted = devices
        .iter()
        .filter(|device| device.trust_status == LocalLinkDeviceTrustStatus::Trusted)
        .collect::<Vec<_>>();
    if trusted.is_empty() {
        stages[3].state = LocalLinkReadinessStageState::Blocked;
        if let Some(device) = devices
            .iter()
            .find(|device| device.trust_status == LocalLinkDeviceTrustStatus::NeedsRePairing)
        {
            return result(
                LocalLinkReadinessState::ActionRequired,
                stages,
                Some(LocalLinkReadinessBlocker::TrustRequiresRePairing),
                Some(LocalLinkReadinessAction::RePairDevice),
                Some(device.device_id.clone()),
                0,
            );
        }
        if let Some(device) = devices
            .iter()
            .find(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
        {
            return result(
                LocalLinkReadinessState::ActionRequired,
                stages,
                Some(LocalLinkReadinessBlocker::PairingIncomplete),
                Some(LocalLinkReadinessAction::CompletePairing),
                Some(device.device_id.clone()),
                0,
            );
        }
        return result(
            LocalLinkReadinessState::ActionRequired,
            stages,
            Some(LocalLinkReadinessBlocker::NoTrustedDevice),
            Some(LocalLinkReadinessAction::PairDevice),
            None,
            0,
        );
    }
    stages[3].state = LocalLinkReadinessStageState::Passed;

    let ready_device_count = trusted.iter().filter(|device| device.online).count();
    if ready_device_count == 0 {
        stages[4].state = LocalLinkReadinessStageState::Blocked;
        return result(
            LocalLinkReadinessState::Offline,
            stages,
            Some(LocalLinkReadinessBlocker::NoAuthenticatedPresence),
            Some(LocalLinkReadinessAction::RefreshDevices),
            None,
            0,
        );
    }
    stages[4].state = LocalLinkReadinessStageState::Passed;
    result(
        LocalLinkReadinessState::Ready,
        stages,
        None,
        None,
        None,
        u32::try_from(ready_device_count).unwrap_or(u32::MAX),
    )
}

fn diagnostic_peer_ref(export_salt: &str, device_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(export_salt.as_bytes());
    hasher.update([0]);
    hasher.update(device_id.as_bytes());
    let digest = hasher.finalize();
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    format!("peer-{suffix}")
}

fn diagnostic_duration_ms(created_at: &str, updated_at: &str) -> Option<u64> {
    let created_at = DateTime::parse_from_rfc3339(created_at).ok()?;
    let updated_at = DateTime::parse_from_rfc3339(updated_at).ok()?;
    u64::try_from((updated_at - created_at).num_milliseconds()).ok()
}

fn diagnostic_os_family() -> LocalLinkDiagnosticOsFamily {
    if cfg!(target_os = "macos") {
        LocalLinkDiagnosticOsFamily::Macos
    } else {
        LocalLinkDiagnosticOsFamily::Other
    }
}

fn diagnostic_architecture() -> LocalLinkDiagnosticArchitecture {
    match std::env::consts::ARCH {
        "aarch64" => LocalLinkDiagnosticArchitecture::Arm64,
        "x86_64" => LocalLinkDiagnosticArchitecture::X86_64,
        _ => LocalLinkDiagnosticArchitecture::Other,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use super::*;

    fn database() -> Arc<Database> {
        Arc::new(Database::open(std::path::Path::new(":memory:")).unwrap())
    }

    fn service(database: Arc<Database>) -> Arc<LocalLinkService> {
        LocalLinkService::new(database).unwrap()
    }

    fn enable_without_keychain(service: &LocalLinkService) {
        service
            .database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_preferences SET enabled = 1, discovery_enabled = 0",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn trusted_peer(service: &LocalLinkService, device_id: &str) -> LocalIdentity {
        trusted_peer_at(service, device_id, &now())
    }

    fn trusted_peer_at(
        service: &LocalLinkService,
        device_id: &str,
        trusted_at: &str,
    ) -> LocalIdentity {
        let peer = LocalIdentity::generate_for_test().unwrap();
        service
            .database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "INSERT INTO local_link_devices
                     (device_id, display_name, peer_public_key, public_key_fingerprint,
                      trust_status, paired_at, trusted_at, protocol_min, protocol_max)
                     VALUES (?1, 'Peer Mac', ?2, ?3, 'trusted', ?4, ?4, 1, 1)",
                    params![
                        device_id,
                        peer.public_key(),
                        fingerprint(peer.public_key()),
                        trusted_at
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        peer
    }

    fn pending_transfer(
        service: &LocalLinkService,
        peer: &LocalIdentity,
        content: &str,
    ) -> LocalLinkTransfer {
        let offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            content.as_bytes(),
        );
        let id = service
            .reserve_inbound_offer(peer.public_key(), offer)
            .unwrap();
        service
            .commit_inbound_payload(&id, content.as_bytes().to_vec())
            .unwrap()
    }

    fn readiness_device(
        device_id: &str,
        trust_status: LocalLinkDeviceTrustStatus,
        online: bool,
    ) -> LocalLinkDevice {
        LocalLinkDevice {
            device_id: device_id.to_owned(),
            display_name: "Private peer label".to_owned(),
            public_key_fingerprint: String::new(),
            trust_status,
            paired_at: now(),
            trusted_at: None,
            trust_duration: LocalLinkTrustDuration::ThirtyDays,
            trust_expires_at: None,
            re_pairing_reason: None,
            revoked_at: None,
            last_seen_at: None,
            last_transfer_at: None,
            online,
            protocol_min: PROTOCOL_VERSION,
            protocol_max: PROTOCOL_VERSION,
        }
    }

    #[test]
    fn defaults_off_and_pairing_cannot_start_without_explicit_opt_in() {
        let service = service(database());
        assert!(!service.preferences().unwrap().enabled);
        let error = service
            .begin_pairing(BeginLocalLinkPairingRequest {
                device_id: "peer".to_owned(),
                display_name: "Peer".to_owned(),
            })
            .unwrap_err();
        assert!(error.to_string().contains("disabled"));
        assert!(service.list_devices().unwrap().is_empty());
    }

    #[test]
    fn discovery_is_off_before_the_explicit_runtime_is_started() {
        let service = service(database());
        let persisted = service.preferences().unwrap();
        assert!(!persisted.enabled);
        assert!(!persisted.discovery_enabled);
    }

    #[test]
    fn readiness_stops_at_the_first_blocker_in_native_order() {
        let off = LocalLinkPreferences {
            enabled: false,
            discovery_enabled: false,
            device_name: "This Mac".to_owned(),
        };
        let snapshot = derive_readiness_snapshot(&off, false, &[]);
        assert_eq!(snapshot.state, LocalLinkReadinessState::ActionRequired);
        assert_eq!(
            snapshot.first_blocker,
            Some(LocalLinkReadinessBlocker::LocalLinkDisabled)
        );
        assert_eq!(
            snapshot.primary_action,
            Some(LocalLinkReadinessAction::EnableLocalLink)
        );
        assert_eq!(
            snapshot
                .stages
                .iter()
                .map(|check| (check.stage, check.state))
                .collect::<Vec<_>>(),
            vec![
                (
                    LocalLinkReadinessStage::LocalNetwork,
                    LocalLinkReadinessStageState::NotChecked,
                ),
                (
                    LocalLinkReadinessStage::Enabled,
                    LocalLinkReadinessStageState::Blocked,
                ),
                (
                    LocalLinkReadinessStage::Discovery,
                    LocalLinkReadinessStageState::NotChecked,
                ),
                (
                    LocalLinkReadinessStage::Trust,
                    LocalLinkReadinessStageState::NotChecked,
                ),
                (
                    LocalLinkReadinessStage::AuthenticatedPresence,
                    LocalLinkReadinessStageState::NotChecked,
                ),
            ]
        );

        let enabled = LocalLinkPreferences {
            enabled: true,
            discovery_enabled: true,
            device_name: "This Mac".to_owned(),
        };
        let no_runtime = derive_readiness_snapshot(&enabled, false, &[]);
        assert_eq!(
            no_runtime.first_blocker,
            Some(LocalLinkReadinessBlocker::LocalNetworkUnavailable)
        );
        assert_eq!(
            no_runtime.stages[1].state,
            LocalLinkReadinessStageState::NotChecked
        );

        let discovery_off = derive_readiness_snapshot(
            &LocalLinkPreferences {
                discovery_enabled: false,
                ..enabled.clone()
            },
            true,
            &[],
        );
        assert_eq!(
            discovery_off.first_blocker,
            Some(LocalLinkReadinessBlocker::DiscoveryDisabled)
        );
        assert_eq!(
            discovery_off.stages[0].state,
            LocalLinkReadinessStageState::Passed
        );
        assert_eq!(
            discovery_off.stages[1].state,
            LocalLinkReadinessStageState::Passed
        );

        let no_trust = derive_readiness_snapshot(&enabled, true, &[]);
        assert_eq!(
            no_trust.first_blocker,
            Some(LocalLinkReadinessBlocker::NoTrustedDevice)
        );
        assert_eq!(
            no_trust.stages[4].state,
            LocalLinkReadinessStageState::NotChecked
        );

        let trusted_offline = derive_readiness_snapshot(
            &enabled,
            true,
            &[readiness_device(
                "trusted",
                LocalLinkDeviceTrustStatus::Trusted,
                false,
            )],
        );
        assert_eq!(trusted_offline.state, LocalLinkReadinessState::Offline);
        assert_eq!(
            trusted_offline.first_blocker,
            Some(LocalLinkReadinessBlocker::NoAuthenticatedPresence)
        );
        assert_eq!(
            trusted_offline.stages[3].state,
            LocalLinkReadinessStageState::Passed
        );
        assert_eq!(
            trusted_offline.stages[4].state,
            LocalLinkReadinessStageState::Blocked
        );
    }

    #[test]
    fn disabled_readiness_is_read_only_and_does_not_start_transport() {
        let service = service(database());
        assert!(service.transport.lock().unwrap().is_none());
        let before = service.preferences().unwrap();

        let snapshot = service.readiness_snapshot().unwrap();

        assert!(service.transport.lock().unwrap().is_none());
        assert_eq!(service.preferences().unwrap(), before);
        assert_eq!(
            snapshot.stages[0].state,
            LocalLinkReadinessStageState::NotChecked
        );
        assert_eq!(
            snapshot.first_blocker,
            Some(LocalLinkReadinessBlocker::LocalLinkDisabled)
        );
    }

    #[test]
    fn stale_start_action_cannot_reopen_transport_after_disable() {
        let service = service(database());
        let generation = {
            let mut lifecycle = service.lifecycle.lock().unwrap();
            let actions = lifecycle.handle(LifecycleEvent::PreferenceChanged { enabled: true });
            let generation = actions
                .into_iter()
                .find_map(|action| match action {
                    LifecycleAction::StartTransport { generation } => Some(generation),
                    _ => None,
                })
                .unwrap();
            lifecycle.handle(LifecycleEvent::PreferenceChanged { enabled: false });
            generation
        };
        let mut retired = Vec::new();

        assert!(service
            .start_transport(false, generation, &mut retired)
            .is_err());
        assert!(retired.is_empty());
        assert!(service.transport.lock().unwrap().is_none());
    }

    #[test]
    fn lifecycle_stop_retires_and_joins_transport_outside_the_executor_lock() {
        let service = service(database());
        service.start_transport_for_test(0).unwrap();
        service.handle_lifecycle_event(LifecycleEvent::PreferenceChanged { enabled: false });
        assert!(service.transport.lock().unwrap().is_none());
        assert!(service.lifecycle_execution.try_lock().is_ok());
    }

    #[test]
    fn readiness_projects_expired_trust_without_writing_the_database() {
        let service = service(database());
        let trusted_at = (Utc::now() - chrono::Duration::days(TRUST_DURATION_DAYS + 1))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        trusted_peer_at(&service, "expired-peer", &trusted_at);
        service.start_transport_for_test(0).unwrap();
        service
            .database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_preferences SET discovery_enabled = 1",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let snapshot = service.readiness_snapshot().unwrap();

        assert_eq!(
            snapshot.first_blocker,
            Some(LocalLinkReadinessBlocker::TrustRequiresRePairing)
        );
        let stored_status = service
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT trust_status FROM local_link_devices WHERE device_id = 'expired-peer'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(stored_status, "trusted");
    }

    #[test]
    fn authenticated_presence_never_substitutes_for_trust() {
        let preferences = LocalLinkPreferences {
            enabled: true,
            discovery_enabled: true,
            device_name: "This Mac".to_owned(),
        };
        let online_untrusted = readiness_device(
            "anonymous-nearby",
            LocalLinkDeviceTrustStatus::Revoked,
            true,
        );

        let snapshot = derive_readiness_snapshot(&preferences, true, &[online_untrusted]);

        assert_eq!(snapshot.state, LocalLinkReadinessState::ActionRequired);
        assert_eq!(
            snapshot.first_blocker,
            Some(LocalLinkReadinessBlocker::NoTrustedDevice)
        );
        assert_eq!(snapshot.ready_device_count, 0);
        assert_eq!(
            snapshot.stages[3].state,
            LocalLinkReadinessStageState::Blocked
        );
        assert_eq!(
            snapshot.stages[4].state,
            LocalLinkReadinessStageState::NotChecked
        );
    }

    #[test]
    fn diagnostics_are_bounded_ephemeral_and_exclude_sensitive_fields() {
        let database = database();
        let service = service(Arc::clone(&database));
        let timestamp = now();
        let private_device_id = "private-device-id-sentinel";
        let private_display_name = "Real Mac Name 192.168.1.7 PrivateSSID /Users/person/secret.txt";
        let private_fingerprint = "FULL-FINGERPRINT-SENTINEL";
        let private_key = b"PRIVATE-KEY-SENTINEL".to_vec();
        database
            .with_local_link_connection(|connection| {
                for index in 0..60 {
                    let device_id = if index == 0 {
                        private_device_id.to_owned()
                    } else {
                        format!("private-device-{index:02}")
                    };
                    let display_name = if index == 0 {
                        private_display_name.to_owned()
                    } else {
                        format!("Private Mac {index:02}")
                    };
                    let fingerprint = if index == 0 {
                        private_fingerprint.to_owned()
                    } else {
                        format!("PRIVATE-FINGERPRINT-{index:02}")
                    };
                    let peer_key = if index == 0 {
                        private_key.clone()
                    } else {
                        Vec::new()
                    };
                    connection.execute(
                        "INSERT INTO local_link_devices
                         (device_id, display_name, peer_public_key, public_key_fingerprint,
                          trust_status, paired_at, trusted_at, revoked_at, last_seen_at,
                          last_transfer_at, protocol_min, protocol_max, trust_duration)
                         VALUES (?1, ?2, ?3, ?4, 'needsRePairing', ?5, NULL, NULL, NULL,
                                 NULL, 1, 1, 'thirtyDays')",
                        params![device_id, display_name, peer_key, fingerprint, timestamp],
                    )?;
                }
                connection.execute(
                    "INSERT INTO local_link_transfers
                     (id, device_id, direction, status, item_kind, byte_size, created_at,
                      updated_at, completed_at, expires_at, failure_reason, receiver_action)
                     VALUES ('private-transfer-id-sentinel', ?1, 'outgoing', 'failed', 'text',
                             17, ?2, ?2, ?2, NULL, 'authFailed', NULL)",
                    params![private_device_id, timestamp],
                )?;
                connection.execute(
                    "INSERT INTO local_link_replay_tombstones
                     (peer_device_id, transfer_id, session_id, nonce, expires_at)
                     VALUES (?1, 'private-replay-transfer', 'private-session-id-sentinel',
                             'private-nonce-sentinel', ?2)",
                    params![private_device_id, timestamp],
                )?;
                Ok(())
            })
            .unwrap();
        database
            .capture_text(
                "clipboard-body-and-preview-sentinel",
                Some("/Users/person/private-source-app"),
            )
            .unwrap();

        let first = service.build_diagnostics().unwrap();
        let second = service.build_diagnostics().unwrap();
        assert_eq!(
            usize::try_from(first.record_count).unwrap(),
            first.peers.len() + first.transfers.len()
        );
        assert!(first.record_count <= MAX_DIAGNOSTIC_RECORDS as u32);
        assert_eq!(first.record_count, MAX_DIAGNOSTIC_RECORDS as u32);
        assert_eq!(first.transfers.len(), 1);
        assert_eq!(
            first.transfers[0].recovery_action,
            Some(crate::models::LocalLinkTransferRecoveryAction::RePairDevice)
        );
        assert_ne!(
            first.transfers[0].peer_ref, second.transfers[0].peer_ref,
            "peer references must be freshly salted for every export"
        );
        assert!(first
            .transfers
            .iter()
            .all(|transfer| transfer.peer_ref.starts_with("peer-")));

        let serialized = serde_json::to_string(&first).unwrap();
        for forbidden in [
            private_device_id,
            "private-transfer-id-sentinel",
            "private-session-id-sentinel",
            "private-nonce-sentinel",
            "clipboard-body-and-preview-sentinel",
            private_display_name,
            "Real Mac Name",
            "192.168.1.7",
            "PrivateSSID",
            "/Users/person",
            private_fingerprint,
            "PRIVATE-KEY-SENTINEL",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "diagnostics leaked forbidden sentinel: {forbidden}"
            );
        }
    }

    #[test]
    fn authenticated_inbound_is_memory_only_and_actions_are_mutually_exclusive() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "one decision only");
        assert_eq!(pending.status, LocalLinkTransferStatus::AwaitingReceiver);
        assert!(!serde_json::to_string(&pending)
            .unwrap()
            .contains("one decision only"));

        let writes = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();
        for _ in 0..16 {
            let service = Arc::clone(&service);
            let transfer_id = pending.id.clone();
            let writes = Arc::clone(&writes);
            workers.push(thread::spawn(move || {
                service.accept_transfer(&transfer_id, LocalLinkReceiveAction::Copy, |_, _| {
                    writes.fetch_add(1, Ordering::SeqCst);
                    Ok(ClipboardWriteEvidence { change_count: 1 })
                })
            }));
        }
        let successes = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().is_ok())
            .filter(|success| *success)
            .count();
        // Every duplicate command converges to the same durable terminal
        // result, while the native side effect itself has one CAS winner.
        assert_eq!(successes, 16);
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert!(database
            .list("one decision", false, 10, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn viewing_a_pending_transfer_is_idempotent_and_never_serializes_plaintext() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "viewed body sentinel");

        let first = service.mark_transfer_viewed(&pending.id).unwrap();
        let second = service.mark_transfer_viewed(&pending.id).unwrap();
        assert_eq!(first.status, LocalLinkTransferStatus::Viewed);
        assert_eq!(second.status, LocalLinkTransferStatus::Viewed);
        assert!(!serde_json::to_string(&second)
            .unwrap()
            .contains("viewed body sentinel"));

        let mut revealed = String::new();
        service
            .reveal_transfer(&pending.id, |payload| {
                revealed.push_str(payload.as_str());
                Ok(())
            })
            .unwrap();
        assert_eq!(revealed, "viewed body sentinel");
        service.reject_transfer(&pending.id).unwrap();
    }

    #[test]
    fn revoke_and_accept_are_linearized_by_one_runtime_lock() {
        let database = database();
        let service = service(database);
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "race sentinel");
        service.revoke_device("peer").unwrap();
        let writes = AtomicUsize::new(0);
        assert!(service
            .accept_transfer(&pending.id, LocalLinkReceiveAction::Copy, |_, _| {
                writes.fetch_add(1, Ordering::SeqCst);
                Ok(ClipboardWriteEvidence { change_count: 1 })
            })
            .is_err());
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        let transfer = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == pending.id)
            .unwrap();
        assert_eq!(
            transfer.failure_reason,
            Some(LocalLinkTransferFailure::DeviceRevoked)
        );
    }

    #[test]
    fn duplicate_transfer_and_nonce_are_rejected_before_payload() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"dedupe",
        );
        service
            .reserve_inbound_offer(peer.public_key(), offer.clone())
            .unwrap();
        assert_eq!(
            service.reserve_inbound_offer(peer.public_key(), offer),
            Err(LocalLinkTransferFailure::ReceiverBusy)
        );
    }

    #[test]
    fn per_peer_and_global_pending_caps_and_rate_limit_are_enforced() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer-1");
        let first = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"first",
        );
        service
            .reserve_inbound_offer(peer.public_key(), first)
            .unwrap();
        for _ in 0..4 {
            let offer = TransferOffer::new(
                uuid::Uuid::new_v4().to_string(),
                uuid::Uuid::new_v4().to_string(),
                b"busy",
            );
            assert_eq!(
                service.reserve_inbound_offer(peer.public_key(), offer),
                Err(LocalLinkTransferFailure::ReceiverBusy)
            );
        }
        let sixth = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"rate",
        );
        assert_eq!(
            service.reserve_inbound_offer(peer.public_key(), sixth),
            Err(LocalLinkTransferFailure::RateLimited)
        );
    }

    #[test]
    fn receiver_lock_clears_pending_plaintext_and_blocks_new_offers() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "lock sentinel");
        service.set_receiver_available(false).unwrap();
        assert!(service.reject_transfer(&pending.id).is_err());
        let offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"locked",
        );
        assert_eq!(
            service.reserve_inbound_offer(peer.public_key(), offer),
            Err(LocalLinkTransferFailure::ReceiverLocked)
        );
    }

    #[test]
    fn disable_clears_pending_and_marks_cancelled_without_affecting_history() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let local = database
            .capture_text("local history", None)
            .unwrap()
            .unwrap();
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "pending sentinel");
        service
            .update_preferences(LocalLinkPreferences {
                enabled: false,
                discovery_enabled: false,
                device_name: "Test Mac".to_owned(),
            })
            .unwrap();
        assert!(service.reject_transfer(&pending.id).is_err());
        assert_eq!(
            database.get_by_id(&local.id).unwrap().content,
            "local history"
        );
        let transfer = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == pending.id)
            .unwrap();
        assert_eq!(transfer.status, LocalLinkTransferStatus::Cancelled);
    }

    #[test]
    fn shutdown_never_claims_cancellation_after_payload_or_copy_effect_commit_point() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "uncertain-peer");

        let outgoing = service
            .persist_outgoing_connecting("uncertain-peer", 12)
            .unwrap();
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers SET status = 'awaitingReceiver'
                     WHERE id = ?1",
                    [&outgoing.id],
                )?;
                Ok(())
            })
            .unwrap();

        let incoming = pending_transfer(&service, &peer, "copy race sentinel");
        database
            .prepare_local_link_copy_effect(&incoming.id, "0123456789abcdef0123456789abcdef")
            .unwrap();

        service
            .cancel_all_active(LocalLinkTransferFailure::PayloadUnavailable, true)
            .unwrap();

        let transfers = service.list_transfers(20).unwrap();
        for transfer_id in [&outgoing.id, &incoming.id] {
            let transfer = transfers
                .iter()
                .find(|transfer| transfer.id == *transfer_id)
                .unwrap();
            assert_eq!(transfer.status, LocalLinkTransferStatus::Failed);
            assert_eq!(
                transfer.failure_reason,
                Some(LocalLinkTransferFailure::OutcomeUnknown)
            );
            assert_eq!(
                terminal_receipt_for_transfer(transfer),
                Some(TerminalReceipt::outcome_unknown())
            );
        }
        assert_eq!(
            database
                .local_link_copy_effect_claim(&incoming.id)
                .unwrap()
                .unwrap()
                .state,
            crate::db::LocalLinkCopyEffectState::Uncertain
        );
    }

    #[test]
    fn revoke_preserves_uncertainty_after_payload_and_copy_commit_points() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "revoked-uncertain-peer");
        let outgoing = service
            .persist_outgoing_connecting("revoked-uncertain-peer", 8)
            .unwrap();
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers SET status = 'awaitingReceiver'
                     WHERE id = ?1",
                    [&outgoing.id],
                )?;
                Ok(())
            })
            .unwrap();
        let incoming = pending_transfer(&service, &peer, "prepared revoke sentinel");
        database
            .prepare_local_link_copy_effect(&incoming.id, "revoked-copy-effect-token-0001")
            .unwrap();

        service.revoke_device("revoked-uncertain-peer").unwrap();

        let transfers = service.list_transfers(20).unwrap();
        for transfer_id in [&outgoing.id, &incoming.id] {
            let transfer = transfers
                .iter()
                .find(|transfer| transfer.id == *transfer_id)
                .unwrap();
            assert_eq!(
                transfer.failure_reason,
                Some(LocalLinkTransferFailure::OutcomeUnknown)
            );
        }
        assert_eq!(
            database
                .local_link_copy_effect_claim(&incoming.id)
                .unwrap()
                .unwrap()
                .state,
            crate::db::LocalLinkCopyEffectState::Uncertain
        );
    }

    #[test]
    fn expiry_never_relabels_a_prepared_copy_effect_as_safely_expired() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "expiry-effect-peer");
        let incoming = pending_transfer(&service, &peer, "prepared expiry sentinel");
        database
            .prepare_local_link_copy_effect(&incoming.id, "expiry-copy-effect-token-00001")
            .unwrap();

        service.expire_pending(i64::MAX).unwrap();

        let transfer = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == incoming.id)
            .unwrap();
        assert_eq!(transfer.status, LocalLinkTransferStatus::Failed);
        assert_eq!(
            transfer.failure_reason,
            Some(LocalLinkTransferFailure::OutcomeUnknown)
        );
        assert_eq!(
            database
                .local_link_copy_effect_claim(&incoming.id)
                .unwrap()
                .unwrap()
                .state,
            crate::db::LocalLinkCopyEffectState::Uncertain
        );
        assert!(!service
            .runtime
            .lock()
            .unwrap()
            .pending_inbound
            .contains_key(&incoming.id));
    }

    #[test]
    fn receiver_suspension_keeps_post_payload_reconciliation_and_copy_uncertainty() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "suspend-effect-peer");
        let outgoing = service
            .persist_outgoing_connecting("suspend-effect-peer", 4)
            .unwrap();
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers SET status = 'awaitingReceiver'
                     WHERE id = ?1",
                    [&outgoing.id],
                )?;
                Ok(())
            })
            .unwrap();
        let incoming = pending_transfer(&service, &peer, "suspend prepared sentinel");
        database
            .prepare_local_link_copy_effect(&incoming.id, "suspend-copy-effect-token-00001")
            .unwrap();

        service.set_receiver_available(false).unwrap();

        let transfers = service.list_transfers(20).unwrap();
        let outgoing = transfers
            .iter()
            .find(|transfer| transfer.id == outgoing.id)
            .unwrap();
        assert_eq!(outgoing.status, LocalLinkTransferStatus::Reconciling);
        assert!(outgoing.expires_at.is_some());
        let incoming = transfers
            .iter()
            .find(|transfer| transfer.id == incoming.id)
            .unwrap();
        assert_eq!(
            incoming.failure_reason,
            Some(LocalLinkTransferFailure::OutcomeUnknown)
        );
        assert_eq!(
            database
                .local_link_copy_effect_claim(&incoming.id)
                .unwrap()
                .unwrap()
                .state,
            crate::db::LocalLinkCopyEffectState::Uncertain
        );
    }

    #[test]
    fn devices_project_authenticated_presence_without_persisting_reachability() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "online-peer");
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_devices
                     SET last_seen_at = '2000-01-01T00:00:00.000Z'
                     WHERE device_id = 'online-peer'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let before = service
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "online-peer")
            .unwrap();
        assert!(!before.online);
        assert_eq!(
            before.last_seen_at.as_deref(),
            Some("2000-01-01T00:00:00.000Z")
        );

        service
            .authenticate_peer("online-peer", peer.public_key())
            .unwrap();
        let online = service
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "online-peer")
            .unwrap();
        assert!(online.online);
        assert_ne!(
            online.last_seen_at.as_deref(),
            Some("2000-01-01T00:00:00.000Z")
        );
        let persisted_last_seen: Option<String> = database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT last_seen_at FROM local_link_devices WHERE device_id = 'online-peer'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(
            persisted_last_seen.as_deref(),
            Some("2000-01-01T00:00:00.000Z")
        );

        service
            .runtime
            .lock()
            .unwrap()
            .device_presence
            .get_mut("online-peer")
            .unwrap()
            .observed_at_ms =
            Utc::now().timestamp_millis() - RUNTIME_PRESENCE_TTL_SECONDS * 1_000 - 1;
        let offline = service
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "online-peer")
            .unwrap();
        assert!(!offline.online);
        assert_eq!(
            offline.last_seen_at.as_deref(),
            Some("2000-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn only_copy_or_save_receipts_record_last_successful_transfer() {
        let database = database();
        let service = service(Arc::clone(&database));
        let _peer = trusted_peer(&service, "receipt-peer");
        let last_transfer_at = |database: &Database| -> Option<String> {
            database
                .with_local_link_connection(|connection| {
                    connection
                        .query_row(
                            "SELECT last_transfer_at FROM local_link_devices WHERE device_id = 'receipt-peer'",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(AppError::from)
                })
                .unwrap()
        };

        let copied = service
            .persist_outgoing_connecting("receipt-peer", 4)
            .unwrap();
        service
            .apply_outgoing_receipt(
                &copied.id,
                TerminalReceipt::completed(ReceiptOutcome::Copied),
            )
            .unwrap();
        assert!(last_transfer_at(&database).is_some());

        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_devices SET last_transfer_at = NULL WHERE device_id = 'receipt-peer'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let rejected = service
            .persist_outgoing_connecting("receipt-peer", 4)
            .unwrap();
        service
            .apply_outgoing_receipt(
                &rejected.id,
                TerminalReceipt::completed(ReceiptOutcome::Rejected),
            )
            .unwrap();
        let failed = service
            .persist_outgoing_connecting("receipt-peer", 4)
            .unwrap();
        service
            .apply_outgoing_receipt(
                &failed.id,
                TerminalReceipt::not_delivered(LocalLinkTransferFailure::DeviceUnavailable),
            )
            .unwrap();
        assert_eq!(last_transfer_at(&database), None);

        let saved = service
            .persist_outgoing_connecting("receipt-peer", 4)
            .unwrap();
        service
            .apply_outgoing_receipt(&saved.id, TerminalReceipt::completed(ReceiptOutcome::Saved))
            .unwrap();
        assert!(last_transfer_at(&database).is_some());
    }

    #[test]
    fn old_retry_semantics_are_removed() {
        let service = service(database());
        let error = service.retry_transfer("old-transfer").unwrap_err();
        assert!(error
            .to_string()
            .contains("Select the current clipboard item"));
    }

    #[test]
    fn expired_trust_requires_repair_and_clears_usable_key_material() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let trusted_at = (Utc::now()
            - chrono::Duration::days(TRUST_DURATION_DAYS)
            - chrono::Duration::seconds(1))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
        trusted_peer_at(&service, "expired-peer", &trusted_at);

        let device = service
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "expired-peer")
            .unwrap();
        assert_eq!(
            device.trust_status,
            LocalLinkDeviceTrustStatus::NeedsRePairing
        );
        assert_eq!(
            device.re_pairing_reason,
            Some(LocalLinkDeviceRePairingReason::TrustExpired)
        );
        assert_eq!(device.trust_duration, LocalLinkTrustDuration::ThirtyDays);
        assert_eq!(
            device.trust_expires_at,
            trust_expires_at(Some(&trusted_at), LocalLinkTrustDuration::ThirtyDays)
        );
        let key_length = database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT length(peer_public_key) FROM local_link_devices WHERE device_id = 'expired-peer'",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(key_length, 0);

        let item = database
            .capture_text("cannot use expired trust", None)
            .unwrap()
            .unwrap();
        let transfer = service
            .send_clipboard_item(&item.id, "expired-peer")
            .unwrap();
        assert_eq!(transfer.status, LocalLinkTransferStatus::Failed);
        assert_eq!(
            transfer.failure_reason,
            Some(LocalLinkTransferFailure::DeviceNotTrusted)
        );
        assert_eq!(
            transfer.recovery_action,
            Some(crate::models::LocalLinkTransferRecoveryAction::RePairDevice)
        );
    }

    #[test]
    fn always_trust_does_not_expire_after_thirty_days() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let trusted_at = (Utc::now() - chrono::Duration::days(TRUST_DURATION_DAYS + 1))
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let peer = trusted_peer_at(&service, "always-peer", &trusted_at);
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_devices SET trust_duration = 'always'
                     WHERE device_id = 'always-peer'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        let device = service
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == "always-peer")
            .unwrap();
        assert_eq!(device.trust_status, LocalLinkDeviceTrustStatus::Trusted);
        assert_eq!(device.trust_duration, LocalLinkTrustDuration::Always);
        assert_eq!(device.trust_expires_at, None);
        service
            .authenticate_peer("always-peer", peer.public_key())
            .unwrap();
    }

    #[test]
    fn terminal_transfers_expose_truthful_recovery_without_payload_replay() {
        let service = service(database());
        enable_without_keychain(&service);
        trusted_peer(&service, "peer");

        let re_pair = service
            .persist_failed_outgoing("peer", 0, LocalLinkTransferFailure::DeviceNotTrusted)
            .unwrap();
        assert_eq!(
            re_pair.recovery_action,
            Some(crate::models::LocalLinkTransferRecoveryAction::RePairDevice)
        );
        let retry_from_original = service
            .persist_failed_outgoing("peer", 0, LocalLinkTransferFailure::DeviceUnavailable)
            .unwrap();
        assert_eq!(
            retry_from_original.recovery_action,
            Some(crate::models::LocalLinkTransferRecoveryAction::StartNewTransferFromOriginalItem)
        );
        let serialized = serde_json::to_string(&retry_from_original).unwrap();
        assert!(serialized.contains("recoveryAction"));
        assert!(!serialized.contains("retry from original"));
    }

    #[test]
    fn recent_terminal_evidence_survives_count_pruning_until_the_recovery_grace_ends() {
        let service = service(database());
        enable_without_keychain(&service);
        trusted_peer(&service, "peer");
        for _ in 0..25 {
            service
                .persist_failed_outgoing("peer", 1, LocalLinkTransferFailure::TransportUnavailable)
                .unwrap();
        }
        let count: i64 = service
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM local_link_transfers", [], |row| {
                        row.get(0)
                    })
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(count, 25_i64);
        assert_eq!(service.list_transfers(20).unwrap().len(), 20);

        service
            .database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers SET updated_at = ?1",
                    [timestamp_from_millis(
                        Utc::now()
                            .timestamp_millis()
                            .saturating_sub(TERMINAL_EVIDENCE_GRACE_MS + 1),
                    )],
                )?;
                Ok(())
            })
            .unwrap();
        service
            .persist_failed_outgoing("peer", 1, LocalLinkTransferFailure::TransportUnavailable)
            .unwrap();
        let count: i64 = service
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM local_link_transfers", [], |row| {
                        row.get(0)
                    })
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(count, 20_i64);
    }

    #[test]
    fn pruning_never_deletes_reconciling_or_prepared_effect_evidence() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "prune-active-peer");
        let reconciling = service
            .persist_outgoing_connecting("prune-active-peer", 1)
            .unwrap();
        database
            .mark_local_link_transfer_reconciling(
                &reconciling.id,
                &timestamp_from_millis(Utc::now().timestamp_millis() + 10_000),
            )
            .unwrap();
        let prepared = pending_transfer(&service, &peer, "prepared prune sentinel");
        database
            .prepare_local_link_copy_effect(&prepared.id, "prune-copy-effect-token-000001")
            .unwrap();
        database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers SET updated_at = '2000-01-01T00:00:00.000Z'
                     WHERE id IN (?1, ?2)",
                    params![reconciling.id, prepared.id],
                )?;
                prune_transfer_summaries(connection)
            })
            .unwrap();

        let remaining = service.list_transfers(20).unwrap();
        assert!(remaining
            .iter()
            .any(|transfer| transfer.id == reconciling.id));
        assert!(remaining.iter().any(|transfer| transfer.id == prepared.id));
        assert_eq!(
            database
                .local_link_copy_effect_claim(&prepared.id)
                .unwrap()
                .unwrap()
                .state,
            crate::db::LocalLinkCopyEffectState::Prepared
        );
    }

    #[test]
    fn transfer_schema_has_no_payload_or_clipboard_item_reference() {
        let service = service(database());
        let columns = service
            .database
            .with_local_link_connection(|connection| {
                let mut statement = connection
                    .prepare("SELECT name FROM pragma_table_info('local_link_transfers')")?;
                let names = statement.query_map([], |row| row.get::<_, String>(0))?;
                names.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
            })
            .unwrap();
        for prohibited in [
            "payload",
            "content",
            "clipboard_item_id",
            "endpoint",
            "session_key",
        ] {
            assert!(!columns.iter().any(|column| column == prohibited));
        }
    }

    #[test]
    fn deadline_reaper_expires_plaintext_without_opening_transfers_ui() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let mut offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"deadline sentinel",
        );
        offer.ttl_seconds = 1;
        let transfer_id = service
            .reserve_inbound_offer(peer.public_key(), offer)
            .unwrap();
        service
            .commit_inbound_payload(&transfer_id, b"deadline sentinel".to_vec())
            .unwrap();

        for _ in 0..50 {
            if !service
                .runtime
                .lock()
                .unwrap()
                .pending_inbound
                .contains_key(&transfer_id)
            {
                break;
            }
            thread::sleep(Duration::from_millis(40));
        }
        assert!(!service
            .runtime
            .lock()
            .unwrap()
            .pending_inbound
            .contains_key(&transfer_id));
        let status: String = service
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT status FROM local_link_transfers WHERE id = ?1",
                        [&transfer_id],
                        |row| row.get(0),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(status, "expired");
    }

    #[test]
    fn direct_deadline_sweep_clears_pending_without_listing_transfers() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"direct sweep",
        );
        let deadline = offer.created_at_ms + i64::from(offer.ttl_seconds) * 1_000;
        let transfer_id = service
            .reserve_inbound_offer(peer.public_key(), offer)
            .unwrap();
        service
            .commit_inbound_payload(&transfer_id, b"direct sweep".to_vec())
            .unwrap();
        service.expire_pending(deadline).unwrap();
        assert!(!service
            .runtime
            .lock()
            .unwrap()
            .pending_inbound
            .contains_key(&transfer_id));
    }

    #[test]
    fn reveal_is_native_callback_only_and_unavailable_after_terminal_paths() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");

        let pending = pending_transfer(&service, &peer, "reveal sentinel");
        let mut revealed = String::new();
        service
            .reveal_transfer(&pending.id, |payload| {
                revealed.push_str(payload.as_str());
                Ok(())
            })
            .unwrap();
        assert_eq!(revealed, "reveal sentinel");
        service.reject_transfer(&pending.id).unwrap();
        assert!(service.reveal_transfer(&pending.id, |_| Ok(())).is_err());

        let expired = pending_transfer(&service, &peer, "expired reveal");
        service
            .expire_pending(Utc::now().timestamp_millis() + 61_000)
            .unwrap();
        assert!(service.reveal_transfer(&expired.id, |_| Ok(())).is_err());

        let revoked = pending_transfer(&service, &peer, "revoked reveal");
        service.revoke_device("peer").unwrap();
        assert!(service.reveal_transfer(&revoked.id, |_| Ok(())).is_err());
    }

    #[test]
    fn sender_cancel_command_cannot_consume_an_incoming_payload() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "reject me explicitly");

        let error = service.cancel_transfer(&pending.id).unwrap_err();
        assert!(error.to_string().contains("Only the sender"));
        let mut revealed = String::new();
        service
            .reveal_transfer(&pending.id, |payload| {
                revealed.push_str(payload.as_str());
                Ok(())
            })
            .unwrap();
        assert_eq!(revealed, "reject me explicitly");
        service.reject_transfer(&pending.id).unwrap();
    }

    #[test]
    fn inbound_cancel_clears_the_pending_body_and_is_idempotent() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "cancel body sentinel");

        let first = service.cancel_inbound_transfer(&pending.id).unwrap();
        let second = service.cancel_inbound_transfer(&pending.id).unwrap();
        assert_eq!(first, TerminalReceipt::completed(ReceiptOutcome::Cancelled));
        assert_eq!(second, first);
        assert!(!service
            .runtime
            .lock()
            .unwrap()
            .pending_inbound
            .contains_key(&pending.id));
        assert!(service.reveal_transfer(&pending.id, |_| Ok(())).is_err());

        let summary = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == pending.id)
            .unwrap();
        assert_eq!(summary.status, LocalLinkTransferStatus::Cancelled);
        assert!(!serde_json::to_string(&summary)
            .unwrap()
            .contains("cancel body sentinel"));
    }

    #[test]
    fn receiver_decision_receipt_replaces_only_provisional_cancel_or_outcome_unknown() {
        let service = service(database());
        enable_without_keychain(&service);
        let _peer = trusted_peer(&service, "peer");
        let outgoing = service.persist_outgoing_connecting("peer", 5).unwrap();

        assert_eq!(
            service.cancel_transfer(&outgoing.id).unwrap().status,
            LocalLinkTransferStatus::Cancelled
        );
        let copied = TerminalReceipt::completed(ReceiptOutcome::Copied);
        service
            .apply_outgoing_receipt(&outgoing.id, copied)
            .unwrap();
        // A retransmitted terminal receipt cannot create a second outcome.
        service
            .apply_outgoing_receipt(&outgoing.id, copied)
            .unwrap();

        let summary = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == outgoing.id)
            .unwrap();
        assert_eq!(summary.status, LocalLinkTransferStatus::Copied);
        assert_eq!(summary.receiver_action, Some(LocalLinkReceiveAction::Copy));
        assert!(service
            .apply_outgoing_receipt(
                &outgoing.id,
                TerminalReceipt::completed(ReceiptOutcome::Cancelled),
            )
            .is_err());

        let unknown = service.persist_outgoing_connecting("peer", 5).unwrap();
        service
            .database
            .with_local_link_connection(|connection| {
                connection.execute(
                    "UPDATE local_link_transfers
                     SET status = 'failed', failure_reason = 'outcomeUnknown',
                         completed_at = ?1, updated_at = ?1
                     WHERE id = ?2",
                    params![now(), unknown.id],
                )?;
                Ok(())
            })
            .unwrap();
        service
            .receipts
            .lock()
            .unwrap()
            .apply_receipt(&unknown.id, TerminalReceipt::outcome_unknown());
        service
            .apply_outgoing_receipt(
                &unknown.id,
                TerminalReceipt::completed(ReceiptOutcome::Saved),
            )
            .unwrap();
        let summary = service
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.id == unknown.id)
            .unwrap();
        assert_eq!(summary.status, LocalLinkTransferStatus::Saved);
    }

    #[test]
    fn explicit_identity_reset_disables_link_clears_pending_and_requires_repair() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let local = database
            .capture_text("reset keeps history", None)
            .unwrap()
            .unwrap();
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "reset sentinel");

        service.reset_identity_for_test().unwrap();

        assert!(!service.preferences().unwrap().enabled);
        assert!(service.reveal_transfer(&pending.id, |_| Ok(())).is_err());
        let (status, key_bytes, fingerprint): (String, i64, String) = service
            .database
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT trust_status, length(peer_public_key), public_key_fingerprint
                         FROM local_link_devices WHERE device_id = 'peer'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(status, "needsRePairing");
        assert_eq!(key_bytes, 0);
        assert!(fingerprint.is_empty());
        assert_eq!(
            service.list_devices().unwrap()[0].re_pairing_reason,
            Some(LocalLinkDeviceRePairingReason::IdentityInvalidated)
        );
        assert_eq!(
            database.get_by_id(&local.id).unwrap().content,
            "reset keeps history"
        );
    }

    #[test]
    fn failed_identity_deletion_leaves_link_disabled_and_process_key_cleared() {
        let service = service(database());
        enable_without_keychain(&service);
        let peer = trusted_peer(&service, "peer");
        let pending = pending_transfer(&service, &peer, "failed reset sentinel");
        *service.identity.lock().unwrap() =
            Some(Arc::new(LocalIdentity::generate_for_test().unwrap()));

        let error = service
            .reset_identity_state(|| {
                Err(AppError::InvalidInput(
                    "synthetic Keychain deletion failure".to_owned(),
                ))
            })
            .unwrap_err();

        assert!(error.to_string().contains("synthetic Keychain"));
        assert!(!service.preferences().unwrap().enabled);
        assert!(service.identity.lock().unwrap().is_none());
        assert!(service.reveal_transfer(&pending.id, |_| Ok(())).is_err());
        // Trust is invalidated only after Keychain deletion succeeds. Keeping
        // the original record here prevents a failed reset from silently
        // binding that peer to a newly generated identity.
        assert_eq!(
            service.list_devices().unwrap()[0].trust_status,
            LocalLinkDeviceTrustStatus::Trusted
        );
    }

    #[test]
    fn clearing_transfers_only_removes_terminal_summaries() {
        let database = database();
        let service = service(Arc::clone(&database));
        enable_without_keychain(&service);
        let local = database
            .capture_text("history survives transfer cleanup", None)
            .unwrap()
            .unwrap();
        let first_peer = trusted_peer(&service, "peer-1");
        let second_peer = trusted_peer(&service, "peer-2");
        let terminal = pending_transfer(&service, &first_peer, "terminal");
        service.reject_transfer(&terminal.id).unwrap();
        let active_offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            b"active",
        );
        let active_id = service
            .reserve_inbound_offer(second_peer.public_key(), active_offer)
            .unwrap();

        assert_eq!(service.clear_transfers().unwrap(), 1);
        let remaining: Vec<(String, String)> = service
            .database
            .with_local_link_connection(|connection| {
                let mut statement = connection
                    .prepare("SELECT id, status FROM local_link_transfers ORDER BY id")?;
                let remaining = statement
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(AppError::from)?;
                Ok(remaining)
            })
            .unwrap();
        assert_eq!(remaining, vec![(active_id, "encrypted".to_owned())]);
        assert_eq!(service.list_devices().unwrap().len(), 2);
        assert_eq!(
            database.get_by_id(&local.id).unwrap().content,
            "history survives transfer cleanup"
        );
    }

    fn wait_for(condition: impl Fn() -> bool, message: &str) {
        for _ in 0..200 {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("{message}");
    }

    fn pair_services_for_direct_transfer(
        sender: &LocalLinkService,
        receiver: &LocalLinkService,
        receiver_endpoint: SocketAddr,
    ) -> LocalLinkDevice {
        let receiver_device = sender.begin_pairing_at_for_test(receiver_endpoint).unwrap();
        wait_for(
            || {
                receiver
                    .list_devices()
                    .unwrap()
                    .iter()
                    .any(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            },
            "receiver did not create pairing candidate",
        );
        let sender_device = receiver
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            .unwrap();
        sender
            .confirm_pairing(
                &receiver_device.device_id,
                LocalLinkTrustDuration::ThirtyDays,
            )
            .unwrap();
        receiver
            .confirm_pairing(&sender_device.device_id, LocalLinkTrustDuration::ThirtyDays)
            .unwrap();
        wait_for(
            || {
                sender.list_devices().unwrap().iter().any(|device| {
                    device.device_id == receiver_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                }) && receiver.list_devices().unwrap().iter().any(|device| {
                    device.device_id == sender_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                })
            },
            "bilateral pairing did not become trusted",
        );
        receiver_device
    }

    #[test]
    fn direct_tcp_pair_send_and_terminal_receipt_round_trip() {
        let sender_db = database();
        let receiver_db = database();
        let sender = service(Arc::clone(&sender_db));
        let receiver = service(Arc::clone(&receiver_db));
        let sender_endpoint = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();

        let receiver_device = sender.begin_pairing_at_for_test(receiver_endpoint).unwrap();
        wait_for(
            || {
                receiver
                    .list_devices()
                    .unwrap()
                    .iter()
                    .any(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            },
            "receiver did not create pairing candidate",
        );
        let sender_device = receiver
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            .unwrap();
        let sender_code = sender
            .pairing_code_for_test(&receiver_device.device_id)
            .unwrap();
        let receiver_code = receiver
            .pairing_code_for_test(&sender_device.device_id)
            .unwrap();
        assert_eq!(sender_code, receiver_code);
        let sender_details = sender.pairing_details(&receiver_device.device_id).unwrap();
        assert_eq!(sender_details.verification_code, sender_code);
        assert_eq!(sender_details.device.device_id, receiver_device.device_id);
        assert_eq!(sender_details.local_fingerprint.len(), 29);
        assert_eq!(sender_details.peer_fingerprint.len(), 29);
        assert_eq!(
            sender
                .active_pairing_details()
                .unwrap()
                .expect("one active pairing")
                .device
                .device_id,
            receiver_device.device_id
        );
        sender
            .confirm_pairing(
                &receiver_device.device_id,
                LocalLinkTrustDuration::ThirtyDays,
            )
            .unwrap();
        receiver
            .confirm_pairing(&sender_device.device_id, LocalLinkTrustDuration::ThirtyDays)
            .unwrap();
        wait_for(
            || {
                sender.list_devices().unwrap().iter().any(|device| {
                    device.device_id == receiver_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                }) && receiver.list_devices().unwrap().iter().any(|device| {
                    device.device_id == sender_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                })
            },
            "bilateral pairing did not become trusted",
        );
        assert_eq!(
            sender
                .list_devices()
                .unwrap()
                .into_iter()
                .find(|device| device.device_id == receiver_device.device_id)
                .unwrap()
                .trust_duration,
            LocalLinkTrustDuration::ThirtyDays
        );

        let item = sender_db
            .capture_text("native direct transport", None)
            .unwrap()
            .unwrap();
        let outgoing = sender
            .send_clipboard_item_to_endpoint_for_test(
                &item.id,
                &receiver_device.device_id,
                receiver_endpoint,
            )
            .unwrap();
        wait_for(
            || {
                receiver.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.direction == LocalLinkTransferDirection::Incoming
                        && transfer.status == LocalLinkTransferStatus::AwaitingReceiver
                })
            },
            "receiver did not receive encrypted payload",
        );
        let incoming = receiver
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.direction == LocalLinkTransferDirection::Incoming)
            .unwrap();
        let viewed = receiver.mark_transfer_viewed(&incoming.id).unwrap();
        assert_eq!(viewed.status, LocalLinkTransferStatus::Viewed);
        wait_for(
            || {
                sender.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id && transfer.status == LocalLinkTransferStatus::Viewed
                })
            },
            "sender did not receive metadata-only viewed acknowledgement",
        );
        let clipboard_writes = AtomicUsize::new(0);
        receiver
            .accept_transfer(&incoming.id, LocalLinkReceiveAction::Copy, |_, _| {
                clipboard_writes.fetch_add(1, Ordering::SeqCst);
                Ok(ClipboardWriteEvidence { change_count: 1 })
            })
            .unwrap();
        assert_eq!(clipboard_writes.load(Ordering::SeqCst), 1);
        wait_for(
            || {
                sender.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id
                        && transfer.status == LocalLinkTransferStatus::Copied
                        && transfer.receiver_action == Some(LocalLinkReceiveAction::Copy)
                })
            },
            "sender did not receive terminal copy receipt",
        );
        assert_ne!(sender_endpoint.port(), 0);
    }

    #[test]
    fn stalled_status_socket_cannot_exceed_the_remaining_query_deadline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let endpoint = listener.local_addr().unwrap();
        let stalled = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(750));
        });
        let service = service(database());
        enable_without_keychain(&service);
        *service.identity.lock().unwrap() =
            Some(Arc::new(LocalIdentity::generate_for_test().unwrap()));
        *service.lifecycle.lock().unwrap() = LifecycleSupervisor::new(true);

        let started = std::time::Instant::now();
        assert!(service
            .query_transfer_status_at_endpoint(
                endpoint,
                "stalled-peer",
                "stalled-transfer",
                Utc::now().timestamp_millis() + 150,
            )
            .is_err());
        assert!(started.elapsed() < Duration::from_millis(650));
        stalled.join().unwrap();
    }

    #[test]
    fn known_lifecycle_pause_extends_the_persisted_active_time_deadline() {
        let service = service(database());
        enable_without_keychain(&service);
        trusted_peer(&service, "paused-reconciliation-peer");
        let transfer = service
            .persist_outgoing_connecting("paused-reconciliation-peer", 1)
            .unwrap();
        let original_deadline_ms = Utc::now().timestamp_millis() + 10_000;
        service
            .database
            .mark_local_link_transfer_reconciling(
                &transfer.id,
                &timestamp_from_millis(original_deadline_ms),
            )
            .unwrap();
        service.runtime.lock().unwrap().reconciliation_paused_at_ms =
            Some(Utc::now().timestamp_millis() - 5_000);

        service.resume_reconciliation_deadlines().unwrap();

        let extended = service.reconciliation_deadline_ms(&transfer.id).unwrap();
        assert!(extended >= original_deadline_ms + 4_900);
        assert!(extended <= original_deadline_ms + 5_500);
        assert_eq!(
            service.runtime.lock().unwrap().reconciliation_paused_at_ms,
            None
        );
    }

    #[test]
    fn dropped_terminal_receipt_reconciles_by_status_without_replaying_payload() {
        let sender_db = database();
        let receiver_db = database();
        let sender = service(Arc::clone(&sender_db));
        let receiver = service(Arc::clone(&receiver_db));
        let _sender_endpoint = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();
        let receiver_device =
            pair_services_for_direct_transfer(&sender, &receiver, receiver_endpoint);

        let item = sender_db
            .capture_text("receipt loss must never replay this body", None)
            .unwrap()
            .unwrap();
        let outgoing = sender
            .send_clipboard_item_to_endpoint_for_test(
                &item.id,
                &receiver_device.device_id,
                receiver_endpoint,
            )
            .unwrap();
        wait_for(
            || {
                receiver.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id
                        && transfer.direction == LocalLinkTransferDirection::Incoming
                        && transfer.status == LocalLinkTransferStatus::AwaitingReceiver
                })
            },
            "receiver did not retain the pending request",
        );
        assert_eq!(
            sender
                .query_transfer_status_at_endpoint(
                    receiver_endpoint,
                    &receiver_device.device_id,
                    &outgoing.id,
                    Utc::now().timestamp_millis() + STATUS_RECONCILIATION_BUDGET_MS,
                )
                .unwrap(),
            StatusResponseState::Pending
        );
        assert_eq!(
            sender
                .query_transfer_status_at_endpoint(
                    receiver_endpoint,
                    &receiver_device.device_id,
                    &uuid::Uuid::new_v4().to_string(),
                    Utc::now().timestamp_millis() + STATUS_RECONCILIATION_BUDGET_MS,
                )
                .unwrap(),
            StatusResponseState::Unknown
        );

        sender_db
            .with_local_link_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER fail_first_reconciled_terminal
                     BEFORE UPDATE OF status ON local_link_transfers
                     WHEN NEW.status = 'copied'
                     BEGIN
                         SELECT RAISE(ABORT, 'synthetic reconciled terminal failure');
                     END;",
                )?;
                Ok(())
            })
            .unwrap();
        let recovery_database = Arc::clone(&sender_db);
        let allow_retry = thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            recovery_database
                .with_local_link_connection(|connection| {
                    connection.execute_batch("DROP TRIGGER fail_first_reconciled_terminal;")?;
                    Ok(())
                })
                .unwrap();
        });
        receiver.drop_next_terminal_receipt_for_test();
        let writes = AtomicUsize::new(0);
        receiver
            .accept_transfer(&outgoing.id, LocalLinkReceiveAction::Copy, |_, _| {
                writes.fetch_add(1, Ordering::SeqCst);
                Ok(ClipboardWriteEvidence { change_count: 7 })
            })
            .unwrap();
        wait_for(
            || {
                sender.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id && transfer.status == LocalLinkTransferStatus::Copied
                })
            },
            "metadata-only status query did not retry after the dropped receipt and DB failure",
        );
        allow_retry.join().unwrap();
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert!(!serde_json::to_string(&sender.list_transfers(20).unwrap())
            .unwrap()
            .contains("receipt loss must never replay this body"));
    }

    #[test]
    fn direct_tcp_post_payload_cancel_and_dropped_receipt_reconcile_without_body_replay() {
        let sender_db = database();
        let receiver_db = database();
        let sender = service(Arc::clone(&sender_db));
        let receiver = service(Arc::clone(&receiver_db));
        let _sender_endpoint = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();
        let receiver_device =
            pair_services_for_direct_transfer(&sender, &receiver, receiver_endpoint);

        let item = sender_db
            .capture_text("cross-device cancel body sentinel", None)
            .unwrap()
            .unwrap();
        let outgoing = sender
            .send_clipboard_item_to_endpoint_for_test(
                &item.id,
                &receiver_device.device_id,
                receiver_endpoint,
            )
            .unwrap();
        wait_for(
            || {
                receiver.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.direction == LocalLinkTransferDirection::Incoming
                        && transfer.status == LocalLinkTransferStatus::AwaitingReceiver
                })
            },
            "receiver did not receive encrypted payload before cancellation",
        );
        let incoming = receiver
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.direction == LocalLinkTransferDirection::Incoming)
            .unwrap();

        receiver.drop_next_terminal_receipt_for_test();
        assert_eq!(
            sender.cancel_transfer(&outgoing.id).unwrap().status,
            LocalLinkTransferStatus::Reconciling
        );
        // The receiver's authenticated Cancelled receipt, not the local click,
        // makes a post-payload cancellation terminal.
        assert!(matches!(
            sender.cancel_transfer(&outgoing.id).unwrap().status,
            LocalLinkTransferStatus::Reconciling | LocalLinkTransferStatus::Cancelled
        ));
        wait_for(
            || {
                receiver.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == incoming.id
                        && transfer.status == LocalLinkTransferStatus::Cancelled
                })
            },
            "receiver did not converge to cancelled",
        );
        wait_for(
            || {
                sender.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id
                        && transfer.status == LocalLinkTransferStatus::Cancelled
                })
            },
            "sender did not retain cancelled terminal summary",
        );
        assert!(!receiver
            .runtime
            .lock()
            .unwrap()
            .pending_inbound
            .contains_key(&incoming.id));
        assert!(receiver.reveal_transfer(&incoming.id, |_| Ok(())).is_err());
        assert!(!serde_json::to_string(
            &receiver
                .list_transfers(20)
                .unwrap()
                .into_iter()
                .find(|transfer| transfer.id == incoming.id)
                .unwrap(),
        )
        .unwrap()
        .contains("cross-device cancel body sentinel"));
    }

    #[test]
    fn direct_tcp_receiver_decision_before_cancel_wins_on_both_devices() {
        let sender_db = database();
        let receiver_db = database();
        let sender = service(Arc::clone(&sender_db));
        let receiver = service(Arc::clone(&receiver_db));
        let _sender_endpoint = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();
        let receiver_device =
            pair_services_for_direct_transfer(&sender, &receiver, receiver_endpoint);

        let item = sender_db
            .capture_text("receiver decision wins", None)
            .unwrap()
            .unwrap();
        let outgoing = sender
            .send_clipboard_item_to_endpoint_for_test(
                &item.id,
                &receiver_device.device_id,
                receiver_endpoint,
            )
            .unwrap();
        wait_for(
            || {
                receiver.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.direction == LocalLinkTransferDirection::Incoming
                        && transfer.status == LocalLinkTransferStatus::AwaitingReceiver
                })
            },
            "receiver did not receive payload before deciding",
        );
        let incoming = receiver
            .list_transfers(20)
            .unwrap()
            .into_iter()
            .find(|transfer| transfer.direction == LocalLinkTransferDirection::Incoming)
            .unwrap();
        receiver
            .accept_transfer(&incoming.id, LocalLinkReceiveAction::Copy, |_, _| {
                Ok(ClipboardWriteEvidence { change_count: 1 })
            })
            .unwrap();

        // The sender may have received the receipt already (then Cancel is
        // rejected as already final) or briefly record a provisional cancel.
        // In both cases the receiver's earlier authenticated decision wins.
        let _ = sender.cancel_transfer(&outgoing.id);
        wait_for(
            || {
                sender.list_transfers(20).unwrap().iter().any(|transfer| {
                    transfer.id == outgoing.id
                        && transfer.status == LocalLinkTransferStatus::Copied
                        && transfer.receiver_action == Some(LocalLinkReceiveAction::Copy)
                })
            },
            "sender did not converge to the receiver's earlier decision",
        );
        assert_eq!(
            receiver
                .list_transfers(20)
                .unwrap()
                .into_iter()
                .find(|transfer| transfer.id == incoming.id)
                .unwrap()
                .status,
            LocalLinkTransferStatus::Copied
        );
    }

    #[test]
    fn session_only_pairing_keeps_key_in_memory_and_forgets_it_when_disabled() {
        let sender_db = database();
        let receiver_db = database();
        let sender = service(Arc::clone(&sender_db));
        let receiver = service(Arc::clone(&receiver_db));
        let _ = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();

        let receiver_device = sender.begin_pairing_at_for_test(receiver_endpoint).unwrap();
        wait_for(
            || {
                receiver
                    .list_devices()
                    .unwrap()
                    .iter()
                    .any(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            },
            "receiver did not create pairing candidate",
        );
        let sender_device = receiver
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.trust_status == LocalLinkDeviceTrustStatus::Pairing)
            .unwrap();

        sender
            .confirm_pairing(
                &receiver_device.device_id,
                LocalLinkTrustDuration::ThisSession,
            )
            .unwrap();
        receiver
            .confirm_pairing(
                &sender_device.device_id,
                LocalLinkTrustDuration::ThisSession,
            )
            .unwrap();
        wait_for(
            || {
                sender.list_devices().unwrap().iter().any(|device| {
                    device.device_id == receiver_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                        && device.trust_duration == LocalLinkTrustDuration::ThisSession
                }) && receiver.list_devices().unwrap().iter().any(|device| {
                    device.device_id == sender_device.device_id
                        && device.trust_status == LocalLinkDeviceTrustStatus::Trusted
                        && device.trust_duration == LocalLinkTrustDuration::ThisSession
                })
            },
            "bilateral session-only pairing did not become trusted",
        );

        let receiver_identity = receiver.ensure_identity().unwrap();
        sender
            .authenticate_peer(&receiver_device.device_id, receiver_identity.public_key())
            .unwrap();
        let persisted: (String, i64, String) = sender_db
            .with_local_link_connection(|connection| {
                connection
                    .query_row(
                        "SELECT trust_status, length(peer_public_key), trust_duration
                         FROM local_link_devices WHERE device_id = ?1",
                        [&receiver_device.device_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(
            persisted,
            ("needsRePairing".to_owned(), 0, "thirtyDays".to_owned())
        );

        let preferences = sender.preferences().unwrap();
        sender
            .update_preferences(LocalLinkPreferences {
                enabled: false,
                discovery_enabled: false,
                device_name: preferences.device_name,
            })
            .unwrap();
        let device = sender
            .list_devices()
            .unwrap()
            .into_iter()
            .find(|device| device.device_id == receiver_device.device_id)
            .unwrap();
        assert_eq!(
            device.trust_status,
            LocalLinkDeviceTrustStatus::NeedsRePairing
        );
        assert_eq!(device.trust_duration, LocalLinkTrustDuration::ThirtyDays);
        assert!(sender
            .authenticate_peer(&receiver_device.device_id, receiver_identity.public_key())
            .is_err());
    }

    #[test]
    fn pairing_does_not_trust_before_both_native_confirmations() {
        let sender = service(database());
        let receiver = service(database());
        let _ = sender.start_transport_for_test(0).unwrap();
        let receiver_endpoint = receiver.start_transport_for_test(0).unwrap();
        let candidate = sender.begin_pairing_at_for_test(receiver_endpoint).unwrap();
        wait_for(
            || sender.pairing_code_for_test(&candidate.device_id).is_some(),
            "sender did not create a pairing session",
        );
        sender
            .confirm_pairing(&candidate.device_id, LocalLinkTrustDuration::ThirtyDays)
            .unwrap();
        thread::sleep(Duration::from_millis(75));
        assert!(sender
            .list_devices()
            .unwrap()
            .iter()
            .all(|device| device.trust_status != LocalLinkDeviceTrustStatus::Trusted));
        sender.cancel_pairing(&candidate.device_id).unwrap();
    }
}
