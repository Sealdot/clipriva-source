//! Local text actions and the safety boundary for future cloud actions.
//!
//! Text actions are deliberately constrained to an allow-list of deterministic
//! transforms. They do not run arbitrary user code and never require a network
//! connection. Audit records store hashes and redacted previews only.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::context::{enrich_text, RedactionPreview};
use crate::error::{AppError, AppResult};

pub const ACTION_SCHEMA_VERSION: u32 = 2;
pub const LOCAL_TEXT_PIPELINE_MAX_STEPS: usize = 8;
pub const LOCAL_TEXT_PIPELINE_MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTextAction {
    pub id: String,
    pub name: String,
    pub transform: LocalTextTransform,
    pub steps: Vec<LocalTextPipelineStep>,
    pub shortcut_slot: Option<u8>,
    pub created_at: String,
    pub updated_at: String,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLocalTextAction {
    pub name: String,
    pub transform: LocalTextTransform,
    #[serde(default)]
    pub steps: Vec<LocalTextPipelineStep>,
    #[serde(default)]
    pub shortcut_slot: Option<u8>,
}

impl CreateLocalTextAction {
    pub fn resolved_steps(&self) -> AppResult<Vec<LocalTextPipelineStep>> {
        if self
            .shortcut_slot
            .is_some_and(|slot| !(1..=9).contains(&slot))
        {
            return Err(AppError::InvalidInput(
                "A local filter shortcut slot must be between 1 and 9.".to_owned(),
            ));
        }
        let steps = if self.steps.is_empty() {
            vec![LocalTextPipelineStep {
                transform: self.transform,
            }]
        } else {
            self.steps.clone()
        };
        if steps
            .first()
            .is_some_and(|step| step.transform != self.transform)
        {
            return Err(AppError::InvalidInput(
                "The first filter step must match its compatibility transform.".to_owned(),
            ));
        }
        if steps.len() > LOCAL_TEXT_PIPELINE_MAX_STEPS {
            return Err(AppError::InvalidInput(
                "A local text filter requires 1 to 8 steps.".to_owned(),
            ));
        }
        Ok(steps)
    }
}

/// The complete allow-list of transformations available in the first release.
/// Adding a new variant is a reviewed product change rather than executable
/// configuration supplied by a user.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalTextTransform {
    Uppercase,
    Lowercase,
    TrimWhitespace,
    NormalizeWhitespace,
    FormatJson,
    RemoveBlankLines,
    DeduplicateLines,
    SortLinesAsc,
    SortLinesDesc,
}

impl LocalTextTransform {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Uppercase => "uppercase",
            Self::Lowercase => "lowercase",
            Self::TrimWhitespace => "trim_whitespace",
            Self::NormalizeWhitespace => "normalize_whitespace",
            Self::FormatJson => "format_json",
            Self::RemoveBlankLines => "remove_blank_lines",
            Self::DeduplicateLines => "deduplicate_lines",
            Self::SortLinesAsc => "sort_lines_asc",
            Self::SortLinesDesc => "sort_lines_desc",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "uppercase" => Some(Self::Uppercase),
            "lowercase" => Some(Self::Lowercase),
            "trim_whitespace" => Some(Self::TrimWhitespace),
            "normalize_whitespace" => Some(Self::NormalizeWhitespace),
            "format_json" => Some(Self::FormatJson),
            "remove_blank_lines" => Some(Self::RemoveBlankLines),
            "deduplicate_lines" => Some(Self::DeduplicateLines),
            "sort_lines_asc" => Some(Self::SortLinesAsc),
            "sort_lines_desc" => Some(Self::SortLinesDesc),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn apply(self, input: &str) -> AppResult<String> {
        apply_local_text_pipeline(&[LocalTextPipelineStep { transform: self }], input)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTextPipelineStep {
    pub transform: LocalTextTransform,
}

/// Runs a bounded sequence of reviewed, deterministic transforms. Every
/// intermediate result is checked before the next step runs so a transform
/// cannot use a small input to create an unbounded in-process value.
pub fn apply_local_text_pipeline(
    steps: &[LocalTextPipelineStep],
    input: &str,
) -> AppResult<String> {
    if steps.is_empty() || steps.len() > LOCAL_TEXT_PIPELINE_MAX_STEPS {
        return Err(AppError::InvalidInput(
            "A local text filter requires 1 to 8 steps.".to_owned(),
        ));
    }

    let mut output = input.to_owned();
    for step in steps {
        output = apply_single_transform(step.transform, &output)?;
        if output.len() > LOCAL_TEXT_PIPELINE_MAX_OUTPUT_BYTES {
            return Err(AppError::InvalidInput(
                "A local text filter result exceeds 256 KiB.".to_owned(),
            ));
        }
    }
    Ok(output)
}

fn apply_single_transform(transform: LocalTextTransform, input: &str) -> AppResult<String> {
    match transform {
        LocalTextTransform::Uppercase => Ok(input.to_uppercase()),
        LocalTextTransform::Lowercase => Ok(input.to_lowercase()),
        LocalTextTransform::TrimWhitespace => Ok(input.trim().to_owned()),
        LocalTextTransform::NormalizeWhitespace => {
            Ok(input.split_whitespace().collect::<Vec<_>>().join(" "))
        }
        LocalTextTransform::FormatJson => {
            let value = serde_json::from_str::<serde_json::Value>(input).map_err(|_| {
                AppError::InvalidInput("The JSON formatter accepts valid JSON only.".to_owned())
            })?;
            serde_json::to_string_pretty(&value).map_err(|_| {
                AppError::InvalidInput("The JSON formatter could not produce an output.".to_owned())
            })
        }
        LocalTextTransform::RemoveBlankLines => Ok(split_lines(input)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")),
        LocalTextTransform::DeduplicateLines => {
            let mut seen = HashSet::new();
            Ok(split_lines(input)
                .filter(|line| seen.insert(*line))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        LocalTextTransform::SortLinesAsc | LocalTextTransform::SortLinesDesc => {
            let mut lines = split_lines(input).collect::<Vec<_>>();
            lines.sort_unstable();
            if transform == LocalTextTransform::SortLinesDesc {
                lines.reverse();
            }
            Ok(lines.join("\n"))
        }
    }
}

fn split_lines(input: &str) -> impl Iterator<Item = &str> {
    input
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TextActionExecution {
    pub action: LocalTextAction,
    /// This value remains on the device and is returned only to let the local
    /// client put the transformed result on the clipboard.
    pub output: String,
    pub audit: ActionAuditRecord,
}

/// A local, non-writing preview. Confirmation re-reads both records and
/// rejects a changed Item or rule before any clipboard effect is attempted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTextActionPreview {
    pub action: LocalTextAction,
    pub clipboard_item_id: String,
    pub item_version: u32,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionAuditRecord {
    pub id: String,
    pub schema_version: u32,
    pub action_id: Option<String>,
    pub action_name: String,
    pub transform: Option<LocalTextTransform>,
    pub execution_mode: ActionExecutionMode,
    pub status: ActionAuditStatus,
    pub clipboard_item_id: String,
    pub input_content_hash: String,
    pub input_preview: String,
    pub output_content_hash: Option<String>,
    pub output_preview: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionMode {
    Local,
    CloudPreview,
}

impl ActionExecutionMode {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::CloudPreview => "cloud_preview",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "local" => Some(Self::Local),
            "cloud_preview" => Some(Self::CloudPreview),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionAuditStatus {
    Completed,
    PreviewOnlyBlocked,
}

impl ActionAuditStatus {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::PreviewOnlyBlocked => "preview_only_blocked",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "completed" => Some(Self::Completed),
            "preview_only_blocked" => Some(Self::PreviewOnlyBlocked),
            _ => None,
        }
    }
}

/// A deliberately non-executable description of the future cloud boundary.
/// There is no cloud executor in this module or in the Tauri command surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CloudActionPreview {
    pub delivery: String,
    pub is_executable: bool,
    pub message: String,
    pub redaction: RedactionPreview,
    pub audit: ActionAuditRecord,
}

pub fn build_cloud_preview(
    clipboard_item_id: String,
    content: &str,
    audit_id: String,
    created_at: String,
) -> CloudActionPreview {
    let redaction = enrich_text(content).redaction;
    let audit = ActionAuditRecord {
        id: audit_id,
        schema_version: ACTION_SCHEMA_VERSION,
        action_id: None,
        action_name: "Future cloud action".to_owned(),
        transform: None,
        execution_mode: ActionExecutionMode::CloudPreview,
        status: ActionAuditStatus::PreviewOnlyBlocked,
        clipboard_item_id,
        input_content_hash: content_hash(content),
        input_preview: redaction.redacted_content.clone(),
        output_content_hash: None,
        output_preview: None,
        created_at,
    };

    CloudActionPreview {
        delivery: "preview_only".to_owned(),
        is_executable: false,
        message: "Cloud actions are disabled. This is a redacted local preview only.".to_owned(),
        redaction,
        audit,
    }
}

pub fn build_local_audit(
    action: &LocalTextAction,
    clipboard_item_id: String,
    _input: &str,
    _output: &str,
    audit_id: String,
    created_at: String,
) -> ActionAuditRecord {
    ActionAuditRecord {
        id: audit_id,
        schema_version: ACTION_SCHEMA_VERSION,
        action_id: Some(action.id.clone()),
        action_name: action.name.clone(),
        transform: Some(action.transform),
        execution_mode: ActionExecutionMode::Local,
        status: ActionAuditStatus::Completed,
        clipboard_item_id,
        input_content_hash: "not-stored".to_owned(),
        input_preview: String::new(),
        output_content_hash: None,
        output_preview: None,
        created_at,
    }
}

pub fn content_hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_local_text_pipeline, build_cloud_preview, LocalTextPipelineStep, LocalTextTransform,
        LOCAL_TEXT_PIPELINE_MAX_OUTPUT_BYTES,
    };

    #[test]
    fn applies_the_allow_listed_text_transforms() {
        let cases = [
            (LocalTextTransform::Uppercase, "ClipRiva", "CLIPRIVA"),
            (LocalTextTransform::Lowercase, "ClipRiva", "clipriva"),
            (LocalTextTransform::TrimWhitespace, "  note\n", "note"),
            (
                LocalTextTransform::NormalizeWhitespace,
                "one\n  two\tthree",
                "one two three",
            ),
            (
                LocalTextTransform::FormatJson,
                "{\"b\":2,\"a\":1}",
                "{\n  \"a\": 1,\n  \"b\": 2\n}",
            ),
            (
                LocalTextTransform::RemoveBlankLines,
                "one\n \n two\r\n\n",
                "one\n two",
            ),
            (LocalTextTransform::DeduplicateLines, "b\na\nb\n", "b\na\n"),
            (LocalTextTransform::SortLinesAsc, "b\nc\na", "a\nb\nc"),
            (LocalTextTransform::SortLinesDesc, "b\nc\na", "c\nb\na"),
        ];

        for (transform, input, expected) in cases {
            assert_eq!(transform.apply(input).unwrap(), expected);
            assert_eq!(
                LocalTextTransform::from_storage_value(transform.as_storage_value()),
                Some(transform)
            );
        }
    }

    #[test]
    fn applies_pipeline_steps_in_order() {
        let output = apply_local_text_pipeline(
            &[
                LocalTextPipelineStep {
                    transform: LocalTextTransform::TrimWhitespace,
                },
                LocalTextPipelineStep {
                    transform: LocalTextTransform::DeduplicateLines,
                },
                LocalTextPipelineStep {
                    transform: LocalTextTransform::SortLinesAsc,
                },
                LocalTextPipelineStep {
                    transform: LocalTextTransform::Uppercase,
                },
            ],
            "  beta\nalpha\nbeta  ",
        )
        .unwrap();

        assert_eq!(output, "ALPHA\nBETA");
    }

    #[test]
    fn rejects_empty_or_oversized_pipelines_with_finite_errors() {
        let empty = apply_local_text_pipeline(&[], "private-input").unwrap_err();
        assert!(empty
            .to_string()
            .ends_with("A local text filter requires 1 to 8 steps."));

        let too_many = vec![
            LocalTextPipelineStep {
                transform: LocalTextTransform::TrimWhitespace,
            };
            9
        ];
        let error = apply_local_text_pipeline(&too_many, "private-input").unwrap_err();
        assert!(error
            .to_string()
            .ends_with("A local text filter requires 1 to 8 steps."));
        assert!(!error.to_string().contains("private-input"));
    }

    #[test]
    fn rejects_an_oversized_intermediate_result_without_echoing_input() {
        let sensitive_input = "İ".repeat(LOCAL_TEXT_PIPELINE_MAX_OUTPUT_BYTES / 2);
        let error = apply_local_text_pipeline(
            &[LocalTextPipelineStep {
                transform: LocalTextTransform::Lowercase,
            }],
            &sensitive_input,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .ends_with("A local text filter result exceeds 256 KiB."));
        assert!(!error.to_string().contains(&sensitive_input));
    }

    #[test]
    fn json_transform_rejects_invalid_input_without_echoing_it() {
        let error = LocalTextTransform::FormatJson
            .apply("not valid json")
            .unwrap_err();
        assert!(error
            .to_string()
            .ends_with("The JSON formatter accepts valid JSON only."));
    }

    #[test]
    fn cloud_preview_is_explicitly_not_executable_and_redacts_values() {
        let preview = build_cloud_preview(
            "clip-1".to_owned(),
            "email alice@example.com",
            "audit-1".to_owned(),
            "2026-01-01T00:00:00.000Z".to_owned(),
        );

        assert!(!preview.is_executable);
        assert_eq!(preview.delivery, "preview_only");
        assert!(preview
            .audit
            .input_preview
            .contains("[REDACTED:email_address]"));
        assert!(!preview.audit.input_preview.contains("alice@example.com"));
    }
}
