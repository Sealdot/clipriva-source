use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::clipboard::foreground_app;

#[derive(Default)]
pub struct QuickPasteState {
    previous_application_pid: Mutex<Option<i32>>,
    self_write_change_count: Mutex<Option<i64>>,
    activation_in_progress: AtomicBool,
}

pub struct ClipboardActivationGuard<'a>(&'a AtomicBool);

impl Drop for ClipboardActivationGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl QuickPasteState {
    /// Keep one clipboard write and optional paste dispatch in flight at a time.
    /// The guard is released on every return path, including an IPC error.
    pub fn try_begin_activation(&self) -> Option<ClipboardActivationGuard<'_>> {
        self.activation_in_progress
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| ClipboardActivationGuard(&self.activation_in_progress))
    }
    pub fn remember_frontmost_application(&self) {
        let process_id = foreground_app::frontmost_process_id();
        *self
            .previous_application_pid
            .lock()
            .expect("quick paste state mutex poisoned") = process_id;
    }

    fn previous_application_pid(&self) -> Option<i32> {
        *self
            .previous_application_pid
            .lock()
            .expect("quick paste state mutex poisoned")
    }

    pub fn mark_clipboard_write(&self) {
        #[cfg(target_os = "macos")]
        self.record_self_write_change_count(crate::clipboard::macos_pasteboard::change_count());
    }

    fn record_self_write_change_count(&self, change_count: i64) {
        *self
            .self_write_change_count
            .lock()
            .expect("quick paste state mutex poisoned") = Some(change_count);
    }

    pub fn consume_self_write(&self, change_count: i64) -> bool {
        let mut expected = self
            .self_write_change_count
            .lock()
            .expect("quick paste state mutex poisoned");
        if *expected == Some(change_count) {
            *expected = None;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PasteOutcome {
    /// The item is ready for a deliberate Command-V in the target app.
    ClipboardRestored,
    /// ClipRiva sent Command-V to the confirmed prior app. macOS cannot prove
    /// that the target accepted it, so callers must not claim input succeeded.
    PasteSent,
    /// No input event was sent. The item remains available on the clipboard.
    PasteNotSent,
}

/// A safe, user-actionable explanation for a Direct Paste attempt. These are
/// categories rather than platform error strings so no clipboard content or
/// sensitive fragments can be surfaced in a failure result.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PasteFailureReason {
    Permission,
    Focus,
    Compatibility,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PastePermissionStatus {
    Granted,
    NotGranted,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasteAttempt {
    pub outcome: PasteOutcome,
    pub failure_reason: Option<PasteFailureReason>,
}

impl PasteAttempt {
    pub const fn clipboard_restored() -> Self {
        Self {
            outcome: PasteOutcome::ClipboardRestored,
            failure_reason: None,
        }
    }

    const fn paste_sent() -> Self {
        Self {
            outcome: PasteOutcome::PasteSent,
            failure_reason: None,
        }
    }

    const fn paste_not_sent(failure_reason: PasteFailureReason) -> Self {
        Self {
            outcome: PasteOutcome::PasteNotSent,
            failure_reason: Some(failure_reason),
        }
    }
}

pub fn request_accessibility_permission() -> bool {
    foreground_app::request_accessibility_permission()
}

pub fn paste_permission_status() -> PastePermissionStatus {
    #[cfg(target_os = "macos")]
    {
        if foreground_app::accessibility_is_trusted() {
            PastePermissionStatus::Granted
        } else {
            PastePermissionStatus::NotGranted
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PastePermissionStatus::NotGranted
    }
}

pub fn open_accessibility_settings() -> bool {
    foreground_app::open_accessibility_settings()
}

pub async fn paste_to_previous_application(
    app: &AppHandle,
    state: &QuickPasteState,
) -> PasteAttempt {
    let readiness = auto_paste_readiness(
        foreground_app::accessibility_is_trusted(),
        state.previous_application_pid(),
    );
    let process_id = match readiness {
        AutoPasteReadiness::Ready(process_id) => process_id,
        AutoPasteReadiness::AccessibilityRequired => {
            return PasteAttempt::paste_not_sent(PasteFailureReason::Permission);
        }
        AutoPasteReadiness::TargetUnavailable => {
            return PasteAttempt::paste_not_sent(PasteFailureReason::Focus);
        }
    };

    if !foreground_app::activate_application(process_id) {
        return PasteAttempt::paste_not_sent(PasteFailureReason::Focus);
    }
    tokio::time::sleep(Duration::from_millis(90)).await;
    if !target_still_frontmost(process_id, foreground_app::frontmost_process_id()) {
        refocus_quick_paste(app);
        return PasteAttempt::paste_not_sent(PasteFailureReason::Focus);
    }

    if !foreground_app::send_command_v() {
        refocus_quick_paste(app);
        return PasteAttempt::paste_not_sent(PasteFailureReason::Compatibility);
    }

    if let Some(window) = app.get_webview_window("quick-paste") {
        let _ = window.hide();
    }
    PasteAttempt::paste_sent()
}

fn target_still_frontmost(expected_process_id: i32, frontmost_process_id: Option<i32>) -> bool {
    expected_process_id > 0 && frontmost_process_id == Some(expected_process_id)
}

fn refocus_quick_paste(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("quick-paste") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoPasteReadiness {
    Ready(i32),
    AccessibilityRequired,
    TargetUnavailable,
}

fn auto_paste_readiness(trusted: bool, process_id: Option<i32>) -> AutoPasteReadiness {
    if !trusted {
        return AutoPasteReadiness::AccessibilityRequired;
    }
    match process_id.filter(|process_id| *process_id > 0) {
        Some(process_id) => AutoPasteReadiness::Ready(process_id),
        None => AutoPasteReadiness::TargetUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auto_paste_readiness, target_still_frontmost, AutoPasteReadiness, PasteAttempt,
        PasteFailureReason, PasteOutcome, QuickPasteState,
    };

    #[test]
    fn auto_paste_requires_explicit_accessibility_trust() {
        assert_eq!(
            auto_paste_readiness(false, Some(42)),
            AutoPasteReadiness::AccessibilityRequired
        );
    }

    #[test]
    fn auto_paste_requires_the_application_that_opened_quick_paste() {
        assert_eq!(
            auto_paste_readiness(true, None),
            AutoPasteReadiness::TargetUnavailable
        );
        assert_eq!(
            auto_paste_readiness(true, Some(42)),
            AutoPasteReadiness::Ready(42)
        );
    }

    #[test]
    fn direct_paste_aborts_when_focus_changes_after_activation() {
        assert!(target_still_frontmost(42, Some(42)));
        assert!(!target_still_frontmost(42, Some(7)));
        assert!(!target_still_frontmost(42, None));
        assert!(!target_still_frontmost(0, Some(0)));
    }

    #[test]
    fn clipboard_self_write_is_consumed_once_by_the_monitor() {
        let state = QuickPasteState::default();
        state.record_self_write_change_count(17);
        assert!(!state.consume_self_write(16));
        assert!(state.consume_self_write(17));
        assert!(!state.consume_self_write(17));
    }

    #[test]
    fn clipboard_activation_guard_rejects_overlap_and_recovers_after_drop() {
        let state = QuickPasteState::default();
        let first = state.try_begin_activation().expect("first activation");
        assert!(state.try_begin_activation().is_none());
        drop(first);
        assert!(state.try_begin_activation().is_some());
    }

    #[test]
    fn direct_paste_results_are_limited_to_safe_recoverable_categories() {
        let result = PasteAttempt::paste_not_sent(PasteFailureReason::Permission);
        assert_eq!(result.outcome, PasteOutcome::PasteNotSent);
        assert_eq!(result.failure_reason, Some(PasteFailureReason::Permission));

        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(
            serialized,
            r#"{"outcome":"pasteNotSent","failureReason":"permission"}"#
        );
        let sensitive_fixture = "-----BEGIN PRIVATE KEY-----\nsecret-value";
        assert!(!serialized.contains(sensitive_fixture));
        assert!(!serialized.contains("clipboard"));
    }

    #[test]
    fn restore_and_sent_results_carry_no_failure_reason() {
        assert_eq!(
            PasteAttempt::clipboard_restored(),
            PasteAttempt {
                outcome: PasteOutcome::ClipboardRestored,
                failure_reason: None,
            }
        );
        assert_eq!(
            PasteAttempt::paste_sent(),
            PasteAttempt {
                outcome: PasteOutcome::PasteSent,
                failure_reason: None,
            }
        );
    }
}
