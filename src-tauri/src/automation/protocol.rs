use serde::{Deserialize, Serialize};

pub const AUTOMATION_PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const MAX_OPAQUE_ID_BYTES: usize = 128;
pub const MAX_QUERY_BYTES: usize = 4 * 1024;
pub const MAX_TEXT_INPUT_BYTES: usize = 48 * 1024;
pub const MAX_SEARCH_LIMIT: u16 = 50;
pub const MAX_COLLECTIONS: usize = 8;
pub const MAX_COLLECTION_CHARACTERS: usize = 24;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AutomationCapability {
    HistoryMetadata,
    HistoryContent,
    ClipboardWrite,
    LibraryWrite,
    FiltersRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationRequest {
    pub version: u16,
    pub request_id: String,
    pub operation: AutomationOperation,
}

impl AutomationRequest {
    pub fn new(request_id: String, operation: AutomationOperation) -> Self {
        Self {
            version: AUTOMATION_PROTOCOL_VERSION,
            request_id,
            operation,
        }
    }

    pub fn validate(&self) -> Result<(), AutomationFailure> {
        if self.version != AUTOMATION_PROTOCOL_VERSION {
            return Err(AutomationFailure::new(
                AutomationErrorCode::ProtocolVersionMismatch,
            ));
        }
        if !valid_opaque_identifier(&self.request_id, MAX_REQUEST_ID_BYTES) {
            return Err(AutomationFailure::new(AutomationErrorCode::InvalidRequest));
        }
        self.operation.validate()
    }
}

/// Complete operation allow-list for protocol v1.
///
/// User-authored search and transform text lives inside the framed request; the
/// CLI accepts those values from stdin only. No variant accepts a path, SQL,
/// shell command, executable, URL endpoint, plugin, Agent, or MCP instruction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "name", rename_all = "camelCase", deny_unknown_fields)]
pub enum AutomationOperation {
    Status {},
    Search {
        query: String,
        limit: u16,
    },
    GetText {
        item_id: String,
    },
    CopyItem {
        item_id: String,
        plain_text: bool,
    },
    ListFilters {},
    ApplyFilterToItem {
        filter_id: String,
        item_id: String,
        copy_output: bool,
    },
    ApplyFilterToText {
        filter_id: String,
        input: String,
        copy_output: bool,
    },
    SaveItem {
        item_id: String,
        collections: Vec<String>,
    },
    RemoveSavedItem {
        item_id: String,
    },
}

impl AutomationOperation {
    pub fn required_capabilities(&self) -> Vec<AutomationCapability> {
        use AutomationCapability::{
            ClipboardWrite, FiltersRun, HistoryContent, HistoryMetadata, LibraryWrite,
        };

        match self {
            Self::Status {} => Vec::new(),
            Self::Search { .. } => vec![HistoryMetadata],
            Self::GetText { .. } => vec![HistoryContent],
            Self::CopyItem { .. } => vec![HistoryContent, ClipboardWrite],
            Self::ListFilters {} => vec![FiltersRun],
            Self::ApplyFilterToItem {
                copy_output: false, ..
            } => vec![FiltersRun, HistoryContent],
            Self::ApplyFilterToItem {
                copy_output: true, ..
            } => vec![FiltersRun, HistoryContent, ClipboardWrite],
            Self::ApplyFilterToText {
                copy_output: false, ..
            } => vec![FiltersRun],
            Self::ApplyFilterToText {
                copy_output: true, ..
            } => vec![FiltersRun, ClipboardWrite],
            Self::SaveItem { .. } | Self::RemoveSavedItem { .. } => vec![LibraryWrite],
        }
    }

    pub fn validate(&self) -> Result<(), AutomationFailure> {
        let invalid = || AutomationFailure::new(AutomationErrorCode::InvalidInput);
        match self {
            Self::Status {} | Self::ListFilters {} => Ok(()),
            Self::Search { query, limit } => {
                if query.len() > MAX_QUERY_BYTES || !(1..=MAX_SEARCH_LIMIT).contains(limit) {
                    return Err(invalid());
                }
                Ok(())
            }
            Self::GetText { item_id }
            | Self::CopyItem { item_id, .. }
            | Self::RemoveSavedItem { item_id } => valid_item_id(item_id),
            Self::ApplyFilterToItem {
                filter_id, item_id, ..
            } => {
                valid_filter_id(filter_id)?;
                valid_item_id(item_id)
            }
            Self::ApplyFilterToText {
                filter_id, input, ..
            } => {
                valid_filter_id(filter_id)?;
                if input.len() > MAX_TEXT_INPUT_BYTES {
                    return Err(invalid());
                }
                Ok(())
            }
            Self::SaveItem {
                item_id,
                collections,
            } => {
                valid_item_id(item_id)?;
                if collections.len() > MAX_COLLECTIONS {
                    return Err(invalid());
                }
                let mut normalized = std::collections::HashSet::new();
                for collection in collections {
                    let trimmed = collection.trim();
                    if trimmed.is_empty()
                        || trimmed.chars().count() > MAX_COLLECTION_CHARACTERS
                        || !normalized.insert(trimmed.to_lowercase())
                    {
                        return Err(invalid());
                    }
                }
                Ok(())
            }
        }
    }
}

fn valid_item_id(value: &str) -> Result<(), AutomationFailure> {
    valid_identifier(value)
}

fn valid_filter_id(value: &str) -> Result<(), AutomationFailure> {
    valid_identifier(value)
}

fn valid_identifier(value: &str) -> Result<(), AutomationFailure> {
    if valid_opaque_identifier(value, MAX_OPAQUE_ID_BYTES) {
        Ok(())
    } else {
        Err(AutomationFailure::new(AutomationErrorCode::InvalidInput))
    }
}

fn valid_opaque_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationResponse {
    pub version: u16,
    pub request_id: Option<String>,
    pub outcome: AutomationOutcome,
}

impl AutomationResponse {
    pub fn success(request_id: String, output: AutomationOutput) -> Self {
        Self {
            version: AUTOMATION_PROTOCOL_VERSION,
            request_id: Some(request_id),
            outcome: AutomationOutcome::Success { output },
        }
    }

    pub fn failure(request_id: Option<String>, error: AutomationFailure) -> Self {
        Self {
            version: AUTOMATION_PROTOCOL_VERSION,
            request_id,
            outcome: AutomationOutcome::Failure { error },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum AutomationOutcome {
    Success { output: AutomationOutput },
    Failure { error: AutomationFailure },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum AutomationOutput {
    Status {
        status: AutomationStatus,
    },
    SearchResults {
        items: Vec<AutomationItemSummary>,
    },
    Text {
        content: String,
    },
    Copied,
    Filters {
        filters: Vec<AutomationFilterSummary>,
    },
    FilterOutput {
        output: String,
        copied: bool,
    },
    LibraryUpdated {
        item_id: String,
        saved: bool,
        collections: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationStatus {
    pub enabled: bool,
    pub protocol_version: u16,
    pub capabilities: Vec<AutomationCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationItemSummary {
    pub id: String,
    pub kind: String,
    pub created_at: String,
    pub is_saved: bool,
}

/// Filter metadata deliberately excludes executable configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationFilterSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationFailure {
    pub code: AutomationErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<AutomationCapability>,
}

impl AutomationFailure {
    pub fn new(code: AutomationErrorCode) -> Self {
        Self {
            code,
            capability: None,
        }
    }

    pub fn capability_denied(capability: AutomationCapability) -> Self {
        Self {
            code: AutomationErrorCode::CapabilityDenied,
            capability: Some(capability),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutomationErrorCode {
    ProtocolVersionMismatch,
    FrameTooLarge,
    OutputTooLarge,
    InvalidJson,
    InvalidRequest,
    InvalidInput,
    AppUnavailable,
    Disabled,
    CapabilityDenied,
    NotFound,
    Busy,
    ExecutionFailed,
    TransportFailed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: AutomationOperation) -> AutomationRequest {
        AutomationRequest::new("request-1".to_owned(), operation)
    }

    #[test]
    fn rejects_unknown_request_and_operation_fields() {
        let unknown_request = r#"{"version":1,"requestId":"request-1","operation":{"name":"status"},"sql":"select *"}"#;
        assert!(serde_json::from_str::<AutomationRequest>(unknown_request).is_err());

        let unknown_operation = r#"{"version":1,"requestId":"request-1","operation":{"name":"status","shell":"echo unsafe"}}"#;
        assert!(serde_json::from_str::<AutomationRequest>(unknown_operation).is_err());
    }

    #[test]
    fn finite_operation_schema_has_no_path_sql_shell_or_mcp_variant() {
        for prohibited in ["path", "sql", "shell", "command", "mcp", "endpoint"] {
            let candidate = format!(
                r#"{{"version":1,"requestId":"request-1","operation":{{"name":"{prohibited}"}}}}"#
            );
            assert!(serde_json::from_str::<AutomationRequest>(&candidate).is_err());
        }
    }

    #[test]
    fn maps_every_operation_to_its_required_capabilities() {
        assert_eq!(
            AutomationOperation::Status {}.required_capabilities(),
            vec![]
        );
        assert_eq!(
            AutomationOperation::Search {
                query: String::new(),
                limit: 1,
            }
            .required_capabilities(),
            vec![AutomationCapability::HistoryMetadata]
        );
        assert_eq!(
            AutomationOperation::ApplyFilterToItem {
                filter_id: "filter-1".to_owned(),
                item_id: "item-1".to_owned(),
                copy_output: true,
            }
            .required_capabilities(),
            vec![
                AutomationCapability::FiltersRun,
                AutomationCapability::HistoryContent,
                AutomationCapability::ClipboardWrite,
            ]
        );
    }

    #[test]
    fn validates_ids_limits_text_and_collection_bounds() {
        assert!(request(AutomationOperation::Search {
            query: "safe".to_owned(),
            limit: MAX_SEARCH_LIMIT,
        })
        .validate()
        .is_ok());
        assert!(request(AutomationOperation::Search {
            query: "safe".to_owned(),
            limit: MAX_SEARCH_LIMIT + 1,
        })
        .validate()
        .is_err());
        assert!(request(AutomationOperation::GetText {
            item_id: "../../history.sqlite3".to_owned(),
        })
        .validate()
        .is_err());
        assert!(request(AutomationOperation::ApplyFilterToText {
            filter_id: "filter-1".to_owned(),
            input: "x".repeat(MAX_TEXT_INPUT_BYTES + 1),
            copy_output: false,
        })
        .validate()
        .is_err());
        assert!(request(AutomationOperation::SaveItem {
            item_id: "item-1".to_owned(),
            collections: vec!["Work".to_owned(), "work".to_owned()],
        })
        .validate()
        .is_err());
    }

    #[test]
    fn response_errors_are_finite_and_do_not_carry_free_form_messages() {
        let response = AutomationResponse::failure(
            Some("request-1".to_owned()),
            AutomationFailure::capability_denied(AutomationCapability::HistoryContent),
        );
        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("capabilityDenied"));
        assert!(!serialized.contains("message"));
        assert_eq!(
            serde_json::from_str::<AutomationResponse>(&serialized).unwrap(),
            response
        );
    }
}
