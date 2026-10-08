use serde::{Deserialize, Deserializer, Serialize};

pub const DEFAULT_QUICK_PASTE_SHORTCUT: &str = "CommandOrControl+Shift+Space";
pub const DEFAULT_STACK_SHORTCUT: &str = "CommandOrControl+Alt+S";

fn default_stack_shortcut() -> String {
    DEFAULT_STACK_SHORTCUT.to_owned()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PasteBehavior {
    Restore,
    AutoPaste,
}

impl PasteBehavior {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Restore => "restore",
            Self::AutoPaste => "autoPaste",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "restore" => Some(Self::Restore),
            "autoPaste" => Some(Self::AutoPaste),
            _ => None,
        }
    }
}

/// Defines how capture responds after locally detecting high-confidence
/// sensitive text. The policy never stores the detected item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SensitiveContentPolicy {
    /// Skip only the sensitive clipboard item and continue monitoring later
    /// clipboard changes. This is the safe, low-interruption default.
    #[default]
    Default,
    /// Skip the sensitive clipboard item and pause capture until the user
    /// explicitly resumes it.
    Strict,
}

impl SensitiveContentPolicy {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Strict => "strict",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    pub fn is_strict(self) -> bool {
        matches!(self, Self::Strict)
    }

    fn from_legacy_pause_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Strict
        } else {
            Self::Default
        }
    }
}

/// The preferences accepted by the native command boundary.
///
/// `sensitive_content_policy` is the authoritative replacement for the legacy
/// `sensitive_pause_enabled` boolean. The boolean remains serialized so older
/// clients can round-trip their setting; when a legacy client omits the new
/// field, `true` is interpreted as `Strict` to preserve its prior behavior.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapturePreferences {
    pub capture_paused: bool,
    pub sensitive_pause_enabled: bool,
    pub sensitive_content_policy: SensitiveContentPolicy,
    pub retention_days: u32,
    pub max_history_items: u32,
    pub denied_apps: Vec<String>,
    pub pause_reason: Option<String>,
    pub labs_enabled: bool,
    /// Optional, local-only reliability measurement. It defaults off and is
    /// intentionally independent from Labs and capture itself.
    pub diagnostics_enabled: bool,
    pub paste_behavior: PasteBehavior,
    pub quick_paste_shortcut: String,
    pub stack_shortcut: String,
    pub onboarding_completed: bool,
}

impl Default for CapturePreferences {
    fn default() -> Self {
        Self {
            capture_paused: false,
            sensitive_pause_enabled: false,
            sensitive_content_policy: SensitiveContentPolicy::Default,
            retention_days: 30,
            max_history_items: 5_000,
            denied_apps: Vec::new(),
            pause_reason: None,
            labs_enabled: false,
            diagnostics_enabled: false,
            paste_behavior: PasteBehavior::Restore,
            quick_paste_shortcut: DEFAULT_QUICK_PASTE_SHORTCUT.to_owned(),
            stack_shortcut: DEFAULT_STACK_SHORTCUT.to_owned(),
            onboarding_completed: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapturePreferencesInput {
    capture_paused: bool,
    #[serde(default)]
    sensitive_pause_enabled: Option<bool>,
    #[serde(default)]
    sensitive_content_policy: Option<SensitiveContentPolicy>,
    retention_days: u32,
    max_history_items: u32,
    denied_apps: Vec<String>,
    pause_reason: Option<String>,
    labs_enabled: bool,
    #[serde(default)]
    diagnostics_enabled: bool,
    paste_behavior: PasteBehavior,
    quick_paste_shortcut: String,
    #[serde(default = "default_stack_shortcut")]
    stack_shortcut: String,
    onboarding_completed: bool,
}

impl<'de> Deserialize<'de> for CapturePreferences {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = CapturePreferencesInput::deserialize(deserializer)?;
        let sensitive_content_policy = input.sensitive_content_policy.unwrap_or_else(|| {
            input
                .sensitive_pause_enabled
                .map(SensitiveContentPolicy::from_legacy_pause_enabled)
                .unwrap_or_default()
        });

        Ok(Self {
            capture_paused: input.capture_paused,
            // Keep legacy API output truthful even when a new client supplied
            // conflicting fields: the explicit policy remains authoritative.
            sensitive_pause_enabled: sensitive_content_policy.is_strict(),
            sensitive_content_policy,
            retention_days: input.retention_days,
            max_history_items: input.max_history_items,
            denied_apps: input.denied_apps,
            pause_reason: input.pause_reason,
            labs_enabled: input.labs_enabled,
            diagnostics_enabled: input.diagnostics_enabled,
            paste_behavior: input.paste_behavior,
            quick_paste_shortcut: input.quick_paste_shortcut,
            stack_shortcut: input.stack_shortcut,
            onboarding_completed: input.onboarding_completed,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureDecision {
    Capture,
    Paused,
    DeniedApp,
    SensitiveContent,
    SensitiveContentStrict,
}

pub fn evaluate_capture(
    preferences: &CapturePreferences,
    content: &str,
    source_app: Option<&str>,
) -> CaptureDecision {
    if preferences.capture_paused {
        return CaptureDecision::Paused;
    }

    if source_app
        .map(|source| is_denied_source(source, &preferences.denied_apps))
        .unwrap_or(false)
    {
        return CaptureDecision::DeniedApp;
    }

    if looks_sensitive(content) {
        return match preferences.sensitive_content_policy {
            SensitiveContentPolicy::Default => CaptureDecision::SensitiveContent,
            SensitiveContentPolicy::Strict => CaptureDecision::SensitiveContentStrict,
        };
    }

    CaptureDecision::Capture
}

pub fn is_denied_source(source_app: &str, denied_apps: &[String]) -> bool {
    let source = source_app.trim().to_lowercase();
    denied_apps
        .iter()
        .map(|app| app.trim().to_lowercase())
        .any(|denied| !denied.is_empty() && denied == source)
}

pub fn looks_sensitive(content: &str) -> bool {
    let trimmed = content.trim();
    let lower = trimmed.to_lowercase();

    lower.contains("-----begin private key-----")
        || lower.contains("-----begin rsa private key-----")
        || lower.contains("-----begin openssh private key-----")
        || lower.contains("password=")
        || lower.contains("authorization: bearer ")
        || lower.contains("xoxb-")
        || lower.contains("xoxp-")
        || lower.contains("ghp_")
        || lower.contains("github_pat_")
        || looks_like_openai_key(trimmed)
        || looks_like_aws_access_key(trimmed)
}

fn looks_like_openai_key(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("sk-")
        && value.len() >= 24
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn looks_like_aws_access_key(value: &str) -> bool {
    let value = value.trim();
    let prefix = value
        .get(..4)
        .map(|prefix| prefix.eq_ignore_ascii_case("AKIA"))
        .unwrap_or(false);

    prefix
        && value.len() == 20
        && value
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_capture, looks_sensitive, CaptureDecision, CapturePreferences, PasteBehavior,
        SensitiveContentPolicy, DEFAULT_QUICK_PASTE_SHORTCUT, DEFAULT_STACK_SHORTCUT,
    };

    #[test]
    fn detects_high_confidence_secrets_without_matching_every_long_string() {
        assert!(looks_sensitive("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(looks_sensitive("sk-abcdefghijklmnopqrstuvwxyz012345"));
        assert!(looks_sensitive("AKIAIOSFODNN7EXAMPLE"));
        assert!(!looks_sensitive("ClipRiva project notes for next Tuesday"));
    }

    #[test]
    fn evaluates_pause_and_deny_list_before_sensitive_content() {
        let preferences = CapturePreferences {
            denied_apps: vec!["1Password".to_owned()],
            ..CapturePreferences::default()
        };

        assert_eq!(
            evaluate_capture(&preferences, "non-sensitive", Some("1password")),
            CaptureDecision::DeniedApp
        );
        assert_eq!(
            evaluate_capture(&preferences, "ghp_abcdefghijklmnopqrstuvwx", Some("Notes")),
            CaptureDecision::SensitiveContent
        );
    }

    #[test]
    fn strict_sensitive_policy_requests_an_explicit_capture_pause() {
        let preferences = CapturePreferences {
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            ..CapturePreferences::default()
        };

        assert_eq!(
            evaluate_capture(&preferences, "ghp_abcdefghijklmnopqrstuvwx", Some("Notes")),
            CaptureDecision::SensitiveContentStrict
        );
    }

    #[test]
    fn legacy_preference_payloads_map_true_to_strict_and_false_to_default() {
        let strict: CapturePreferences = serde_json::from_value(serde_json::json!({
            "capturePaused": false,
            "sensitivePauseEnabled": true,
            "retentionDays": 30,
            "maxHistoryItems": 5000,
            "deniedApps": [],
            "pauseReason": null,
            "labsEnabled": false,
            "pasteBehavior": "restore",
            "quickPasteShortcut": "CommandOrControl+Shift+Space",
            "onboardingCompleted": false
        }))
        .unwrap();
        let default: CapturePreferences = serde_json::from_value(serde_json::json!({
            "capturePaused": false,
            "sensitivePauseEnabled": false,
            "retentionDays": 30,
            "maxHistoryItems": 5000,
            "deniedApps": [],
            "pauseReason": null,
            "labsEnabled": false,
            "pasteBehavior": "restore",
            "quickPasteShortcut": "CommandOrControl+Shift+Space",
            "onboardingCompleted": false
        }))
        .unwrap();

        assert_eq!(
            strict.sensitive_content_policy,
            SensitiveContentPolicy::Strict
        );
        assert!(strict.sensitive_pause_enabled);
        assert_eq!(
            default.sensitive_content_policy,
            SensitiveContentPolicy::Default
        );
        assert!(!default.sensitive_pause_enabled);
    }

    #[test]
    fn explicit_policy_wins_over_legacy_boolean_when_both_are_present() {
        let preferences: CapturePreferences = serde_json::from_value(serde_json::json!({
            "capturePaused": false,
            "sensitivePauseEnabled": true,
            "sensitiveContentPolicy": "default",
            "retentionDays": 30,
            "maxHistoryItems": 5000,
            "deniedApps": [],
            "pauseReason": null,
            "labsEnabled": false,
            "pasteBehavior": "restore",
            "quickPasteShortcut": "CommandOrControl+Shift+Space",
            "onboardingCompleted": false
        }))
        .unwrap();

        assert_eq!(
            preferences.sensitive_content_policy,
            SensitiveContentPolicy::Default
        );
        assert!(!preferences.sensitive_pause_enabled);
    }

    #[test]
    fn daily_driver_features_are_the_safe_defaults() {
        let preferences = CapturePreferences::default();

        assert_eq!(
            preferences.sensitive_content_policy,
            SensitiveContentPolicy::Default
        );
        assert!(!preferences.sensitive_pause_enabled);
        assert!(!preferences.labs_enabled);
        assert_eq!(preferences.paste_behavior, PasteBehavior::Restore);
        assert_eq!(
            preferences.quick_paste_shortcut,
            DEFAULT_QUICK_PASTE_SHORTCUT
        );
        assert_eq!(preferences.stack_shortcut, DEFAULT_STACK_SHORTCUT);
        assert!(!preferences.onboarding_completed);
    }
}
