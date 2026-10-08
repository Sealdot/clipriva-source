use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::protocol::{
    AutomationCapability, AutomationErrorCode, AutomationFailure, AutomationFilterSummary,
    AutomationItemSummary, AutomationOperation, AutomationOutput, AutomationStatus,
    AUTOMATION_PROTOCOL_VERSION,
};
use super::server::AutomationServer;
use super::AutomationExecutor;
use crate::db::Database;
use crate::error::AppError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationPreferences {
    pub enabled: bool,
    pub history_metadata: bool,
    pub history_content: bool,
    pub clipboard_write: bool,
    pub library_write: bool,
    pub filters_run: bool,
}

impl AutomationPreferences {
    fn capability_enabled(&self, capability: AutomationCapability) -> bool {
        match capability {
            AutomationCapability::HistoryMetadata => self.history_metadata,
            AutomationCapability::HistoryContent => self.history_content,
            AutomationCapability::ClipboardWrite => self.clipboard_write,
            AutomationCapability::LibraryWrite => self.library_write,
            AutomationCapability::FiltersRun => self.filters_run,
        }
    }

    fn enabled_capabilities(&self) -> Vec<AutomationCapability> {
        [
            AutomationCapability::HistoryMetadata,
            AutomationCapability::HistoryContent,
            AutomationCapability::ClipboardWrite,
            AutomationCapability::LibraryWrite,
            AutomationCapability::FiltersRun,
        ]
        .into_iter()
        .filter(|capability| self.capability_enabled(*capability))
        .collect()
    }
}

struct NativeAutomationExecutor {
    database: Arc<Database>,
    app: AppHandle,
    access: Arc<RwLock<AutomationPreferences>>,
}

impl NativeAutomationExecutor {
    fn failure(error: AppError) -> AutomationFailure {
        match error {
            AppError::NotFound => AutomationFailure::new(AutomationErrorCode::NotFound),
            AppError::InvalidInput(_) => AutomationFailure::new(AutomationErrorCode::InvalidInput),
            _ => AutomationFailure::new(AutomationErrorCode::ExecutionFailed),
        }
    }

    fn copy_text(&self, content: &str) -> Result<(), AutomationFailure> {
        self.app
            .clipboard()
            .write_text(content)
            .map_err(|_| AutomationFailure::new(AutomationErrorCode::ExecutionFailed))?;
        if let Some(state) = self.app.try_state::<crate::quick_paste::QuickPasteState>() {
            state.mark_clipboard_write();
        }
        Ok(())
    }
}

impl AutomationExecutor for NativeAutomationExecutor {
    fn status(&self) -> AutomationStatus {
        let preferences = self.access.read().expect("automation access lock poisoned");
        AutomationStatus {
            enabled: preferences.enabled,
            protocol_version: AUTOMATION_PROTOCOL_VERSION,
            capabilities: preferences.enabled_capabilities(),
        }
    }

    fn capability_enabled(&self, capability: AutomationCapability) -> bool {
        self.access
            .read()
            .expect("automation access lock poisoned")
            .capability_enabled(capability)
    }

    fn execute(
        &self,
        operation: &AutomationOperation,
    ) -> Result<AutomationOutput, AutomationFailure> {
        // Hold one read guard across authorization and execution. Settings
        // updates take the write guard, so a revoked grant cannot race a body
        // read or side effect after admission.
        let preferences = self.access.read().expect("automation access lock poisoned");
        if !preferences.enabled {
            return Err(AutomationFailure::new(AutomationErrorCode::Disabled));
        }
        for capability in operation.required_capabilities() {
            if !preferences.capability_enabled(capability) {
                return Err(AutomationFailure::capability_denied(capability));
            }
        }

        match operation {
            AutomationOperation::Status {} => Ok(AutomationOutput::Status {
                status: AutomationStatus {
                    enabled: preferences.enabled,
                    protocol_version: AUTOMATION_PROTOCOL_VERSION,
                    capabilities: preferences.enabled_capabilities(),
                },
            }),
            AutomationOperation::Search { query, limit } => {
                let items = self
                    .database
                    .list(query, false, u32::from(*limit), 0)
                    .map_err(Self::failure)?
                    .into_iter()
                    .map(|item| AutomationItemSummary {
                        id: item.id,
                        kind: item.kind,
                        created_at: item.created_at,
                        is_saved: item.is_pinned,
                    })
                    .collect();
                Ok(AutomationOutput::SearchResults { items })
            }
            AutomationOperation::GetText { item_id } => {
                let item = self.database.get_by_id(item_id).map_err(Self::failure)?;
                if !item.representations.is_empty() {
                    return Err(AutomationFailure::new(AutomationErrorCode::InvalidInput));
                }
                Ok(AutomationOutput::Text {
                    content: item.content,
                })
            }
            AutomationOperation::CopyItem {
                item_id,
                plain_text: _,
            } => {
                let item = self.database.get_by_id(item_id).map_err(Self::failure)?;
                if !item.representations.is_empty() {
                    return Err(AutomationFailure::new(AutomationErrorCode::InvalidInput));
                }
                self.copy_text(&item.content)?;
                self.database.record_copy(item_id).map_err(Self::failure)?;
                Ok(AutomationOutput::Copied)
            }
            AutomationOperation::ListFilters {} => {
                let filters = self
                    .database
                    .list_local_text_actions()
                    .map_err(Self::failure)?
                    .into_iter()
                    .map(|filter| AutomationFilterSummary {
                        id: filter.id,
                        name: filter.name,
                    })
                    .collect();
                Ok(AutomationOutput::Filters { filters })
            }
            AutomationOperation::ApplyFilterToItem {
                filter_id,
                item_id,
                copy_output,
            } => {
                let item = self.database.get_by_id(item_id).map_err(Self::failure)?;
                if !item.representations.is_empty() {
                    return Err(AutomationFailure::new(AutomationErrorCode::InvalidInput));
                }
                let output = self
                    .database
                    .evaluate_local_text_filter(filter_id, &item.content)
                    .map_err(Self::failure)?;
                if *copy_output {
                    self.copy_text(&output)?;
                }
                Ok(AutomationOutput::FilterOutput {
                    output,
                    copied: *copy_output,
                })
            }
            AutomationOperation::ApplyFilterToText {
                filter_id,
                input,
                copy_output,
            } => {
                let output = self
                    .database
                    .evaluate_local_text_filter(filter_id, input)
                    .map_err(Self::failure)?;
                if *copy_output {
                    self.copy_text(&output)?;
                }
                Ok(AutomationOutput::FilterOutput {
                    output,
                    copied: *copy_output,
                })
            }
            AutomationOperation::SaveItem {
                item_id,
                collections,
            } => {
                let item = self
                    .database
                    .save_snippet(item_id, Some(collections.clone()))
                    .map_err(Self::failure)?;
                Ok(AutomationOutput::LibraryUpdated {
                    item_id: item.id,
                    saved: item.is_pinned,
                    collections: item.tags,
                })
            }
            AutomationOperation::RemoveSavedItem { item_id } => {
                let item = self
                    .database
                    .remove_snippet(item_id)
                    .map_err(Self::failure)?;
                Ok(AutomationOutput::LibraryUpdated {
                    item_id: item.id,
                    saved: item.is_pinned,
                    collections: item.tags,
                })
            }
        }
    }
}

pub struct AutomationRuntime {
    database: Arc<Database>,
    socket_path: PathBuf,
    access: Arc<RwLock<AutomationPreferences>>,
    executor: Arc<NativeAutomationExecutor>,
    server: Mutex<Option<AutomationServer>>,
}

impl AutomationRuntime {
    pub fn new(
        database: Arc<Database>,
        app: AppHandle,
        socket_path: PathBuf,
    ) -> Result<Self, String> {
        let preferences = database
            .automation_preferences()
            .map_err(|error| error.to_string())?;
        let access = Arc::new(RwLock::new(preferences.clone()));
        let executor = Arc::new(NativeAutomationExecutor {
            database: Arc::clone(&database),
            app,
            access: Arc::clone(&access),
        });
        let server = if preferences.enabled {
            Some(
                AutomationServer::start(socket_path.clone(), executor.clone())
                    .map_err(|error| format!("Could not start local automation: {error:?}"))?,
            )
        } else {
            None
        };
        Ok(Self {
            database,
            socket_path,
            access,
            executor,
            server: Mutex::new(server),
        })
    }

    pub fn preferences(&self) -> AutomationPreferences {
        self.access
            .read()
            .expect("automation access lock poisoned")
            .clone()
    }

    pub fn update_preferences(
        &self,
        next: AutomationPreferences,
    ) -> Result<AutomationPreferences, String> {
        let mut access = self
            .access
            .write()
            .expect("automation access lock poisoned");
        let previous = access.clone();
        self.database
            .update_automation_preferences(&next)
            .map_err(|error| error.to_string())?;

        let mut server = self.server.lock().expect("automation server lock poisoned");
        let transition = match (previous.enabled, next.enabled) {
            (false, true) => {
                AutomationServer::start(self.socket_path.clone(), self.executor.clone())
                    .map(|started| *server = Some(started))
            }
            (true, false) => server
                .as_mut()
                .map(AutomationServer::shutdown)
                .transpose()
                .map(|_| *server = None),
            _ => Ok(()),
        };
        if let Err(error) = transition {
            let _ = self.database.update_automation_preferences(&previous);
            return Err(format!("Could not update local automation: {error:?}"));
        }
        *access = next.clone();
        Ok(next)
    }
}

#[tauri::command]
pub fn get_automation_preferences(runtime: State<'_, AutomationRuntime>) -> AutomationPreferences {
    runtime.preferences()
}

#[tauri::command]
pub fn update_automation_preferences(
    runtime: State<'_, AutomationRuntime>,
    preferences: AutomationPreferences,
) -> Result<AutomationPreferences, String> {
    runtime.update_preferences(preferences)
}
