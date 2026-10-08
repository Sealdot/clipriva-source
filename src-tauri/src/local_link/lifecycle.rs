//! Platform-neutral lifecycle policy for the Local Link transport.
//!
//! This module deliberately does not own sockets, discovery, pending plaintext,
//! or timers. It is a deterministic reducer: the embedding service feeds it
//! lifecycle/transport events and executes the returned actions. Every delayed
//! action and transport worker is tagged with a generation so an explicit
//! disable, sleep, session switch, or path loss invalidates stale callbacks.

use std::sync::Arc;
use std::time::Duration;

pub(crate) const RESTART_DEBOUNCE: Duration = Duration::from_millis(500);
pub(crate) const RESTART_RETRY_DELAYS: [Duration; 4] = [
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
];

/// The native path adapter may initially be unavailable. `Unknown` permits an
/// optimistic first bind; a reported `Unsatisfied` path always stops transport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NetworkPathState {
    #[default]
    Unknown,
    Satisfied,
    Unsatisfied,
}

/// Diagnostic health of the native network-path observer. This is separate
/// from `TransportHealth`: a listener may still be running when path-change
/// supervision is unavailable, and restarting that listener cannot repair the
/// missing native observer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NetworkMonitorState {
    #[default]
    Pending,
    Available,
    Degraded,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TransportHealth {
    #[default]
    Stopped,
    Recovering,
    Starting,
    Running,
    Degraded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecureSuspensionReason {
    Disabled,
    Sleep,
    SessionInactive,
    Termination,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleEvent {
    PreferenceChanged { enabled: bool },
    WillSleep,
    DidWake,
    SessionInactive,
    SessionActive,
    NetworkPathChanged(NetworkPathState),
    NetworkMonitorStarted,
    NetworkMonitorFailed,
    WillTerminate,
    RestartDebounceElapsed { generation: u64 },
    RetryElapsed { generation: u64, attempt: usize },
    TransportStarted { generation: u64 },
    TransportStartFailed { generation: u64 },
    TransportWorkerDied { generation: u64 },
}

/// Effects are intentionally content-free. The service maps
/// `SuspendSensitiveRuntime` to closing native previews, zeroizing pending
/// bodies, cancelling pairing/receipt waiters, and marking the receiver locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleAction {
    CancelScheduledWork {
        generation: u64,
    },
    SuspendSensitiveRuntime {
        reason: SecureSuspensionReason,
    },
    SetReceiverAvailable(bool),
    StopTransport {
        generation: u64,
    },
    ScheduleRestartDebounce {
        generation: u64,
        delay: Duration,
    },
    StartTransport {
        generation: u64,
    },
    ScheduleRetry {
        generation: u64,
        attempt: usize,
        delay: Duration,
    },
    MarkTransportDegraded {
        generation: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleSnapshot {
    pub enabled: bool,
    pub sleeping: bool,
    pub session_active: bool,
    pub terminating: bool,
    pub network_path: NetworkPathState,
    pub network_monitor_state: NetworkMonitorState,
    pub transport_health: TransportHealth,
    pub generation: u64,
    pub receiver_available: bool,
}

/// Deterministic lifecycle and restart controller.
///
/// The owner must serialize calls to `handle`. In production it is expected to
/// live behind the same mutex as transport ownership. Tests can drive it
/// directly without wall-clock sleeps or native notifications.
#[derive(Debug)]
pub(crate) struct LifecycleSupervisor {
    enabled: bool,
    sleeping: bool,
    session_active: bool,
    terminating: bool,
    network_path: NetworkPathState,
    network_monitor_state: NetworkMonitorState,
    transport_health: TransportHealth,
    generation: u64,
    pending_debounce: bool,
    pending_retry: Option<usize>,
    next_retry_attempt: usize,
}

impl Default for LifecycleSupervisor {
    fn default() -> Self {
        Self::new(false)
    }
}

impl LifecycleSupervisor {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            sleeping: false,
            session_active: true,
            terminating: false,
            network_path: NetworkPathState::Unknown,
            network_monitor_state: NetworkMonitorState::Pending,
            transport_health: TransportHealth::Stopped,
            generation: 1,
            pending_debounce: false,
            pending_retry: None,
            next_retry_attempt: 0,
        }
    }

    pub(crate) fn snapshot(&self) -> LifecycleSnapshot {
        LifecycleSnapshot {
            enabled: self.enabled,
            sleeping: self.sleeping,
            session_active: self.session_active,
            terminating: self.terminating,
            network_path: self.network_path,
            network_monitor_state: self.network_monitor_state,
            transport_health: self.transport_health,
            generation: self.generation,
            receiver_available: self.receiver_should_be_available(),
        }
    }

    pub(crate) fn handle(&mut self, event: LifecycleEvent) -> Vec<LifecycleAction> {
        match event {
            LifecycleEvent::PreferenceChanged { enabled } => {
                self.handle_preference_changed(enabled)
            }
            LifecycleEvent::WillSleep => {
                if self.sleeping {
                    return Vec::new();
                }
                self.sleeping = true;
                self.suspend(SecureSuspensionReason::Sleep)
            }
            LifecycleEvent::DidWake => {
                self.sleeping = false;
                self.resume_after_environment_change()
            }
            LifecycleEvent::SessionInactive => {
                if !self.session_active {
                    return Vec::new();
                }
                self.session_active = false;
                self.suspend(SecureSuspensionReason::SessionInactive)
            }
            LifecycleEvent::SessionActive => {
                self.session_active = true;
                self.resume_after_environment_change()
            }
            LifecycleEvent::NetworkPathChanged(path) => self.handle_network_path_changed(path),
            LifecycleEvent::NetworkMonitorStarted => {
                if self.network_monitor_state == NetworkMonitorState::Pending {
                    self.network_monitor_state = NetworkMonitorState::Available;
                }
                Vec::new()
            }
            LifecycleEvent::NetworkMonitorFailed => self.handle_network_monitor_failed(),
            LifecycleEvent::WillTerminate => {
                if self.terminating {
                    return Vec::new();
                }
                self.terminating = true;
                self.suspend(SecureSuspensionReason::Termination)
            }
            LifecycleEvent::RestartDebounceElapsed { generation } => {
                self.handle_debounce_elapsed(generation)
            }
            LifecycleEvent::RetryElapsed {
                generation,
                attempt,
            } => self.handle_retry_elapsed(generation, attempt),
            LifecycleEvent::TransportStarted { generation } => {
                self.handle_transport_started(generation)
            }
            LifecycleEvent::TransportStartFailed { generation } => {
                self.handle_transport_start_failed(generation)
            }
            LifecycleEvent::TransportWorkerDied { generation } => {
                self.handle_transport_worker_died(generation)
            }
        }
    }

    fn handle_preference_changed(&mut self, enabled: bool) -> Vec<LifecycleAction> {
        if enabled == self.enabled {
            if !enabled {
                return Vec::new();
            }
            if !matches!(
                self.transport_health,
                TransportHealth::Stopped | TransportHealth::Degraded
            ) {
                return vec![LifecycleAction::SetReceiverAvailable(
                    self.receiver_should_be_available(),
                )];
            }
        }
        self.enabled = enabled;
        if !enabled {
            return self.suspend(SecureSuspensionReason::Disabled);
        }

        if !self.can_run_transport() {
            return vec![LifecycleAction::SetReceiverAvailable(
                self.receiver_should_be_available(),
            )];
        }

        // Explicit opt-in starts immediately; environment-driven restarts are
        // debounced separately to coalesce wake and path notifications.
        self.begin_start_now()
    }

    fn handle_network_path_changed(&mut self, path: NetworkPathState) -> Vec<LifecycleAction> {
        if path == self.network_path {
            return Vec::new();
        }
        self.network_path = path;
        if path == NetworkPathState::Unsatisfied {
            return self.stop_for_unavailable_environment();
        }
        self.resume_after_environment_change()
    }

    fn handle_network_monitor_failed(&mut self) -> Vec<LifecycleAction> {
        if self.network_monitor_state == NetworkMonitorState::Degraded {
            return Vec::new();
        }
        self.network_monitor_state = NetworkMonitorState::Degraded;
        vec![LifecycleAction::MarkTransportDegraded {
            generation: self.generation,
        }]
    }

    fn handle_debounce_elapsed(&mut self, generation: u64) -> Vec<LifecycleAction> {
        if generation != self.generation || !self.pending_debounce || !self.can_run_transport() {
            return Vec::new();
        }
        self.pending_debounce = false;
        self.transport_health = TransportHealth::Starting;
        vec![LifecycleAction::StartTransport { generation }]
    }

    fn handle_retry_elapsed(&mut self, generation: u64, attempt: usize) -> Vec<LifecycleAction> {
        if generation != self.generation
            || self.pending_retry != Some(attempt)
            || !self.can_run_transport()
        {
            return Vec::new();
        }
        self.pending_retry = None;
        self.transport_health = TransportHealth::Starting;
        vec![LifecycleAction::StartTransport { generation }]
    }

    fn handle_transport_started(&mut self, generation: u64) -> Vec<LifecycleAction> {
        if generation != self.generation
            || self.transport_health != TransportHealth::Starting
            || !self.can_run_transport()
        {
            return Vec::new();
        }
        self.pending_debounce = false;
        self.pending_retry = None;
        self.next_retry_attempt = 0;
        self.transport_health = TransportHealth::Running;
        vec![LifecycleAction::SetReceiverAvailable(
            self.receiver_should_be_available(),
        )]
    }

    fn handle_transport_start_failed(&mut self, generation: u64) -> Vec<LifecycleAction> {
        if generation != self.generation
            || self.transport_health != TransportHealth::Starting
            || !self.can_run_transport()
        {
            return Vec::new();
        }
        self.schedule_next_retry()
    }

    fn handle_transport_worker_died(&mut self, generation: u64) -> Vec<LifecycleAction> {
        if generation != self.generation
            || !matches!(
                self.transport_health,
                TransportHealth::Starting | TransportHealth::Running
            )
            || !self.can_run_transport()
        {
            return Vec::new();
        }
        self.transport_health = TransportHealth::Recovering;
        self.pending_debounce = false;
        self.pending_retry = None;
        self.next_retry_attempt = 0;
        self.schedule_next_retry()
    }

    fn begin_start_now(&mut self) -> Vec<LifecycleAction> {
        let mut actions = self.begin_generation();
        self.transport_health = TransportHealth::Starting;
        actions.push(LifecycleAction::SetReceiverAvailable(true));
        actions.push(LifecycleAction::StartTransport {
            generation: self.generation,
        });
        actions
    }

    fn resume_after_environment_change(&mut self) -> Vec<LifecycleAction> {
        let mut actions = vec![LifecycleAction::SetReceiverAvailable(
            self.receiver_should_be_available(),
        )];
        if !self.can_run_transport() || self.transport_health == TransportHealth::Running {
            return actions;
        }

        actions.extend(self.begin_generation());
        self.pending_debounce = true;
        self.transport_health = TransportHealth::Recovering;
        actions.push(LifecycleAction::ScheduleRestartDebounce {
            generation: self.generation,
            delay: RESTART_DEBOUNCE,
        });
        actions
    }

    fn suspend(&mut self, reason: SecureSuspensionReason) -> Vec<LifecycleAction> {
        let mut actions = self.begin_generation();
        self.transport_health = TransportHealth::Stopped;
        actions.push(LifecycleAction::SuspendSensitiveRuntime { reason });
        actions.push(LifecycleAction::SetReceiverAvailable(false));
        actions.push(LifecycleAction::StopTransport {
            generation: self.generation,
        });
        actions
    }

    fn stop_for_unavailable_environment(&mut self) -> Vec<LifecycleAction> {
        let mut actions = self.begin_generation();
        self.transport_health = TransportHealth::Stopped;
        actions.push(LifecycleAction::StopTransport {
            generation: self.generation,
        });
        actions
    }

    fn begin_generation(&mut self) -> Vec<LifecycleAction> {
        let old_generation = self.generation;
        self.generation = self.generation.wrapping_add(1);
        self.pending_debounce = false;
        self.pending_retry = None;
        self.next_retry_attempt = 0;
        vec![LifecycleAction::CancelScheduledWork {
            generation: old_generation,
        }]
    }

    fn schedule_next_retry(&mut self) -> Vec<LifecycleAction> {
        let Some(delay) = RESTART_RETRY_DELAYS.get(self.next_retry_attempt).copied() else {
            self.pending_retry = None;
            self.transport_health = TransportHealth::Degraded;
            return vec![LifecycleAction::MarkTransportDegraded {
                generation: self.generation,
            }];
        };

        let attempt = self.next_retry_attempt;
        self.next_retry_attempt += 1;
        self.pending_retry = Some(attempt);
        self.transport_health = TransportHealth::Recovering;
        vec![LifecycleAction::ScheduleRetry {
            generation: self.generation,
            attempt,
            delay,
        }]
    }

    fn can_run_transport(&self) -> bool {
        self.enabled
            && !self.sleeping
            && self.session_active
            && !self.terminating
            && self.network_path != NetworkPathState::Unsatisfied
    }

    fn receiver_should_be_available(&self) -> bool {
        self.enabled && !self.sleeping && self.session_active && !self.terminating
    }
}

/// Callback type shared by native lifecycle and injectable network adapters.
pub(crate) type LifecycleEventSink = Arc<dyn Fn(LifecycleEvent) + Send + Sync + 'static>;

/// Content-free seam shared by the production NWPathMonitor adapter,
/// deterministic integration tests, or another runtime component that already
/// owns native reachability monitoring. It creates no threads and retains no
/// endpoint or user data.
#[derive(Clone)]
pub(crate) struct NetworkPathReporter {
    sink: LifecycleEventSink,
}

impl NetworkPathReporter {
    pub(crate) fn new(sink: LifecycleEventSink) -> Self {
        Self { sink }
    }

    pub(crate) fn report(&self, state: NetworkPathState) {
        (self.sink)(LifecycleEvent::NetworkPathChanged(state));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains_start(actions: &[LifecycleAction], generation: u64) -> bool {
        actions.contains(&LifecycleAction::StartTransport { generation })
    }

    fn running_supervisor() -> (LifecycleSupervisor, u64) {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        let generation = supervisor.snapshot().generation;
        let _ = supervisor.handle(LifecycleEvent::TransportStarted { generation });
        (supervisor, generation)
    }

    #[test]
    fn explicit_enable_starts_immediately() {
        let mut supervisor = LifecycleSupervisor::default();
        let actions = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        let snapshot = supervisor.snapshot();

        assert_eq!(snapshot.generation, 2);
        assert_eq!(snapshot.transport_health, TransportHealth::Starting);
        assert!(contains_start(&actions, 2));
        assert!(actions.contains(&LifecycleAction::SetReceiverAvailable(true)));
    }

    #[test]
    fn explicit_disable_invalidates_old_callbacks_and_suspends_sensitive_state() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        let running_generation = supervisor.snapshot().generation;
        let _ = supervisor.handle(LifecycleEvent::TransportStarted {
            generation: running_generation,
        });

        let actions = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: false });
        let disabled_generation = supervisor.snapshot().generation;

        assert!(actions.contains(&LifecycleAction::CancelScheduledWork {
            generation: running_generation,
        }));
        assert!(actions.contains(&LifecycleAction::SuspendSensitiveRuntime {
            reason: SecureSuspensionReason::Disabled,
        }));
        assert!(actions.contains(&LifecycleAction::StopTransport {
            generation: disabled_generation,
        }));
        assert!(supervisor
            .handle(LifecycleEvent::TransportStarted {
                generation: running_generation,
            })
            .is_empty());
    }

    #[test]
    fn secure_suspend_closes_native_content_before_receiver_and_transport_cleanup() {
        let (mut supervisor, running_generation) = running_supervisor();
        let actions = supervisor.handle(LifecycleEvent::WillSleep);
        let suspended_generation = supervisor.snapshot().generation;

        assert_eq!(
            actions,
            vec![
                LifecycleAction::CancelScheduledWork {
                    generation: running_generation,
                },
                LifecycleAction::SuspendSensitiveRuntime {
                    reason: SecureSuspensionReason::Sleep,
                },
                LifecycleAction::SetReceiverAvailable(false),
                LifecycleAction::StopTransport {
                    generation: suspended_generation,
                },
            ]
        );
    }

    #[test]
    fn sleep_suspends_and_wake_debounces_restart() {
        let (mut supervisor, _) = running_supervisor();

        let sleep_actions = supervisor.handle(LifecycleEvent::WillSleep);
        assert!(
            sleep_actions.contains(&LifecycleAction::SuspendSensitiveRuntime {
                reason: SecureSuspensionReason::Sleep,
            })
        );
        assert!(!supervisor.snapshot().receiver_available);

        let wake_actions = supervisor.handle(LifecycleEvent::DidWake);
        let wake_generation = supervisor.snapshot().generation;
        assert!(
            wake_actions.contains(&LifecycleAction::ScheduleRestartDebounce {
                generation: wake_generation,
                delay: RESTART_DEBOUNCE,
            })
        );
        assert!(!contains_start(&wake_actions, wake_generation));

        let elapsed = supervisor.handle(LifecycleEvent::RestartDebounceElapsed {
            generation: wake_generation,
        });
        assert!(contains_start(&elapsed, wake_generation));
    }

    #[test]
    fn session_must_be_active_before_wake_can_restart() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::WillSleep);
        let _ = supervisor.handle(LifecycleEvent::SessionInactive);

        let wake_actions = supervisor.handle(LifecycleEvent::DidWake);
        assert!(!wake_actions
            .iter()
            .any(|action| matches!(action, LifecycleAction::ScheduleRestartDebounce { .. })));
        assert!(!supervisor.snapshot().receiver_available);

        let active_actions = supervisor.handle(LifecycleEvent::SessionActive);
        assert!(active_actions
            .iter()
            .any(|action| matches!(action, LifecycleAction::ScheduleRestartDebounce { .. })));
        assert!(supervisor.snapshot().receiver_available);
    }

    #[test]
    fn unsatisfied_path_stops_and_satisfied_path_debounces_restart() {
        let (mut supervisor, _) = running_supervisor();

        let stop_actions = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Unsatisfied,
        ));
        let stopped_generation = supervisor.snapshot().generation;
        assert!(stop_actions.contains(&LifecycleAction::StopTransport {
            generation: stopped_generation,
        }));
        // Existing, already received requests remain locally actionable on a
        // path loss. Only sleep/session/disable clears receiver availability.
        assert!(supervisor.snapshot().receiver_available);

        let restart_actions = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Satisfied,
        ));
        assert!(restart_actions.iter().any(|action| matches!(
            action,
            LifecycleAction::ScheduleRestartDebounce { delay, .. }
                if *delay == RESTART_DEBOUNCE
        )));
    }

    #[test]
    fn duplicate_path_signal_does_not_churn_generation() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Satisfied,
        ));
        let generation = supervisor.snapshot().generation;

        let actions = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Satisfied,
        ));
        assert!(actions.is_empty());
        assert_eq!(supervisor.snapshot().generation, generation);
    }

    #[test]
    fn network_monitor_failure_is_content_free_idempotent_and_diagnosable() {
        let (mut supervisor, generation) = running_supervisor();
        assert_eq!(
            supervisor.snapshot().network_monitor_state,
            NetworkMonitorState::Pending
        );

        assert_eq!(
            supervisor.handle(LifecycleEvent::NetworkMonitorFailed),
            vec![LifecycleAction::MarkTransportDegraded { generation }]
        );
        let snapshot = supervisor.snapshot();
        assert_eq!(
            snapshot.network_monitor_state,
            NetworkMonitorState::Degraded
        );
        // Native path supervision failed, not the already-running listener.
        assert_eq!(snapshot.transport_health, TransportHealth::Running);
        assert!(supervisor
            .handle(LifecycleEvent::NetworkMonitorFailed)
            .is_empty());
    }

    #[test]
    fn successful_network_monitor_start_does_not_mask_a_prior_failure() {
        let mut supervisor = LifecycleSupervisor::default();
        assert!(supervisor
            .handle(LifecycleEvent::NetworkMonitorStarted)
            .is_empty());
        assert_eq!(
            supervisor.snapshot().network_monitor_state,
            NetworkMonitorState::Available
        );

        let generation = supervisor.snapshot().generation;
        assert_eq!(
            supervisor.handle(LifecycleEvent::NetworkMonitorFailed),
            vec![LifecycleAction::MarkTransportDegraded { generation }]
        );
        assert!(supervisor
            .handle(LifecycleEvent::NetworkMonitorStarted)
            .is_empty());
        assert_eq!(
            supervisor.snapshot().network_monitor_state,
            NetworkMonitorState::Degraded
        );
    }

    #[test]
    fn normal_network_callbacks_do_not_degrade_monitor_health() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::NetworkMonitorStarted);
        let _ = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Satisfied,
        ));
        let _ = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Unsatisfied,
        ));

        assert_eq!(
            supervisor.snapshot().network_monitor_state,
            NetworkMonitorState::Available
        );
    }

    #[test]
    fn stale_debounce_is_ignored_after_disable() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let _ = supervisor.handle(LifecycleEvent::WillSleep);
        let wake_actions = supervisor.handle(LifecycleEvent::DidWake);
        let stale_generation = wake_actions
            .iter()
            .find_map(|action| match action {
                LifecycleAction::ScheduleRestartDebounce { generation, .. } => Some(*generation),
                _ => None,
            })
            .unwrap();

        let _ = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: false });
        assert!(supervisor
            .handle(LifecycleEvent::RestartDebounceElapsed {
                generation: stale_generation,
            })
            .is_empty());
    }

    #[test]
    fn start_failures_follow_bounded_retry_schedule_then_degrade() {
        let mut supervisor = LifecycleSupervisor::default();
        let _ = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        let generation = supervisor.snapshot().generation;

        for (attempt, expected_delay) in RESTART_RETRY_DELAYS.iter().enumerate() {
            let actions = supervisor.handle(LifecycleEvent::TransportStartFailed { generation });
            assert_eq!(
                actions,
                vec![LifecycleAction::ScheduleRetry {
                    generation,
                    attempt,
                    delay: *expected_delay,
                }]
            );
            let actions = supervisor.handle(LifecycleEvent::RetryElapsed {
                generation,
                attempt,
            });
            assert!(contains_start(&actions, generation));
        }

        let actions = supervisor.handle(LifecycleEvent::TransportStartFailed { generation });
        assert_eq!(
            actions,
            vec![LifecycleAction::MarkTransportDegraded { generation }]
        );
        assert_eq!(
            supervisor.snapshot().transport_health,
            TransportHealth::Degraded
        );
        assert_eq!(
            RESTART_DEBOUNCE + RESTART_RETRY_DELAYS.iter().copied().sum::<Duration>(),
            Duration::from_millis(8_750)
        );
    }

    #[test]
    fn stale_retry_timer_is_ignored() {
        let mut supervisor = LifecycleSupervisor::default();
        let _ = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        let generation = supervisor.snapshot().generation;
        let _ = supervisor.handle(LifecycleEvent::TransportStartFailed { generation });

        assert!(supervisor
            .handle(LifecycleEvent::RetryElapsed {
                generation,
                attempt: 1,
            })
            .is_empty());
        assert_eq!(
            supervisor.snapshot().transport_health,
            TransportHealth::Recovering
        );
    }

    #[test]
    fn worker_death_marks_health_non_running_and_schedules_recovery() {
        let (mut supervisor, generation) = running_supervisor();

        let actions = supervisor.handle(LifecycleEvent::TransportWorkerDied { generation });
        assert_eq!(
            actions,
            vec![LifecycleAction::ScheduleRetry {
                generation,
                attempt: 0,
                delay: RESTART_RETRY_DELAYS[0],
            }]
        );
        assert_eq!(
            supervisor.snapshot().transport_health,
            TransportHealth::Recovering
        );
    }

    #[test]
    fn old_worker_death_cannot_stop_new_generation() {
        let (mut supervisor, old_generation) = running_supervisor();
        let _ = supervisor.handle(LifecycleEvent::WillSleep);
        let _ = supervisor.handle(LifecycleEvent::DidWake);
        let new_generation = supervisor.snapshot().generation;
        assert_ne!(old_generation, new_generation);

        assert!(supervisor
            .handle(LifecycleEvent::TransportWorkerDied {
                generation: old_generation,
            })
            .is_empty());
        assert_eq!(
            supervisor.snapshot().transport_health,
            TransportHealth::Recovering
        );
    }

    #[test]
    fn duplicate_enable_and_sleep_notifications_are_idempotent() {
        let (mut supervisor, running_generation) = running_supervisor();

        let enable_actions = supervisor.handle(LifecycleEvent::PreferenceChanged { enabled: true });
        assert_eq!(
            enable_actions,
            vec![LifecycleAction::SetReceiverAvailable(true)]
        );
        assert_eq!(supervisor.snapshot().generation, running_generation);

        let _ = supervisor.handle(LifecycleEvent::WillSleep);
        let sleep_generation = supervisor.snapshot().generation;
        assert!(supervisor.handle(LifecycleEvent::WillSleep).is_empty());
        assert_eq!(supervisor.snapshot().generation, sleep_generation);
    }

    #[test]
    fn termination_is_terminal_even_if_environment_recovers() {
        let mut supervisor = LifecycleSupervisor::new(true);
        let actions = supervisor.handle(LifecycleEvent::WillTerminate);
        assert!(actions.contains(&LifecycleAction::SuspendSensitiveRuntime {
            reason: SecureSuspensionReason::Termination,
        }));

        let wake_actions = supervisor.handle(LifecycleEvent::DidWake);
        let path_actions = supervisor.handle(LifecycleEvent::NetworkPathChanged(
            NetworkPathState::Satisfied,
        ));
        assert!(!wake_actions
            .iter()
            .chain(&path_actions)
            .any(|action| matches!(
                action,
                LifecycleAction::StartTransport { .. }
                    | LifecycleAction::ScheduleRestartDebounce { .. }
            )));
        assert!(!supervisor.snapshot().receiver_available);
    }

    #[test]
    fn network_reporter_forwards_content_free_state() {
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_observed = Arc::clone(&observed);
        let reporter = NetworkPathReporter::new(Arc::new(move |event| {
            sink_observed.lock().unwrap().push(event);
        }));

        reporter.report(NetworkPathState::Satisfied);
        reporter.report(NetworkPathState::Unsatisfied);

        assert_eq!(
            *observed.lock().unwrap(),
            vec![
                LifecycleEvent::NetworkPathChanged(NetworkPathState::Satisfied),
                LifecycleEvent::NetworkPathChanged(NetworkPathState::Unsatisfied),
            ]
        );
    }
}
