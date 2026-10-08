use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::actions::{
    ActionAuditRecord, CloudActionPreview, CreateLocalTextAction, LocalTextAction,
    LocalTextActionPreview, TextActionExecution,
};
use crate::clipboard::image_capture::decode_clipriva_rgba_png;
use crate::clipboard::policy::CapturePreferences;
use crate::context::ContextEnrichment;
use crate::db::{LocalSemanticSearchItem, QuickPasteSearchItem};
use crate::error::AppError;
use crate::media::BlobKind;
use crate::models::{
    BeginLocalLinkPairingRequest, CaptureStatusEvent, ClipboardCleanupPreview,
    ClipboardCleanupRequest, ClipboardCleanupResult, ClipboardCollection,
    ClipboardCollectionMutationResult, ClipboardFilterOptions, ClipboardImageTextExtraction,
    ClipboardImageTextExtractionResult, ClipboardImageTextExtractionStatus, ClipboardItem,
    ClipboardItemNote, ClipboardItemNoteMutationResult, ClipboardListFilters,
    ClipboardSmartCollectionInput, ConfirmLocalLinkPairingRequest, LocalDiagnosticEventInput,
    LocalDiagnosticRecordResult, LocalDiagnosticsExport, LocalDiagnosticsSummary, LocalLinkDevice,
    LocalLinkDiagnostics, LocalLinkPairingDetails, LocalLinkPreferences,
    LocalLinkReadinessSnapshot, LocalLinkReceiveAction, LocalLinkTransfer, RecycleBinItem,
};
use crate::quick_paste::{
    PasteAttempt, PasteFailureReason, PasteOutcome, PastePermissionStatus, QuickPasteState,
};
use crate::state::AppState;

type CommandResult<T> = Result<T, String>;

#[cfg(any(test, target_os = "macos"))]
const EFFECT_TASK_PENDING: u8 = 0;
#[cfg(any(test, target_os = "macos"))]
const EFFECT_TASK_RUNNING: u8 = 1;
#[cfg(any(test, target_os = "macos"))]
const EFFECT_TASK_CANCELLED: u8 = 2;
#[cfg(any(test, target_os = "macos"))]
const EFFECT_TASK_FINISHED: u8 = 3;

#[cfg(any(test, target_os = "macos"))]
fn claim_effect_task(gate: &std::sync::atomic::AtomicU8) -> bool {
    gate.compare_exchange(
        EFFECT_TASK_PENDING,
        EFFECT_TASK_RUNNING,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    )
    .is_ok()
}

#[cfg(any(test, target_os = "macos"))]
fn cancel_pending_effect_task(gate: &std::sync::atomic::AtomicU8) -> bool {
    gate.compare_exchange(
        EFFECT_TASK_PENDING,
        EFFECT_TASK_CANCELLED,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    )
    .is_ok()
}

#[tauri::command]
pub fn list_clipboard_items(
    state: State<'_, AppState>,
    query: String,
    pinned_only: bool,
    limit: u32,
    offset: u32,
    filters: Option<ClipboardListFilters>,
) -> CommandResult<Vec<ClipboardItem>> {
    state
        .database
        .list_with_filters(&query, pinned_only, limit, offset, filters.as_ref())
        .map_err(|error| error.to_string())
}

/// Resolves one active Item for an explicit Stack activation, regardless of
/// the current Quick Paste query. A recycled or expired Item returns `None`.
#[tauri::command]
pub fn get_clipboard_item(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<Option<ClipboardItem>> {
    match state.database.get_by_id(&id) {
        Ok(item) => Ok(Some(item)),
        Err(AppError::NotFound) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub fn get_clipboard_filter_options(
    state: State<'_, AppState>,
) -> CommandResult<ClipboardFilterOptions> {
    state
        .database
        .clipboard_filter_options()
        .map_err(|error| error.to_string())
}

/// Runs only after an explicit Inspector action. The encoded image and
/// recognized text stay in-process; only the dedicated extraction result is
/// returned to the requesting webview.
#[tauri::command]
pub async fn extract_clipboard_image_text(
    state: State<'_, AppState>,
    id: String,
    request_id: String,
) -> CommandResult<ClipboardImageTextExtractionResult> {
    let jobs = crate::macos_ocr::job_coordinator();
    let token = jobs.begin(&request_id).map_err(str::to_owned)?;
    let database = std::sync::Arc::clone(&state.database);
    let image_bytes = match database.image_bytes_for_text_extraction(&id) {
        Ok(bytes) => bytes,
        Err(error) => {
            return if jobs.discard(&token) {
                Err(error.to_string())
            } else {
                Ok(cancelled_image_text_extraction())
            };
        }
    };
    let worker_token = token.clone();
    let mut worker = match tauri::async_runtime::spawn_blocking(move || {
        crate::macos_ocr::OcrWorkerOutput::run(worker_token, &image_bytes)
    })
    .await
    {
        Ok(worker) => worker,
        Err(_) => {
            jobs.discard(&token);
            return Err("Local image text extraction could not finish.".to_owned());
        }
    };
    let recognized = match worker.take_result() {
        Ok(recognized) => recognized,
        Err(error) => {
            return if jobs.discard(&token) {
                Err(error.to_owned())
            } else {
                Ok(cancelled_image_text_extraction())
            };
        }
    };

    match jobs.complete_if_current(&token, || {
        database.save_image_text_extraction(&id, &recognized)
    }) {
        Some(result) => result.map_err(|error| error.to_string()),
        None => Ok(cancelled_image_text_extraction()),
    }
}

fn cancelled_image_text_extraction() -> ClipboardImageTextExtractionResult {
    ClipboardImageTextExtractionResult {
        status: ClipboardImageTextExtractionStatus::Cancelled,
        extraction: None,
    }
}

#[tauri::command]
pub fn cancel_clipboard_image_text_extraction(request_id: String) -> CommandResult<bool> {
    Ok(crate::macos_ocr::job_coordinator().cancel(&request_id))
}

#[tauri::command]
pub fn get_clipboard_image_text(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<Option<ClipboardImageTextExtraction>> {
    state
        .database
        .image_text_extraction(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_clipboard_image_text(state: State<'_, AppState>, id: String) -> CommandResult<bool> {
    state
        .database
        .delete_image_text_extraction(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_clipboard_collections(
    state: State<'_, AppState>,
) -> CommandResult<Vec<ClipboardCollection>> {
    state
        .database
        .list_collections()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_clipboard_smart_collection(
    state: State<'_, AppState>,
    input: ClipboardSmartCollectionInput,
) -> CommandResult<ClipboardCollection> {
    state
        .database
        .create_smart_collection(&input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_clipboard_smart_collection(
    state: State<'_, AppState>,
    id: String,
    input: ClipboardSmartCollectionInput,
) -> CommandResult<ClipboardCollection> {
    state
        .database
        .update_smart_collection(&id, &input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_clipboard_smart_collection(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    state
        .database
        .delete_smart_collection(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_clipboard_item_note(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<Option<ClipboardItemNote>> {
    state
        .database
        .item_note(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_clipboard_item_note(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> CommandResult<ClipboardItemNoteMutationResult> {
    state
        .database
        .save_item_note(&id, &text)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_clipboard_item_note(state: State<'_, AppState>, id: String) -> CommandResult<bool> {
    state
        .database
        .delete_item_note(&id)
        .map_err(|error| error.to_string())
}

/// Compact, deterministic local ranking for the Quick Paste overlay. This is
/// intentionally separate from History's general FTS listing so the overlay's
/// ordering stays explainable and repeatable.
#[tauri::command]
pub fn search_quick_paste_items(
    state: State<'_, AppState>,
    query: String,
    pinned_only: bool,
    limit: u32,
    offset: u32,
) -> CommandResult<Vec<QuickPasteSearchItem>> {
    state
        .database
        .quick_paste_search(&query, pinned_only, limit, offset)
        .map_err(|error| error.to_string())
}

/// Search locally with the deterministic lexical fallback adapter.
///
/// It intentionally has no embedding provider and never transmits clipboard
/// text. The result includes local score/highlight evidence for the workspace.
#[tauri::command]
pub fn search_local_semantic_clipboard_items(
    state: State<'_, AppState>,
    query: String,
    pinned_only: bool,
    limit: u32,
    offset: u32,
) -> CommandResult<Vec<LocalSemanticSearchItem>> {
    state
        .database
        .search_local_semantic(&query, pinned_only, limit, offset)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn copy_clipboard_item(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ClipboardItem> {
    restore_clipboard_item(&app, &state, &id)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardUseResult {
    /// Deliberately excludes the clip content and preview. A recovery result
    /// must remain safe to display even if an older local record is sensitive.
    pub outcome: ClipboardUseOutcome,
    pub failure_reason: Option<PasteFailureReason>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardUseOutcome {
    ClipboardRestored,
    PasteSent,
    PasteNotSent,
    InvalidItem,
    WriteFailed,
    HistoryRecordFailed,
    Busy,
}

impl ClipboardUseResult {
    fn stopped(outcome: ClipboardUseOutcome) -> Self {
        Self {
            outcome,
            failure_reason: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardUseMode {
    DirectPaste,
    PlainTextPaste,
    Copy,
}

#[tauri::command]
pub async fn use_clipboard_item(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    quick_paste_state: State<'_, QuickPasteState>,
    id: String,
    mode: Option<ClipboardUseMode>,
) -> CommandResult<ClipboardUseResult> {
    let Some(_activation) = quick_paste_state.try_begin_activation() else {
        return Ok(ClipboardUseResult::stopped(ClipboardUseOutcome::Busy));
    };
    let mode = mode.unwrap_or(ClipboardUseMode::Copy);
    let item = match state.database.get_by_id(&id) {
        Ok(item) => item,
        Err(AppError::NotFound) => {
            return Ok(ClipboardUseResult::stopped(
                ClipboardUseOutcome::InvalidItem,
            ));
        }
        Err(error) => return Err(error.to_string()),
    };
    let write_result = write_clipboard_item_for_mode(&app, &state, &item, mode);
    if write_result.is_err() {
        return Ok(ClipboardUseResult::stopped(
            ClipboardUseOutcome::WriteFailed,
        ));
    }
    if state.database.record_copy(&id).is_err() {
        return Ok(ClipboardUseResult::stopped(
            ClipboardUseOutcome::HistoryRecordFailed,
        ));
    }
    let attempt = match mode {
        ClipboardUseMode::Copy => PasteAttempt::clipboard_restored(),
        ClipboardUseMode::DirectPaste | ClipboardUseMode::PlainTextPaste => {
            crate::quick_paste::paste_to_previous_application(&app, &quick_paste_state).await
        }
    };

    // A Direct Paste failure must leave an actionable fallback, even if the
    // target app changed the system clipboard while ClipRiva was trying to
    // activate it. Re-assert the same local item without recording a second
    // use so one retry never creates another History row or double-counts.
    if attempt.outcome == PasteOutcome::PasteNotSent
        && write_clipboard_item_for_mode(&app, &state, &item, mode).is_err()
    {
        return Ok(ClipboardUseResult::stopped(
            ClipboardUseOutcome::WriteFailed,
        ));
    }

    Ok(ClipboardUseResult {
        outcome: match attempt.outcome {
            PasteOutcome::ClipboardRestored => ClipboardUseOutcome::ClipboardRestored,
            PasteOutcome::PasteSent => ClipboardUseOutcome::PasteSent,
            PasteOutcome::PasteNotSent => ClipboardUseOutcome::PasteNotSent,
        },
        failure_reason: attempt.failure_reason,
    })
}

fn write_clipboard_item_for_mode(
    app: &tauri::AppHandle,
    state: &AppState,
    item: &ClipboardItem,
    mode: ClipboardUseMode,
) -> CommandResult<()> {
    if mode == ClipboardUseMode::PlainTextPaste {
        app.clipboard()
            .write_text(&item.content)
            .map_err(|error| error.to_string())?;
        mark_clipboard_self_write(app);
        Ok(())
    } else {
        write_clipboard_item(app, state, item)
    }
}

#[tauri::command]
pub fn get_paste_permission_status() -> PastePermissionStatus {
    crate::quick_paste::paste_permission_status()
}

#[tauri::command]
pub fn request_paste_permission() -> PastePermissionStatus {
    crate::quick_paste::request_accessibility_permission();
    crate::quick_paste::paste_permission_status()
}

#[tauri::command]
pub fn open_paste_permission_settings() -> CommandResult<()> {
    crate::quick_paste::open_accessibility_settings()
        .then_some(())
        .ok_or_else(|| "Accessibility settings are unavailable on this platform.".to_owned())
}

/// Restores a selected local item to the system clipboard and records that
/// explicit use. The tray uses the same path as the WebView command so its
/// "recent" shortcuts cannot bypass image handling or self-capture
/// suppression.
pub(crate) fn restore_clipboard_item(
    app: &tauri::AppHandle,
    state: &AppState,
    id: &str,
) -> CommandResult<ClipboardItem> {
    let item = state
        .database
        .get_by_id(id)
        .map_err(|error| error.to_string())?;
    write_clipboard_item(app, state, &item)?;
    state
        .database
        .record_copy(id)
        .map_err(|error| error.to_string())
}

fn write_clipboard_item(
    app: &tauri::AppHandle,
    state: &AppState,
    item: &ClipboardItem,
) -> CommandResult<()> {
    match item.kind.as_str() {
        "image" => {
            let representation = item
                .representations
                .iter()
                .find(|representation| representation.kind == BlobKind::Image)
                .ok_or_else(|| "Image clipboard data is unavailable for this item.".to_owned())?;
            if representation.mime_type != "image/png" {
                return Err(
                    "This image representation cannot be restored to the system clipboard."
                        .to_owned(),
                );
            }
            let bytes = state
                .database
                .read_blob(&representation.storage_key)
                .map_err(|error| error.to_string())?;
            let decoded = decode_clipriva_rgba_png(&bytes).map_err(|error| error.to_string())?;
            let image = tauri::image::Image::new_owned(
                decoded.pixels,
                decoded.dimensions.width,
                decoded.dimensions.height,
            );
            app.clipboard()
                .write_image(&image)
                .map_err(|error| error.to_string())?;
        }
        "richText" => {
            #[cfg(target_os = "macos")]
            {
                let mut representations = Vec::new();
                for representation in item
                    .representations
                    .iter()
                    .filter(|representation| representation.kind == BlobKind::RichText)
                {
                    let bytes = state
                        .database
                        .read_blob(&representation.storage_key)
                        .map_err(|error| error.to_string())?;
                    representations.push((representation.mime_type.clone(), bytes));
                }
                if representations.is_empty() {
                    return Err("Rich-text clipboard data is unavailable for this item.".to_owned());
                }
                crate::clipboard::macos_pasteboard::write_rich_text(
                    &item.content,
                    &representations,
                )?;
            }
            #[cfg(not(target_os = "macos"))]
            app.clipboard()
                .write_text(&item.content)
                .map_err(|error| error.to_string())?;
        }
        "file" => {
            #[cfg(target_os = "macos")]
            {
                let representation = item
                    .representations
                    .iter()
                    .find(|representation| representation.kind == BlobKind::File)
                    .ok_or_else(|| {
                        "Local file references are unavailable for this item.".to_owned()
                    })?;
                let bytes = state
                    .database
                    .read_blob(&representation.storage_key)
                    .map_err(|error| error.to_string())?;
                let paths: Vec<String> =
                    serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
                crate::clipboard::macos_pasteboard::write_file_paths(&paths)?;
            }
            #[cfg(not(target_os = "macos"))]
            app.clipboard()
                .write_text(&item.content)
                .map_err(|error| error.to_string())?;
        }
        _ => {
            app.clipboard()
                .write_text(&item.content)
                .map_err(|error| error.to_string())?;
        }
    }
    mark_clipboard_self_write(app);
    Ok(())
}

fn mark_clipboard_self_write(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<QuickPasteState>() {
        state.mark_clipboard_write();
    }
}

#[tauri::command]
pub fn toggle_clipboard_pin(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ClipboardItem> {
    state
        .database
        .toggle_pin(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn replace_clipboard_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> CommandResult<ClipboardItem> {
    state
        .database
        .replace_tags(&id, tags)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_clipboard_snippet(
    state: State<'_, AppState>,
    id: String,
    collections: Option<Vec<String>>,
) -> CommandResult<ClipboardItem> {
    state
        .database
        .save_snippet(&id, collections)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn remove_clipboard_snippet(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ClipboardItem> {
    state
        .database
        .remove_snippet(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_clipboard_collection(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> CommandResult<ClipboardCollectionMutationResult> {
    state
        .database
        .rename_collection(&from, &to)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_clipboard_collection(
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<ClipboardCollectionMutationResult> {
    state
        .database
        .delete_collection(&name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_clipboard_item(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state
        .database
        .delete(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_clipboard_history(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .database
        .clear_unpinned()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_clipboard_cleanup(
    state: State<'_, AppState>,
    request: ClipboardCleanupRequest,
) -> CommandResult<ClipboardCleanupPreview> {
    state
        .database
        .preview_cleanup(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn move_clipboard_items_to_recycle_bin(
    state: State<'_, AppState>,
    request: ClipboardCleanupRequest,
) -> CommandResult<ClipboardCleanupResult> {
    state
        .database
        .move_to_recycle_bin(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_recycle_bin_items(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<RecycleBinItem>> {
    state
        .database
        .recycle_bin_items(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn restore_clipboard_item_from_recycle_bin(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ClipboardItem> {
    state
        .database
        .restore_from_recycle_bin(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn permanently_delete_recycle_bin_items(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> CommandResult<u32> {
    state
        .database
        .permanently_delete_recycled_items(&item_ids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn empty_clipboard_recycle_bin(state: State<'_, AppState>) -> CommandResult<u32> {
    state
        .database
        .empty_recycle_bin()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_capture_preferences(state: State<'_, AppState>) -> CommandResult<CapturePreferences> {
    state
        .database
        .capture_preferences()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_capture_preferences(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut preferences: CapturePreferences,
) -> CommandResult<CapturePreferences> {
    preferences.quick_paste_shortcut = preferences.quick_paste_shortcut.trim().to_owned();
    preferences.stack_shortcut = preferences.stack_shortcut.trim().to_owned();
    if preferences.quick_paste_shortcut.is_empty() {
        return Err("Quick Paste shortcut cannot be empty.".to_owned());
    }
    if preferences.stack_shortcut.is_empty() {
        return Err("Stack shortcut cannot be empty.".to_owned());
    }
    if preferences.quick_paste_shortcut == preferences.stack_shortcut {
        return Err("Quick Paste and Stack shortcuts must be different.".to_owned());
    }

    let previous = state
        .database
        .capture_preferences()
        .map_err(|error| error.to_string())?;

    #[cfg(desktop)]
    crate::replace_global_shortcuts(&app, &previous, &preferences)?;

    let result = state
        .database
        .update_capture_preferences(&preferences)
        .map_err(|error| error.to_string());

    #[cfg(desktop)]
    if result.is_err() {
        crate::restore_global_shortcuts(&app, &previous, &preferences);
    }

    result
}

#[tauri::command]
pub fn resume_clipboard_capture(state: State<'_, AppState>) -> CommandResult<CapturePreferences> {
    state
        .database
        .resume_capture()
        .map_err(|error| error.to_string())
}

/// Recent local explanations for clipboard changes that were intentionally or
/// necessarily not captured. The returned data never includes clipboard text,
/// previews, representations, or content hashes.
#[tauri::command]
pub fn list_recent_capture_status_events(
    state: State<'_, AppState>,
) -> CommandResult<Vec<CaptureStatusEvent>> {
    state
        .database
        .recent_capture_status_events()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_capture_status_events(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .database
        .clear_capture_status_events()
        .map_err(|error| error.to_string())
}

/// Records an opt-in, anonymous local reliability signal. The typed request
/// has no content, source, file, path, document, ID or timestamp fields.
#[tauri::command]
pub fn record_local_diagnostic_event(
    state: State<'_, AppState>,
    event: LocalDiagnosticEventInput,
) -> CommandResult<LocalDiagnosticRecordResult> {
    state
        .database
        .record_local_diagnostic_event(&event)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_local_diagnostics_summary(
    state: State<'_, AppState>,
) -> CommandResult<LocalDiagnosticsSummary> {
    state
        .database
        .local_diagnostics_summary()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_local_diagnostics(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .database
        .clear_local_diagnostics()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_local_diagnostics(
    state: State<'_, AppState>,
) -> CommandResult<LocalDiagnosticsExport> {
    state
        .database
        .export_local_diagnostics()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_clipboard_enrichment(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<ContextEnrichment> {
    state
        .database
        .context_enrichment(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_local_text_actions(state: State<'_, AppState>) -> CommandResult<Vec<LocalTextAction>> {
    state
        .database
        .list_local_text_actions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_local_text_action(
    state: State<'_, AppState>,
    draft: CreateLocalTextAction,
) -> CommandResult<LocalTextAction> {
    state
        .database
        .create_local_text_action(&draft)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_local_text_action(
    state: State<'_, AppState>,
    id: String,
    draft: CreateLocalTextAction,
) -> CommandResult<LocalTextAction> {
    state
        .database
        .update_local_text_action(&id, &draft)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_local_text_action(state: State<'_, AppState>, id: String) -> CommandResult<bool> {
    state
        .database
        .delete_local_text_action(&id)
        .map_err(|error| error.to_string())
}

/// Computes an allow-listed local result without writing the clipboard or DB.
#[tauri::command]
pub fn preview_local_text_action(
    state: State<'_, AppState>,
    action_id: String,
    clipboard_item_id: String,
) -> CommandResult<LocalTextActionPreview> {
    state
        .database
        .preview_local_text_action(&action_id, &clipboard_item_id)
        .map_err(|error| error.to_string())
}

/// Rechecks the Item and rule snapshots from preview before the local write.
#[tauri::command]
pub fn confirm_local_text_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    preview: LocalTextActionPreview,
) -> CommandResult<TextActionExecution> {
    let execution = state
        .database
        .confirm_local_text_action(&preview)
        .map_err(|error| error.to_string())?;
    app.clipboard()
        .write_text(&execution.output)
        .map_err(|error| error.to_string())?;
    mark_clipboard_self_write(&app);
    state
        .database
        .record_action_audit(&execution.audit)
        .map_err(|error| {
            format!("Filter result copied, but local audit could not be saved: {error}")
        })?;
    Ok(execution)
}

#[tauri::command]
pub fn list_action_audit(
    state: State<'_, AppState>,
    clipboard_item_id: Option<String>,
    limit: u32,
) -> CommandResult<Vec<ActionAuditRecord>> {
    state
        .database
        .list_action_audit(clipboard_item_id.as_deref(), limit)
        .map_err(|error| error.to_string())
}

/// This command is intentionally a preview endpoint only. It records a
/// redacted audit entry and cannot execute any cloud action.
#[tauri::command]
pub fn preview_cloud_action(
    state: State<'_, AppState>,
    clipboard_item_id: String,
) -> CommandResult<CloudActionPreview> {
    state
        .database
        .preview_cloud_action(&clipboard_item_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_local_link_preferences(
    state: State<'_, AppState>,
) -> CommandResult<LocalLinkPreferences> {
    state
        .local_link
        .preferences()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_local_link_readiness_snapshot(
    state: State<'_, AppState>,
) -> CommandResult<LocalLinkReadinessSnapshot> {
    state
        .local_link
        .readiness_snapshot()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn build_local_link_diagnostics(
    state: State<'_, AppState>,
) -> CommandResult<LocalLinkDiagnostics> {
    state
        .local_link
        .build_diagnostics()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_local_link_identity_fingerprint(state: State<'_, AppState>) -> CommandResult<String> {
    state
        .local_link
        .local_identity_fingerprint()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reset_local_link_identity(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .local_link
        .reset_identity()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_local_link_preferences(
    state: State<'_, AppState>,
    preferences: LocalLinkPreferences,
) -> CommandResult<LocalLinkPreferences> {
    state
        .local_link
        .update_preferences(preferences)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_local_link_devices(state: State<'_, AppState>) -> CommandResult<Vec<LocalLinkDevice>> {
    state
        .local_link
        .list_devices()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn begin_local_link_pairing(
    state: State<'_, AppState>,
    request: BeginLocalLinkPairingRequest,
) -> CommandResult<LocalLinkDevice> {
    state
        .local_link
        .begin_pairing(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_local_link_pairing(
    state: State<'_, AppState>,
) -> CommandResult<Option<LocalLinkPairingDetails>> {
    state
        .local_link
        .active_pairing_details()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_local_link_pairing(
    state: State<'_, AppState>,
    request: ConfirmLocalLinkPairingRequest,
) -> CommandResult<LocalLinkDevice> {
    state
        .local_link
        .confirm_pairing(&request.device_id, request.trust_duration)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel_local_link_pairing(
    state: State<'_, AppState>,
    device_id: String,
) -> CommandResult<()> {
    state
        .local_link
        .cancel_pairing(&device_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn revoke_local_link_device(
    state: State<'_, AppState>,
    device_id: String,
) -> CommandResult<LocalLinkDevice> {
    state
        .local_link
        .revoke_device(&device_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn send_clipboard_item_to_device(
    state: State<'_, AppState>,
    clipboard_item_id: String,
    device_id: String,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .send_clipboard_item(&clipboard_item_id, &device_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_local_link_transfers(
    state: State<'_, AppState>,
    limit: u32,
) -> CommandResult<Vec<LocalLinkTransfer>> {
    state
        .local_link
        .list_transfers(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_local_link_transfers(state: State<'_, AppState>) -> CommandResult<u64> {
    state
        .local_link
        .clear_transfers()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reveal_local_link_transfer(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    transfer_id: String,
) -> CommandResult<()> {
    state
        .local_link
        .reveal_transfer(&transfer_id, |payload| {
            reveal_local_link_text_natively(&app, payload)
        })
        .map_err(|error| error.to_string())
}

/// Marks an Incoming Center request as opened. The returned transfer is
/// metadata-only; plaintext remains inside the native Local Link runtime.
#[tauri::command]
pub fn mark_local_link_transfer_viewed(
    state: State<'_, AppState>,
    transfer_id: String,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .mark_transfer_viewed(&transfer_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cancel_local_link_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .cancel_transfer(&transfer_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn accept_local_link_transfer(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    transfer_id: String,
    action: LocalLinkReceiveAction,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .accept_transfer(&transfer_id, action, |payload, effect_token| {
            write_local_link_clipboard_natively(&app, payload, effect_token)
        })
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn write_local_link_clipboard_natively(
    app: &tauri::AppHandle,
    payload: &str,
    effect_token: &str,
) -> crate::error::AppResult<crate::local_link::ClipboardWriteEvidence> {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    const MARKER_TYPE: &str = crate::clipboard::macos_pasteboard::LOCAL_LINK_EFFECT_PASTEBOARD_TYPE;
    fn write(payload: &str, effect_token: &str) -> (bool, i64) {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        let marker_type = NSString::from_str(MARKER_TYPE);
        // SAFETY: AppKit exposes NSPasteboardTypeString as a process-lifetime
        // immutable NSString constant.
        let string_type = unsafe { NSPasteboardTypeString };
        let wrote_text = pasteboard.setString_forType(&NSString::from_str(payload), string_type);
        let wrote_marker =
            pasteboard.setString_forType(&NSString::from_str(effect_token), &marker_type);
        (wrote_text && wrote_marker, pasteboard.changeCount() as i64)
    }
    let (written, change_count) = if MainThreadMarker::new().is_some() {
        write(payload, effect_token)
    } else {
        let payload = zeroize::Zeroizing::new(payload.to_owned());
        let effect_token = effect_token.to_owned();
        let gate = Arc::new(AtomicU8::new(EFFECT_TASK_PENDING));
        let task_gate = Arc::clone(&gate);
        let (sender, receiver) = mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            if !claim_effect_task(&task_gate) {
                let _ = sender.send(None);
                return;
            }
            let result = write(payload.as_str(), &effect_token);
            task_gate.store(EFFECT_TASK_FINISHED, Ordering::Release);
            let _ = sender.send(Some(result));
        })
        .map_err(|_| {
            crate::error::AppError::InvalidInput(
                "Could not schedule the accepted Local Link pasteboard write.".to_owned(),
            )
        })?;
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(Some(result)) => result,
            Ok(None) => {
                return Err(crate::error::AppError::InvalidInput(
                    "The queued Local Link pasteboard write was cancelled before it started."
                        .to_owned(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) if cancel_pending_effect_task(&gate) => {
                // The main-thread closure may still be queued, but the CAS
                // makes it impossible for that closure to write after this
                // function has reported timeout/uncertainty.
                return Err(crate::error::AppError::InvalidInput(
                    "The Local Link pasteboard write timed out before it started and was cancelled."
                        .to_owned(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => receiver
                .recv()
                .map_err(|_| {
                    crate::error::AppError::InvalidInput(
                        "The running Local Link pasteboard write did not return evidence."
                            .to_owned(),
                    )
                })?
                .ok_or_else(|| {
                    crate::error::AppError::InvalidInput(
                        "The Local Link pasteboard write was cancelled.".to_owned(),
                    )
                })?,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(crate::error::AppError::InvalidInput(
                    "The Local Link pasteboard write ended without evidence.".to_owned(),
                ));
            }
        }
    };
    if !written {
        return Err(crate::error::AppError::InvalidInput(
            "Could not write the accepted Local Link text and recovery marker.".to_owned(),
        ));
    }
    mark_clipboard_self_write(app);
    Ok(crate::local_link::ClipboardWriteEvidence { change_count })
}

#[cfg(not(target_os = "macos"))]
fn write_local_link_clipboard_natively(
    app: &tauri::AppHandle,
    payload: &str,
    _effect_token: &str,
) -> crate::error::AppResult<crate::local_link::ClipboardWriteEvidence> {
    app.clipboard().write_text(payload).map_err(|_| {
        crate::error::AppError::InvalidInput(
            "Could not write the accepted Local Link text to the system clipboard.".to_owned(),
        )
    })?;
    mark_clipboard_self_write(app);
    Ok(crate::local_link::ClipboardWriteEvidence { change_count: 0 })
}

#[cfg(target_os = "macos")]
pub(crate) fn recover_local_link_copy_effects_natively(
    app: &tauri::AppHandle,
    service: &crate::local_link::LocalLinkService,
) -> crate::error::AppResult<usize> {
    service.recover_copy_effects(|| {
        use std::sync::mpsc;
        use std::time::Duration;

        use objc2::MainThreadMarker;
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;

        const MARKER_TYPE: &str =
            crate::clipboard::macos_pasteboard::LOCAL_LINK_EFFECT_PASTEBOARD_TYPE;
        fn read() -> (Option<String>, i64) {
            let pasteboard = NSPasteboard::generalPasteboard();
            let marker_type = NSString::from_str(MARKER_TYPE);
            let marker = pasteboard
                .stringForType(&marker_type)
                .map(|value| value.to_string());
            (marker, pasteboard.changeCount() as i64)
        }
        if MainThreadMarker::new().is_some() {
            return Ok(read());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            let _ = sender.send(read());
        })
        .map_err(|_| {
            crate::error::AppError::InvalidInput(
                "Could not schedule Local Link pasteboard recovery.".to_owned(),
            )
        })?;
        receiver.recv_timeout(Duration::from_secs(2)).map_err(|_| {
            crate::error::AppError::InvalidInput(
                "Local Link pasteboard recovery did not complete in time.".to_owned(),
            )
        })
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn recover_local_link_copy_effects_natively(
    _app: &tauri::AppHandle,
    service: &crate::local_link::LocalLinkService,
) -> crate::error::AppResult<usize> {
    service.recover_copy_effects(|| Ok((None, 0)))
}

#[tauri::command]
pub fn reject_local_link_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .reject_transfer(&transfer_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn retry_local_link_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> CommandResult<LocalLinkTransfer> {
    state
        .local_link
        .retry_transfer(&transfer_id)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
static ACTIVE_LOCAL_LINK_ALERT: std::sync::atomic::AtomicPtr<objc2_app_kit::NSAlert> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
#[cfg(target_os = "macos")]
static ACTIVE_LOCAL_LINK_REVEAL_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
#[cfg(target_os = "macos")]
static LOCAL_LINK_REVEAL_APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

#[cfg(target_os = "macos")]
fn reveal_local_link_text_natively(
    app: &tauri::AppHandle,
    payload: zeroize::Zeroizing<String>,
) -> crate::error::AppResult<()> {
    use std::sync::atomic::Ordering;

    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertStyle};
    use objc2_foundation::NSString;
    let _ = LOCAL_LINK_REVEAL_APP.set(app.clone());
    let generation = ACTIVE_LOCAL_LINK_REVEAL_GENERATION
        .fetch_add(1, Ordering::AcqRel)
        .wrapping_add(1);
    let _ = std::thread::Builder::new()
        .name("clipriva-local-link-preview-timeout".to_owned())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if ACTIVE_LOCAL_LINK_REVEAL_GENERATION.load(Ordering::Acquire) == generation {
                close_local_link_reveals_natively();
            }
        });
    app.run_on_main_thread(move || {
        if ACTIVE_LOCAL_LINK_REVEAL_GENERATION.load(Ordering::Acquire) != generation {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(NSAlertStyle::Informational);
        alert.setMessageText(&NSString::from_str("Local Link secure preview"));
        alert.setInformativeText(&NSString::from_str(payload.as_str()));
        ACTIVE_LOCAL_LINK_ALERT.store(Retained::as_ptr(&alert).cast_mut(), Ordering::Release);
        let _ = alert.runModal();
        alert.setInformativeText(&NSString::from_str(""));
        let _ = ACTIVE_LOCAL_LINK_ALERT.compare_exchange(
            Retained::as_ptr(&alert).cast_mut(),
            std::ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        let _ = ACTIVE_LOCAL_LINK_REVEAL_GENERATION.compare_exchange(
            generation,
            generation.wrapping_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    })
    .map_err(|_| {
        crate::error::AppError::InvalidInput(
            "Could not open the native Local Link secure preview.".to_owned(),
        )
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn close_local_link_reveals_natively() {
    use std::sync::atomic::Ordering;

    // Invalidate both the currently visible alert and any reveal closure that
    // has been queued on the main thread but has not displayed yet.
    ACTIVE_LOCAL_LINK_REVEAL_GENERATION.fetch_add(1, Ordering::AcqRel);

    fn close_on_main_thread() {
        use std::sync::atomic::Ordering;

        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAlert, NSApplication};
        use objc2_foundation::NSString;

        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let pointer = ACTIVE_LOCAL_LINK_ALERT.swap(std::ptr::null_mut(), Ordering::AcqRel);
        if pointer.is_null() {
            return;
        }
        // SAFETY: the pointer is published only while the retained alert is
        // inside runModal on this same main thread and is cleared before that
        // retained value is dropped.
        let alert: &NSAlert = unsafe { &*pointer };
        alert.setInformativeText(&NSString::from_str(""));
        alert.window().close();
        NSApplication::sharedApplication(mtm).abortModal();
    }

    if objc2::MainThreadMarker::new().is_some() {
        close_on_main_thread();
    } else if let Some(app) = LOCAL_LINK_REVEAL_APP.get() {
        let _ = app.run_on_main_thread(close_on_main_thread);
    }
}

#[cfg(not(target_os = "macos"))]
fn reveal_local_link_text_natively(
    _app: &tauri::AppHandle,
    _payload: zeroize::Zeroizing<String>,
) -> crate::error::AppResult<()> {
    Err(crate::error::AppError::InvalidInput(
        "Local Link secure preview is available on macOS only.".to_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn close_local_link_reveals_natively() {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU8, Ordering};

    use super::{
        cancel_pending_effect_task, claim_effect_task, ClipboardUseMode, ClipboardUseOutcome,
        ClipboardUseResult, EFFECT_TASK_CANCELLED, EFFECT_TASK_FINISHED, EFFECT_TASK_PENDING,
        EFFECT_TASK_RUNNING,
    };

    #[test]
    fn clipboard_use_modes_have_an_explicit_stable_command_contract() {
        assert_eq!(
            serde_json::from_str::<ClipboardUseMode>(r#""directPaste""#).unwrap(),
            ClipboardUseMode::DirectPaste
        );
        assert_eq!(
            serde_json::from_str::<ClipboardUseMode>(r#""plainTextPaste""#).unwrap(),
            ClipboardUseMode::PlainTextPaste
        );
        assert_eq!(
            serde_json::from_str::<ClipboardUseMode>(r#""copy""#).unwrap(),
            ClipboardUseMode::Copy
        );
    }

    #[test]
    fn stopped_recovery_results_are_bounded_and_contain_no_item_content() {
        for (outcome, expected) in [
            (ClipboardUseOutcome::InvalidItem, "invalidItem"),
            (ClipboardUseOutcome::WriteFailed, "writeFailed"),
            (
                ClipboardUseOutcome::HistoryRecordFailed,
                "historyRecordFailed",
            ),
            (ClipboardUseOutcome::Busy, "busy"),
        ] {
            let serialized = serde_json::to_string(&ClipboardUseResult::stopped(outcome)).unwrap();
            assert_eq!(
                serialized,
                format!(r#"{{"outcome":"{expected}","failureReason":null}}"#)
            );
        }
    }

    #[test]
    fn cancelled_queued_effect_can_never_start_later() {
        let gate = AtomicU8::new(EFFECT_TASK_PENDING);
        assert!(cancel_pending_effect_task(&gate));
        assert_eq!(gate.load(Ordering::Acquire), EFFECT_TASK_CANCELLED);
        assert!(!claim_effect_task(&gate));
    }

    #[test]
    fn running_effect_cannot_be_reported_as_cancelled() {
        let gate = AtomicU8::new(EFFECT_TASK_PENDING);
        assert!(claim_effect_task(&gate));
        assert_eq!(gate.load(Ordering::Acquire), EFFECT_TASK_RUNNING);
        assert!(!cancel_pending_effect_task(&gate));
        gate.store(EFFECT_TASK_FINISHED, Ordering::Release);
        assert_eq!(gate.load(Ordering::Acquire), EFFECT_TASK_FINISHED);
    }
}
