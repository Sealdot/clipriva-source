use serde::{Deserialize, Serialize};

use crate::media::StoredBlob;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardOccurrence {
    pub id: String,
    pub source_app: Option<String>,
    pub device_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    /// Stable per-record ID for a future separately-designed sync protocol.
    pub global_id: String,
    /// Local installation identity, never an account or remote device lookup.
    pub device_id: String,
    /// Opaque exact-duplicate group ID. The content hash is not exposed to UI.
    pub group_id: Option<String>,
    pub version: u32,
    pub content: String,
    pub kind: String,
    pub source_app: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub retention_until: String,
    pub is_pinned: bool,
    pub copy_count: u32,
    pub restored_at: Option<String>,
    pub last_used_at: Option<String>,
    pub occurrence_count: u32,
    pub occurrences: Vec<ClipboardOccurrence>,
    pub representations: Vec<StoredBlob>,
    /// User-authored labels stored only in ClipRiva's local database. Labels
    /// are never included in Local Link payloads or diagnostics.
    pub tags: Vec<String>,
    /// True only when the current History query matched manually extracted
    /// image text. The extraction itself stays behind its dedicated command.
    pub text_in_image_match: bool,
    /// True only when the current local search matched an Item-owned Note.
    /// Note plaintext is available only through the dedicated Inspector API.
    pub note_match: bool,
    /// Local search metadata such as file basenames. It is never serialized,
    /// so full paths and internal ranking material cannot cross the command
    /// boundary accidentally.
    #[serde(skip)]
    pub(crate) search_text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardImageTextExtraction {
    pub clipboard_item_id: String,
    pub text: String,
    pub extracted_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardImageTextExtractionStatus {
    Extracted,
    NoText,
    SensitiveBlocked,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardImageTextExtractionResult {
    pub status: ClipboardImageTextExtractionStatus,
    pub extraction: Option<ClipboardImageTextExtraction>,
}

/// Structured local History filters. Every variant is finite and every
/// free-form value is bound as a SQLite parameter by the repository.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardPinFilter {
    #[default]
    All,
    Pinned,
    Unpinned,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardTimeFilter {
    #[default]
    All,
    Past24Hours,
    Past7Days,
    Past30Days,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardListFilters {
    #[serde(default)]
    pub kinds: Vec<String>,
    pub source_app: Option<String>,
    /// Collections deliberately reuse the bounded, local Item-tag storage
    /// introduced in v1.5. No collection name crosses the local IPC boundary
    /// except in response to this explicit History query.
    pub collection: Option<String>,
    /// A persisted, allow-listed rule loaded and evaluated by the repository.
    /// Clients never supply executable rule text with a History query.
    pub smart_collection_id: Option<String>,
    #[serde(default)]
    pub pin_filter: ClipboardPinFilter,
    #[serde(default)]
    pub time_filter: ClipboardTimeFilter,
    #[serde(default)]
    pub recently_used_only: bool,
    #[serde(default)]
    pub local_link_only: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardCollectionKind {
    Manual,
    Smart,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSmartCollectionRule {
    #[serde(default)]
    pub kinds: Vec<String>,
    pub source_app: Option<String>,
    #[serde(default)]
    pub pin_filter: ClipboardPinFilter,
    #[serde(default)]
    pub time_filter: ClipboardTimeFilter,
    #[serde(default)]
    pub recently_used_only: bool,
    #[serde(default)]
    pub local_link_only: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSmartCollectionInput {
    pub name: String,
    pub rule: ClipboardSmartCollectionRule,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCollection {
    pub id: Option<String>,
    pub name: String,
    pub kind: ClipboardCollectionKind,
    pub rule: Option<ClipboardSmartCollectionRule>,
    pub item_count: u32,
    pub saved_item_count: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFilterOptions {
    pub source_apps: Vec<String>,
    pub collections: Vec<ClipboardCollection>,
    pub total_count: u32,
    pub unpinned_count: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCollectionMutationResult {
    pub affected_item_count: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItemNote {
    pub clipboard_item_id: String,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardItemNoteMutationStatus {
    Saved,
    Deleted,
    SensitiveBlocked,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItemNoteMutationResult {
    pub status: ClipboardItemNoteMutationStatus,
    pub note: Option<ClipboardItemNote>,
}

/// The cleanup target is deliberately explicit so a user-visible preview can
/// be computed from exactly the same request that will later move items to the
/// local recycle bin.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardCleanupScope {
    Selected,
    AllUnpinned,
    RetentionPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCleanupRequest {
    pub scope: ClipboardCleanupScope,
    #[serde(default)]
    pub item_ids: Vec<String>,
    /// `true` is an explicit acknowledgement from the confirmation UI that a
    /// manually selected pinned item may also be moved to the recycle bin.
    #[serde(default)]
    pub include_pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCleanupRepresentative {
    pub id: String,
    pub kind: String,
    pub source_app: Option<String>,
    pub captured_at: String,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinRetention {
    pub maximum_days: u8,
    pub earliest_expires_at: Option<String>,
    pub latest_expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCleanupPreview {
    pub scope: ClipboardCleanupScope,
    pub affected_count: u32,
    pub pinned_skipped_count: u32,
    pub rule_conditions: Vec<String>,
    /// Representatives intentionally omit clipboard content, hashes and
    /// previews so the confirmation flow cannot become a sensitive-data
    /// disclosure surface.
    pub representative_items: Vec<ClipboardCleanupRepresentative>,
    pub recycle_bin_retention: RecycleBinRetention,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardCleanupResult {
    pub preview: ClipboardCleanupPreview,
    pub moved_to_recycle_bin_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinItem {
    pub item: ClipboardItem,
    pub deleted_at: String,
    pub recycle_expires_at: String,
}

/// A privacy-preserving explanation of why a recent clipboard event was not
/// captured. This deliberately contains only the reason, timestamp and source
/// application metadata—never the clipboard contents or a content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureStatusReason {
    ManualPause,
    SensitiveContentDefault,
    SensitiveContentStrict,
    ExcludedApplication,
    UnsupportedFormat,
    ContentTooLarge,
}

impl CaptureStatusReason {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::ManualPause => "manualPause",
            Self::SensitiveContentDefault => "sensitiveContentDefault",
            Self::SensitiveContentStrict => "sensitiveContentStrict",
            Self::ExcludedApplication => "excludedApplication",
            Self::UnsupportedFormat => "unsupportedFormat",
            Self::ContentTooLarge => "contentTooLarge",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "manualPause" => Some(Self::ManualPause),
            "sensitiveContentDefault" => Some(Self::SensitiveContentDefault),
            "sensitiveContentStrict" => Some(Self::SensitiveContentStrict),
            "excludedApplication" => Some(Self::ExcludedApplication),
            "unsupportedFormat" => Some(Self::UnsupportedFormat),
            "contentTooLarge" => Some(Self::ContentTooLarge),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatusEvent {
    pub id: String,
    pub reason_type: CaptureStatusReason,
    pub source_app: Option<String>,
    pub occurred_at: String,
    pub expires_at: String,
}

/// The deliberately small allow-list of local experience signals. These
/// categories are designed for product reliability analysis without exposing
/// clipboard contents, file metadata, application document names, paths, or
/// any network identifier.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum LocalDiagnosticEventType {
    FirstRecovery,
    DirectPaste,
    RecycleBinRestore,
    DuplicateGroupExpand,
    ClipboardRestoreError,
    CapturePermissionRestricted,
    SystemPolicyRestricted,
}

impl LocalDiagnosticEventType {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::FirstRecovery => "firstRecovery",
            Self::DirectPaste => "directPaste",
            Self::RecycleBinRestore => "recycleBinRestore",
            Self::DuplicateGroupExpand => "duplicateGroupExpand",
            Self::ClipboardRestoreError => "clipboardRestoreError",
            Self::CapturePermissionRestricted => "capturePermissionRestricted",
            Self::SystemPolicyRestricted => "systemPolicyRestricted",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "firstRecovery" => Some(Self::FirstRecovery),
            "directPaste" => Some(Self::DirectPaste),
            "recycleBinRestore" => Some(Self::RecycleBinRestore),
            "duplicateGroupExpand" => Some(Self::DuplicateGroupExpand),
            "clipboardRestoreError" => Some(Self::ClipboardRestoreError),
            "capturePermissionRestricted" => Some(Self::CapturePermissionRestricted),
            "systemPolicyRestricted" => Some(Self::SystemPolicyRestricted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum LocalDiagnosticOutcome {
    Started,
    Completed,
    Degraded,
    Failed,
    Restricted,
    Restored,
    Expanded,
}

impl LocalDiagnosticOutcome {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
            Self::Restricted => "restricted",
            Self::Restored => "restored",
            Self::Expanded => "expanded",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "started" => Some(Self::Started),
            "completed" => Some(Self::Completed),
            "degraded" => Some(Self::Degraded),
            "failed" => Some(Self::Failed),
            "restricted" => Some(Self::Restricted),
            "restored" => Some(Self::Restored),
            "expanded" => Some(Self::Expanded),
            _ => None,
        }
    }
}

/// The command boundary accepts no free-form metadata. In particular, callers
/// cannot attach a clip ID, text, preview, source app, path, filename, URL, or
/// timestamp; the local runtime supplies the coarse day and app version.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticEventInput {
    pub event_type: LocalDiagnosticEventType,
    pub outcome: LocalDiagnosticOutcome,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticRecordResult {
    pub recorded: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticRate {
    pub attempts: u32,
    pub completed_or_degraded: u32,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticErrorCategory {
    pub event_type: LocalDiagnosticEventType,
    pub outcome: LocalDiagnosticOutcome,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticsSummary {
    pub enabled: bool,
    pub stored_event_count: u32,
    pub first_recovery: LocalDiagnosticRate,
    pub direct_paste: LocalDiagnosticRate,
    pub recycle_bin_restore_count: u32,
    pub duplicate_group_expand_count: u32,
    pub critical_error_categories: Vec<LocalDiagnosticErrorCategory>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticsExportEvent {
    pub event_type: LocalDiagnosticEventType,
    pub outcome: LocalDiagnosticOutcome,
    /// Date-only, UTC-derived bucket. No precise event time is exported.
    pub occurred_day: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalDiagnosticsExport {
    pub schema_version: u32,
    pub exported_at_day: String,
    pub app_version: String,
    pub summary: LocalDiagnosticsSummary,
    pub events: Vec<LocalDiagnosticsExportEvent>,
}

/// Settings for the explicit, device-to-device Local Link experiment. The
/// feature is disabled by default and has no account or cloud identity.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkPreferences {
    pub enabled: bool,
    /// Discovery is separately opt-in. It remains false even when the Local
    /// Link experiment is enabled until the user turns discovery on.
    pub discovery_enabled: bool,
    pub device_name: String,
}

/// Ordered native checks used to answer whether Local Link can start an
/// explicit send. These values intentionally describe only finite control
/// state; they never carry an endpoint, raw permission error or peer label.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReadinessStage {
    LocalNetwork,
    Enabled,
    Discovery,
    Trust,
    AuthenticatedPresence,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReadinessStageState {
    Passed,
    Blocked,
    NotChecked,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReadinessState {
    Ready,
    ActionRequired,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReadinessBlocker {
    LocalNetworkUnavailable,
    LocalLinkDisabled,
    DiscoveryDisabled,
    PairingIncomplete,
    TrustRequiresRePairing,
    NoTrustedDevice,
    NoAuthenticatedPresence,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReadinessAction {
    RestartLocalLink,
    EnableLocalLink,
    EnableDiscovery,
    CompletePairing,
    PairDevice,
    RePairDevice,
    RefreshDevices,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkReadinessCheck {
    pub stage: LocalLinkReadinessStage,
    pub state: LocalLinkReadinessStageState,
}

/// A read-only, content-free readiness projection. `action_device_id` is the
/// same opaque command identifier already exposed by `LocalLinkDevice`; it is
/// present only when the one safe recovery action targets an existing peer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkReadinessSnapshot {
    pub state: LocalLinkReadinessState,
    pub stages: Vec<LocalLinkReadinessCheck>,
    pub first_blocker: Option<LocalLinkReadinessBlocker>,
    pub primary_action: Option<LocalLinkReadinessAction>,
    pub action_device_id: Option<String>,
    pub ready_device_count: u32,
    pub pending_request_capacity: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkDeviceTrustStatus {
    Pairing,
    Trusted,
    NeedsRePairing,
    Revoked,
}

impl LocalLinkDeviceTrustStatus {
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "pairing" => Some(Self::Pairing),
            "trusted" => Some(Self::Trusted),
            "needsRePairing" => Some(Self::NeedsRePairing),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

/// Why a device that was previously paired now needs a new native pairing
/// ceremony. This is deliberately an explicit enum rather than a UI-derived
/// guess, so a client can present a safe recovery path without inspecting a
/// key, endpoint, or clipboard payload.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkDeviceRePairingReason {
    TrustExpired,
    IdentityInvalidated,
}

/// The locally chosen lifetime for a verified peer identity. The pairing code
/// remains mandatory for every option; this selection never crosses the
/// network or acts as a trust signal for the other Mac.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkTrustDuration {
    ThisSession,
    #[default]
    ThirtyDays,
    Always,
}

impl LocalLinkTrustDuration {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::ThisSession => "thisSession",
            Self::ThirtyDays => "thirtyDays",
            Self::Always => "always",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "thisSession" => Some(Self::ThisSession),
            "thirtyDays" => Some(Self::ThirtyDays),
            "always" => Some(Self::Always),
            _ => None,
        }
    }
}

/// A locally stored pairing record. It intentionally contains no network
/// address, account identifier, discovery token or clipboard content.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkDevice {
    pub device_id: String,
    pub display_name: String,
    /// SHA-256 fingerprint of the peer's authenticated Noise static key.
    /// It remains empty for anonymous discovery candidates and never accepts
    /// a WebView-supplied value as a trust anchor.
    pub public_key_fingerprint: String,
    pub trust_status: LocalLinkDeviceTrustStatus,
    pub paired_at: String,
    pub trusted_at: Option<String>,
    /// The native-enforced lifetime selected on this Mac after SAS
    /// confirmation. `thisSession` is runtime-only and is never persisted as
    /// usable peer key material.
    pub trust_duration: LocalLinkTrustDuration,
    /// Derived from the last verified pairing. It is metadata only and never
    /// changes the 30-day trust policy through a WebView request.
    pub trust_expires_at: Option<String>,
    /// Present only while `trust_status` is `needsRePairing` so the recovery
    /// UI can distinguish an elapsed trust period from an invalidated identity.
    pub re_pairing_reason: Option<LocalLinkDeviceRePairingReason>,
    pub revoked_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_transfer_at: Option<String>,
    /// Reachability is runtime-only and is never persisted as an endpoint.
    pub online: bool,
    pub protocol_min: u16,
    pub protocol_max: u16,
}

/// Pairing details for a currently active, encrypted native session. The SAS
/// and fingerprints are short-lived comparison values and are never persisted.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkPairingDetails {
    pub device: LocalLinkDevice,
    pub verification_code: String,
    pub expires_at: String,
    pub local_fingerprint: String,
    pub peer_fingerprint: String,
    pub local_confirmed: bool,
    pub peer_confirmed: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BeginLocalLinkPairingRequest {
    /// Opaque peer-generated identifier. It is not an account identifier and
    /// is never resolved through a network service.
    pub device_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmLocalLinkPairingRequest {
    pub device_id: String,
    /// Older callers safely retain the default instead of gaining indefinite
    /// trust through an omitted request field.
    #[serde(default)]
    pub trust_duration: LocalLinkTrustDuration,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkTransferDirection {
    Outgoing,
    Incoming,
}

impl LocalLinkTransferDirection {
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "outgoing" => Some(Self::Outgoing),
            "incoming" => Some(Self::Incoming),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkTransferStatus {
    Connecting,
    Encrypted,
    AwaitingReceiver,
    /// The receiving person opened the pending request. This is metadata only:
    /// it never includes the transferred body, preview, or action details.
    Viewed,
    /// The payload may already have reached the authenticated receiver, but
    /// the terminal receipt was lost. Only metadata-only status queries are
    /// allowed from this state; the payload is never retained or replayed.
    Reconciling,
    Copied,
    Saved,
    Rejected,
    Cancelled,
    Expired,
    Failed,
}

/// The receiver decides exactly one destination for an in-memory Local Link
/// payload. This value is metadata only: it records the user's decision, never
/// the payload, preview, path, or clipboard contents.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkReceiveAction {
    Copy,
    Save,
    Reject,
}

impl LocalLinkReceiveAction {
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "copy" => Some(Self::Copy),
            "save" => Some(Self::Save),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }
}

impl LocalLinkTransferStatus {
    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "connecting" => Some(Self::Connecting),
            "encrypted" => Some(Self::Encrypted),
            "awaitingReceiver" => Some(Self::AwaitingReceiver),
            "viewed" => Some(Self::Viewed),
            "reconciling" => Some(Self::Reconciling),
            "copied" => Some(Self::Copied),
            "saved" => Some(Self::Saved),
            "rejected" => Some(Self::Rejected),
            "cancelled" => Some(Self::Cancelled),
            "expired" => Some(Self::Expired),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Fixed, content-free reasons that can safely cross the IPC boundary. Error
/// messages and transfer records must not include the transferred plaintext.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkTransferFailure {
    TransportUnavailable,
    DeviceUnavailable,
    AuthFailed,
    VersionMismatch,
    ReceiverLocked,
    ReceiverBusy,
    RateLimited,
    Duplicate,
    ProtocolViolation,
    CaptureBlocked,
    ItemUnavailable,
    UnsupportedItem,
    DeviceNotTrusted,
    DeviceRevoked,
    PayloadUnavailable,
    /// The implementation deliberately refused to repeat an external side
    /// effect or plaintext delivery after a crash/connection ambiguity.
    OutcomeUnknown,
}

/// A content-free next step for a terminal transfer. Local Link deliberately
/// does not retain payloads or item references for replay, so this enum never
/// promises an automatic retry that the native service cannot safely perform.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkTransferRecoveryAction {
    StartNewTransferFromOriginalItem,
    RePairDevice,
    UpdateClipRiva,
    ReviewLocalCapturePolicy,
    SelectSupportedItem,
    CheckTransferHistory,
}

impl LocalLinkTransferFailure {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::TransportUnavailable => "transportUnavailable",
            Self::DeviceUnavailable => "deviceUnavailable",
            Self::AuthFailed => "authFailed",
            Self::VersionMismatch => "versionMismatch",
            Self::ReceiverLocked => "receiverLocked",
            Self::ReceiverBusy => "receiverBusy",
            Self::RateLimited => "rateLimited",
            Self::Duplicate => "duplicate",
            Self::ProtocolViolation => "protocolViolation",
            Self::CaptureBlocked => "captureBlocked",
            Self::ItemUnavailable => "itemUnavailable",
            Self::UnsupportedItem => "unsupportedItem",
            Self::DeviceNotTrusted => "deviceNotTrusted",
            Self::DeviceRevoked => "deviceRevoked",
            Self::PayloadUnavailable => "payloadUnavailable",
            Self::OutcomeUnknown => "outcomeUnknown",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "transportUnavailable" => Some(Self::TransportUnavailable),
            "deviceUnavailable" => Some(Self::DeviceUnavailable),
            "authFailed" => Some(Self::AuthFailed),
            "versionMismatch" => Some(Self::VersionMismatch),
            "receiverLocked" => Some(Self::ReceiverLocked),
            "receiverBusy" => Some(Self::ReceiverBusy),
            "rateLimited" => Some(Self::RateLimited),
            "duplicate" => Some(Self::Duplicate),
            "protocolViolation" => Some(Self::ProtocolViolation),
            "captureBlocked" => Some(Self::CaptureBlocked),
            "itemUnavailable" => Some(Self::ItemUnavailable),
            "unsupportedItem" => Some(Self::UnsupportedItem),
            "deviceNotTrusted" => Some(Self::DeviceNotTrusted),
            "deviceRevoked" => Some(Self::DeviceRevoked),
            "payloadUnavailable" => Some(Self::PayloadUnavailable),
            "outcomeUnknown" => Some(Self::OutcomeUnknown),
            _ => None,
        }
    }
}

/// A metadata-only transfer view. Pending incoming payloads remain in
/// process-memory only until accepted or rejected and are never serialized to
/// the UI, written to logs, or persisted to disk.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkTransfer {
    pub id: String,
    pub device_id: String,
    pub peer_display_name: String,
    pub direction: LocalLinkTransferDirection,
    pub status: LocalLinkTransferStatus,
    pub item_kind: String,
    pub byte_size: u32,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub expires_at: Option<String>,
    pub failure_reason: Option<LocalLinkTransferFailure>,
    /// A deterministic recovery hint derived only from terminal metadata. It
    /// never exposes a payload and is intentionally not a replay capability.
    pub recovery_action: Option<LocalLinkTransferRecoveryAction>,
    /// The completed receiver action, if any. It is deliberately content-free
    /// so Transfers can explain the result without becoming another history.
    pub receiver_action: Option<LocalLinkReceiveAction>,
}

impl LocalLinkTransfer {
    pub fn recovery_action(&self) -> Option<LocalLinkTransferRecoveryAction> {
        match self.status {
            LocalLinkTransferStatus::Rejected
            | LocalLinkTransferStatus::Cancelled
            | LocalLinkTransferStatus::Expired => {
                Some(LocalLinkTransferRecoveryAction::StartNewTransferFromOriginalItem)
            }
            LocalLinkTransferStatus::Failed => match self.failure_reason {
                Some(
                    LocalLinkTransferFailure::TransportUnavailable
                    | LocalLinkTransferFailure::DeviceUnavailable
                    | LocalLinkTransferFailure::ReceiverLocked
                    | LocalLinkTransferFailure::ReceiverBusy
                    | LocalLinkTransferFailure::RateLimited
                    | LocalLinkTransferFailure::PayloadUnavailable
                    | LocalLinkTransferFailure::OutcomeUnknown,
                ) => Some(LocalLinkTransferRecoveryAction::StartNewTransferFromOriginalItem),
                Some(
                    LocalLinkTransferFailure::AuthFailed
                    | LocalLinkTransferFailure::DeviceNotTrusted
                    | LocalLinkTransferFailure::DeviceRevoked
                    | LocalLinkTransferFailure::ProtocolViolation,
                ) => Some(LocalLinkTransferRecoveryAction::RePairDevice),
                Some(LocalLinkTransferFailure::VersionMismatch) => {
                    Some(LocalLinkTransferRecoveryAction::UpdateClipRiva)
                }
                Some(LocalLinkTransferFailure::CaptureBlocked) => {
                    Some(LocalLinkTransferRecoveryAction::ReviewLocalCapturePolicy)
                }
                Some(LocalLinkTransferFailure::ItemUnavailable)
                | Some(LocalLinkTransferFailure::UnsupportedItem) => {
                    Some(LocalLinkTransferRecoveryAction::SelectSupportedItem)
                }
                Some(LocalLinkTransferFailure::Duplicate) => {
                    Some(LocalLinkTransferRecoveryAction::CheckTransferHistory)
                }
                None => Some(LocalLinkTransferRecoveryAction::StartNewTransferFromOriginalItem),
            },
            LocalLinkTransferStatus::Connecting
            | LocalLinkTransferStatus::Encrypted
            | LocalLinkTransferStatus::AwaitingReceiver
            | LocalLinkTransferStatus::Viewed
            | LocalLinkTransferStatus::Reconciling
            | LocalLinkTransferStatus::Copied
            | LocalLinkTransferStatus::Saved => None,
        }
    }
}

/// Internal-only plaintext used while a Local Link transfer is being handed to
/// a transport. This type is deliberately not serializable and must never be
/// included in an IPC response or diagnostic output.
#[derive(Clone)]
pub(crate) struct LocalLinkClipboardPayload {
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkDiagnosticOsFamily {
    Macos,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum LocalLinkDiagnosticArchitecture {
    #[serde(rename = "arm64")]
    Arm64,
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "other")]
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocalLinkDiagnosticAvailability {
    Online,
    Offline,
}

/// One device projection inside a user-requested diagnostics bundle. The
/// per-export salted reference is intentionally not stable across exports.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkDiagnosticPeer {
    pub peer_ref: String,
    pub trust_status: LocalLinkDeviceTrustStatus,
    pub trust_duration: LocalLinkTrustDuration,
    pub availability: LocalLinkDiagnosticAvailability,
}

/// Content-free transfer metadata used for local support. It deliberately has
/// no transfer ID, byte count, receiver label, payload digest or item pointer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkDiagnosticTransfer {
    pub peer_ref: String,
    pub direction: LocalLinkTransferDirection,
    pub status: LocalLinkTransferStatus,
    pub failure_reason: Option<LocalLinkTransferFailure>,
    pub recovery_action: Option<LocalLinkTransferRecoveryAction>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Built on demand and returned only to the requesting UI for preview/export.
/// No copy of this bundle is written to SQLite, a file, a log or a network.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalLinkDiagnostics {
    pub schema_version: u32,
    pub app_version: String,
    pub os_family: LocalLinkDiagnosticOsFamily,
    pub architecture: LocalLinkDiagnosticArchitecture,
    pub generated_at: String,
    pub local_link_enabled: bool,
    pub discovery_enabled: bool,
    pub readiness: LocalLinkReadinessSnapshot,
    pub record_count: u32,
    pub peers: Vec<LocalLinkDiagnosticPeer>,
    pub transfers: Vec<LocalLinkDiagnosticTransfer>,
}
