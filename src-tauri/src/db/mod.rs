mod migrations;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{SecondsFormat, Utc};
use rusqlite::{
    params, params_from_iter,
    types::{Type, Value},
    Connection, OptionalExtension, Row, TransactionBehavior,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::actions::{
    apply_local_text_pipeline, build_cloud_preview, build_local_audit, ActionAuditRecord,
    ActionAuditStatus, ActionExecutionMode, CloudActionPreview, CreateLocalTextAction,
    LocalTextAction, LocalTextActionPreview, LocalTextPipelineStep, LocalTextTransform,
    TextActionExecution,
};
use crate::automation::runtime::AutomationPreferences;
use crate::clipboard::image_capture::{store_captured_image, CapturedImage};
use crate::clipboard::policy::{
    evaluate_capture, looks_sensitive, CaptureDecision, CapturePreferences, PasteBehavior,
    SensitiveContentPolicy,
};
use crate::clipboard::{classify_text, max_bytes_for_kind, normalize_text_for_storage};
use crate::context::semantic::{
    LocalLexicalFallbackAdapter, SemanticSearchAdapter, SemanticSearchDocument,
    SemanticSearchQuery, SEMANTIC_SEARCH_SCHEMA_VERSION,
};
use crate::context::{enrich_text, ContextEnrichment, TextRange};
use crate::error::{AppError, AppResult};
use crate::media::{BlobInput, BlobKind, BlobStore, ImageDimensions, StoredBlob};
use crate::models::{
    CaptureStatusEvent, CaptureStatusReason, ClipboardCleanupPreview,
    ClipboardCleanupRepresentative, ClipboardCleanupRequest, ClipboardCleanupResult,
    ClipboardCleanupScope, ClipboardCollection, ClipboardCollectionKind,
    ClipboardCollectionMutationResult, ClipboardFilterOptions, ClipboardImageTextExtraction,
    ClipboardImageTextExtractionResult, ClipboardImageTextExtractionStatus, ClipboardItem,
    ClipboardItemNote, ClipboardItemNoteMutationResult, ClipboardItemNoteMutationStatus,
    ClipboardListFilters, ClipboardOccurrence, ClipboardPinFilter, ClipboardSmartCollectionInput,
    ClipboardSmartCollectionRule, ClipboardTimeFilter, LocalDiagnosticErrorCategory,
    LocalDiagnosticEventInput, LocalDiagnosticEventType, LocalDiagnosticOutcome,
    LocalDiagnosticRate, LocalDiagnosticRecordResult, LocalDiagnosticsExport,
    LocalDiagnosticsExportEvent, LocalDiagnosticsSummary, LocalLinkClipboardPayload,
    RecycleBinItem, RecycleBinRetention,
};

pub struct Database {
    connection: Mutex<Connection>,
    blob_store: BlobStore,
    ephemeral_blob_root: Option<PathBuf>,
}

impl Drop for Database {
    fn drop(&mut self) {
        if let Some(root) = &self.ephemeral_blob_root {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

/// A locally ranked clipboard item returned by the built-in lexical fallback.
///
/// The result shape intentionally keeps the rank evidence alongside the source
/// clip so the UI can disclose that this is not embedding-backed retrieval.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSemanticSearchItem {
    pub item: ClipboardItem,
    pub score: f32,
    pub matched_terms: Vec<String>,
    pub highlight_ranges: Vec<TextRange>,
}

/// The deterministic local evidence used by Quick Paste's high-frequency
/// search. It is deliberately small and explainable: no embeddings, remote
/// providers, or opaque score are involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QuickPasteMatchKind {
    Suggestion,
    Exact,
    Prefix,
    Contains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QuickPasteMatchField {
    Content,
    DisplayName,
    SourceApp,
    Tag,
    Note,
    ImageText,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPasteSearchItem {
    pub item: ClipboardItem,
    pub match_kind: QuickPasteMatchKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_field: Option<QuickPasteMatchField>,
}

/// The durable, content-free lifecycle of a claimed macOS pasteboard write.
/// `Prepared` is intentionally recoverable without replaying the plaintext:
/// startup code compares the persisted random marker with the current native
/// pasteboard marker, then moves the claim to one of the two terminal states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalLinkCopyEffectState {
    Prepared,
    Completed,
    Uncertain,
}

impl LocalLinkCopyEffectState {
    fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "prepared" => Some(Self::Prepared),
            "completed" => Some(Self::Completed),
            "uncertain" => Some(Self::Uncertain),
            _ => None,
        }
    }
}

/// Recovery metadata for one Copy claim. The random token is supplied by the
/// native caller and is unrelated to a transfer, peer, body, or content hash.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LocalLinkCopyEffectClaim {
    pub transfer_id: String,
    pub state: LocalLinkCopyEffectState,
    pub effect_token: String,
    pub pasteboard_change_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl std::fmt::Debug for LocalLinkCopyEffectClaim {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalLinkCopyEffectClaim")
            .field("transfer_id", &self.transfer_id)
            .field("state", &self.state)
            .field("effect_token", &"[redacted]")
            .field("pasteboard_change_count", &self.pasteboard_change_count)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalLinkCopyClaimOutcome {
    Claimed(LocalLinkCopyEffectClaim),
    Existing(LocalLinkCopyEffectClaim),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalLinkTerminalCasOutcome {
    Applied,
    AlreadyApplied,
}

#[derive(Debug, Clone)]
pub(crate) enum LocalLinkSaveFinalization {
    Saved(Box<ClipboardItem>),
    AlreadySaved,
}

// The default retention limit is 5,000 records. Keep the offline fallback
// bounded above that value so a custom large history cannot make a keystroke
// allocate an unbounded amount of memory.
const LOCAL_SEMANTIC_CANDIDATE_LIMIT: u32 = 10_000;
const RECENT_UNCAPTURED_EVENT_LIMIT: i64 = 20;
const RECENT_UNCAPTURED_EVENT_RETENTION_DAYS: i64 = 7;
const RECYCLE_BIN_MAX_RETENTION_DAYS: i64 = 7;
const STRICT_RECYCLE_BIN_MAX_RETENTION_DAYS: i64 = 1;
const CLEANUP_PREVIEW_REPRESENTATIVE_LIMIT: usize = 3;
const LOCAL_DIAGNOSTIC_EVENT_LIMIT: i64 = 2_000;
const LOCAL_DIAGNOSTIC_RETENTION_DAYS: i64 = 90;
const MAX_CLIPBOARD_TAGS: usize = 8;
const MAX_CLIPBOARD_TAG_CHARACTERS: usize = 24;
const MAX_SMART_COLLECTIONS: u32 = 20;
const MAX_CLIPBOARD_NOTE_BYTES: usize = 16 * 1024;
const LOCAL_DIAGNOSTICS_SCHEMA_VERSION: u32 = 1;
const SENSITIVE_STRICT_PAUSE_REASON: &str =
    "Sensitive content detected. Capture is paused until you resume it.";
// Zero-input suggestions are bounded; nonempty search prefilters across the
// full retained history before applying the deterministic Rust rank.
const QUICK_PASTE_CANDIDATE_LIMIT: u32 = 10_000;

struct RepresentedItem<'a> {
    content: &'a str,
    search_text: &'a str,
    kind: &'a str,
    source_app: Option<&'a str>,
    captured_at: &'a str,
    representations: &'a [StoredBlob],
}

#[derive(Debug, Clone)]
struct CleanupCandidate {
    id: String,
    kind: String,
    source_app: Option<String>,
    captured_at: String,
    retention_until: String,
    is_pinned: bool,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        let (blob_root, ephemeral_blob_root) = blob_root_for_database(path);
        let blob_store =
            BlobStore::open(&blob_root).map_err(|error| AppError::Media(error.to_string()))?;
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )?;

        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version < 1 {
            connection.execute_batch(migrations::MIGRATION_1)?;
        }
        if version < 2 {
            connection.execute_batch(migrations::MIGRATION_2)?;
        }
        if version < 3 {
            connection.execute_batch(migrations::MIGRATION_3)?;
        }
        if version < 4 {
            connection.execute_batch(migrations::MIGRATION_4)?;
        }
        if version < 5 {
            connection.execute_batch(migrations::MIGRATION_5)?;
        }
        if version < 6 {
            connection.execute_batch(migrations::MIGRATION_6)?;
        }
        if version < 7 {
            connection.execute_batch(migrations::MIGRATION_7)?;
        }
        if version < 8 {
            connection.execute_batch(migrations::MIGRATION_8)?;
        }
        if version < 9 {
            connection.execute_batch(migrations::MIGRATION_9)?;
        }
        if version < 10 {
            connection.execute_batch(migrations::MIGRATION_10)?;
        }
        if version < 11 {
            connection.execute_batch(migrations::MIGRATION_11)?;
        }
        if version < 12 {
            connection.execute_batch(migrations::MIGRATION_12)?;
        }
        if version < 13 {
            connection.execute_batch(migrations::MIGRATION_13)?;
        }
        if version < 14 {
            connection.execute_batch(migrations::MIGRATION_14)?;
        }
        if version < 15 {
            connection.execute_batch(migrations::MIGRATION_15)?;
        }
        if version < 16 {
            connection.execute_batch(migrations::MIGRATION_16)?;
        }
        if version < 17 {
            connection.execute_batch(migrations::MIGRATION_17)?;
        }
        if version < 18 {
            connection.execute_batch(migrations::MIGRATION_18)?;
        }
        if version < 19 {
            connection.execute_batch(migrations::MIGRATION_19)?;
        }
        if version < 20 {
            connection.execute_batch(migrations::MIGRATION_20)?;
        }
        if version < 21 {
            connection.execute_batch(migrations::MIGRATION_21)?;
        }
        if version < 22 {
            connection.execute_batch(migrations::MIGRATION_22)?;
        }
        if version < 23 {
            connection.execute_batch(migrations::MIGRATION_23)?;
        }
        if version < 24 {
            connection.execute_batch(migrations::MIGRATION_24)?;
        }
        if version < 25 {
            connection.execute_batch(migrations::MIGRATION_25)?;
        }
        if version < 26 {
            connection.execute_batch(migrations::MIGRATION_26)?;
        }

        let database = Self {
            connection: Mutex::new(connection),
            blob_store,
            ephemeral_blob_root,
        };
        database.enforce_retention_policy()?;
        database.purge_expired_recycle_bin()?;
        Ok(database)
    }

    /// Stores a policy-approved image after it has been encoded locally. No
    /// blob is written until pause and deny-list rules have been checked.
    pub fn capture_image(
        &self,
        image: &CapturedImage,
        source_app: Option<&str>,
    ) -> AppResult<Option<ClipboardItem>> {
        let timestamp = timestamp();
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        // There is no automatic OCR in the capture path; manual OCR happens
        // later. Keep this placeholder in the capture policy gate so image
        // storage retains Strict-policy pause semantics.
        if !Self::capture_allowed_with_connection(
            &connection,
            &preferences,
            "[image clipboard capture]",
            source_app,
            &timestamp,
        )? {
            return Ok(None);
        }

        let stored = store_captured_image(&self.blob_store, image)
            .map_err(|error| AppError::Media(error.to_string()))?;
        let result = (|| {
            let id =
                Self::upsert_image_with_connection(&connection, &stored, source_app, &timestamp)?;
            let preferences = Self::capture_preferences_with_connection(&connection)?;
            self.enforce_retention_policy_with_connection(&connection, &preferences)?;
            self.purge_expired_recycle_bin_with_connection(&connection)?;
            Self::get_by_id_with_connection(&connection, &id).map(Some)
        })();
        if result.is_err() {
            let _ =
                self.cleanup_unreferenced_blob_with_connection(&connection, &stored.storage_key);
        }
        result
    }

    pub fn read_blob(&self, storage_key: &str) -> AppResult<Vec<u8>> {
        self.blob_store
            .read(storage_key)
            .map_err(|error| AppError::Media(error.to_string()))
    }

    pub fn capture_rich_text(
        &self,
        content: &str,
        representations: &[(String, Vec<u8>)],
        source_app: Option<&str>,
    ) -> AppResult<Option<ClipboardItem>> {
        if content.trim().is_empty() && representations.is_empty() {
            return Ok(None);
        }

        let timestamp = timestamp();
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        if !Self::capture_allowed_with_connection(
            &connection,
            &preferences,
            content,
            source_app,
            &timestamp,
        )? {
            return Ok(None);
        }
        if content.trim().is_empty() || content.len() > 1_000_000 || representations.is_empty() {
            Self::record_capture_status_event_with_connection(
                &connection,
                CaptureStatusReason::UnsupportedFormat,
                source_app,
                &timestamp,
            )?;
            return Ok(None);
        }

        let mut stored = Vec::with_capacity(representations.len());
        for (mime_type, bytes) in representations {
            let representation = self
                .blob_store
                .store(BlobInput {
                    kind: BlobKind::RichText,
                    mime_type,
                    display_name: None,
                    image_dimensions: None,
                    bytes,
                })
                .map_err(|error| AppError::Media(error.to_string()))?;
            stored.push(representation);
        }

        let result = self.persist_represented_item(
            &connection,
            &preferences,
            RepresentedItem {
                content,
                search_text: content,
                kind: "richText",
                source_app,
                captured_at: &timestamp,
                representations: &stored,
            },
        );
        if result.is_err() {
            for representation in &stored {
                let _ = self.cleanup_unreferenced_blob_with_connection(
                    &connection,
                    &representation.storage_key,
                );
            }
        }
        result
    }

    pub fn capture_file_references(
        &self,
        paths: &[String],
        source_app: Option<&str>,
    ) -> AppResult<Option<ClipboardItem>> {
        let paths = paths
            .iter()
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            self.record_unsupported_capture_format(source_app)?;
            return Ok(None);
        }

        let preview = file_reference_preview(&paths);
        let search_text = file_reference_search_text(&paths);
        let timestamp = timestamp();
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        if !Self::capture_allowed_with_connection(
            &connection,
            &preferences,
            &preview,
            source_app,
            &timestamp,
        )? {
            return Ok(None);
        }

        let bytes = serde_json::to_vec(&paths)?;
        let display_name = if paths.len() == 1 {
            std::path::Path::new(paths[0])
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("File")
                .to_owned()
        } else {
            format!("{} files", paths.len())
        };
        let stored = self
            .blob_store
            .store(BlobInput {
                kind: BlobKind::File,
                mime_type: "application/x-clipriva-file-list+json",
                display_name: Some(&display_name),
                image_dimensions: None,
                bytes: &bytes,
            })
            .map_err(|error| AppError::Media(error.to_string()))?;

        let result = self.persist_represented_item(
            &connection,
            &preferences,
            RepresentedItem {
                content: &preview,
                search_text: &search_text,
                kind: "file",
                source_app,
                captured_at: &timestamp,
                representations: std::slice::from_ref(&stored),
            },
        );
        if result.is_err() {
            let _ =
                self.cleanup_unreferenced_blob_with_connection(&connection, &stored.storage_key);
        }
        result
    }

    pub fn capture_text(
        &self,
        content: &str,
        source_app: Option<&str>,
    ) -> AppResult<Option<ClipboardItem>> {
        if content.trim().is_empty() {
            return Ok(None);
        }
        let timestamp = timestamp();
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;

        if !Self::capture_allowed_with_connection(
            &connection,
            &preferences,
            content,
            source_app,
            &timestamp,
        )? {
            return Ok(None);
        }

        let kind = classify_text(content);
        let byte_limit = max_bytes_for_kind(kind);
        if content.len() > byte_limit {
            Self::record_capture_status_event_with_connection(
                &connection,
                CaptureStatusReason::ContentTooLarge,
                source_app,
                &timestamp,
            )?;
            return Err(AppError::InvalidInput(format!(
                "Clipboard {} is too large ({} bytes). The local {} limit is {} KiB, so it was not saved.",
                content_kind_label(kind),
                content.len(),
                content_kind_label(kind),
                byte_limit / 1024,
            )));
        }

        let content = normalize_text_for_storage(content, kind);
        if content.is_empty() {
            return Ok(None);
        }
        let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

        let id = Self::upsert_exact_clipboard_item_with_connection(
            &connection,
            &preferences,
            &content,
            &content,
            &content_hash,
            kind,
            source_app,
            &timestamp,
        )?;

        if preferences.labs_enabled {
            Self::upsert_context_enrichment(&connection, &id, &content, &timestamp)?;
        }
        self.enforce_retention_policy_with_connection(&connection, &preferences)?;
        self.purge_expired_recycle_bin_with_connection(&connection)?;
        Self::get_by_id_with_connection(&connection, &id).map(Some)
    }

    /// Saves one accepted Local Link body and finalizes its incoming transfer
    /// in the same SQLite transaction. Re-entering a transfer that is already
    /// `saved` is a no-op, so an acknowledgement retry cannot create another
    /// history occurrence. No clipboard-item identifier is copied into the
    /// Local Link transfer or effect-claim tables.
    pub(crate) fn finalize_local_link_save(
        &self,
        transfer_id: &str,
        peer_device_id: &str,
        content: &str,
    ) -> AppResult<LocalLinkSaveFinalization> {
        if content.is_empty() {
            return Err(AppError::InvalidInput(
                "The pending Local Link text is empty.".to_owned(),
            ));
        }
        let kind = classify_text(content);
        if content.len() > max_bytes_for_kind(kind) {
            return Err(AppError::InvalidInput(format!(
                "Received {} exceeds the local {} KiB limit and was not saved.",
                content_kind_label(kind),
                max_bytes_for_kind(kind) / 1024,
            )));
        }
        let content = normalize_text_for_storage(content, kind);
        if content.is_empty() {
            return Err(AppError::InvalidInput(
                "The pending Local Link text is empty after normalization.".to_owned(),
            ));
        }

        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (stored_peer_id, direction, status, receiver_action) = transaction
            .query_row(
                "SELECT device_id, direction, status, receiver_action
                 FROM local_link_transfers WHERE id = ?1",
                [transfer_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        if stored_peer_id != peer_device_id || direction != "incoming" {
            return Err(AppError::InvalidInput(
                "The Local Link transfer does not belong to this incoming peer.".to_owned(),
            ));
        }
        if status == "saved" && receiver_action.as_deref() == Some("save") {
            return Ok(LocalLinkSaveFinalization::AlreadySaved);
        }
        if !is_local_link_receiver_active_status(&status) {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already finalized.".to_owned(),
            ));
        }
        let has_effect_claim: bool = transaction.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM local_link_effect_claims WHERE transfer_id = ?1
             )",
            [transfer_id],
            |row| row.get(0),
        )?;
        if has_effect_claim {
            return Err(AppError::InvalidInput(
                "This Local Link transfer already has a receiver action claim.".to_owned(),
            ));
        }

        let display_name: String = transaction
            .query_row(
                "SELECT display_name FROM local_link_devices WHERE device_id = ?1",
                [peer_device_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)?;
        let source_app = format!("Local Link from {display_name}");
        let preferences = Self::capture_preferences_with_connection(&transaction)?;
        let saved_at = timestamp();
        let content_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
        let item_id = Self::upsert_exact_clipboard_item_with_connection(
            &transaction,
            &preferences,
            &content,
            &content,
            &content_hash,
            kind,
            Some(&source_app),
            &saved_at,
        )?;
        if preferences.labs_enabled {
            Self::upsert_context_enrichment(&transaction, &item_id, &content, &saved_at)?;
        }

        let changed = transaction.execute(
            "UPDATE local_link_transfers
             SET status = 'saved', receiver_action = 'save', failure_reason = NULL,
                 updated_at = ?1, completed_at = ?1, expires_at = NULL
             WHERE id = ?2 AND direction = 'incoming'
               AND status IN ('awaitingReceiver', 'viewed')
               AND NOT EXISTS (
                   SELECT 1 FROM local_link_effect_claims WHERE transfer_id = ?2
               )",
            params![saved_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already claimed or finalized.".to_owned(),
            ));
        }
        self.enforce_retention_policy_with_connection(&transaction, &preferences)?;
        let item = Self::get_by_id_with_connection(&transaction, &item_id)?;
        transaction.commit()?;
        Ok(LocalLinkSaveFinalization::Saved(Box::new(item)))
    }

    /// Claims the single allowed Copy side effect using a caller-generated
    /// random marker. The insert and active-transfer check are serialized by
    /// an IMMEDIATE transaction. Existing claims are returned for recovery;
    /// the newly supplied token never replaces their durable marker.
    pub(crate) fn prepare_local_link_copy_effect(
        &self,
        transfer_id: &str,
        effect_token: &str,
    ) -> AppResult<LocalLinkCopyClaimOutcome> {
        validate_local_link_effect_token(effect_token)?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        if let Some(existing) =
            local_link_copy_effect_claim_with_connection(&transaction, transfer_id)?
        {
            return Ok(LocalLinkCopyClaimOutcome::Existing(existing));
        }
        let (direction, status): (String, String) = transaction
            .query_row(
                "SELECT direction, status FROM local_link_transfers WHERE id = ?1",
                [transfer_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        if direction != "incoming" || !is_local_link_receiver_active_status(&status) {
            return Err(AppError::InvalidInput(
                "This Local Link transfer is not available for Copy.".to_owned(),
            ));
        }
        let prepared_at = timestamp();
        transaction.execute(
            "INSERT INTO local_link_effect_claims
             (transfer_id, action, state, effect_token, pasteboard_change_count,
              created_at, updated_at)
             VALUES (?1, 'copy', 'prepared', ?2, NULL, ?3, ?3)",
            params![transfer_id, effect_token, prepared_at],
        )?;
        let claim = local_link_copy_effect_claim_with_connection(&transaction, transfer_id)?
            .ok_or(AppError::NotFound)?;
        transaction.commit()?;
        Ok(LocalLinkCopyClaimOutcome::Claimed(claim))
    }

    #[cfg(test)]
    pub(crate) fn local_link_copy_effect_claim(
        &self,
        transfer_id: &str,
    ) -> AppResult<Option<LocalLinkCopyEffectClaim>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        local_link_copy_effect_claim_with_connection(&connection, transfer_id)
    }

    /// Returns a bounded oldest-first list of prepared claims that startup
    /// recovery must resolve from the native pasteboard marker. Plaintext is
    /// neither selected nor reachable through this query.
    pub(crate) fn local_link_copy_effect_claims_for_recovery(
        &self,
        limit: u32,
    ) -> AppResult<Vec<LocalLinkCopyEffectClaim>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT transfer_id, state, effect_token, pasteboard_change_count,
                    created_at, updated_at
             FROM local_link_effect_claims
             WHERE state = 'prepared'
             ORDER BY updated_at ASC, transfer_id ASC
             LIMIT ?1",
        )?;
        let claims = statement
            .query_map(
                [i64::from(limit.clamp(1, 1_000))],
                map_local_link_copy_effect_claim,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(claims)
    }

    /// Atomically marks both the pasteboard effect and transfer as completed.
    /// This method must only be called after the native write containing the
    /// same random marker succeeds. It never performs or retries that write.
    pub(crate) fn complete_local_link_copy_effect(
        &self,
        transfer_id: &str,
        effect_token: &str,
        pasteboard_change_count: i64,
    ) -> AppResult<LocalLinkTerminalCasOutcome> {
        validate_local_link_effect_token(effect_token)?;
        if pasteboard_change_count < 0 {
            return Err(AppError::InvalidInput(
                "Pasteboard change count cannot be negative.".to_owned(),
            ));
        }
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let claim = local_link_copy_effect_claim_with_connection(&transaction, transfer_id)?
            .ok_or(AppError::NotFound)?;
        if claim.effect_token != effect_token {
            return Err(AppError::InvalidInput(
                "The Local Link Copy marker does not match its durable claim.".to_owned(),
            ));
        }
        if claim.state == LocalLinkCopyEffectState::Completed {
            let copied: bool = transaction.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM local_link_transfers
                     WHERE id = ?1 AND status = 'copied' AND receiver_action = 'copy'
                 )",
                [transfer_id],
                |row| row.get(0),
            )?;
            return if copied {
                Ok(LocalLinkTerminalCasOutcome::AlreadyApplied)
            } else {
                Err(AppError::InvalidInput(
                    "The Local Link Copy claim has inconsistent terminal metadata.".to_owned(),
                ))
            };
        }
        if claim.state == LocalLinkCopyEffectState::Uncertain {
            return Err(AppError::InvalidInput(
                "The Local Link Copy outcome is already uncertain.".to_owned(),
            ));
        }

        let completed_at = timestamp();
        let changed = transaction.execute(
            "UPDATE local_link_transfers
             SET status = 'copied', receiver_action = 'copy', failure_reason = NULL,
                 updated_at = ?1, completed_at = ?1, expires_at = NULL
             WHERE id = ?2 AND direction = 'incoming'
               AND status IN ('awaitingReceiver', 'viewed')",
            params![completed_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already finalized.".to_owned(),
            ));
        }
        let changed = transaction.execute(
            "UPDATE local_link_effect_claims
             SET state = 'completed', pasteboard_change_count = ?1, updated_at = ?2
             WHERE transfer_id = ?3 AND state = 'prepared' AND effect_token = ?4",
            params![
                pasteboard_change_count,
                completed_at,
                transfer_id,
                effect_token
            ],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "The Local Link Copy claim was already resolved.".to_owned(),
            ));
        }
        transaction.commit()?;
        Ok(LocalLinkTerminalCasOutcome::Applied)
    }

    /// Resolves a prepared Copy after recovery cannot prove that its native
    /// marker still owns the pasteboard. The side effect is never replayed;
    /// instead both rows atomically become truthful terminal uncertainty.
    pub(crate) fn mark_local_link_copy_effect_uncertain(
        &self,
        transfer_id: &str,
        effect_token: &str,
    ) -> AppResult<LocalLinkTerminalCasOutcome> {
        validate_local_link_effect_token(effect_token)?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let claim = local_link_copy_effect_claim_with_connection(&transaction, transfer_id)?
            .ok_or(AppError::NotFound)?;
        if claim.effect_token != effect_token {
            return Err(AppError::InvalidInput(
                "The Local Link Copy marker does not match its durable claim.".to_owned(),
            ));
        }
        if claim.state == LocalLinkCopyEffectState::Uncertain {
            let unknown: bool = transaction.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM local_link_transfers
                     WHERE id = ?1 AND status = 'failed'
                       AND failure_reason = 'outcomeUnknown'
                 )",
                [transfer_id],
                |row| row.get(0),
            )?;
            return if unknown {
                Ok(LocalLinkTerminalCasOutcome::AlreadyApplied)
            } else {
                Err(AppError::InvalidInput(
                    "The Local Link Copy claim has inconsistent terminal metadata.".to_owned(),
                ))
            };
        }
        if claim.state == LocalLinkCopyEffectState::Completed {
            return Err(AppError::InvalidInput(
                "The Local Link Copy effect is already complete.".to_owned(),
            ));
        }

        let resolved_at = timestamp();
        let changed = transaction.execute(
            "UPDATE local_link_transfers
             SET status = 'failed', receiver_action = 'copy',
                 failure_reason = 'outcomeUnknown', updated_at = ?1,
                 completed_at = ?1, expires_at = NULL
             WHERE id = ?2 AND direction = 'incoming'
               AND status IN ('awaitingReceiver', 'viewed')",
            params![resolved_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already finalized.".to_owned(),
            ));
        }
        let changed = transaction.execute(
            "UPDATE local_link_effect_claims
             SET state = 'uncertain', updated_at = ?1
             WHERE transfer_id = ?2 AND state = 'prepared' AND effect_token = ?3",
            params![resolved_at, transfer_id, effect_token],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "The Local Link Copy claim was already resolved.".to_owned(),
            ));
        }
        transaction.commit()?;
        Ok(LocalLinkTerminalCasOutcome::Applied)
    }

    /// Reject wins only while no Copy effect claim exists. The caller can
    /// safely drop the in-memory body after this CAS returns `Applied` or
    /// `AlreadyApplied`; a database error leaves the transfer pending.
    pub(crate) fn finalize_local_link_reject(
        &self,
        transfer_id: &str,
    ) -> AppResult<LocalLinkTerminalCasOutcome> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (direction, status, action): (String, String, Option<String>) = transaction
            .query_row(
                "SELECT direction, status, receiver_action
                 FROM local_link_transfers WHERE id = ?1",
                [transfer_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        if direction != "incoming" {
            return Err(AppError::InvalidInput(
                "Only an incoming Local Link transfer can be rejected.".to_owned(),
            ));
        }
        if status == "rejected" && action.as_deref() == Some("reject") {
            return Ok(LocalLinkTerminalCasOutcome::AlreadyApplied);
        }
        if !is_local_link_receiver_active_status(&status) {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already finalized.".to_owned(),
            ));
        }
        let rejected_at = timestamp();
        let changed = transaction.execute(
            "UPDATE local_link_transfers
             SET status = 'rejected', receiver_action = 'reject', failure_reason = NULL,
                 updated_at = ?1, completed_at = ?1, expires_at = NULL
             WHERE id = ?2 AND direction = 'incoming'
               AND status IN ('awaitingReceiver', 'viewed')
               AND NOT EXISTS (
                   SELECT 1 FROM local_link_effect_claims WHERE transfer_id = ?2
               )",
            params![rejected_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer already has a receiver action claim.".to_owned(),
            ));
        }
        transaction.commit()?;
        Ok(LocalLinkTerminalCasOutcome::Applied)
    }

    /// Persists a sender's ambiguous post-payload state. This transition has
    /// no body or retry material; it authorizes status reconciliation only.
    pub(crate) fn mark_local_link_transfer_reconciling(
        &self,
        transfer_id: &str,
        expires_at: &str,
    ) -> AppResult<LocalLinkTerminalCasOutcome> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let status: Option<(String, String)> = connection
            .query_row(
                "SELECT direction, status FROM local_link_transfers WHERE id = ?1",
                [transfer_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (direction, status) = status.ok_or(AppError::NotFound)?;
        if direction != "outgoing" {
            return Err(AppError::InvalidInput(
                "Only an outgoing Local Link transfer can reconcile delivery.".to_owned(),
            ));
        }
        if status == "reconciling" {
            return Ok(LocalLinkTerminalCasOutcome::AlreadyApplied);
        }
        let changed = connection.execute(
            "UPDATE local_link_transfers
             SET status = 'reconciling', failure_reason = NULL,
                 updated_at = ?1, completed_at = NULL, expires_at = ?2
             WHERE id = ?3 AND direction = 'outgoing'
               AND status IN ('connecting', 'encrypted', 'awaitingReceiver', 'viewed')",
            params![timestamp(), expires_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer was already finalized.".to_owned(),
            ));
        }
        Ok(LocalLinkTerminalCasOutcome::Applied)
    }

    /// Ends a bounded sender reconciliation without inventing a delivery
    /// result. The transfer remains content-free and cannot be replayed.
    pub(crate) fn finalize_local_link_outcome_unknown(
        &self,
        transfer_id: &str,
    ) -> AppResult<LocalLinkTerminalCasOutcome> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let state: Option<(String, String, Option<String>)> = connection
            .query_row(
                "SELECT direction, status, failure_reason
                 FROM local_link_transfers WHERE id = ?1",
                [transfer_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (direction, status, failure_reason) = state.ok_or(AppError::NotFound)?;
        if direction != "outgoing" {
            return Err(AppError::InvalidInput(
                "Only an outgoing Local Link transfer can finish reconciliation.".to_owned(),
            ));
        }
        if status == "failed" && failure_reason.as_deref() == Some("outcomeUnknown") {
            return Ok(LocalLinkTerminalCasOutcome::AlreadyApplied);
        }
        let resolved_at = timestamp();
        let changed = connection.execute(
            "UPDATE local_link_transfers
             SET status = 'failed', failure_reason = 'outcomeUnknown',
                 updated_at = ?1, completed_at = ?1, expires_at = NULL
             WHERE id = ?2 AND direction = 'outgoing' AND status = 'reconciling'",
            params![resolved_at, transfer_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidInput(
                "This Local Link transfer is not awaiting status reconciliation.".to_owned(),
            ));
        }
        Ok(LocalLinkTerminalCasOutcome::Applied)
    }

    fn upsert_image_with_connection(
        connection: &Connection,
        representation: &StoredBlob,
        source_app: Option<&str>,
        captured_at: &str,
    ) -> AppResult<String> {
        let dimensions = representation.image_dimensions;
        let preview = image_preview(dimensions);
        let preferences = Self::capture_preferences_with_connection(connection)?;
        let id = Self::upsert_exact_clipboard_item_with_connection(
            connection,
            &preferences,
            &preview,
            &preview,
            &representation.content_hash,
            "image",
            source_app,
            captured_at,
        )?;

        connection.execute(
            "INSERT INTO clipboard_representations
             (clipboard_item_id, storage_key, content_hash, kind, mime_type, display_name,
              image_width, image_height, byte_size, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(clipboard_item_id, storage_key) DO UPDATE SET
                 mime_type = excluded.mime_type,
                 display_name = excluded.display_name,
                 image_width = excluded.image_width,
                 image_height = excluded.image_height,
                 byte_size = excluded.byte_size",
            params![
                id,
                representation.storage_key,
                representation.content_hash,
                representation.kind.as_str(),
                representation.mime_type,
                representation.display_name,
                dimensions.map(|value| value.width),
                dimensions.map(|value| value.height),
                representation.byte_size,
                captured_at,
            ],
        )?;
        if preferences.labs_enabled {
            Self::upsert_context_enrichment(connection, &id, &preview, captured_at)?;
        }
        Ok(id)
    }

    fn capture_allowed_with_connection(
        connection: &Connection,
        preferences: &CapturePreferences,
        content: &str,
        source_app: Option<&str>,
        captured_at: &str,
    ) -> AppResult<bool> {
        match evaluate_capture(preferences, content, source_app) {
            CaptureDecision::Capture => Ok(true),
            CaptureDecision::Paused => {
                let reason =
                    if preferences.pause_reason.as_deref() == Some(SENSITIVE_STRICT_PAUSE_REASON) {
                        CaptureStatusReason::SensitiveContentStrict
                    } else {
                        CaptureStatusReason::ManualPause
                    };
                Self::record_capture_status_event_with_connection(
                    connection,
                    reason,
                    source_app,
                    captured_at,
                )?;
                Ok(false)
            }
            CaptureDecision::DeniedApp => {
                Self::record_capture_status_event_with_connection(
                    connection,
                    CaptureStatusReason::ExcludedApplication,
                    source_app,
                    captured_at,
                )?;
                Ok(false)
            }
            CaptureDecision::SensitiveContent => {
                Self::record_capture_status_event_with_connection(
                    connection,
                    CaptureStatusReason::SensitiveContentDefault,
                    source_app,
                    captured_at,
                )?;
                Ok(false)
            }
            CaptureDecision::SensitiveContentStrict => {
                connection.execute(
                    "UPDATE capture_preferences
                     SET capture_paused = 1,
                         pause_reason = ?1,
                         updated_at = ?2
                     WHERE singleton = 1",
                    params![SENSITIVE_STRICT_PAUSE_REASON, captured_at],
                )?;
                Self::record_capture_status_event_with_connection(
                    connection,
                    CaptureStatusReason::SensitiveContentStrict,
                    source_app,
                    captured_at,
                )?;
                Ok(false)
            }
        }
    }

    pub(crate) fn record_unsupported_capture_format(
        &self,
        source_app: Option<&str>,
    ) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let occurred_at = timestamp();
        Self::record_capture_status_event_with_connection(
            &connection,
            CaptureStatusReason::UnsupportedFormat,
            source_app,
            &occurred_at,
        )
    }

    fn record_capture_status_event_with_connection(
        connection: &Connection,
        reason_type: CaptureStatusReason,
        source_app: Option<&str>,
        occurred_at: &str,
    ) -> AppResult<()> {
        Self::purge_expired_capture_status_events_with_connection(connection)?;
        let expires_at = (Utc::now()
            + chrono::Duration::days(RECENT_UNCAPTURED_EVENT_RETENTION_DAYS))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
        connection.execute(
            "INSERT INTO uncaptured_clipboard_events
             (id, reason_type, source_app, occurred_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                reason_type.as_storage_value(),
                source_app,
                occurred_at,
                expires_at,
            ],
        )?;
        connection.execute(
            "DELETE FROM uncaptured_clipboard_events
             WHERE id IN (
                 SELECT id FROM uncaptured_clipboard_events
                 ORDER BY occurred_at DESC, id DESC
                 LIMIT -1 OFFSET ?1
             )",
            [RECENT_UNCAPTURED_EVENT_LIMIT],
        )?;
        Ok(())
    }

    fn purge_expired_capture_status_events_with_connection(
        connection: &Connection,
    ) -> AppResult<()> {
        connection.execute(
            "DELETE FROM uncaptured_clipboard_events WHERE expires_at <= ?1",
            [timestamp()],
        )?;
        Ok(())
    }

    fn ensure_labs_enabled_with_connection(connection: &Connection) -> AppResult<()> {
        if Self::capture_preferences_with_connection(connection)?.labs_enabled {
            return Ok(());
        }
        Err(AppError::InvalidInput(
            "ClipRiva Labs is disabled. Enable it in Settings to use experimental features."
                .to_owned(),
        ))
    }

    fn persist_represented_item(
        &self,
        connection: &Connection,
        preferences: &CapturePreferences,
        item: RepresentedItem<'_>,
    ) -> AppResult<Option<ClipboardItem>> {
        let RepresentedItem {
            content,
            search_text,
            kind,
            source_app,
            captured_at,
            representations,
        } = item;
        let content_hash = if is_textual_item_kind(kind) {
            format!("{:x}", Sha256::digest(content.as_bytes()))
        } else {
            represented_content_hash(kind, content, representations)
        };
        let id = Self::upsert_exact_clipboard_item_with_connection(
            connection,
            preferences,
            content,
            search_text,
            &content_hash,
            kind,
            source_app,
            captured_at,
        )?;

        for representation in representations {
            let dimensions = representation.image_dimensions;
            connection.execute(
                "INSERT INTO clipboard_representations
                 (clipboard_item_id, storage_key, content_hash, kind, mime_type, display_name,
                  image_width, image_height, byte_size, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(clipboard_item_id, storage_key) DO UPDATE SET
                     mime_type = excluded.mime_type,
                     display_name = excluded.display_name,
                     image_width = excluded.image_width,
                     image_height = excluded.image_height,
                     byte_size = excluded.byte_size",
                params![
                    id,
                    representation.storage_key,
                    representation.content_hash,
                    representation.kind.as_str(),
                    representation.mime_type,
                    representation.display_name,
                    dimensions.map(|value| value.width),
                    dimensions.map(|value| value.height),
                    representation.byte_size,
                    captured_at,
                ],
            )?;
        }

        if preferences.labs_enabled {
            Self::upsert_context_enrichment(connection, &id, content, captured_at)?;
        }
        self.enforce_retention_policy_with_connection(connection, preferences)?;
        self.purge_expired_recycle_bin_with_connection(connection)?;
        Self::get_by_id_with_connection(connection, &id).map(Some)
    }

    #[allow(clippy::too_many_arguments)]
    fn upsert_exact_clipboard_item_with_connection(
        connection: &Connection,
        preferences: &CapturePreferences,
        content: &str,
        search_text: &str,
        content_hash: &str,
        kind: &str,
        source_app: Option<&str>,
        captured_at: &str,
    ) -> AppResult<String> {
        let device_id = Self::local_device_id_with_connection(connection)?;
        // Only collapse an immediately repeated clipboard event. A later
        // repeat after another capture represents a distinct point in the
        // user's history; globally merging it makes the list appear to lose
        // history and hides its intervening context.
        let existing_id = connection
            .query_row(
                "SELECT item.id
                 FROM clipboard_occurrences AS occurrence
                 INNER JOIN clipboard_items AS item ON item.id = occurrence.clipboard_item_id
                 WHERE occurrence.rowid = (
                     SELECT rowid
                     FROM clipboard_occurrences
                     ORDER BY occurred_at DESC, rowid DESC
                     LIMIT 1
                 )
                   AND item.deleted_at IS NULL
                   AND item.content_hash = ?1
                   AND (
                     (?2 = 1 AND item.kind IN ('text', 'code', 'command', 'url', 'color', 'richText'))
                     OR (?2 = 0 AND item.kind = ?3)
                   )",
                params![content_hash, is_textual_item_kind(kind), kind],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let retention_until = retention_until_for(captured_at, preferences.retention_days);

        let id = if let Some(id) = existing_id {
            connection.execute(
                "UPDATE clipboard_items
                 SET content = ?1,
                     search_text = ?2,
                     content_hash = ?3,
                     kind = CASE
                         WHEN kind = 'richText' AND ?4 <> 'richText' THEN kind
                         ELSE ?4
                     END,
                     source_app = COALESCE(?5, source_app),
                     updated_at = ?6,
                     retention_until = ?7,
                     deleted_at = NULL,
                     recycle_expires_at = NULL,
                     restored_at = NULL,
                     group_id = NULL,
                     version = 1
                 WHERE id = ?8",
                params![
                    content,
                    search_text,
                    content_hash,
                    kind,
                    source_app,
                    captured_at,
                    retention_until,
                    id,
                ],
            )?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            connection.execute(
                "INSERT INTO clipboard_items
                 (id, global_id, device_id, content, content_hash, group_id, version, kind,
                  source_app, created_at, updated_at, retention_until, search_text)
                 VALUES (?1, ?1, ?2, ?3, ?4, NULL, 1, ?5, ?6, ?7, ?7, ?8, ?9)",
                params![
                    id,
                    device_id,
                    content,
                    content_hash,
                    kind,
                    source_app,
                    captured_at,
                    retention_until,
                    search_text,
                ],
            )?;
            id
        };

        connection.execute(
            "INSERT INTO clipboard_occurrences
             (id, clipboard_item_id, device_id, source_app, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                id,
                device_id,
                source_app,
                captured_at,
            ],
        )?;
        Ok(id)
    }

    fn local_device_id_with_connection(connection: &Connection) -> AppResult<String> {
        connection
            .query_row(
                "SELECT device_id FROM clipriva_local_identity WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn capture_preferences(&self) -> AppResult<CapturePreferences> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::capture_preferences_with_connection(&connection)
    }

    pub fn update_capture_preferences(
        &self,
        preferences: &CapturePreferences,
    ) -> AppResult<CapturePreferences> {
        let retention_days = preferences.retention_days.clamp(1, 3_650);
        let max_history_items = preferences.max_history_items.clamp(100, 100_000);
        let denied_apps =
            serde_json::to_string(&preferences.denied_apps).unwrap_or_else(|_| "[]".to_owned());
        let connection = self.connection.lock().expect("database mutex poisoned");

        connection.execute(
            "UPDATE capture_preferences
             SET capture_paused = ?1,
                 sensitive_pause_enabled = ?2,
                 sensitive_content_policy = ?3,
                 retention_days = ?4,
                 max_history_items = ?5,
                 denied_apps_json = ?6,
                 pause_reason = ?7,
                 labs_enabled = ?8,
                 diagnostics_enabled = ?9,
                 paste_behavior = ?10,
                 quick_paste_shortcut = ?11,
                 stack_shortcut = ?12,
                 onboarding_completed = ?13,
                 updated_at = ?14
             WHERE singleton = 1",
            params![
                preferences.capture_paused,
                preferences.sensitive_content_policy.is_strict(),
                preferences.sensitive_content_policy.as_storage_value(),
                retention_days,
                max_history_items,
                denied_apps,
                preferences.pause_reason,
                preferences.labs_enabled,
                preferences.diagnostics_enabled,
                preferences.paste_behavior.as_storage_value(),
                preferences.quick_paste_shortcut.trim(),
                preferences.stack_shortcut.trim(),
                preferences.onboarding_completed,
                timestamp(),
            ],
        )?;

        let updated = Self::capture_preferences_with_connection(&connection)?;
        Self::recalculate_active_retention_with_connection(&connection, updated.retention_days)?;
        self.enforce_retention_policy_with_connection(&connection, &updated)?;
        self.purge_expired_recycle_bin_with_connection(&connection)?;
        Ok(updated)
    }

    pub fn resume_capture(&self) -> AppResult<CapturePreferences> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute(
            "UPDATE capture_preferences
             SET capture_paused = 0, pause_reason = NULL, updated_at = ?1
             WHERE singleton = 1",
            [timestamp()],
        )?;
        Self::capture_preferences_with_connection(&connection)
    }

    /// Returns the most recent local explanations for skipped capture events.
    /// Expired records are removed before reading, and the schema never stores
    /// clipboard contents or hashes for these records.
    pub fn recent_capture_status_events(&self) -> AppResult<Vec<CaptureStatusEvent>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::purge_expired_capture_status_events_with_connection(&connection)?;
        let mut statement = connection.prepare(
            "SELECT id, reason_type, source_app, occurred_at, expires_at
             FROM uncaptured_clipboard_events
             ORDER BY occurred_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows =
            statement.query_map([RECENT_UNCAPTURED_EVENT_LIMIT], map_capture_status_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    /// Lets a user clear the short-lived, local status history without
    /// affecting their captured clipboard history or capture preferences.
    pub fn clear_capture_status_events(&self) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute("DELETE FROM uncaptured_clipboard_events", [])?;
        Ok(())
    }

    /// Records a deliberately bounded, local reliability signal. This command
    /// accepts only allow-listed event/result categories; content, source-app
    /// metadata, filenames, paths, clip identifiers, precise timestamps, and
    /// network identifiers have no representation in the schema or API.
    pub fn record_local_diagnostic_event(
        &self,
        event: &LocalDiagnosticEventInput,
    ) -> AppResult<LocalDiagnosticRecordResult> {
        if !is_valid_local_diagnostic_event(event) {
            return Err(AppError::InvalidInput(
                "This diagnostic event/result combination is not supported.".to_owned(),
            ));
        }

        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        if !preferences.diagnostics_enabled {
            return Ok(LocalDiagnosticRecordResult { recorded: false });
        }

        Self::purge_local_diagnostic_events_with_connection(&connection)?;
        connection.execute(
            "INSERT INTO local_diagnostic_events
             (event_type, outcome, occurred_day, app_version)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                event.event_type.as_storage_value(),
                event.outcome.as_storage_value(),
                local_diagnostic_day(),
                env!("CARGO_PKG_VERSION"),
            ],
        )?;
        Self::enforce_local_diagnostic_event_limit_with_connection(&connection)?;
        Ok(LocalDiagnosticRecordResult { recorded: true })
    }

    /// Returns local, content-free reliability counts for the optional
    /// Diagnostics panel. It never enables collection as a side effect.
    pub fn local_diagnostics_summary(&self) -> AppResult<LocalDiagnosticsSummary> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        Self::purge_local_diagnostic_events_with_connection(&connection)?;
        let events = Self::local_diagnostic_events_with_connection(&connection)?;
        Ok(local_diagnostics_summary_from_events(
            preferences.diagnostics_enabled,
            &events,
        ))
    }

    /// Produces a shareable anonymous artifact. It includes only the same
    /// finite event/result categories, UTC date buckets and public app version
    /// as local storage; it intentionally excludes device, account, path,
    /// application, document and clipboard metadata.
    pub fn export_local_diagnostics(&self) -> AppResult<LocalDiagnosticsExport> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        Self::purge_local_diagnostic_events_with_connection(&connection)?;
        let events = Self::local_diagnostic_events_with_connection(&connection)?;
        let summary =
            local_diagnostics_summary_from_events(preferences.diagnostics_enabled, &events);
        Ok(LocalDiagnosticsExport {
            schema_version: LOCAL_DIAGNOSTICS_SCHEMA_VERSION,
            exported_at_day: local_diagnostic_day(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            summary,
            events,
        })
    }

    pub fn clear_local_diagnostics(&self) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute("DELETE FROM local_diagnostic_events", [])?;
        Ok(())
    }

    fn local_diagnostic_events_with_connection(
        connection: &Connection,
    ) -> AppResult<Vec<LocalDiagnosticsExportEvent>> {
        let mut statement = connection.prepare(
            "SELECT event_type, outcome, occurred_day, app_version
             FROM local_diagnostic_events
             ORDER BY occurred_day ASC, id ASC",
        )?;
        let rows = statement.query_map([], map_local_diagnostic_export_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn purge_local_diagnostic_events_with_connection(connection: &Connection) -> AppResult<()> {
        let retention_cutoff = (Utc::now()
            - chrono::Duration::days(LOCAL_DIAGNOSTIC_RETENTION_DAYS))
        .format("%Y-%m-%d")
        .to_string();
        connection.execute(
            "DELETE FROM local_diagnostic_events WHERE occurred_day < ?1",
            [retention_cutoff],
        )?;
        Self::enforce_local_diagnostic_event_limit_with_connection(connection)
    }

    fn enforce_local_diagnostic_event_limit_with_connection(
        connection: &Connection,
    ) -> AppResult<()> {
        connection.execute(
            "DELETE FROM local_diagnostic_events
             WHERE id IN (
                 SELECT id FROM local_diagnostic_events
                 ORDER BY occurred_day DESC, id DESC
                 LIMIT -1 OFFSET ?1
             )",
            [LOCAL_DIAGNOSTIC_EVENT_LIMIT],
        )?;
        Ok(())
    }

    pub fn context_enrichment(&self, id: &str) -> AppResult<ContextEnrichment> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        if let Some(enrichment_json) = connection
            .query_row(
                "SELECT enrichment_json FROM context_enrichments WHERE clipboard_item_id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return serde_json::from_str(&enrichment_json).map_err(AppError::from);
        }

        let content = connection
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        let updated_at = timestamp();
        Self::upsert_context_enrichment(&connection, id, &content, &updated_at)?;
        let enrichment = enrich_text(&content);
        Ok(enrichment)
    }

    pub fn list_local_text_actions(&self) -> AppResult<Vec<LocalTextAction>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let mut statement = connection.prepare(
            "SELECT id, name, transform, pipeline_json, shortcut_slot, is_builtin, created_at, updated_at
             FROM local_text_actions
             ORDER BY is_builtin DESC, name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], map_local_text_action)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn local_text_action(&self, id: &str) -> AppResult<LocalTextAction> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        Self::local_text_action_with_connection(&connection, id)
    }

    pub fn evaluate_local_text_filter(&self, id: &str, input: &str) -> AppResult<String> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let action = Self::local_text_action_with_connection(&connection, id)?;
        apply_local_text_pipeline(&action.steps, input)
    }

    pub fn automation_preferences(&self) -> AppResult<AutomationPreferences> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection
            .query_row(
                "SELECT enabled, history_metadata, history_content, clipboard_write,
                        library_write, filters_run
                 FROM automation_preferences WHERE singleton = 1",
                [],
                |row| {
                    Ok(AutomationPreferences {
                        enabled: row.get::<_, i64>(0)? != 0,
                        history_metadata: row.get::<_, i64>(1)? != 0,
                        history_content: row.get::<_, i64>(2)? != 0,
                        clipboard_write: row.get::<_, i64>(3)? != 0,
                        library_write: row.get::<_, i64>(4)? != 0,
                        filters_run: row.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .map_err(AppError::from)
    }

    pub fn update_automation_preferences(
        &self,
        preferences: &AutomationPreferences,
    ) -> AppResult<AutomationPreferences> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute(
            "UPDATE automation_preferences
             SET enabled = ?1, history_metadata = ?2, history_content = ?3,
                 clipboard_write = ?4, library_write = ?5, filters_run = ?6,
                 updated_at = ?7
             WHERE singleton = 1",
            params![
                preferences.enabled,
                preferences.history_metadata,
                preferences.history_content,
                preferences.clipboard_write,
                preferences.library_write,
                preferences.filters_run,
                timestamp(),
            ],
        )?;
        Ok(preferences.clone())
    }

    pub fn create_local_text_action(
        &self,
        draft: &CreateLocalTextAction,
    ) -> AppResult<LocalTextAction> {
        let name = draft.name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(AppError::InvalidInput(
                "Action names must contain 1 to 64 characters.".to_owned(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let timestamp = timestamp();
        let steps = draft.resolved_steps()?;
        let pipeline_json = serde_json::to_string(&steps)?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        connection.execute(
            "INSERT INTO local_text_actions
             (id, name, transform, pipeline_json, shortcut_slot, is_builtin, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
            params![
                id,
                name,
                draft.transform.as_storage_value(),
                pipeline_json,
                draft.shortcut_slot,
                timestamp
            ],
        )?;

        Self::local_text_action_with_connection(&connection, &id)
    }

    pub fn update_local_text_action(
        &self,
        id: &str,
        draft: &CreateLocalTextAction,
    ) -> AppResult<LocalTextAction> {
        let name = draft.name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(AppError::InvalidInput(
                "Action names must contain 1 to 64 characters.".to_owned(),
            ));
        }
        let steps = draft.resolved_steps()?;
        let pipeline_json = serde_json::to_string(&steps)?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let existing = Self::local_text_action_with_connection(&connection, id)?;
        if existing.is_builtin {
            return Err(AppError::InvalidInput(
                "Built-in local filters cannot be changed.".to_owned(),
            ));
        }
        connection.execute(
            "UPDATE local_text_actions
             SET name = ?1, transform = ?2, pipeline_json = ?3, shortcut_slot = ?4, updated_at = ?5
             WHERE id = ?6",
            params![
                name,
                draft.transform.as_storage_value(),
                pipeline_json,
                draft.shortcut_slot,
                timestamp(),
                id,
            ],
        )?;
        Self::local_text_action_with_connection(&connection, id)
    }

    pub fn delete_local_text_action(&self, id: &str) -> AppResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let existing = Self::local_text_action_with_connection(&connection, id)?;
        if existing.is_builtin {
            return Err(AppError::InvalidInput(
                "Built-in local filters cannot be deleted.".to_owned(),
            ));
        }
        Ok(connection.execute("DELETE FROM local_text_actions WHERE id = ?1", [id])? == 1)
    }

    pub fn preview_local_text_action(
        &self,
        action_id: &str,
        clipboard_item_id: &str,
    ) -> AppResult<LocalTextActionPreview> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let (action, item, output) = Self::evaluate_local_text_action_with_connection(
            &connection,
            action_id,
            clipboard_item_id,
        )?;
        Ok(LocalTextActionPreview {
            action,
            clipboard_item_id: item.id,
            item_version: item.version,
            input: item.content,
            output,
        })
    }

    pub fn confirm_local_text_action(
        &self,
        preview: &LocalTextActionPreview,
    ) -> AppResult<TextActionExecution> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let (action, item, output) = Self::evaluate_local_text_action_with_connection(
            &connection,
            &preview.action.id,
            &preview.clipboard_item_id,
        )
        .map_err(|error| match error {
            AppError::NotFound => AppError::InvalidInput(
                "Filter preview is out of date. Preview again before copying.".to_owned(),
            ),
            other => other,
        })?;
        if action != preview.action
            || item.version != preview.item_version
            || item.content != preview.input
            || output != preview.output
        {
            return Err(AppError::InvalidInput(
                "Filter preview is out of date. Preview again before copying.".to_owned(),
            ));
        }
        let audit = build_local_audit(
            &action,
            item.id,
            &item.content,
            &output,
            Uuid::new_v4().to_string(),
            timestamp(),
        );
        Ok(TextActionExecution {
            action,
            output,
            audit,
        })
    }

    fn evaluate_local_text_action_with_connection(
        connection: &Connection,
        action_id: &str,
        clipboard_item_id: &str,
    ) -> AppResult<(LocalTextAction, ClipboardItem, String)> {
        let action = Self::local_text_action_with_connection(connection, action_id)?;
        let item = Self::get_by_id_with_connection(connection, clipboard_item_id)?;
        if !item.representations.is_empty() {
            return Err(AppError::InvalidInput(
                "Local text actions are available for text clips only.".to_owned(),
            ));
        }
        let output = apply_local_text_pipeline(&action.steps, &item.content)?;
        Ok((action, item, output))
    }

    pub fn record_action_audit(&self, audit: &ActionAuditRecord) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        Self::insert_action_audit_with_connection(&connection, audit)
    }

    pub fn list_action_audit(
        &self,
        clipboard_item_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<ActionAuditRecord>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let limit = limit.clamp(1, 100);
        let mut statement = connection.prepare(
            "SELECT id, schema_version, action_id, action_name, transform, execution_mode, status,
                    clipboard_item_id, input_content_hash, input_preview, output_content_hash,
                    output_preview, created_at
             FROM action_audit_log
             WHERE (?1 IS NULL OR clipboard_item_id = ?1)
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![clipboard_item_id, limit], map_action_audit)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn preview_cloud_action(&self, clipboard_item_id: &str) -> AppResult<CloudActionPreview> {
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = timestamp();
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let item = Self::get_by_id_with_connection(&connection, clipboard_item_id)?;
        let preview = build_cloud_preview(item.id, &item.content, audit_id, timestamp);
        Self::insert_action_audit_with_connection(&connection, &preview.audit)?;
        Ok(preview)
    }

    pub fn list(
        &self,
        query: &str,
        pinned_only: bool,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<ClipboardItem>> {
        self.list_with_filters(query, pinned_only, limit, offset, None)
    }

    /// Lists local History after applying every structured filter in SQLite.
    /// Filtering deliberately happens before LIMIT/OFFSET so an older match is
    /// never hidden merely because a newer unfiltered page was fetched first.
    pub fn list_with_filters(
        &self,
        query: &str,
        pinned_only: bool,
        limit: u32,
        offset: u32,
        filters: Option<&ClipboardListFilters>,
    ) -> AppResult<Vec<ClipboardItem>> {
        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let query = query.trim();
        let limit = limit.min(500);
        let filters = filters.cloned().unwrap_or_default();
        validate_clipboard_list_filters(&filters)?;
        let smart_filters = filters
            .smart_collection_id
            .as_deref()
            .map(|id| Self::smart_collection_rule_with_connection(&connection, id))
            .transpose()?
            .map(smart_rule_as_list_filters);
        let effective_pin_filter = if pinned_only {
            ClipboardPinFilter::Pinned
        } else {
            filters.pin_filter
        };
        let fts_query = if query.is_empty() {
            None
        } else {
            fts_prefix_query(query)
        };
        let image_match_projection = if fts_query.is_some() {
            "CASE WHEN image_text_match.rowid IS NULL THEN 0 ELSE 1 END"
        } else {
            "0"
        };
        let note_match_projection = if fts_query.is_some() {
            "CASE WHEN note_match.rowid IS NULL THEN 0 ELSE 1 END"
        } else {
            "0"
        };
        let mut values = Vec::<Value>::new();
        let mut sql = format!(
            "SELECT items.id, items.content, items.kind, items.source_app,
                    items.created_at, items.updated_at, items.is_pinned, items.copy_count,
                    items.restored_at, items.global_id, items.device_id, items.group_id,
                    items.version, items.retention_until, items.last_used_at, items.search_text,
                    {image_match_projection}, {note_match_projection}
             FROM clipboard_items AS items
             LEFT JOIN clipboard_image_text_extractions AS image_text
                ON image_text.clipboard_item_id = items.id
             LEFT JOIN clipboard_item_notes AS item_note
                ON item_note.clipboard_item_id = items.id",
        );
        if let Some(fts_query) = &fts_query {
            sql.push_str(
                " LEFT JOIN (
                      SELECT rowid, bm25(clipboard_items_fts) AS score
                      FROM clipboard_items_fts
                      WHERE clipboard_items_fts MATCH ?
                  ) AS content_match ON content_match.rowid = items.rowid",
            );
            values.push(Value::Text(fts_query.clone()));
            sql.push_str(
                " LEFT JOIN (
                      SELECT rowid, bm25(clipboard_item_notes_fts) AS score
                      FROM clipboard_item_notes_fts
                      WHERE clipboard_item_notes_fts MATCH ?
                  ) AS note_match ON note_match.rowid = item_note.rowid",
            );
            values.push(Value::Text(fts_query.clone()));
            sql.push_str(
                " LEFT JOIN (
                      SELECT rowid, bm25(clipboard_image_text_extractions_fts) AS score
                      FROM clipboard_image_text_extractions_fts
                      WHERE clipboard_image_text_extractions_fts MATCH ?
                  ) AS image_text_match ON image_text_match.rowid = image_text.rowid",
            );
            values.push(Value::Text(fts_query.clone()));
        }
        sql.push_str(" WHERE items.deleted_at IS NULL");

        if !query.is_empty() {
            let escaped_query = escape_like(query);
            if fts_query.is_some() {
                sql.push_str(
                    " AND (content_match.rowid IS NOT NULL
                           OR image_text_match.rowid IS NOT NULL
                           OR note_match.rowid IS NOT NULL
                           OR items.source_app LIKE '%' || ? || '%' ESCAPE '\\'
                           OR EXISTS (
                               SELECT 1 FROM clipboard_item_tags AS tag
                               WHERE tag.clipboard_item_id = items.id
                                 AND tag.label LIKE '%' || ? || '%' ESCAPE '\\'
                           )
                           OR EXISTS (
                               SELECT 1 FROM clipboard_occurrences AS occurrence
                               WHERE occurrence.clipboard_item_id = items.id
                                 AND occurrence.source_app LIKE '%' || ? || '%' ESCAPE '\\'
                           ))",
                );
                values.extend((0..3).map(|_| Value::Text(escaped_query.clone())));
            } else {
                // FTS syntax can be empty for punctuation-only input. Keep
                // that input useful with the same bound, local fields.
                sql.push_str(
                    " AND (items.search_text LIKE '%' || ? || '%' ESCAPE '\\'
                           OR image_text.text LIKE '%' || ? || '%' ESCAPE '\\'
                           OR item_note.text LIKE '%' || ? || '%' ESCAPE '\\'
                           OR items.source_app LIKE '%' || ? || '%' ESCAPE '\\'
                           OR EXISTS (
                               SELECT 1 FROM clipboard_item_tags AS tag
                               WHERE tag.clipboard_item_id = items.id
                                 AND tag.label LIKE '%' || ? || '%' ESCAPE '\\'
                           )
                           OR EXISTS (
                               SELECT 1 FROM clipboard_occurrences AS occurrence
                               WHERE occurrence.clipboard_item_id = items.id
                                 AND occurrence.source_app LIKE '%' || ? || '%' ESCAPE '\\'
                           ))",
                );
                values.extend((0..6).map(|_| Value::Text(escaped_query.clone())));
            }
        }

        if let Some(smart_filters) = &smart_filters {
            append_clipboard_list_filters(
                &mut sql,
                &mut values,
                smart_filters,
                smart_filters.pin_filter,
            );
        }
        append_clipboard_list_filters(&mut sql, &mut values, &filters, effective_pin_filter);
        sql.push_str(" ORDER BY items.is_pinned DESC");
        if fts_query.is_some() {
            sql.push_str(
                ", CASE
                     WHEN content_match.rowid IS NOT NULL THEN 0
                     WHEN image_text_match.rowid IS NOT NULL THEN 1
                     WHEN note_match.rowid IS NOT NULL THEN 2
                     ELSE 3
                   END,
                 COALESCE(content_match.score, image_text_match.score, note_match.score) ASC",
            );
        }
        sql.push_str(", items.created_at DESC, items.id ASC LIMIT ? OFFSET ?");
        values.push(Value::Integer(i64::from(limit)));
        values.push(Value::Integer(i64::from(offset)));

        let mut statement = connection.prepare(&sql)?;
        let rows =
            statement.query_map(params_from_iter(values.iter()), map_clipboard_search_item)?;
        let mut items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        // This helper also attaches Occurrences and bounded local tags. Keep
        // all query branches on the same hydration path.
        Self::attach_representations(&connection, &mut items)?;
        Ok(items)
    }

    /// Returns complete local facets independently of the current result page.
    /// Collection names are existing bounded Item tags presented through the
    /// Saved/Collections product vocabulary.
    pub fn clipboard_filter_options(&self) -> AppResult<ClipboardFilterOptions> {
        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut sources_statement = connection.prepare(
            "SELECT source_app
             FROM (
                 SELECT source_app
                 FROM clipboard_items
                 WHERE deleted_at IS NULL AND source_app IS NOT NULL
                 UNION
                 SELECT occurrence.source_app
                 FROM clipboard_occurrences AS occurrence
                 INNER JOIN clipboard_items AS item ON item.id = occurrence.clipboard_item_id
                 WHERE item.deleted_at IS NULL AND occurrence.source_app IS NOT NULL
             )
             WHERE trim(source_app) <> ''
             GROUP BY source_app COLLATE NOCASE
             ORDER BY source_app COLLATE NOCASE ASC",
        )?;
        let source_apps = sources_statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let collections = Self::list_collections_with_connection(&connection)?;
        let (total_count, unpinned_count) = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN is_pinned = 0 THEN 1 ELSE 0 END), 0)
             FROM clipboard_items
             WHERE deleted_at IS NULL",
            [],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?)),
        )?;
        Ok(ClipboardFilterOptions {
            source_apps,
            collections,
            total_count,
            unpinned_count,
        })
    }

    pub fn list_collections(&self) -> AppResult<Vec<ClipboardCollection>> {
        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::list_collections_with_connection(&connection)
    }

    pub fn create_smart_collection(
        &self,
        input: &ClipboardSmartCollectionInput,
    ) -> AppResult<ClipboardCollection> {
        let name = normalized_collection_lookup(&input.name)?;
        validate_smart_collection_rule(&input.rule)?;
        let rule_json = serde_json::to_string(&input.rule)
            .map_err(|error| AppError::InvalidInput(error.to_string()))?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        ensure_collection_name_available(&connection, &name, None)?;
        let count = connection.query_row(
            "SELECT COUNT(*) FROM clipboard_smart_collections",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        if count >= MAX_SMART_COLLECTIONS {
            return Err(AppError::InvalidInput(
                "ClipRiva supports at most 20 Smart Collections.".to_owned(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = timestamp();
        connection.execute(
            "INSERT INTO clipboard_smart_collections
                (id, name, rule_schema_version, rule_json, created_at, updated_at)
             VALUES (?1, ?2, 1, ?3, ?4, ?4)",
            params![id, name, rule_json, now],
        )?;
        smart_collection_with_connection(&connection, &id)
    }

    pub fn update_smart_collection(
        &self,
        id: &str,
        input: &ClipboardSmartCollectionInput,
    ) -> AppResult<ClipboardCollection> {
        let name = normalized_collection_lookup(&input.name)?;
        validate_smart_collection_rule(&input.rule)?;
        let rule_json = serde_json::to_string(&input.rule)
            .map_err(|error| AppError::InvalidInput(error.to_string()))?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        ensure_collection_name_available(&connection, &name, Some(id))?;
        let changed = connection.execute(
            "UPDATE clipboard_smart_collections
             SET name = ?1, rule_json = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, rule_json, timestamp(), id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        smart_collection_with_connection(&connection, id)
    }

    pub fn delete_smart_collection(&self, id: &str) -> AppResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Ok(connection.execute(
            "DELETE FROM clipboard_smart_collections WHERE id = ?1",
            [id],
        )? > 0)
    }

    /// Returns the compact Quick Paste set using only deterministic, local
    /// text matching. Search tiers are exact, prefix, then contains; ties
    /// prefer a pinned item, the most recent occurrence/use, then its stable
    /// id. Empty input remains useful without allowing pins to permanently
    /// outrank the user's recent work.
    pub fn quick_paste_search(
        &self,
        query: &str,
        pinned_only: bool,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<QuickPasteSearchItem>> {
        let requested_limit = limit.min(20) as usize;
        if requested_limit == 0 {
            return Ok(Vec::new());
        }

        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let pinned_filter = if pinned_only { 1_i64 } else { 0_i64 };
        let normalized_query = normalize_quick_paste_text(query);
        let (sql, values) = if normalized_query.is_empty() {
            (
                "SELECT item.id, item.content, item.kind, item.source_app, item.created_at,
                        item.updated_at, item.is_pinned, item.copy_count, item.restored_at,
                        item.global_id, item.device_id, item.group_id, item.version,
                        item.retention_until, item.last_used_at, item.search_text
                 FROM clipboard_items AS item
                 WHERE item.deleted_at IS NULL
                   AND (?1 = 0 OR item.is_pinned = 1)
                 ORDER BY COALESCE(item.last_used_at, item.updated_at, item.created_at) DESC,
                          item.id ASC
                 LIMIT ?2"
                    .to_owned(),
                vec![
                    Value::Integer(pinned_filter),
                    Value::Integer(i64::from(QUICK_PASTE_CANDIDATE_LIMIT)),
                ],
            )
        } else {
            // Search a single required term in SQLite. Rust then checks the
            // complete whitespace-normalized query and ranks every match.
            // Parameters keep punctuation and quotes out of SQL syntax.
            let anchor = quick_paste_query_anchor(&normalized_query);
            (
                "WITH match_ids AS (
                   SELECT id FROM clipboard_items
                     WHERE instr(lower(content), ?1) > 0
                        OR instr(lower(search_text), ?1) > 0
                        OR instr(lower(coalesce(source_app, '')), ?1) > 0
                   UNION SELECT clipboard_item_id FROM clipboard_item_notes
                     WHERE instr(lower(text), ?1) > 0
                   UNION SELECT clipboard_item_id FROM clipboard_image_text_extractions
                     WHERE instr(lower(text), ?1) > 0
                   UNION SELECT clipboard_item_id FROM clipboard_item_tags
                     WHERE instr(lower(label), ?1) > 0
                   UNION SELECT clipboard_item_id FROM clipboard_occurrences
                     WHERE instr(lower(coalesce(source_app, '')), ?1) > 0
                 )
                 SELECT item.id, item.content, item.kind, item.source_app, item.created_at,
                        item.updated_at, item.is_pinned, item.copy_count, item.restored_at,
                        item.global_id, item.device_id, item.group_id, item.version,
                        item.retention_until, item.last_used_at, item.search_text
                 FROM clipboard_items AS item
                 INNER JOIN match_ids ON match_ids.id = item.id
                 WHERE item.deleted_at IS NULL
                   AND (?2 = 0 OR item.is_pinned = 1)"
                    .to_owned(),
                vec![
                    Value::Text(anchor.to_owned()),
                    Value::Integer(pinned_filter),
                ],
            )
        };
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(values.iter()))?;
        let mut candidates = Vec::new();
        while let Some(row) = rows.next()? {
            candidates.push(map_clipboard_item(row)?);
        }
        Self::attach_occurrences(&connection, &mut candidates)?;
        Self::attach_tags(&connection, &mut candidates)?;
        let (note_text_by_id, image_text_by_id) = if normalized_query.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            (
                Self::quick_paste_text_by_item_with_connection(
                    &connection,
                    &candidates,
                    QuickPasteAuxiliaryText::Note,
                )?,
                Self::quick_paste_text_by_item_with_connection(
                    &connection,
                    &candidates,
                    QuickPasteAuxiliaryText::ImageText,
                )?,
            )
        };
        let mut ranked = if normalized_query.is_empty() {
            zero_input_quick_paste_suggestions(
                candidates,
                requested_limit.saturating_add(offset as usize),
            )
        } else {
            candidates
                .into_iter()
                .filter_map(|item| {
                    quick_paste_match(
                        &item,
                        note_text_by_id.get(&item.id).map(String::as_str),
                        image_text_by_id.get(&item.id).map(String::as_str),
                        &normalized_query,
                    )
                    .map(|evidence| QuickPasteSearchItem {
                        item,
                        match_kind: evidence.match_kind,
                        match_field: Some(evidence.match_field),
                    })
                })
                .collect::<Vec<_>>()
        };

        if !normalized_query.is_empty() {
            let visible_window = requested_limit.saturating_add(offset as usize);
            if visible_window > 0 && visible_window < ranked.len() {
                ranked.select_nth_unstable_by(visible_window - 1, compare_quick_paste_search_items);
                ranked.truncate(visible_window);
            }
            ranked.sort_by(compare_quick_paste_search_items);
        }

        let items = ranked
            .into_iter()
            .skip(offset as usize)
            .take(requested_limit)
            .collect::<Vec<_>>();
        let mut clips = items
            .iter()
            .map(|result| result.item.clone())
            .collect::<Vec<_>>();
        Self::attach_representations(&connection, &mut clips)?;
        let clips_by_id = clips
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();

        Ok(items
            .into_iter()
            .filter_map(|result| {
                clips_by_id
                    .get(&result.item.id)
                    .cloned()
                    .map(|item| QuickPasteSearchItem {
                        item,
                        match_kind: result.match_kind,
                        match_field: result.match_field,
                    })
            })
            .collect())
    }

    /// Rank clipboard text with the built-in offline lexical fallback.
    ///
    /// This path deliberately does not create embeddings or make a network
    /// request. It ranks the current local candidate set by deterministic token
    /// overlap and returns its score and source-text highlight ranges.
    pub fn search_local_semantic(
        &self,
        query: &str,
        pinned_only: bool,
        limit: u32,
        offset: u32,
    ) -> AppResult<Vec<LocalSemanticSearchItem>> {
        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::ensure_labs_enabled_with_connection(&connection)?;
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let pinned_filter = if pinned_only { 1_i64 } else { 0_i64 };
        let requested_limit = limit.min(500);
        if requested_limit == 0 {
            return Ok(Vec::new());
        }

        let mut statement = connection.prepare(
            "SELECT id, content, kind, source_app, created_at, updated_at, is_pinned, copy_count,
                    restored_at, global_id, device_id, group_id, version, retention_until,
                    last_used_at, search_text
             FROM clipboard_items
             WHERE deleted_at IS NULL
               AND (?1 = 0 OR is_pinned = 1)
             ORDER BY is_pinned DESC, created_at DESC, id ASC
             LIMIT ?2",
        )?;
        let candidate_rows = statement.query_map(
            params![pinned_filter, LOCAL_SEMANTIC_CANDIDATE_LIMIT],
            map_clipboard_item,
        )?;
        let mut candidates = candidate_rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Self::attach_representations(&connection, &mut candidates)?;
        let documents = candidates
            .iter()
            .map(|item| SemanticSearchDocument {
                document_id: item.id.clone(),
                text: item.content.clone(),
                vector: None,
            })
            .collect::<Vec<_>>();
        let items_by_id = candidates
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let ranking_limit = requested_limit.saturating_add(offset) as usize;
        let adapter = LocalLexicalFallbackAdapter;
        let response = adapter
            .search(
                &SemanticSearchQuery {
                    schema_version: SEMANTIC_SEARCH_SCHEMA_VERSION,
                    text: query.to_owned(),
                    limit: ranking_limit,
                    minimum_score: None,
                },
                &documents,
            )
            .map_err(|error| AppError::InvalidInput(error.to_string()))?;

        Ok(response
            .results
            .into_iter()
            .skip(offset as usize)
            .filter_map(|result| {
                items_by_id
                    .get(&result.document_id)
                    .cloned()
                    .map(|item| LocalSemanticSearchItem {
                        item,
                        score: result.score,
                        matched_terms: result.matched_terms,
                        highlight_ranges: result.highlight_ranges,
                    })
            })
            .collect())
    }

    pub fn get_by_id(&self, id: &str) -> AppResult<ClipboardItem> {
        self.enforce_retention_policy()?;
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Returns the encoded image bytes only for an active image Item. The
    /// content-addressed storage key never crosses the OCR command boundary.
    pub fn image_bytes_for_text_extraction(&self, id: &str) -> AppResult<Vec<u8>> {
        let storage_key = {
            let connection = self.connection.lock().expect("database mutex poisoned");
            connection
                .query_row(
                    "SELECT representation.storage_key
                     FROM clipboard_items AS item
                     INNER JOIN clipboard_representations AS representation
                        ON representation.clipboard_item_id = item.id
                     WHERE item.id = ?1
                       AND item.deleted_at IS NULL
                       AND item.kind = 'image'
                       AND representation.kind = 'image'
                     ORDER BY representation.created_at ASC
                     LIMIT 1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(AppError::NotFound)?
        };
        self.read_blob(&storage_key)
    }

    /// Persists a completed manual extraction only after applying the same
    /// high-confidence secret detector used by clipboard capture.
    pub fn save_image_text_extraction(
        &self,
        id: &str,
        text: &str,
    ) -> AppResult<ClipboardImageTextExtractionResult> {
        {
            let connection = self.connection.lock().expect("database mutex poisoned");
            ensure_active_image_item(&connection, id)?;
        }
        let text = text.trim();
        if text.is_empty() {
            let connection = self.connection.lock().expect("database mutex poisoned");
            connection.execute(
                "DELETE FROM clipboard_image_text_extractions WHERE clipboard_item_id = ?1",
                [id],
            )?;
            return Ok(ClipboardImageTextExtractionResult {
                status: ClipboardImageTextExtractionStatus::NoText,
                extraction: None,
            });
        }
        if text.len() > 256 * 1024 {
            return Err(AppError::InvalidInput(
                "Extracted image text exceeds the 256 KiB local storage limit.".to_owned(),
            ));
        }
        if looks_sensitive(text) {
            return Ok(ClipboardImageTextExtractionResult {
                status: ClipboardImageTextExtractionStatus::SensitiveBlocked,
                extraction: None,
            });
        }

        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        ensure_active_image_item(&transaction, id)?;
        let now = timestamp();
        transaction.execute(
            "INSERT INTO clipboard_image_text_extractions
                (clipboard_item_id, text, extracted_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(clipboard_item_id) DO UPDATE SET
                text = excluded.text,
                updated_at = excluded.updated_at",
            params![id, text, now],
        )?;
        let extraction =
            image_text_extraction_with_connection(&transaction, id)?.ok_or(AppError::NotFound)?;
        transaction.commit()?;
        Ok(ClipboardImageTextExtractionResult {
            status: ClipboardImageTextExtractionStatus::Extracted,
            extraction: Some(extraction),
        })
    }

    pub fn image_text_extraction(
        &self,
        id: &str,
    ) -> AppResult<Option<ClipboardImageTextExtraction>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        ensure_active_image_item(&connection, id)?;
        image_text_extraction_with_connection(&connection, id)
    }

    pub fn delete_image_text_extraction(&self, id: &str) -> AppResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        ensure_active_image_item(&connection, id)?;
        Ok(connection.execute(
            "DELETE FROM clipboard_image_text_extractions WHERE clipboard_item_id = ?1",
            [id],
        )? > 0)
    }

    pub fn item_note(&self, id: &str) -> AppResult<Option<ClipboardItemNote>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)?;
        item_note_with_connection(&connection, id)
    }

    /// Saves one bounded, explicit local Note. A rejected value leaves any
    /// existing Note and its FTS row untouched and never changes capture state.
    pub fn save_item_note(
        &self,
        id: &str,
        text: &str,
    ) -> AppResult<ClipboardItemNoteMutationResult> {
        let text = text.trim();
        if text.is_empty() {
            self.delete_item_note(id)?;
            return Ok(ClipboardItemNoteMutationResult {
                status: ClipboardItemNoteMutationStatus::Deleted,
                note: None,
            });
        }
        if text.len() > MAX_CLIPBOARD_NOTE_BYTES {
            return Err(AppError::InvalidInput(
                "Notes must be at most 16 KiB of UTF-8 text.".to_owned(),
            ));
        }
        if looks_sensitive(text) {
            return Ok(ClipboardItemNoteMutationResult {
                status: ClipboardItemNoteMutationStatus::SensitiveBlocked,
                note: None,
            });
        }

        let mut connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)?;
        let transaction = connection.transaction()?;
        let now = timestamp();
        transaction.execute(
            "INSERT INTO clipboard_item_notes
                (clipboard_item_id, text, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(clipboard_item_id) DO UPDATE SET
                text = excluded.text, updated_at = excluded.updated_at",
            params![id, text, now],
        )?;
        let note = item_note_with_connection(&transaction, id)?.ok_or(AppError::NotFound)?;
        transaction.commit()?;
        Ok(ClipboardItemNoteMutationResult {
            status: ClipboardItemNoteMutationStatus::Saved,
            note: Some(note),
        })
    }

    pub fn delete_item_note(&self, id: &str) -> AppResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)?;
        Ok(connection.execute(
            "DELETE FROM clipboard_item_notes WHERE clipboard_item_id = ?1",
            [id],
        )? > 0)
    }

    /// Replaces the complete local tag set for one active clip. The bounded
    /// allow-list shape prevents a free-form metadata field from growing
    /// without limit while preserving the user's visible capitalization.
    pub fn replace_tags(&self, id: &str, tags: Vec<String>) -> AppResult<ClipboardItem> {
        let tags = normalize_clipboard_tags(tags)?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)?;
        let transaction = connection.transaction()?;
        Self::replace_tags_with_connection(&transaction, id, &tags)?;
        transaction.commit()?;
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Saves an active Item as a reusable local snippet. The existing pin is
    /// the retention-protection fact; optional Collections reuse bounded tags.
    /// Both changes commit atomically so a collection assignment cannot be
    /// persisted without its requested Saved state.
    pub fn save_snippet(
        &self,
        id: &str,
        collections: Option<Vec<String>>,
    ) -> AppResult<ClipboardItem> {
        let collections = collections.map(normalize_clipboard_tags).transpose()?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        Self::get_by_id_with_connection(&connection, id)?;
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE clipboard_items
             SET is_pinned = 1, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![timestamp(), id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        if let Some(collections) = &collections {
            Self::replace_tags_with_connection(&transaction, id, collections)?;
        }
        transaction.commit()?;
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Removes Saved retention protection while retaining Collection labels.
    /// The Item immediately returns to the normal local retention policy.
    pub fn remove_snippet(&self, id: &str) -> AppResult<ClipboardItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let changed = connection.execute(
            "UPDATE clipboard_items
             SET is_pinned = 0, updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![timestamp(), id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Renames a Collection across active and recycled Items. If the target
    /// name already exists on an Item the two memberships merge without
    /// exceeding the established eight-Collection bound.
    pub fn rename_collection(
        &self,
        from: &str,
        to: &str,
    ) -> AppResult<ClipboardCollectionMutationResult> {
        let from = normalized_collection_lookup(from)?;
        let mut normalized_to = normalize_clipboard_tags(vec![to.to_owned()])?;
        let to = normalized_to.pop().ok_or_else(|| {
            AppError::InvalidInput("Collection names cannot be empty.".to_owned())
        })?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let smart_conflict = connection.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM clipboard_smart_collections WHERE name = ?1 COLLATE NOCASE
             )",
            [&to],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if smart_conflict {
            return Err(AppError::InvalidInput(
                "A Smart Collection already uses this name.".to_owned(),
            ));
        }
        let transaction = connection.transaction()?;
        let mut statement = transaction.prepare(
            "SELECT DISTINCT clipboard_item_id
             FROM clipboard_item_tags
             WHERE label = ?1 COLLATE NOCASE
             ORDER BY clipboard_item_id ASC",
        )?;
        let item_ids = statement
            .query_map([&from], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);

        for item_id in &item_ids {
            let tags = Self::tags_with_connection(&transaction, item_id)?;
            let renamed = tags
                .into_iter()
                .map(|tag| {
                    if tag.eq_ignore_ascii_case(&from) {
                        to.clone()
                    } else {
                        tag
                    }
                })
                .collect::<Vec<_>>();
            let renamed = normalize_clipboard_tags(renamed)?;
            Self::replace_tags_with_connection(&transaction, item_id, &renamed)?;
        }
        transaction.commit()?;
        Ok(ClipboardCollectionMutationResult {
            affected_item_count: item_ids.len() as u32,
        })
    }

    /// Deletes only Collection memberships. Clipboard content, Saved state
    /// and recycle/retention metadata are unchanged.
    pub fn delete_collection(&self, name: &str) -> AppResult<ClipboardCollectionMutationResult> {
        let name = normalized_collection_lookup(name)?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let affected_item_count = connection.query_row(
            "SELECT COUNT(DISTINCT clipboard_item_id)
             FROM clipboard_item_tags
             WHERE label = ?1 COLLATE NOCASE",
            [&name],
            |row| row.get::<_, u32>(0),
        )?;
        connection.execute(
            "DELETE FROM clipboard_item_tags WHERE label = ?1 COLLATE NOCASE",
            [&name],
        )?;
        Ok(ClipboardCollectionMutationResult {
            affected_item_count,
        })
    }

    /// Returns the plaintext for an explicitly selected Local Link transfer
    /// only after applying the same local capture/privacy rules used when the
    /// item entered history. The returned payload is crate-private and never
    /// serializes across Tauri IPC.
    pub(crate) fn local_link_export_clipboard_item(
        &self,
        id: &str,
    ) -> AppResult<LocalLinkClipboardPayload> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let item = Self::get_by_id_with_connection(&connection, id)?;
        if !is_local_link_text_item_kind(&item.kind) {
            return Err(AppError::InvalidInput(
                "This clipboard item type is not available for Local Link yet.".to_owned(),
            ));
        }

        let preferences = Self::capture_preferences_with_connection(&connection)?;
        if !Self::capture_allowed_with_connection(
            &connection,
            &preferences,
            &item.content,
            item.source_app.as_deref(),
            &timestamp(),
        )? {
            return Err(AppError::InvalidInput(
                "This clipboard item is blocked by local capture privacy controls.".to_owned(),
            ));
        }

        Ok(LocalLinkClipboardPayload {
            content: item.content,
        })
    }

    /// Local Link owns its own normalized tables but uses the database mutex
    /// so pairing and transfer transitions remain atomic with other local
    /// state. This deliberately does not expose the connection outside the
    /// crate or through the IPC boundary.
    pub(crate) fn with_local_link_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> AppResult<T>,
    ) -> AppResult<T> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        operation(&connection)
    }

    pub fn record_copy(&self, id: &str) -> AppResult<ClipboardItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let changed = connection.execute(
            "UPDATE clipboard_items
             SET copy_count = copy_count + 1, updated_at = ?1, last_used_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![timestamp(), id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Self::get_by_id_with_connection(&connection, id)
    }

    pub fn toggle_pin(&self, id: &str) -> AppResult<ClipboardItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let changed = connection.execute(
            "UPDATE clipboard_items
             SET is_pinned = CASE is_pinned WHEN 0 THEN 1 ELSE 0 END,
                 updated_at = ?1
             WHERE id = ?2 AND deleted_at IS NULL",
            params![timestamp(), id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Computes the exact impact of an upcoming cleanup without changing
    /// history. The same request can be submitted to `move_to_recycle_bin` so
    /// single-item, batch and automatic policy cleanup all disclose the same
    /// local impact before any data is hidden.
    pub fn preview_cleanup(
        &self,
        request: &ClipboardCleanupRequest,
    ) -> AppResult<ClipboardCleanupPreview> {
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        Self::cleanup_preview_with_connection(&connection, &preferences, request)
    }

    /// Moves active clips into the local recycle bin. Pinned clips are never
    /// included unless a manual confirmation sends `include_pinned: true`.
    /// The original row remains in place, preserving captured metadata and
    /// media representations for a later restore.
    pub fn move_to_recycle_bin(
        &self,
        request: &ClipboardCleanupRequest,
    ) -> AppResult<ClipboardCleanupResult> {
        self.purge_expired_recycle_bin()?;
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        let preview = Self::cleanup_preview_with_connection(&connection, &preferences, request)?;
        let candidates =
            Self::cleanup_candidates_with_connection(&connection, &preferences, request)?;
        let eligible = Self::eligible_cleanup_candidates(candidates, request);
        let transaction = connection.transaction()?;
        let moved_to_recycle_bin_count = self.move_candidates_to_recycle_bin_with_connection(
            &transaction,
            &preferences,
            &eligible,
        )?;
        transaction.commit()?;

        Ok(ClipboardCleanupResult {
            preview,
            moved_to_recycle_bin_count,
        })
    }

    /// Compatibility wrapper for the existing single-delete command. A pin is
    /// intentionally protected here; callers must unpin it or use the newer
    /// explicit `include_pinned` confirmation request.
    pub fn delete(&self, id: &str) -> AppResult<()> {
        let request = ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::Selected,
            item_ids: vec![id.to_owned()],
            include_pinned: false,
        };
        let result = self.move_to_recycle_bin(&request)?;
        if result.preview.pinned_skipped_count > 0 {
            return Err(AppError::InvalidInput(
                "Pinned clips are protected. Unpin the clip or explicitly confirm its deletion."
                    .to_owned(),
            ));
        }
        if result.moved_to_recycle_bin_count == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    /// Compatibility wrapper for the current "clear unpinned" action. It now
    /// has recycle-bin semantics while preserving the established pin rule.
    pub fn clear_unpinned(&self) -> AppResult<()> {
        self.move_to_recycle_bin(&ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::AllUnpinned,
            item_ids: Vec::new(),
            include_pinned: false,
        })?;
        Ok(())
    }

    pub fn recycle_bin_items(&self, limit: u32) -> AppResult<Vec<RecycleBinItem>> {
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, content, kind, source_app, created_at, updated_at, is_pinned, copy_count,
                    restored_at, global_id, device_id, group_id, version, retention_until,
                    last_used_at, search_text,
                    deleted_at, recycle_expires_at
             FROM clipboard_items
             WHERE deleted_at IS NOT NULL
             ORDER BY deleted_at DESC, id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit.min(500)], |row| {
            let item = map_clipboard_item(row)?;
            Ok(RecycleBinItem {
                item,
                deleted_at: row.get(16)?,
                recycle_expires_at: row.get(17)?,
            })
        })?;
        let mut entries = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        let mut items = entries
            .iter()
            .map(|entry| entry.item.clone())
            .collect::<Vec<_>>();
        Self::attach_representations(&connection, &mut items)?;
        for (entry, item) in entries.iter_mut().zip(items) {
            entry.item = item;
        }
        Ok(entries)
    }

    /// Restoring clears only recycle-bin state. `created_at`, source, pin and
    /// any future grouping columns stay on the original row; `restored_at`
    /// records the recovery without promoting the clip to pinned.
    pub fn restore_from_recycle_bin(&self, id: &str) -> AppResult<ClipboardItem> {
        self.purge_expired_recycle_bin()?;
        let connection = self.connection.lock().expect("database mutex poisoned");
        let restored_at = timestamp();
        let changed = connection.execute(
            "UPDATE clipboard_items
             SET deleted_at = NULL,
                 recycle_expires_at = NULL,
                 restored_at = ?1
             WHERE id = ?2 AND deleted_at IS NOT NULL",
            params![restored_at, id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Self::get_by_id_with_connection(&connection, id)
    }

    /// Permanently removes only clips that are already in the recycle bin.
    /// The caller must place this behind its own irreversible-confirmation UI.
    pub fn permanently_delete_recycled_items(&self, item_ids: &[String]) -> AppResult<u32> {
        if item_ids.is_empty() {
            return Ok(0);
        }
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let ids = item_ids.iter().cloned().collect::<HashSet<_>>();
        let blob_keys = Self::blob_keys_for_recycled_ids_with_connection(&connection, &ids)?;
        let transaction = connection.transaction()?;
        let mut changed = 0_u32;
        for id in ids {
            changed += transaction.execute(
                "DELETE FROM clipboard_items WHERE id = ?1 AND deleted_at IS NOT NULL",
                [id],
            )? as u32;
        }
        transaction.commit()?;
        self.cleanup_unreferenced_blobs_with_connection(&connection, &blob_keys)?;
        Ok(changed)
    }

    /// Empties the local recycle bin. This is intentionally separate from
    /// normal history cleanup so the UI can require an irreversible second
    /// confirmation before calling it.
    pub fn empty_recycle_bin(&self) -> AppResult<u32> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let blob_keys = Self::blob_keys_for_recycle_bin_with_connection(&connection)?;
        let changed = connection.execute(
            "DELETE FROM clipboard_items WHERE deleted_at IS NOT NULL",
            [],
        )?;
        self.cleanup_unreferenced_blobs_with_connection(&connection, &blob_keys)?;
        Ok(changed as u32)
    }

    fn cleanup_preview_with_connection(
        connection: &Connection,
        preferences: &CapturePreferences,
        request: &ClipboardCleanupRequest,
    ) -> AppResult<ClipboardCleanupPreview> {
        let candidates =
            Self::cleanup_candidates_with_connection(connection, preferences, request)?;
        let eligible = Self::eligible_cleanup_candidates(candidates.clone(), request);
        let pinned_skipped_count = candidates
            .iter()
            .filter(|candidate| candidate.is_pinned)
            .count() as u32
            - eligible
                .iter()
                .filter(|candidate| candidate.is_pinned)
                .count() as u32;
        let deleted_at = timestamp();
        let maximum_retention_days = recycle_bin_max_retention_days(preferences);
        let expiry_bounds = eligible
            .iter()
            .map(|candidate| {
                recycle_expiry_for(
                    &candidate.retention_until,
                    &deleted_at,
                    maximum_retention_days,
                )
            })
            .fold(None, |bounds, expiry| match bounds {
                None => Some((expiry.clone(), expiry)),
                Some((earliest, latest)) => {
                    Some((earliest.min(expiry.clone()), latest.max(expiry)))
                }
            });

        Ok(ClipboardCleanupPreview {
            scope: request.scope,
            affected_count: eligible.len() as u32,
            pinned_skipped_count,
            rule_conditions: cleanup_rule_conditions(request, preferences),
            representative_items: eligible
                .iter()
                .take(CLEANUP_PREVIEW_REPRESENTATIVE_LIMIT)
                .map(|candidate| ClipboardCleanupRepresentative {
                    id: candidate.id.clone(),
                    kind: candidate.kind.clone(),
                    source_app: candidate.source_app.clone(),
                    captured_at: candidate.captured_at.clone(),
                    is_pinned: candidate.is_pinned,
                })
                .collect(),
            recycle_bin_retention: RecycleBinRetention {
                maximum_days: maximum_retention_days as u8,
                earliest_expires_at: expiry_bounds.as_ref().map(|(earliest, _)| earliest.clone()),
                latest_expires_at: expiry_bounds.map(|(_, latest)| latest),
            },
        })
    }

    fn cleanup_candidates_with_connection(
        connection: &Connection,
        preferences: &CapturePreferences,
        request: &ClipboardCleanupRequest,
    ) -> AppResult<Vec<CleanupCandidate>> {
        let mut statement = connection.prepare(
            "SELECT id, kind, source_app, created_at, retention_until, is_pinned
             FROM clipboard_items
             WHERE deleted_at IS NULL
             ORDER BY created_at DESC, id ASC",
        )?;
        let active = statement
            .query_map([], |row| {
                Ok(CleanupCandidate {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    source_app: row.get(2)?,
                    captured_at: row.get(3)?,
                    retention_until: row.get(4)?,
                    is_pinned: row.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;

        match request.scope {
            ClipboardCleanupScope::Selected => {
                let selected_ids = request.item_ids.iter().collect::<HashSet<_>>();
                Ok(active
                    .into_iter()
                    .filter(|candidate| selected_ids.contains(&candidate.id))
                    .collect())
            }
            ClipboardCleanupScope::AllUnpinned => Ok(active
                .into_iter()
                .filter(|candidate| !candidate.is_pinned)
                .collect()),
            ClipboardCleanupScope::RetentionPolicy => {
                let now = timestamp();
                let mut unpinned_seen = 0_u32;
                Ok(active
                    .into_iter()
                    .filter(|candidate| {
                        if candidate.is_pinned {
                            return false;
                        }
                        unpinned_seen += 1;
                        candidate.retention_until <= now
                            || unpinned_seen > preferences.max_history_items
                    })
                    .collect())
            }
        }
    }

    fn eligible_cleanup_candidates(
        candidates: Vec<CleanupCandidate>,
        request: &ClipboardCleanupRequest,
    ) -> Vec<CleanupCandidate> {
        candidates
            .into_iter()
            .filter(|candidate| match request.scope {
                ClipboardCleanupScope::RetentionPolicy => !candidate.is_pinned,
                ClipboardCleanupScope::Selected | ClipboardCleanupScope::AllUnpinned => {
                    !candidate.is_pinned || request.include_pinned
                }
            })
            .collect()
    }

    /// Applies the configured retention policy without requiring a UI cleanup
    /// action. This runs at open, after capture, and immediately after a
    /// preference change so an expired unpinned record cannot quietly remain
    /// in active history. Pinned clips are filtered by the policy itself.
    fn enforce_retention_policy(&self) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let preferences = Self::capture_preferences_with_connection(&connection)?;
        self.enforce_retention_policy_with_connection(&connection, &preferences)
    }

    fn enforce_retention_policy_with_connection(
        &self,
        connection: &Connection,
        preferences: &CapturePreferences,
    ) -> AppResult<()> {
        // The normal read path has no expired or over-limit Item. Keep the
        // exact cleanup behavior, but avoid hydrating the whole History when
        // a scalar SQLite check proves that no row can be eligible.
        let (active_unpinned, earliest_expiry): (i64, Option<String>) = connection.query_row(
            "SELECT COUNT(*), MIN(retention_until)
             FROM clipboard_items
             WHERE deleted_at IS NULL AND is_pinned = 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if active_unpinned <= i64::from(preferences.max_history_items)
            && earliest_expiry
                .as_deref()
                .is_none_or(|expiry| expiry > timestamp().as_str())
        {
            return Ok(());
        }
        let request = ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::RetentionPolicy,
            item_ids: Vec::new(),
            include_pinned: false,
        };
        let candidates =
            Self::cleanup_candidates_with_connection(connection, preferences, &request)?;
        let eligible = Self::eligible_cleanup_candidates(candidates, &request);
        self.move_candidates_to_recycle_bin_with_connection(connection, preferences, &eligible)?;
        Ok(())
    }

    fn purge_expired_recycle_bin(&self) -> AppResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        Self::purge_expired_recycle_bin_with_connection(self, &connection)
    }

    fn purge_expired_recycle_bin_with_connection(&self, connection: &Connection) -> AppResult<()> {
        let now = timestamp();
        let blob_keys = Self::blob_keys_for_expired_recycle_bin_with_connection(connection, &now)?;
        connection.execute(
            "DELETE FROM clipboard_items
             WHERE deleted_at IS NOT NULL AND recycle_expires_at <= ?1",
            [now],
        )?;
        self.cleanup_unreferenced_blobs_with_connection(connection, &blob_keys)
    }

    fn move_candidates_to_recycle_bin_with_connection(
        &self,
        connection: &Connection,
        preferences: &CapturePreferences,
        candidates: &[CleanupCandidate],
    ) -> AppResult<u32> {
        let deleted_at = timestamp();
        let maximum_retention_days = recycle_bin_max_retention_days(preferences);
        let mut moved = 0_u32;
        for candidate in candidates {
            let recycle_expires_at = recycle_expiry_for(
                &candidate.retention_until,
                &deleted_at,
                maximum_retention_days,
            );
            moved += connection.execute(
                "UPDATE clipboard_items
                 SET deleted_at = ?1,
                     recycle_expires_at = ?2
                 WHERE id = ?3 AND deleted_at IS NULL",
                params![deleted_at, recycle_expires_at, candidate.id],
            )? as u32;
        }
        Ok(moved)
    }

    fn capture_preferences_with_connection(
        connection: &Connection,
    ) -> AppResult<CapturePreferences> {
        connection
            .query_row(
                "SELECT capture_paused, sensitive_content_policy, retention_days, max_history_items,
                    denied_apps_json, pause_reason, labs_enabled, diagnostics_enabled,
                    paste_behavior, quick_paste_shortcut, stack_shortcut, onboarding_completed
             FROM capture_preferences WHERE singleton = 1",
                [],
                |row| {
                    let sensitive_content_policy: String = row.get(1)?;
                    let sensitive_content_policy =
                        SensitiveContentPolicy::from_storage_value(&sensitive_content_policy)
                            .ok_or_else(|| {
                                rusqlite::Error::InvalidColumnType(
                                    1,
                                    "sensitive_content_policy".to_owned(),
                                    Type::Text,
                                )
                            })?;
                    let denied_apps: String = row.get(4)?;
                    let paste_behavior: String = row.get(8)?;
                    let paste_behavior = PasteBehavior::from_storage_value(&paste_behavior)
                        .ok_or_else(|| {
                            rusqlite::Error::InvalidColumnType(
                                8,
                                "paste_behavior".to_owned(),
                                Type::Text,
                            )
                        })?;
                    Ok(CapturePreferences {
                        capture_paused: row.get::<_, i64>(0)? != 0,
                        sensitive_pause_enabled: sensitive_content_policy.is_strict(),
                        sensitive_content_policy,
                        retention_days: row.get(2)?,
                        max_history_items: row.get(3)?,
                        denied_apps: serde_json::from_str(&denied_apps).unwrap_or_default(),
                        pause_reason: row.get(5)?,
                        labs_enabled: row.get::<_, i64>(6)? != 0,
                        diagnostics_enabled: row.get::<_, i64>(7)? != 0,
                        paste_behavior,
                        quick_paste_shortcut: row.get(9)?,
                        stack_shortcut: row.get(10)?,
                        onboarding_completed: row.get::<_, i64>(11)? != 0,
                    })
                },
            )
            .map_err(AppError::from)
    }

    fn recalculate_active_retention_with_connection(
        connection: &Connection,
        retention_days: u32,
    ) -> AppResult<()> {
        let mut statement = connection.prepare(
            "SELECT id, created_at
             FROM clipboard_items
             WHERE deleted_at IS NULL AND is_pinned = 0",
        )?;
        let items = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);

        for (id, created_at) in items {
            connection.execute(
                "UPDATE clipboard_items SET retention_until = ?1 WHERE id = ?2",
                params![retention_until_for(&created_at, retention_days), id],
            )?;
        }
        Ok(())
    }

    fn blob_keys_for_recycled_ids_with_connection(
        connection: &Connection,
        item_ids: &HashSet<String>,
    ) -> AppResult<Vec<String>> {
        if item_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = connection.prepare(
            "SELECT representations.storage_key, items.id
             FROM clipboard_representations AS representations
             INNER JOIN clipboard_items AS items
                ON items.id = representations.clipboard_item_id
             WHERE items.deleted_at IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.filter_map(|row| match row {
            Ok((storage_key, id)) if item_ids.contains(&id) => Some(Ok(storage_key)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)
    }

    fn blob_keys_for_recycle_bin_with_connection(
        connection: &Connection,
    ) -> AppResult<Vec<String>> {
        let mut statement = connection.prepare(
            "SELECT representations.storage_key
             FROM clipboard_representations AS representations
             INNER JOIN clipboard_items AS items
                ON items.id = representations.clipboard_item_id
             WHERE items.deleted_at IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn blob_keys_for_expired_recycle_bin_with_connection(
        connection: &Connection,
        now: &str,
    ) -> AppResult<Vec<String>> {
        let mut statement = connection.prepare(
            "SELECT representations.storage_key
             FROM clipboard_representations AS representations
             INNER JOIN clipboard_items AS items
                ON items.id = representations.clipboard_item_id
             WHERE items.deleted_at IS NOT NULL
               AND items.recycle_expires_at <= ?1",
        )?;
        let rows = statement.query_map([now], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn cleanup_unreferenced_blobs_with_connection(
        &self,
        connection: &Connection,
        storage_keys: &[String],
    ) -> AppResult<()> {
        for storage_key in storage_keys {
            self.cleanup_unreferenced_blob_with_connection(connection, storage_key)?;
        }
        Ok(())
    }

    fn cleanup_unreferenced_blob_with_connection(
        &self,
        connection: &Connection,
        storage_key: &str,
    ) -> AppResult<()> {
        let references: i64 = connection.query_row(
            "SELECT COUNT(*) FROM clipboard_representations WHERE storage_key = ?1",
            [storage_key],
            |row| row.get(0),
        )?;
        if references == 0 {
            self.blob_store
                .remove_unreferenced(storage_key)
                .map_err(|error| AppError::Media(error.to_string()))?;
        }
        Ok(())
    }

    fn upsert_context_enrichment(
        connection: &Connection,
        clipboard_item_id: &str,
        content: &str,
        updated_at: &str,
    ) -> AppResult<()> {
        let enrichment = enrich_text(content);
        let enrichment_json = serde_json::to_string(&enrichment)?;

        connection.execute(
            "INSERT INTO context_enrichments
             (clipboard_item_id, schema_version, enricher_id, enricher_version, enrichment_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(clipboard_item_id) DO UPDATE SET
                 schema_version = excluded.schema_version,
                 enricher_id = excluded.enricher_id,
                 enricher_version = excluded.enricher_version,
                 enrichment_json = excluded.enrichment_json,
                 updated_at = excluded.updated_at",
            params![
                clipboard_item_id,
                enrichment.metadata.schema_version,
                enrichment.metadata.enricher_id,
                enrichment.metadata.enricher_version,
                enrichment_json,
                updated_at,
            ],
        )?;
        Ok(())
    }

    fn local_text_action_with_connection(
        connection: &Connection,
        id: &str,
    ) -> AppResult<LocalTextAction> {
        connection
            .query_row(
                "SELECT id, name, transform, pipeline_json, shortcut_slot, is_builtin, created_at, updated_at
                 FROM local_text_actions WHERE id = ?1",
                [id],
                map_local_text_action,
            )
            .optional()?
            .ok_or(AppError::NotFound)
    }

    fn insert_action_audit_with_connection(
        connection: &Connection,
        audit: &ActionAuditRecord,
    ) -> AppResult<()> {
        connection.execute(
            "INSERT INTO action_audit_log
             (id, schema_version, action_id, action_name, transform, execution_mode, status,
              clipboard_item_id, input_content_hash, input_preview, output_content_hash,
              output_preview, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                audit.id,
                audit.schema_version,
                audit.action_id,
                audit.action_name,
                audit.transform.map(LocalTextTransform::as_storage_value),
                audit.execution_mode.as_storage_value(),
                audit.status.as_storage_value(),
                audit.clipboard_item_id,
                audit.input_content_hash,
                audit.input_preview,
                audit.output_content_hash,
                audit.output_preview,
                audit.created_at,
            ],
        )?;
        Ok(())
    }

    fn get_by_id_with_connection(connection: &Connection, id: &str) -> AppResult<ClipboardItem> {
        let mut item = connection
            .query_row(
                "SELECT id, content, kind, source_app, created_at, updated_at, is_pinned, copy_count,
                        restored_at, global_id, device_id, group_id, version, retention_until,
                        last_used_at, search_text
                 FROM clipboard_items
                 WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                map_clipboard_item,
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        item.representations = Self::representations_with_connection(connection, &item.id)?;
        item.occurrences = Self::occurrences_with_connection(connection, &item.id)?;
        item.occurrence_count = item.occurrences.len() as u32;
        item.tags = Self::tags_with_connection(connection, &item.id)?;
        Ok(item)
    }

    fn attach_representations(
        connection: &Connection,
        items: &mut [ClipboardItem],
    ) -> AppResult<()> {
        for item in items.iter_mut() {
            item.representations = Self::representations_with_connection(connection, &item.id)?;
        }
        Self::attach_occurrences(connection, items)?;
        Self::attach_tags(connection, items)
    }

    fn list_collections_with_connection(
        connection: &Connection,
    ) -> AppResult<Vec<ClipboardCollection>> {
        let mut statement = connection.prepare(
            "SELECT tag.label,
                    COUNT(DISTINCT item.id) AS item_count,
                    SUM(CASE WHEN item.is_pinned = 1 THEN 1 ELSE 0 END) AS saved_item_count
             FROM clipboard_item_tags AS tag
             INNER JOIN clipboard_items AS item ON item.id = tag.clipboard_item_id
             WHERE item.deleted_at IS NULL
             GROUP BY tag.label COLLATE NOCASE
             ORDER BY tag.label COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ClipboardCollection {
                id: None,
                name: row.get(0)?,
                kind: ClipboardCollectionKind::Manual,
                rule: None,
                item_count: row.get(1)?,
                saved_item_count: row.get(2)?,
            })
        })?;
        let mut collections = rows.collect::<Result<Vec<_>, _>>()?;
        let mut smart_statement = connection.prepare(
            "SELECT id, name, rule_json
             FROM clipboard_smart_collections
             ORDER BY name COLLATE NOCASE ASC",
        )?;
        let smart_rows = smart_statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in smart_rows {
            let (id, name, rule_json) = row?;
            let rule: ClipboardSmartCollectionRule =
                serde_json::from_str(&rule_json).map_err(|_| {
                    AppError::InvalidInput("Stored Smart Collection rule is invalid.".to_owned())
                })?;
            validate_smart_collection_rule(&rule)?;
            let (item_count, saved_item_count) =
                count_smart_collection_with_connection(connection, &rule)?;
            collections.push(ClipboardCollection {
                id: Some(id),
                name,
                kind: ClipboardCollectionKind::Smart,
                rule: Some(rule),
                item_count,
                saved_item_count,
            });
        }
        collections.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(collections)
    }

    fn smart_collection_rule_with_connection(
        connection: &Connection,
        id: &str,
    ) -> AppResult<ClipboardSmartCollectionRule> {
        let rule_json = connection
            .query_row(
                "SELECT rule_json FROM clipboard_smart_collections WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        let rule = serde_json::from_str(&rule_json).map_err(|_| {
            AppError::InvalidInput("Stored Smart Collection rule is invalid.".to_owned())
        })?;
        validate_smart_collection_rule(&rule)?;
        Ok(rule)
    }

    fn replace_tags_with_connection(
        connection: &Connection,
        id: &str,
        tags: &[String],
    ) -> AppResult<()> {
        for tag in tags {
            let conflicts_with_smart = connection.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM clipboard_smart_collections
                     WHERE name = ?1 COLLATE NOCASE
                 )",
                [tag],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if conflicts_with_smart {
                return Err(AppError::InvalidInput(
                    "A Smart Collection already uses this name.".to_owned(),
                ));
            }
        }
        connection.execute(
            "DELETE FROM clipboard_item_tags WHERE clipboard_item_id = ?1",
            [id],
        )?;
        let created_at = timestamp();
        for (position, tag) in tags.iter().enumerate() {
            connection.execute(
                "INSERT INTO clipboard_item_tags (clipboard_item_id, label, position, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, tag, position, created_at],
            )?;
        }
        Ok(())
    }

    fn attach_tags(connection: &Connection, items: &mut [ClipboardItem]) -> AppResult<()> {
        for item in items.iter_mut() {
            item.tags.clear();
        }
        let item_indexes = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.clone(), index))
            .collect::<HashMap<_, _>>();
        for chunk_start in (0..items.len()).step_by(400) {
            let chunk_ids = items[chunk_start..items.len().min(chunk_start + 400)]
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>();
            let placeholders = std::iter::repeat_n("?", chunk_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT clipboard_item_id, label
                 FROM clipboard_item_tags
                 WHERE clipboard_item_id IN ({placeholders})
                 ORDER BY clipboard_item_id ASC, position ASC"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params_from_iter(chunk_ids.iter().map(String::as_str)),
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            for row in rows {
                let (item_id, tag) = row?;
                if let Some(index) = item_indexes.get(&item_id) {
                    items[*index].tags.push(tag);
                }
            }
        }
        Ok(())
    }

    fn attach_occurrences(connection: &Connection, items: &mut [ClipboardItem]) -> AppResult<()> {
        for item in items.iter_mut() {
            item.occurrences.clear();
            item.occurrence_count = 0;
        }
        let item_indexes = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.clone(), index))
            .collect::<HashMap<_, _>>();
        for chunk_start in (0..items.len()).step_by(400) {
            let chunk_ids = items[chunk_start..items.len().min(chunk_start + 400)]
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>();
            let placeholders = std::iter::repeat_n("?", chunk_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, clipboard_item_id, source_app, device_id, occurred_at
                 FROM clipboard_occurrences
                 WHERE clipboard_item_id IN ({placeholders})
                 ORDER BY occurred_at DESC, id DESC"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params_from_iter(chunk_ids.iter().map(String::as_str)),
                |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        ClipboardOccurrence {
                            id: row.get(0)?,
                            source_app: row.get(2)?,
                            device_id: row.get(3)?,
                            occurred_at: row.get(4)?,
                        },
                    ))
                },
            )?;
            for row in rows {
                let (item_id, occurrence) = row?;
                if let Some(index) = item_indexes.get(&item_id) {
                    items[*index].occurrences.push(occurrence);
                    items[*index].occurrence_count += 1;
                }
            }
        }
        Ok(())
    }

    fn occurrences_with_connection(
        connection: &Connection,
        clipboard_item_id: &str,
    ) -> AppResult<Vec<ClipboardOccurrence>> {
        let mut statement = connection.prepare(
            "SELECT id, source_app, device_id, occurred_at
             FROM clipboard_occurrences
             WHERE clipboard_item_id = ?1
             ORDER BY occurred_at DESC, id DESC",
        )?;
        let rows = statement.query_map([clipboard_item_id], |row| {
            Ok(ClipboardOccurrence {
                id: row.get(0)?,
                source_app: row.get(1)?,
                device_id: row.get(2)?,
                occurred_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn representations_with_connection(
        connection: &Connection,
        clipboard_item_id: &str,
    ) -> AppResult<Vec<StoredBlob>> {
        let mut statement = connection.prepare(
            "SELECT storage_key, content_hash, kind, mime_type, display_name,
                    image_width, image_height, byte_size
             FROM clipboard_representations
             WHERE clipboard_item_id = ?1
             ORDER BY created_at ASC, storage_key ASC",
        )?;
        let rows = statement.query_map([clipboard_item_id], map_stored_blob)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn tags_with_connection(
        connection: &Connection,
        clipboard_item_id: &str,
    ) -> AppResult<Vec<String>> {
        let table_exists = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'clipboard_item_tags'
             )",
            [],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if !table_exists {
            return Ok(Vec::new());
        }
        let mut statement = connection.prepare(
            "SELECT label FROM clipboard_item_tags
             WHERE clipboard_item_id = ?1
             ORDER BY position ASC",
        )?;
        let rows = statement.query_map([clipboard_item_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn quick_paste_text_by_item_with_connection(
        connection: &Connection,
        items: &[ClipboardItem],
        source: QuickPasteAuxiliaryText,
    ) -> AppResult<HashMap<String, String>> {
        let table = match source {
            QuickPasteAuxiliaryText::Note => "clipboard_item_notes",
            QuickPasteAuxiliaryText::ImageText => "clipboard_image_text_extractions",
        };
        let mut by_id = HashMap::new();
        for chunk in items.chunks(400) {
            let placeholders = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT clipboard_item_id, text FROM {table} WHERE clipboard_item_id IN ({placeholders})"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(
                params_from_iter(chunk.iter().map(|item| item.id.as_str())),
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            by_id.extend(rows.collect::<Result<Vec<_>, _>>()?);
        }
        Ok(by_id)
    }
}

fn map_clipboard_item(row: &Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        content: row.get(1)?,
        kind: row.get(2)?,
        source_app: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        is_pinned: row.get::<_, i64>(6)? != 0,
        copy_count: row.get(7)?,
        restored_at: row.get(8)?,
        global_id: row.get(9)?,
        device_id: row.get(10)?,
        group_id: row.get(11)?,
        version: row.get(12)?,
        retention_until: row.get(13)?,
        last_used_at: row.get(14)?,
        occurrence_count: 0,
        occurrences: Vec::new(),
        representations: Vec::new(),
        tags: Vec::new(),
        text_in_image_match: false,
        note_match: false,
        search_text: row.get(15)?,
    })
}

fn map_clipboard_search_item(row: &Row<'_>) -> rusqlite::Result<ClipboardItem> {
    let mut item = map_clipboard_item(row)?;
    item.text_in_image_match = row.get::<_, i64>(16)? != 0;
    item.note_match = row.get::<_, i64>(17)? != 0;
    Ok(item)
}

fn ensure_active_image_item(connection: &Connection, id: &str) -> AppResult<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM clipboard_items
            WHERE id = ?1 AND deleted_at IS NULL AND kind = 'image'
         )",
        [id],
        |row| row.get::<_, i64>(0),
    )? != 0;
    if !exists {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn image_text_extraction_with_connection(
    connection: &Connection,
    id: &str,
) -> AppResult<Option<ClipboardImageTextExtraction>> {
    connection
        .query_row(
            "SELECT clipboard_item_id, text, extracted_at, updated_at
             FROM clipboard_image_text_extractions
             WHERE clipboard_item_id = ?1",
            [id],
            |row| {
                Ok(ClipboardImageTextExtraction {
                    clipboard_item_id: row.get(0)?,
                    text: row.get(1)?,
                    extracted_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
}

fn item_note_with_connection(
    connection: &Connection,
    id: &str,
) -> AppResult<Option<ClipboardItemNote>> {
    connection
        .query_row(
            "SELECT clipboard_item_id, text, created_at, updated_at
             FROM clipboard_item_notes WHERE clipboard_item_id = ?1",
            [id],
            |row| {
                Ok(ClipboardItemNote {
                    clipboard_item_id: row.get(0)?,
                    text: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
}

fn map_capture_status_event(row: &Row<'_>) -> rusqlite::Result<CaptureStatusEvent> {
    let reason_type: String = row.get(1)?;
    let reason_type = CaptureStatusReason::from_storage_value(&reason_type).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(1, "reason_type".to_owned(), Type::Text)
    })?;
    Ok(CaptureStatusEvent {
        id: row.get(0)?,
        reason_type,
        source_app: row.get(2)?,
        occurred_at: row.get(3)?,
        expires_at: row.get(4)?,
    })
}

fn map_local_link_copy_effect_claim(row: &Row<'_>) -> rusqlite::Result<LocalLinkCopyEffectClaim> {
    let state: String = row.get(1)?;
    let state = LocalLinkCopyEffectState::from_storage_value(&state)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(1, "state".to_owned(), Type::Text))?;
    Ok(LocalLinkCopyEffectClaim {
        transfer_id: row.get(0)?,
        state,
        effect_token: row.get(2)?,
        pasteboard_change_count: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn local_link_copy_effect_claim_with_connection(
    connection: &Connection,
    transfer_id: &str,
) -> AppResult<Option<LocalLinkCopyEffectClaim>> {
    connection
        .query_row(
            "SELECT transfer_id, state, effect_token, pasteboard_change_count,
                    created_at, updated_at
             FROM local_link_effect_claims WHERE transfer_id = ?1",
            [transfer_id],
            map_local_link_copy_effect_claim,
        )
        .optional()
        .map_err(AppError::from)
}

fn map_local_diagnostic_export_event(
    row: &Row<'_>,
) -> rusqlite::Result<LocalDiagnosticsExportEvent> {
    let event_type: String = row.get(0)?;
    let event_type =
        LocalDiagnosticEventType::from_storage_value(&event_type).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(0, "event_type".to_owned(), Type::Text)
        })?;
    let outcome: String = row.get(1)?;
    let outcome = LocalDiagnosticOutcome::from_storage_value(&outcome)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(1, "outcome".to_owned(), Type::Text))?;
    Ok(LocalDiagnosticsExportEvent {
        event_type,
        outcome,
        occurred_day: row.get(2)?,
        app_version: row.get(3)?,
    })
}

fn is_valid_local_diagnostic_event(event: &LocalDiagnosticEventInput) -> bool {
    use LocalDiagnosticEventType as Event;
    use LocalDiagnosticOutcome as Outcome;

    matches!(
        (event.event_type, event.outcome),
        (Event::FirstRecovery, Outcome::Started | Outcome::Completed)
            | (
                Event::DirectPaste,
                Outcome::Started
                    | Outcome::Completed
                    | Outcome::Degraded
                    | Outcome::Failed
                    | Outcome::Restricted
            )
            | (
                Event::RecycleBinRestore,
                Outcome::Restored | Outcome::Failed
            )
            | (Event::DuplicateGroupExpand, Outcome::Expanded)
            | (Event::ClipboardRestoreError, Outcome::Failed)
            | (Event::CapturePermissionRestricted, Outcome::Restricted)
            | (Event::SystemPolicyRestricted, Outcome::Restricted)
    )
}

fn local_diagnostic_day() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn local_diagnostics_summary_from_events(
    enabled: bool,
    events: &[LocalDiagnosticsExportEvent],
) -> LocalDiagnosticsSummary {
    use LocalDiagnosticEventType as Event;
    use LocalDiagnosticOutcome as Outcome;

    let first_recovery_attempts =
        count_local_diagnostics(events, Event::FirstRecovery, Outcome::Started);
    let first_recovery_completed =
        count_local_diagnostics(events, Event::FirstRecovery, Outcome::Completed);
    let direct_paste_attempts =
        count_local_diagnostics(events, Event::DirectPaste, Outcome::Started);
    let direct_paste_degraded =
        count_local_diagnostics(events, Event::DirectPaste, Outcome::Degraded);
    let recycle_bin_restore_count =
        count_local_diagnostics(events, Event::RecycleBinRestore, Outcome::Restored);
    let duplicate_group_expand_count =
        count_local_diagnostics(events, Event::DuplicateGroupExpand, Outcome::Expanded);

    let mut critical_counts =
        HashMap::<(LocalDiagnosticEventType, LocalDiagnosticOutcome), u32>::new();
    for event in events
        .iter()
        .filter(|event| matches!(event.outcome, Outcome::Failed | Outcome::Restricted))
    {
        *critical_counts
            .entry((event.event_type, event.outcome))
            .or_default() += 1;
    }
    let mut critical_error_categories = critical_counts
        .into_iter()
        .map(
            |((event_type, outcome), count)| LocalDiagnosticErrorCategory {
                event_type,
                outcome,
                count,
            },
        )
        .collect::<Vec<_>>();
    critical_error_categories.sort_by(|left, right| {
        left.event_type
            .as_storage_value()
            .cmp(right.event_type.as_storage_value())
            .then_with(|| {
                left.outcome
                    .as_storage_value()
                    .cmp(right.outcome.as_storage_value())
            })
    });

    LocalDiagnosticsSummary {
        enabled,
        stored_event_count: events.len() as u32,
        first_recovery: local_diagnostic_rate(first_recovery_attempts, first_recovery_completed),
        direct_paste: local_diagnostic_rate(direct_paste_attempts, direct_paste_degraded),
        recycle_bin_restore_count,
        duplicate_group_expand_count,
        critical_error_categories,
    }
}

fn count_local_diagnostics(
    events: &[LocalDiagnosticsExportEvent],
    event_type: LocalDiagnosticEventType,
    outcome: LocalDiagnosticOutcome,
) -> u32 {
    events
        .iter()
        .filter(|event| event.event_type == event_type && event.outcome == outcome)
        .count() as u32
}

fn local_diagnostic_rate(attempts: u32, completed_or_degraded: u32) -> LocalDiagnosticRate {
    let percent = (attempts > 0)
        .then(|| ((u64::from(completed_or_degraded) * 100) / u64::from(attempts)).min(100) as u8);
    LocalDiagnosticRate {
        attempts,
        completed_or_degraded,
        percent,
    }
}

fn map_stored_blob(row: &Row<'_>) -> rusqlite::Result<StoredBlob> {
    let kind: String = row.get(2)?;
    let kind = BlobKind::from_storage_value(&kind)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(2, "kind".to_owned(), Type::Text))?;
    let width: Option<u32> = row.get(5)?;
    let height: Option<u32> = row.get(6)?;
    let image_dimensions = match (width, height) {
        (Some(width), Some(height)) => Some(ImageDimensions { width, height }),
        (None, None) => None,
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                5,
                "image_width".to_owned(),
                Type::Integer,
            ));
        }
    };

    Ok(StoredBlob {
        storage_key: row.get(0)?,
        content_hash: row.get(1)?,
        kind,
        mime_type: row.get(3)?,
        display_name: row.get(4)?,
        image_dimensions,
        byte_size: row.get(7)?,
    })
}

fn map_local_text_action(row: &Row<'_>) -> rusqlite::Result<LocalTextAction> {
    let transform: String = row.get(2)?;
    let transform = LocalTextTransform::from_storage_value(&transform)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(2, "transform".to_owned(), Type::Text))?;

    let pipeline_json: Option<String> = row.get(3)?;
    let steps = match pipeline_json {
        Some(value) => {
            serde_json::from_str::<Vec<LocalTextPipelineStep>>(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(error))
            })?
        }
        None => vec![LocalTextPipelineStep { transform }],
    };

    Ok(LocalTextAction {
        id: row.get(0)?,
        name: row.get(1)?,
        transform,
        steps,
        shortcut_slot: row.get(4)?,
        is_builtin: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn map_action_audit(row: &Row<'_>) -> rusqlite::Result<ActionAuditRecord> {
    let transform = row
        .get::<_, Option<String>>(4)?
        .map(|value| {
            LocalTextTransform::from_storage_value(&value).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(4, "transform".to_owned(), Type::Text)
            })
        })
        .transpose()?;
    let execution_mode: String = row.get(5)?;
    let execution_mode =
        ActionExecutionMode::from_storage_value(&execution_mode).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(5, "execution_mode".to_owned(), Type::Text)
        })?;
    let status: String = row.get(6)?;
    let status = ActionAuditStatus::from_storage_value(&status)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(6, "status".to_owned(), Type::Text))?;

    Ok(ActionAuditRecord {
        id: row.get(0)?,
        schema_version: row.get(1)?,
        action_id: row.get(2)?,
        action_name: row.get(3)?,
        transform,
        execution_mode,
        status,
        clipboard_item_id: row.get(7)?,
        input_content_hash: row.get(8)?,
        input_preview: row.get(9)?,
        output_content_hash: row.get(10)?,
        output_preview: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn validate_clipboard_list_filters(filters: &ClipboardListFilters) -> AppResult<()> {
    const KINDS: [&str; 8] = [
        "text", "code", "command", "url", "color", "image", "richText", "file",
    ];
    if let Some(kind) = filters
        .kinds
        .iter()
        .find(|kind| !KINDS.contains(&kind.as_str()))
    {
        return Err(AppError::InvalidInput(format!(
            "Unsupported clipboard kind filter: {kind}"
        )));
    }
    Ok(())
}

fn validate_smart_collection_rule(rule: &ClipboardSmartCollectionRule) -> AppResult<()> {
    let filters = smart_rule_as_list_filters(rule.clone());
    validate_clipboard_list_filters(&filters)?;
    if rule.kinds.is_empty()
        && rule
            .source_app
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        && rule.pin_filter == ClipboardPinFilter::All
        && rule.time_filter == ClipboardTimeFilter::All
        && !rule.recently_used_only
        && !rule.local_link_only
    {
        return Err(AppError::InvalidInput(
            "A Smart Collection needs at least one supported filter.".to_owned(),
        ));
    }
    if let Some(source) = rule.source_app.as_deref() {
        if source.chars().count() > 255 || source.chars().any(char::is_control) {
            return Err(AppError::InvalidInput(
                "Smart Collection source applications must be at most 255 visible characters."
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn smart_rule_as_list_filters(rule: ClipboardSmartCollectionRule) -> ClipboardListFilters {
    ClipboardListFilters {
        kinds: rule.kinds,
        source_app: rule.source_app,
        collection: None,
        smart_collection_id: None,
        pin_filter: rule.pin_filter,
        time_filter: rule.time_filter,
        recently_used_only: rule.recently_used_only,
        local_link_only: rule.local_link_only,
    }
}

fn count_smart_collection_with_connection(
    connection: &Connection,
    rule: &ClipboardSmartCollectionRule,
) -> AppResult<(u32, u32)> {
    let filters = smart_rule_as_list_filters(rule.clone());
    let mut sql = "SELECT COUNT(*),
                          COALESCE(SUM(CASE WHEN items.is_pinned = 1 THEN 1 ELSE 0 END), 0)
                   FROM clipboard_items AS items WHERE items.deleted_at IS NULL"
        .to_owned();
    let mut values = Vec::new();
    append_clipboard_list_filters(&mut sql, &mut values, &filters, filters.pin_filter);
    connection
        .query_row(&sql, params_from_iter(values.iter()), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(AppError::from)
}

fn smart_collection_with_connection(
    connection: &Connection,
    id: &str,
) -> AppResult<ClipboardCollection> {
    let (name, rule_json) = connection
        .query_row(
            "SELECT name, rule_json FROM clipboard_smart_collections WHERE id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(AppError::NotFound)?;
    let rule: ClipboardSmartCollectionRule = serde_json::from_str(&rule_json).map_err(|_| {
        AppError::InvalidInput("Stored Smart Collection rule is invalid.".to_owned())
    })?;
    validate_smart_collection_rule(&rule)?;
    let (item_count, saved_item_count) = count_smart_collection_with_connection(connection, &rule)?;
    Ok(ClipboardCollection {
        id: Some(id.to_owned()),
        name,
        kind: ClipboardCollectionKind::Smart,
        rule: Some(rule),
        item_count,
        saved_item_count,
    })
}

fn ensure_collection_name_available(
    connection: &Connection,
    name: &str,
    excluding_smart_id: Option<&str>,
) -> AppResult<()> {
    let manual_exists = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM clipboard_item_tags WHERE label = ?1 COLLATE NOCASE
         )",
        [name],
        |row| row.get::<_, i64>(0),
    )? != 0;
    let smart_exists = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM clipboard_smart_collections
             WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2)
         )",
        params![name, excluding_smart_id],
        |row| row.get::<_, i64>(0),
    )? != 0;
    if manual_exists || smart_exists {
        return Err(AppError::InvalidInput(
            "A Collection with this name already exists.".to_owned(),
        ));
    }
    Ok(())
}

fn append_clipboard_list_filters(
    sql: &mut String,
    values: &mut Vec<Value>,
    filters: &ClipboardListFilters,
    pin_filter: ClipboardPinFilter,
) {
    if !filters.kinds.is_empty() {
        let placeholders = std::iter::repeat_n("?", filters.kinds.len())
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!(" AND items.kind IN ({placeholders})"));
        values.extend(filters.kinds.iter().cloned().map(Value::Text));
    }

    if let Some(source_app) = filters
        .source_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sql.push_str(
            " AND (items.source_app = ? COLLATE NOCASE
                   OR EXISTS (
                       SELECT 1 FROM clipboard_occurrences AS source_occurrence
                       WHERE source_occurrence.clipboard_item_id = items.id
                         AND source_occurrence.source_app = ? COLLATE NOCASE
                   ))",
        );
        values.push(Value::Text(source_app.to_owned()));
        values.push(Value::Text(source_app.to_owned()));
    }

    if let Some(collection) = filters
        .collection
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sql.push_str(
            " AND EXISTS (
                  SELECT 1 FROM clipboard_item_tags AS collection_tag
                  WHERE collection_tag.clipboard_item_id = items.id
                    AND collection_tag.label = ? COLLATE NOCASE
              )",
        );
        values.push(Value::Text(collection.to_owned()));
    }

    match pin_filter {
        ClipboardPinFilter::All => {}
        ClipboardPinFilter::Pinned => sql.push_str(" AND items.is_pinned = 1"),
        ClipboardPinFilter::Unpinned => sql.push_str(" AND items.is_pinned = 0"),
    }

    let time_delta = match filters.time_filter {
        ClipboardTimeFilter::All => None,
        ClipboardTimeFilter::Past24Hours => Some(chrono::Duration::hours(24)),
        ClipboardTimeFilter::Past7Days => Some(chrono::Duration::days(7)),
        ClipboardTimeFilter::Past30Days => Some(chrono::Duration::days(30)),
    };
    if let Some(time_delta) = time_delta {
        sql.push_str(
            " AND COALESCE(
                  (SELECT MAX(time_occurrence.occurred_at)
                   FROM clipboard_occurrences AS time_occurrence
                   WHERE time_occurrence.clipboard_item_id = items.id),
                  items.created_at
              ) >= ?",
        );
        values.push(Value::Text(
            (Utc::now() - time_delta).to_rfc3339_opts(SecondsFormat::Millis, true),
        ));
    }

    if filters.recently_used_only {
        sql.push_str(" AND items.copy_count > 0");
    }
    if filters.local_link_only {
        sql.push_str(" AND items.source_app LIKE 'Local Link from %'");
    }
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn is_textual_item_kind(kind: &str) -> bool {
    matches!(
        kind,
        "text" | "code" | "command" | "url" | "color" | "richText"
    )
}

/// Local Link v0.7 narrows the broader clipboard text family to plain text
/// sources. Rich text is deliberately excluded: its formatting and embedded
/// references are not part of the Local Link Text Beta contract.
fn is_local_link_text_item_kind(kind: &str) -> bool {
    matches!(kind, "text" | "code" | "command" | "url" | "color")
}

fn is_local_link_receiver_active_status(status: &str) -> bool {
    matches!(status, "awaitingReceiver" | "viewed")
}

fn validate_local_link_effect_token(effect_token: &str) -> AppResult<()> {
    if !(16..=128).contains(&effect_token.len())
        || !effect_token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(AppError::InvalidInput(
            "A Local Link effect marker must be 16-128 printable ASCII bytes.".to_owned(),
        ));
    }
    Ok(())
}

fn content_kind_label(kind: &str) -> &'static str {
    match kind {
        "url" => "link",
        "color" => "colour value",
        "code" => "code clip",
        "command" => "command clip",
        _ => "text clip",
    }
}

fn retention_until_for(captured_at: &str, retention_days: u32) -> String {
    let captured_at = chrono::DateTime::parse_from_rfc3339(captured_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    (captured_at + chrono::Duration::days(i64::from(retention_days)))
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn recycle_bin_max_retention_days(preferences: &CapturePreferences) -> i64 {
    if preferences.sensitive_content_policy.is_strict() {
        STRICT_RECYCLE_BIN_MAX_RETENTION_DAYS
    } else {
        RECYCLE_BIN_MAX_RETENTION_DAYS
    }
}

fn recycle_expiry_for(_retention_until: &str, deleted_at: &str, maximum_days: i64) -> String {
    let deleted_at = chrono::DateTime::parse_from_rfc3339(deleted_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    // Recycle recovery is intentionally independent from active-history
    // retention. A manual delete must never become permanent immediately just
    // because the original history deadline is close or already elapsed.
    (deleted_at + chrono::Duration::days(maximum_days)).to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn cleanup_rule_conditions(
    request: &ClipboardCleanupRequest,
    preferences: &CapturePreferences,
) -> Vec<String> {
    let mut conditions = match request.scope {
        ClipboardCleanupScope::Selected => vec!["Only the selected clips are included.".to_owned()],
        ClipboardCleanupScope::AllUnpinned => {
            vec!["All unpinned active clips are included.".to_owned()]
        }
        ClipboardCleanupScope::RetentionPolicy => vec![format!(
            "Unpinned clips older than {} days or beyond the {}-clip limit are included.",
            preferences.retention_days, preferences.max_history_items
        )],
    };
    if request.scope == ClipboardCleanupScope::RetentionPolicy || !request.include_pinned {
        conditions.push("Pinned clips are skipped by default.".to_owned());
    } else {
        conditions.push("Pinned clips are included after explicit confirmation.".to_owned());
    }
    let recovery_days = recycle_bin_max_retention_days(preferences);
    conditions.push(format!(
        "Recovery is available for {} {} after deletion.",
        recovery_days,
        if recovery_days == 1 { "day" } else { "days" }
    ));
    conditions
}

fn blob_root_for_database(path: &Path) -> (PathBuf, Option<PathBuf>) {
    if path == Path::new(":memory:") {
        let root = std::env::temp_dir().join(format!("clipriva-test-blobs-{}", Uuid::new_v4()));
        return (root.clone(), Some(root));
    }

    let root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("blobs");
    (root, None)
}

fn image_preview(dimensions: Option<ImageDimensions>) -> String {
    match dimensions {
        Some(dimensions) => format!("Image · {} × {}", dimensions.width, dimensions.height),
        None => "Image".to_owned(),
    }
}

fn file_reference_preview(paths: &[&str]) -> String {
    let names = paths
        .iter()
        .filter_map(|path| Path::new(path).file_name().and_then(|name| name.to_str()))
        .take(3)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return if paths.len() == 1 {
            "File reference".to_owned()
        } else {
            format!("{} file references", paths.len())
        };
    }

    let hidden = paths.len().saturating_sub(names.len());
    let mut preview = names.join(" · ");
    if hidden > 0 {
        preview.push_str(&format!(" · +{hidden} more"));
    }
    preview
}

fn file_reference_search_text(paths: &[&str]) -> String {
    paths
        .iter()
        .filter_map(|path| Path::new(path).file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn represented_content_hash(kind: &str, content: &str, representations: &[StoredBlob]) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(content.as_bytes());
    for representation in representations {
        digest.update([0]);
        digest.update(representation.kind.as_str().as_bytes());
        digest.update([0]);
        digest.update(representation.mime_type.as_bytes());
        digest.update([0]);
        digest.update(representation.content_hash.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn normalize_quick_paste_text(value: &str) -> String {
    value
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quick_paste_query_anchor(query: &str) -> &str {
    query
        .split(' ')
        .max_by_key(|term| term.len())
        .unwrap_or(query)
}

#[derive(Debug, Clone, Copy)]
enum QuickPasteAuxiliaryText {
    Note,
    ImageText,
}

#[derive(Debug, Clone, Copy)]
struct QuickPasteMatchEvidence {
    match_kind: QuickPasteMatchKind,
    match_field: QuickPasteMatchField,
}

fn quick_paste_match(
    item: &ClipboardItem,
    note: Option<&str>,
    image_text: Option<&str>,
    query: &str,
) -> Option<QuickPasteMatchEvidence> {
    let mut best = None;
    let mut consider = |match_field: QuickPasteMatchField, value: &str| {
        if let Some(match_kind) =
            quick_paste_text_match_kind(&normalize_quick_paste_text(value), query)
        {
            let next = QuickPasteMatchEvidence {
                match_kind,
                match_field,
            };
            let next_rank = (
                quick_paste_match_rank(next.match_kind),
                quick_paste_field_rank(next.match_field),
            );
            if best.is_none_or(|previous: QuickPasteMatchEvidence| {
                next_rank
                    < (
                        quick_paste_match_rank(previous.match_kind),
                        quick_paste_field_rank(previous.match_field),
                    )
            }) {
                best = Some(next);
            }
        }
    };

    consider(QuickPasteMatchField::Content, &item.content);
    if item.search_text != item.content {
        consider(QuickPasteMatchField::DisplayName, &item.search_text);
    }
    if let Some(source) = item.source_app.as_deref() {
        consider(QuickPasteMatchField::SourceApp, source);
    }
    for source in item
        .occurrences
        .iter()
        .filter_map(|occurrence| occurrence.source_app.as_deref())
    {
        consider(QuickPasteMatchField::SourceApp, source);
    }
    for tag in &item.tags {
        consider(QuickPasteMatchField::Tag, tag);
    }
    if let Some(note) = note {
        consider(QuickPasteMatchField::Note, note);
    }
    if let Some(text) = image_text {
        consider(QuickPasteMatchField::ImageText, text);
    }
    best
}

fn quick_paste_field_rank(field: QuickPasteMatchField) -> u8 {
    match field {
        QuickPasteMatchField::Content => 0,
        QuickPasteMatchField::DisplayName => 1,
        QuickPasteMatchField::SourceApp => 2,
        QuickPasteMatchField::Tag => 3,
        QuickPasteMatchField::Note => 4,
        QuickPasteMatchField::ImageText => 5,
    }
}

fn normalize_clipboard_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for raw_tag in tags {
        let tag = raw_tag.split_whitespace().collect::<Vec<_>>().join(" ");
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > MAX_CLIPBOARD_TAG_CHARACTERS || tag.chars().any(char::is_control) {
            return Err(AppError::InvalidInput(format!(
                "Clipboard tags must contain 1 to {MAX_CLIPBOARD_TAG_CHARACTERS} visible characters."
            )));
        }
        let comparison_key = tag.to_lowercase();
        if seen.insert(comparison_key) {
            normalized.push(tag);
        }
        if normalized.len() > MAX_CLIPBOARD_TAGS {
            return Err(AppError::InvalidInput(format!(
                "A clipboard item can have at most {MAX_CLIPBOARD_TAGS} tags."
            )));
        }
    }
    Ok(normalized)
}

fn normalized_collection_lookup(value: &str) -> AppResult<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let character_count = normalized.chars().count();
    if character_count == 0
        || character_count > MAX_CLIPBOARD_TAG_CHARACTERS
        || normalized.chars().any(char::is_control)
    {
        return Err(AppError::InvalidInput(format!(
            "Collection names must contain 1 to {MAX_CLIPBOARD_TAG_CHARACTERS} visible characters."
        )));
    }
    Ok(normalized)
}

fn quick_paste_text_match_kind(value: &str, query: &str) -> Option<QuickPasteMatchKind> {
    if value == query {
        return Some(QuickPasteMatchKind::Exact);
    }
    if value.starts_with(query) {
        return Some(QuickPasteMatchKind::Prefix);
    }
    value
        .contains(query)
        .then_some(QuickPasteMatchKind::Contains)
}

fn quick_paste_match_rank(match_kind: QuickPasteMatchKind) -> u8 {
    match match_kind {
        QuickPasteMatchKind::Exact => 0,
        QuickPasteMatchKind::Prefix => 1,
        QuickPasteMatchKind::Contains => 2,
        QuickPasteMatchKind::Suggestion => 3,
    }
}

fn compare_quick_paste_search_items(
    left: &QuickPasteSearchItem,
    right: &QuickPasteSearchItem,
) -> std::cmp::Ordering {
    quick_paste_match_rank(left.match_kind)
        .cmp(&quick_paste_match_rank(right.match_kind))
        .then_with(|| right.item.is_pinned.cmp(&left.item.is_pinned))
        .then_with(|| most_recent_activity_at(&right.item).cmp(most_recent_activity_at(&left.item)))
        .then_with(|| right.item.copy_count.cmp(&left.item.copy_count))
        .then_with(|| left.item.global_id.cmp(&right.item.global_id))
        .then_with(|| left.item.id.cmp(&right.item.id))
}

fn most_recent_activity_at(item: &ClipboardItem) -> &str {
    let last_occurrence = item
        .occurrences
        .first()
        .map(|occurrence| occurrence.occurred_at.as_str())
        .unwrap_or(item.created_at.as_str());
    item.last_used_at
        .as_deref()
        .filter(|last_used_at| *last_used_at > last_occurrence)
        .unwrap_or(last_occurrence)
}

fn zero_input_quick_paste_suggestions(
    mut candidates: Vec<ClipboardItem>,
    visible_window: usize,
) -> Vec<QuickPasteSearchItem> {
    candidates.sort_by(|left, right| {
        most_recent_activity_at(right)
            .cmp(most_recent_activity_at(left))
            .then_with(|| right.copy_count.cmp(&left.copy_count))
            .then_with(|| left.global_id.cmp(&right.global_id))
            .then_with(|| left.id.cmp(&right.id))
    });

    // A pin is a deliberate recall signal, but it should not permanently take
    // over the first result. Ensure one is visible in the compact initial set
    // when possible, retaining the recency order everywhere else.
    if visible_window > 0
        && !candidates
            .iter()
            .take(visible_window)
            .any(|item| item.is_pinned)
    {
        if let Some(pinned_index) = candidates.iter().position(|item| item.is_pinned) {
            let pinned = candidates.remove(pinned_index);
            candidates.insert(
                visible_window.saturating_sub(1).min(candidates.len()),
                pinned,
            );
        }
    }

    candidates
        .into_iter()
        .map(|item| QuickPasteSearchItem {
            item,
            match_kind: QuickPasteMatchKind::Suggestion,
            match_field: None,
        })
        .collect()
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn fts_prefix_query(value: &str) -> Option<String> {
    let terms = value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>();

    (!terms.is_empty()).then(|| terms.join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::{
        file_reference_preview, file_reference_search_text, fts_prefix_query, migrations, Database,
        LocalLinkCopyClaimOutcome, LocalLinkCopyEffectState, LocalLinkSaveFinalization,
        LocalLinkTerminalCasOutcome, QuickPasteMatchField, QuickPasteMatchKind,
    };
    use crate::actions::CreateLocalTextAction;
    use crate::clipboard::image_capture::{encode_rgba_png, CapturedImage};
    use crate::clipboard::policy::{
        PasteBehavior, SensitiveContentPolicy, DEFAULT_QUICK_PASTE_SHORTCUT,
    };
    use crate::media::{BlobInput, BlobKind, BlobStore, ImageDimensions};
    use crate::models::{
        CaptureStatusReason, ClipboardCleanupRequest, ClipboardCleanupScope, ClipboardCollection,
        ClipboardCollectionKind, ClipboardImageTextExtractionStatus,
        ClipboardItemNoteMutationStatus, ClipboardListFilters, ClipboardPinFilter,
        ClipboardSmartCollectionInput, ClipboardSmartCollectionRule, ClipboardTimeFilter,
        LocalDiagnosticEventInput, LocalDiagnosticEventType, LocalDiagnosticOutcome,
    };
    use rusqlite::{params, Connection};

    fn enable_labs(database: &Database) {
        let mut preferences = database.capture_preferences().unwrap();
        preferences.labs_enabled = true;
        database.update_capture_preferences(&preferences).unwrap();
    }

    fn seed_local_link_transfer(
        database: &Database,
        transfer_id: &str,
        direction: &str,
        status: &str,
    ) {
        let connection = database.connection.lock().unwrap();
        connection
            .execute(
                "INSERT OR IGNORE INTO local_link_devices
                 (device_id, display_name, peer_public_key, public_key_fingerprint,
                  trust_status, paired_at, trusted_at, protocol_min, protocol_max,
                  trust_duration)
                 VALUES ('atomic-peer', 'Atomic Peer', X'01', 'A1B2', 'trusted',
                         '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z',
                         2, 2, 'thirtyDays')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO local_link_transfers
                 (id, device_id, direction, status, item_kind, byte_size,
                  created_at, updated_at, expires_at)
                 VALUES (?1, 'atomic-peer', ?2, ?3, 'text', 32,
                         '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z',
                         '2026-07-31T00:01:00.000Z')",
                params![transfer_id, direction, status],
            )
            .unwrap();
    }

    #[test]
    fn collapses_only_consecutive_canonical_text_captures() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let first = database
            .capture_text(" ClipRiva\tGuide\r\n", Some("Notes"))
            .unwrap()
            .unwrap();
        let second = database
            .capture_text("ClipRiva Guide\n", Some("Terminal"))
            .unwrap()
            .unwrap();
        let intervening = database
            .capture_text("an intervening clipboard event", None)
            .unwrap()
            .unwrap();
        let later_repeat = database
            .capture_text("ClipRiva Guide", None)
            .unwrap()
            .unwrap();
        let different_case = database
            .capture_text("clipriva guide", None)
            .unwrap()
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(first.global_id, second.global_id);
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(second.group_id, None);
        assert_eq!(second.version, 1);
        assert_eq!(second.occurrence_count, 2);
        assert_eq!(second.occurrences.len(), 2);
        assert!(second
            .occurrences
            .iter()
            .any(|occurrence| occurrence.source_app.as_deref() == Some("Notes")));
        assert!(second
            .occurrences
            .iter()
            .any(|occurrence| occurrence.source_app.as_deref() == Some("Terminal")));
        assert_eq!(second.content, "ClipRiva Guide\n");
        assert_ne!(first.id, intervening.id);
        assert_ne!(first.id, later_repeat.id);
        assert_ne!(first.id, different_case.id);
        assert_eq!(later_repeat.occurrence_count, 1);
        assert_eq!(database.list("", false, 100, 0).unwrap().len(), 4);
    }

    #[test]
    fn captures_an_image_with_a_local_representation_and_cleans_its_blob_after_permanent_delete() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[255, 0, 0, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };

        let item = database
            .capture_image(&image, Some("Preview"))
            .unwrap()
            .unwrap();
        assert_eq!(item.kind, "image");
        assert_eq!(item.representations.len(), 1);
        assert_eq!(
            item.representations[0].image_dimensions,
            Some(image.dimensions)
        );
        let storage_key = item.representations[0].storage_key.clone();
        assert!(!database.read_blob(&storage_key).unwrap().is_empty());

        database.delete(&item.id).unwrap();
        assert!(!database.read_blob(&storage_key).unwrap().is_empty());
        database
            .permanently_delete_recycled_items(std::slice::from_ref(&item.id))
            .unwrap();
        assert!(database.read_blob(&storage_key).is_err());
    }

    #[test]
    fn manual_image_text_is_searchable_marked_and_user_deletable() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[255, 255, 255, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };
        let item = database
            .capture_image(&image, Some("Synthetic OCR Test"))
            .unwrap()
            .unwrap();

        let saved = database
            .save_image_text_extraction(&item.id, "Quarterly launch checklist")
            .unwrap();
        assert_eq!(saved.status, ClipboardImageTextExtractionStatus::Extracted);
        assert_eq!(
            database
                .image_text_extraction(&item.id)
                .unwrap()
                .unwrap()
                .text,
            "Quarterly launch checklist"
        );
        let matches = database.list("quarterly launch", false, 20, 0).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, item.id);
        assert!(matches[0].text_in_image_match);
        let quick = database
            .quick_paste_search("quarterly launch", false, 9, 0)
            .unwrap();
        assert_eq!(quick.len(), 1);
        assert_eq!(quick[0].match_field, Some(QuickPasteMatchField::ImageText));
        assert!(!serde_json::to_string(&quick)
            .unwrap()
            .contains("Quarterly launch checklist"));

        assert!(database.delete_image_text_extraction(&item.id).unwrap());
        assert!(database.image_text_extraction(&item.id).unwrap().is_none());
        assert!(database
            .list("quarterly launch", false, 20, 0)
            .unwrap()
            .is_empty());
        assert!(database
            .quick_paste_search("quarterly launch", false, 9, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn image_text_reuses_sensitive_detector_and_caps_local_storage() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[0, 0, 0, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };
        let item = database.capture_image(&image, None).unwrap().unwrap();

        let blocked = database
            .save_image_text_extraction(&item.id, "-----BEGIN OPENSSH PRIVATE KEY-----")
            .unwrap();
        assert_eq!(
            blocked.status,
            ClipboardImageTextExtractionStatus::SensitiveBlocked
        );
        assert!(database.image_text_extraction(&item.id).unwrap().is_none());

        let oversized = "a".repeat(256 * 1024 + 1);
        assert!(database
            .save_image_text_extraction(&item.id, &oversized)
            .unwrap_err()
            .to_string()
            .contains("256 KiB"));
        assert!(database.image_text_extraction(&item.id).unwrap().is_none());
    }

    #[test]
    fn image_text_is_owned_by_item_and_cascades_on_permanent_delete() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[8, 16, 24, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };
        let item = database.capture_image(&image, None).unwrap().unwrap();
        database
            .save_image_text_extraction(&item.id, "safe synthetic fixture")
            .unwrap();

        database.delete(&item.id).unwrap();
        database
            .permanently_delete_recycled_items(std::slice::from_ref(&item.id))
            .unwrap();
        let connection = database.connection.lock().unwrap();
        let extraction_count: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_image_text_extractions
                 WHERE clipboard_item_id = ?1",
                [&item.id],
                |row| row.get(0),
            )
            .unwrap();
        let fts_count: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_image_text_extractions_fts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(extraction_count, 0);
        assert_eq!(fts_count, 0);
    }

    #[test]
    fn image_text_database_error_rolls_back_body_and_fts_together() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[32, 64, 96, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };
        let item = database.capture_image(&image, None).unwrap().unwrap();
        database
            .save_image_text_extraction(&item.id, "stable original OCR text")
            .unwrap();
        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_synthetic_image_text_update
                     BEFORE UPDATE ON clipboard_image_text_extractions
                     BEGIN
                         SELECT RAISE(ABORT, 'synthetic OCR storage failure');
                     END;",
                )
                .unwrap();
        }

        assert!(database
            .save_image_text_extraction(&item.id, "replacement partial OCR text")
            .is_err());
        assert_eq!(
            database
                .image_text_extraction(&item.id)
                .unwrap()
                .unwrap()
                .text,
            "stable original OCR text"
        );
        assert_eq!(
            database
                .list("stable original", false, 20, 0)
                .unwrap()
                .len(),
            1
        );
        assert!(database
            .list("replacement partial", false, 20, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn corrupted_blob_fails_before_reaching_the_clipboard_restore_boundary() {
        let root =
            std::env::temp_dir().join(format!("clipriva-integrity-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let database = Database::open(&root.join("clipriva.sqlite3")).unwrap();
        let image = CapturedImage {
            png: encode_rgba_png(&[255, 0, 0, 255], 1, 1).unwrap(),
            dimensions: ImageDimensions {
                width: 1,
                height: 1,
            },
        };
        let item = database
            .capture_image(&image, Some("SyntheticPreview"))
            .unwrap()
            .unwrap();
        let storage_key = &item.representations[0].storage_key;
        let hash = storage_key.strip_prefix("sha256/").unwrap();
        let blob_path = root
            .join("blobs")
            .join("sha256")
            .join(&hash[..2])
            .join(hash);
        std::fs::write(&blob_path, b"synthetic tampered bytes").unwrap();

        let error = database.read_blob(storage_key).unwrap_err().to_string();
        assert!(error.contains("integrity check"));
        assert!(!error.contains(storage_key));
        assert!(!error.contains(root.to_string_lossy().as_ref()));

        drop(database);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn captures_and_restores_rich_text_representation_metadata() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let plain = database
            .capture_text("A formatted note", Some("Notes"))
            .unwrap()
            .unwrap();
        let item = database
            .capture_rich_text(
                "A formatted note",
                &[
                    ("text/rtf".to_owned(), br"{\rtf1 A formatted note}".to_vec()),
                    (
                        "text/html".to_owned(),
                        b"<strong>A formatted note</strong>".to_vec(),
                    ),
                ],
                Some("TextEdit"),
            )
            .unwrap()
            .unwrap();

        assert_eq!(item.id, plain.id);
        assert_eq!(item.kind, "richText");
        assert_eq!(item.occurrence_count, 2);
        assert_eq!(item.representations.len(), 2);
        assert_eq!(item.source_app.as_deref(), Some("TextEdit"));
    }

    #[test]
    fn captures_file_references_without_reading_file_contents() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_file_references(
                &[
                    "/Users/example/Documents/brief.pdf".to_owned(),
                    "/Users/example/Documents/notes.txt".to_owned(),
                ],
                Some("Finder"),
            )
            .unwrap()
            .unwrap();

        assert_eq!(item.kind, "file");
        assert_eq!(item.content, "brief.pdf · notes.txt");
        assert_eq!(item.representations.len(), 1);
        assert_eq!(
            item.representations[0].display_name.as_deref(),
            Some("2 files")
        );
    }

    #[test]
    fn builds_a_bounded_file_reference_preview() {
        assert_eq!(
            file_reference_preview(&[
                "/tmp/one.txt",
                "/tmp/two.txt",
                "/tmp/three.txt",
                "/tmp/four.txt",
            ]),
            "one.txt · two.txt · three.txt · +1 more"
        );
        assert_eq!(
            file_reference_search_text(&[
                "/tmp/one.txt",
                "/tmp/two.txt",
                "/tmp/three.txt",
                "/private/example/four.txt",
            ]),
            "one.txt\ntwo.txt\nthree.txt\nfour.txt"
        );
    }

    #[test]
    fn persists_bounded_tags_and_searches_them_locally() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("Prepare the desktop release", Some("Notes"))
            .unwrap()
            .unwrap();

        let tagged = database
            .replace_tags(
                &item.id,
                vec![
                    "  Release   Train ".to_owned(),
                    "release train".to_owned(),
                    "Triage".to_owned(),
                ],
            )
            .unwrap();
        assert_eq!(tagged.tags, vec!["Release Train", "Triage"]);

        let history_matches = database.list("triage", false, 20, 0).unwrap();
        assert_eq!(history_matches.len(), 1);
        assert_eq!(history_matches[0].id, item.id);
        assert_eq!(history_matches[0].tags, vec!["Release Train", "Triage"]);
        assert_eq!(history_matches[0].occurrence_count, 1);
        let quick_matches = database
            .quick_paste_search("release train", false, 9, 0)
            .unwrap();
        assert_eq!(quick_matches.len(), 1);
        assert_eq!(
            quick_matches[0].match_field,
            Some(QuickPasteMatchField::Tag)
        );

        let too_many = (0..9).map(|index| format!("tag-{index}")).collect();
        assert!(database.replace_tags(&item.id, too_many).is_err());
        assert_eq!(database.get_by_id(&item.id).unwrap().tags, tagged.tags);
    }

    #[test]
    fn applies_structured_history_filters_before_paging_and_returns_complete_facets() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let target = database
            .capture_text("cargo test --workspace", Some("Terminal"))
            .unwrap()
            .unwrap();
        database
            .replace_tags(&target.id, vec!["Release".to_owned()])
            .unwrap();
        database.record_copy(&target.id).unwrap();

        // The matching Item is older than an entire 100-row UI page. A
        // client-side post-filter would miss it.
        for index in 0..101 {
            database
                .capture_text(&format!("newer unrelated note {index}"), Some("Notes"))
                .unwrap()
                .unwrap();
        }

        let filters = ClipboardListFilters {
            kinds: vec!["command".to_owned()],
            source_app: Some("terminal".to_owned()),
            collection: Some("release".to_owned()),
            smart_collection_id: None,
            pin_filter: ClipboardPinFilter::Unpinned,
            time_filter: ClipboardTimeFilter::Past24Hours,
            recently_used_only: true,
            local_link_only: false,
        };
        let results = database
            .list_with_filters("", false, 1, 0, Some(&filters))
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, target.id);
        assert_eq!(results[0].tags, vec!["Release"]);
        assert_eq!(results[0].occurrence_count, 1);

        let options = database.clipboard_filter_options().unwrap();
        assert_eq!(options.total_count, 102);
        assert_eq!(options.unpinned_count, 102);
        assert!(options
            .source_apps
            .iter()
            .any(|source| source == "Terminal"));
        assert_eq!(
            options.collections,
            vec![ClipboardCollection {
                id: None,
                name: "Release".to_owned(),
                kind: ClipboardCollectionKind::Manual,
                rule: None,
                item_count: 1,
                saved_item_count: 0,
            }]
        );

        let invalid_filters = ClipboardListFilters {
            kinds: vec!["remoteSecret".to_owned()],
            ..ClipboardListFilters::default()
        };
        assert!(database
            .list_with_filters("", false, 20, 0, Some(&invalid_filters))
            .unwrap_err()
            .to_string()
            .contains("Unsupported clipboard kind"));
    }

    #[test]
    fn saves_snippets_and_renames_or_deletes_collections_without_touching_content() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let first = database
            .capture_text("first reusable answer", Some("Notes"))
            .unwrap()
            .unwrap();
        let second = database
            .capture_text("second reusable answer", Some("Notes"))
            .unwrap()
            .unwrap();
        database
            .replace_tags(&first.id, vec!["Inbox".to_owned(), "Work".to_owned()])
            .unwrap();
        database
            .replace_tags(&second.id, vec!["inbox".to_owned()])
            .unwrap();

        let first_saved = database.save_snippet(&first.id, None).unwrap();
        let second_saved = database.save_snippet(&second.id, None).unwrap();
        assert!(first_saved.is_pinned);
        assert!(second_saved.is_pinned);
        assert_eq!(first_saved.content, "first reusable answer");

        let renamed = database.rename_collection("inbox", "Work").unwrap();
        assert_eq!(renamed.affected_item_count, 2);
        assert_eq!(database.get_by_id(&first.id).unwrap().tags, vec!["Work"]);
        assert_eq!(database.get_by_id(&second.id).unwrap().tags, vec!["Work"]);
        assert_eq!(
            database.list_collections().unwrap(),
            vec![ClipboardCollection {
                id: None,
                name: "Work".to_owned(),
                kind: ClipboardCollectionKind::Manual,
                rule: None,
                item_count: 2,
                saved_item_count: 2,
            }]
        );

        let deleted = database.delete_collection("work").unwrap();
        assert_eq!(deleted.affected_item_count, 2);
        assert!(database.get_by_id(&first.id).unwrap().tags.is_empty());
        assert!(database.get_by_id(&first.id).unwrap().is_pinned);

        let removed = database.remove_snippet(&first.id).unwrap();
        assert!(!removed.is_pinned);
        assert_eq!(removed.content, first.content);
        assert!(database.get_by_id(&second.id).unwrap().is_pinned);
    }

    #[test]
    fn searchable_notes_are_item_owned_bounded_and_not_serialized_in_list_items() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("synthetic body", Some("Notes"))
            .unwrap()
            .unwrap();
        let original = database.get_by_id(&item.id).unwrap();

        let saved = database
            .save_item_note(&item.id, "Quarterly launch owner")
            .unwrap();
        assert_eq!(saved.status, ClipboardItemNoteMutationStatus::Saved);
        let unchanged = database.get_by_id(&item.id).unwrap();
        assert_eq!(unchanged.content, original.content);
        assert_eq!(unchanged.copy_count, original.copy_count);
        assert_eq!(unchanged.is_pinned, original.is_pinned);
        assert_eq!(unchanged.retention_until, original.retention_until);

        let matches = database.list("quarterly launch", false, 20, 0).unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].note_match);
        let serialized = serde_json::to_string(&matches).unwrap();
        assert!(!serialized.contains("Quarterly launch owner"));
        let quick = database
            .quick_paste_search("launch owner", false, 9, 0)
            .unwrap();
        assert_eq!(quick[0].match_field, Some(QuickPasteMatchField::Note));

        let blocked = database
            .save_item_note(&item.id, "-----BEGIN OPENSSH PRIVATE KEY-----")
            .unwrap();
        assert_eq!(
            blocked.status,
            ClipboardItemNoteMutationStatus::SensitiveBlocked
        );
        assert_eq!(
            database.item_note(&item.id).unwrap().unwrap().text,
            "Quarterly launch owner"
        );
        assert!(database
            .save_item_note(&item.id, &"x".repeat(16 * 1024 + 1))
            .is_err());
        assert_eq!(
            database.save_item_note(&item.id, "  ").unwrap().status,
            ClipboardItemNoteMutationStatus::Deleted
        );
        assert!(database
            .list("quarterly launch", false, 20, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn notes_follow_recycle_restore_and_cascade_on_permanent_delete() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("synthetic lifecycle body", Some("Notes"))
            .unwrap()
            .unwrap();
        database
            .save_item_note(&item.id, "Synthetic lifecycle marker")
            .unwrap();
        let request = ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::Selected,
            item_ids: vec![item.id.clone()],
            include_pinned: true,
        };
        database.move_to_recycle_bin(&request).unwrap();
        assert!(database
            .list("lifecycle marker", false, 20, 0)
            .unwrap()
            .is_empty());

        database.restore_from_recycle_bin(&item.id).unwrap();
        assert_eq!(
            database.list("lifecycle marker", false, 20, 0).unwrap()[0].id,
            item.id
        );

        database.move_to_recycle_bin(&request).unwrap();
        database
            .permanently_delete_recycled_items(std::slice::from_ref(&item.id))
            .unwrap();
        let connection = database.connection.lock().unwrap();
        let note_rows: u32 = connection
            .query_row("SELECT COUNT(*) FROM clipboard_item_notes", [], |row| {
                row.get(0)
            })
            .unwrap();
        let fts_rows: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_item_notes_fts WHERE clipboard_item_notes_fts MATCH 'lifecycle'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(note_rows, 0);
        assert_eq!(fts_rows, 0);
    }

    #[test]
    fn smart_collections_apply_saved_filters_before_paging_and_never_mutate_items() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let target = database
            .capture_text("cargo test --locked", Some("Terminal"))
            .unwrap()
            .unwrap();
        for index in 0..101 {
            database
                .capture_text(&format!("unrelated synthetic note {index}"), Some("Notes"))
                .unwrap()
                .unwrap();
        }
        let smart = database
            .create_smart_collection(&ClipboardSmartCollectionInput {
                name: "Terminal commands".to_owned(),
                rule: ClipboardSmartCollectionRule {
                    kinds: vec!["command".to_owned()],
                    source_app: Some("Terminal".to_owned()),
                    ..ClipboardSmartCollectionRule::default()
                },
            })
            .unwrap();
        assert_eq!(smart.kind, ClipboardCollectionKind::Smart);
        assert_eq!(smart.item_count, 1);
        let filters = ClipboardListFilters {
            smart_collection_id: smart.id.clone(),
            ..ClipboardListFilters::default()
        };
        let results = database
            .list_with_filters("cargo", false, 1, 0, Some(&filters))
            .unwrap();
        assert_eq!(results[0].id, target.id);

        let before = database.get_by_id(&target.id).unwrap();
        assert!(database
            .delete_smart_collection(smart.id.as_deref().unwrap())
            .unwrap());
        let after = database.get_by_id(&target.id).unwrap();
        assert_eq!(before.content, after.content);
        assert_eq!(before.is_pinned, after.is_pinned);
        assert!(database
            .create_smart_collection(&ClipboardSmartCollectionInput {
                name: "Empty".to_owned(),
                rule: ClipboardSmartCollectionRule::default(),
            })
            .is_err());
    }

    #[test]
    fn command_items_keep_the_plain_text_local_link_boundary() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("cargo test --locked", Some("Terminal"))
            .unwrap()
            .unwrap();
        assert_eq!(item.kind, "command");

        let payload = database.local_link_export_clipboard_item(&item.id).unwrap();
        assert_eq!(payload.content, "cargo test --locked");
    }

    #[test]
    fn tags_follow_recycle_restore_and_cascade_on_permanent_delete() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("Temporary tagged note", Some("Notes"))
            .unwrap()
            .unwrap();
        database
            .replace_tags(&item.id, vec!["Follow up".to_owned()])
            .unwrap();

        database.delete(&item.id).unwrap();
        let recycled = database.recycle_bin_items(20).unwrap();
        assert_eq!(recycled[0].item.tags, vec!["Follow up"]);
        let restored = database.restore_from_recycle_bin(&item.id).unwrap();
        assert_eq!(restored.tags, vec!["Follow up"]);
        database.delete(&item.id).unwrap();
        database
            .permanently_delete_recycled_items(std::slice::from_ref(&item.id))
            .unwrap();

        let connection = database.connection.lock().unwrap();
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_item_tags WHERE clipboard_item_id = ?1",
                [&item.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn preserves_pinned_items_when_history_is_cleared() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let pinned = database.capture_text("keep", None).unwrap().unwrap();
        database.toggle_pin(&pinned.id).unwrap();
        database.capture_text("remove", None).unwrap();

        database.clear_unpinned().unwrap();

        let items = database.list("", false, 100, 0).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_pinned);
    }

    #[test]
    fn previews_cleanup_without_content_and_requires_explicit_pinned_confirmation() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let unpinned = database
            .capture_text("remove this locally", Some("Notes"))
            .unwrap()
            .unwrap();
        let pinned = database
            .capture_text("keep this pinned", Some("Terminal"))
            .unwrap()
            .unwrap();
        database.toggle_pin(&pinned.id).unwrap();

        let request = ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::Selected,
            item_ids: vec![unpinned.id.clone(), pinned.id.clone()],
            include_pinned: false,
        };
        let preview = database.preview_cleanup(&request).unwrap();
        assert_eq!(preview.affected_count, 1);
        assert_eq!(preview.pinned_skipped_count, 1);
        assert_eq!(preview.representative_items.len(), 1);
        assert_eq!(preview.representative_items[0].id, unpinned.id);
        assert!(preview
            .rule_conditions
            .iter()
            .any(|condition| condition.contains("Pinned clips are skipped")));
        assert_eq!(preview.recycle_bin_retention.maximum_days, 7);

        let result = database.move_to_recycle_bin(&request).unwrap();
        assert_eq!(result.moved_to_recycle_bin_count, 1);
        assert!(database.get_by_id(&unpinned.id).is_err());
        assert!(database.get_by_id(&pinned.id).is_ok());
        assert_eq!(database.recycle_bin_items(20).unwrap().len(), 1);

        let protected = database.delete(&pinned.id).unwrap_err();
        assert!(protected.to_string().contains("Pinned clips are protected"));
        let explicit = database
            .move_to_recycle_bin(&ClipboardCleanupRequest {
                scope: ClipboardCleanupScope::Selected,
                item_ids: vec![pinned.id.clone()],
                include_pinned: true,
            })
            .unwrap();
        assert_eq!(explicit.moved_to_recycle_bin_count, 1);
    }

    #[test]
    fn strict_policy_shortens_recycle_bin_recovery_to_one_day() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut preferences = database.capture_preferences().unwrap();
        preferences.sensitive_content_policy = SensitiveContentPolicy::Strict;
        preferences.sensitive_pause_enabled = true;
        database.update_capture_preferences(&preferences).unwrap();
        let item = database
            .capture_text("safe strict-mode note", Some("Notes"))
            .unwrap()
            .unwrap();
        let request = ClipboardCleanupRequest {
            scope: ClipboardCleanupScope::Selected,
            item_ids: vec![item.id.clone()],
            include_pinned: false,
        };

        let preview = database.preview_cleanup(&request).unwrap();
        assert_eq!(preview.recycle_bin_retention.maximum_days, 1);
        assert!(preview
            .rule_conditions
            .iter()
            .any(|condition| condition.contains("1 day after deletion")));

        database.move_to_recycle_bin(&request).unwrap();
        let recycled = database.recycle_bin_items(20).unwrap().remove(0);
        let deleted_at = chrono::DateTime::parse_from_rfc3339(&recycled.deleted_at).unwrap();
        let expires_at =
            chrono::DateTime::parse_from_rfc3339(&recycled.recycle_expires_at).unwrap();
        let recovery_window = expires_at.signed_duration_since(deleted_at);
        assert!(recovery_window >= chrono::Duration::hours(23));
        assert!(recovery_window <= chrono::Duration::days(1));
    }

    #[test]
    fn restoring_reuses_the_original_clip_row_and_preserves_capture_metadata() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("restore this note", Some("Notes"))
            .unwrap()
            .unwrap();
        let original_created_at = item.created_at.clone();
        let original_source = item.source_app.clone();
        assert!(!item.is_pinned);

        database
            .move_to_recycle_bin(&ClipboardCleanupRequest {
                scope: ClipboardCleanupScope::Selected,
                item_ids: vec![item.id.clone()],
                include_pinned: false,
            })
            .unwrap();
        let restored = database.restore_from_recycle_bin(&item.id).unwrap();

        assert_eq!(restored.id, item.id);
        assert_eq!(restored.created_at, original_created_at);
        assert_eq!(restored.source_app, original_source);
        assert!(!restored.is_pinned, "restoring must not force pinning");
        assert!(restored.restored_at.is_some());
        assert!(database.recycle_bin_items(20).unwrap().is_empty());
        assert_eq!(
            database.list("restore this", false, 20, 0).unwrap()[0].id,
            item.id
        );
    }

    #[test]
    fn recycle_bin_expiry_persists_across_restart_and_then_purges_media() {
        let root =
            std::env::temp_dir().join(format!("clipriva-recycle-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let database_path = root.join("clipriva.sqlite3");

        let (id, storage_key, recycle_expires_at) = {
            let database = Database::open(&database_path).unwrap();
            let image = CapturedImage {
                png: encode_rgba_png(&[255, 0, 0, 255], 1, 1).unwrap(),
                dimensions: ImageDimensions {
                    width: 1,
                    height: 1,
                },
            };
            let item = database
                .capture_image(&image, Some("Preview"))
                .unwrap()
                .unwrap();
            let storage_key = item.representations[0].storage_key.clone();
            database
                .move_to_recycle_bin(&ClipboardCleanupRequest {
                    scope: ClipboardCleanupScope::Selected,
                    item_ids: vec![item.id.clone()],
                    include_pinned: false,
                })
                .unwrap();
            let recycle_expires_at = database.recycle_bin_items(20).unwrap()[0]
                .recycle_expires_at
                .clone();
            assert!(!database.read_blob(&storage_key).unwrap().is_empty());
            (item.id, storage_key, recycle_expires_at)
        };

        let database = Database::open(&database_path).unwrap();
        let entry = database.recycle_bin_items(20).unwrap().remove(0);
        assert_eq!(entry.item.id, id);
        assert_eq!(entry.recycle_expires_at, recycle_expires_at);
        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute(
                    "UPDATE clipboard_items
                     SET recycle_expires_at = '2000-01-01T00:00:00.000Z'
                     WHERE id = ?1",
                    [&id],
                )
                .unwrap();
        }
        assert!(database.recycle_bin_items(20).unwrap().is_empty());
        assert!(database.read_blob(&storage_key).is_err());

        drop(database);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_policy_skips_sensitive_content_without_pausing_or_enriching_it() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);

        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("Terminal"))
            .unwrap()
            .is_none());
        let preferences = database.capture_preferences().unwrap();
        assert_eq!(
            preferences.sensitive_content_policy,
            SensitiveContentPolicy::Default
        );
        assert!(!preferences.capture_paused);
        assert_eq!(preferences.pause_reason, None);

        let safe_item = database
            .capture_text("safe note", Some("Notes"))
            .unwrap()
            .expect("the next safe clipboard item should still be captured");
        let connection = database.connection.lock().unwrap();
        let enrichment_count: u32 = connection
            .query_row("SELECT COUNT(*) FROM context_enrichments", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(enrichment_count, 1);
        let stored_content: String = connection
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                [&safe_item.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_content, "safe note");
    }

    #[test]
    fn strict_policy_pauses_capture_with_a_resumable_reason() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let preferences = crate::clipboard::policy::CapturePreferences {
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            ..Default::default()
        };
        database.update_capture_preferences(&preferences).unwrap();

        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("Terminal"))
            .unwrap()
            .is_none());
        let preferences = database.capture_preferences().unwrap();
        assert!(preferences.capture_paused);
        assert_eq!(
            preferences.pause_reason.as_deref(),
            Some("Sensitive content detected. Capture is paused until you resume it.")
        );

        database.resume_capture().unwrap();
        assert!(database
            .capture_text("safe note", Some("Notes"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn manual_pause_and_denied_app_semantics_take_precedence_over_sensitive_policy() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let preferences = crate::clipboard::policy::CapturePreferences {
            capture_paused: true,
            pause_reason: Some("Paused manually".to_owned()),
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            ..Default::default()
        };
        database.update_capture_preferences(&preferences).unwrap();

        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("Terminal"))
            .unwrap()
            .is_none());
        let preferences = database.capture_preferences().unwrap();
        assert!(preferences.capture_paused);
        assert_eq!(preferences.pause_reason.as_deref(), Some("Paused manually"));

        database.resume_capture().unwrap();
        let preferences = crate::clipboard::policy::CapturePreferences {
            denied_apps: vec!["1Password".to_owned()],
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            ..database.capture_preferences().unwrap()
        };
        database.update_capture_preferences(&preferences).unwrap();
        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("1Password"))
            .unwrap()
            .is_none());
        assert!(!database.capture_preferences().unwrap().capture_paused);
        assert!(database
            .capture_text("safe note", Some("Notes"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn records_privacy_safe_capture_status_reasons_and_resume_recovers_explicitly() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let manual_pause = crate::clipboard::policy::CapturePreferences {
            capture_paused: true,
            pause_reason: Some("Paused manually".to_owned()),
            ..Default::default()
        };
        database.update_capture_preferences(&manual_pause).unwrap();
        assert!(database
            .capture_text("do not retain this manual-pause value", Some("Notes"))
            .unwrap()
            .is_none());

        let resumed = database.resume_capture().unwrap();
        assert!(!resumed.capture_paused);
        assert_eq!(resumed.pause_reason, None);

        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("Terminal"))
            .unwrap()
            .is_none());

        let denied = crate::clipboard::policy::CapturePreferences {
            denied_apps: vec!["1Password".to_owned()],
            ..database.capture_preferences().unwrap()
        };
        database.update_capture_preferences(&denied).unwrap();
        assert!(database
            .capture_text("do not retain this excluded-app value", Some("1Password"))
            .unwrap()
            .is_none());

        let strict = crate::clipboard::policy::CapturePreferences {
            sensitive_content_policy: SensitiveContentPolicy::Strict,
            sensitive_pause_enabled: true,
            denied_apps: Vec::new(),
            ..database.capture_preferences().unwrap()
        };
        database.update_capture_preferences(&strict).unwrap();
        assert!(database
            .capture_text("-----BEGIN PRIVATE KEY-----", Some("Terminal"))
            .unwrap()
            .is_none());
        assert!(database.capture_preferences().unwrap().capture_paused);
        database.resume_capture().unwrap();

        assert!(database
            .capture_text(&"x".repeat(1_000_001), Some("Large Editor"))
            .is_err());

        let events = database.recent_capture_status_events().unwrap();
        assert!(events.iter().any(|event| {
            event.reason_type == CaptureStatusReason::ManualPause
                && event.source_app.as_deref() == Some("Notes")
        }));
        assert!(events.iter().any(|event| {
            event.reason_type == CaptureStatusReason::SensitiveContentDefault
                && event.source_app.as_deref() == Some("Terminal")
        }));
        assert!(events.iter().any(|event| {
            event.reason_type == CaptureStatusReason::ExcludedApplication
                && event.source_app.as_deref() == Some("1Password")
        }));
        assert!(events.iter().any(|event| {
            event.reason_type == CaptureStatusReason::SensitiveContentStrict
                && event.source_app.as_deref() == Some("Terminal")
        }));
        assert!(events.iter().any(|event| {
            event.reason_type == CaptureStatusReason::ContentTooLarge
                && event.source_app.as_deref() == Some("Large Editor")
        }));
        assert!(events
            .iter()
            .all(|event| !event.id.is_empty() && !event.expires_at.is_empty()));
        assert!(database.list("", false, 100, 0).unwrap().is_empty());

        let connection = database.connection.lock().unwrap();
        let columns = connection
            .prepare("PRAGMA table_info(uncaptured_clipboard_events)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            [
                "id",
                "reason_type",
                "source_app",
                "occurred_at",
                "expires_at"
            ]
        );
    }

    #[test]
    fn bounds_expires_and_clears_recent_capture_status_events() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let preferences = crate::clipboard::policy::CapturePreferences {
            denied_apps: vec!["1Password".to_owned()],
            ..Default::default()
        };
        database.update_capture_preferences(&preferences).unwrap();
        for index in 0..23 {
            assert!(database
                .capture_text(&format!("skip-{index}"), Some("1Password"))
                .unwrap()
                .is_none());
        }

        let events = database.recent_capture_status_events().unwrap();
        assert_eq!(events.len(), 20);
        let expired_id = events[0].id.clone();
        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute(
                    "UPDATE uncaptured_clipboard_events
                     SET expires_at = '2000-01-01T00:00:00.000Z'
                     WHERE id = ?1",
                    [&expired_id],
                )
                .unwrap();
        }

        assert_eq!(database.recent_capture_status_events().unwrap().len(), 19);
        database.clear_capture_status_events().unwrap();
        assert!(database.recent_capture_status_events().unwrap().is_empty());
    }

    #[test]
    fn honors_the_source_app_deny_list() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let preferences = crate::clipboard::policy::CapturePreferences {
            denied_apps: vec!["1Password".to_owned()],
            ..Default::default()
        };
        database.update_capture_preferences(&preferences).unwrap();

        assert!(database
            .capture_text("do not store", Some("1Password"))
            .unwrap()
            .is_none());
        assert!(database.list("", false, 100, 0).unwrap().is_empty());
    }

    #[test]
    fn app_exclusion_takes_effect_before_persistence_and_updates_immediately() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let retained = database
            .capture_text("captured before exclusion", Some("Private Browser"))
            .unwrap()
            .unwrap();

        let mut preferences = database.capture_preferences().unwrap();
        preferences.denied_apps = vec!["  private browser  ".to_owned()];
        database.update_capture_preferences(&preferences).unwrap();

        assert!(database
            .capture_text("must not reach clipboard_items", Some("PRIVATE BROWSER"))
            .unwrap()
            .is_none());
        assert_eq!(
            database.list("", false, 10, 0).unwrap()[0].id,
            retained.id,
            "an exclusion changes future capture immediately without mutating existing history"
        );

        preferences.denied_apps.clear();
        database.update_capture_preferences(&preferences).unwrap();
        assert!(database
            .capture_text("capture resumes immediately", Some("Private Browser"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn enforces_per_type_byte_limits_without_storing_content() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        assert!(database
            .capture_text(&"x".repeat(256 * 1024), Some("Notes"))
            .unwrap()
            .is_some());

        let oversized_text = database
            .capture_text(&"x".repeat(256 * 1024 + 1), Some("Notes"))
            .unwrap_err();
        assert!(oversized_text.to_string().contains("256 KiB"));
        assert!(!oversized_text.to_string().contains("xxxxx"));

        let oversized_link = format!("https://clipriva.local/{}", "x".repeat(8 * 1024));
        let oversized_link_error = database
            .capture_text(&oversized_link, Some("Browser"))
            .unwrap_err();
        assert!(oversized_link_error.to_string().contains("link"));
        assert!(oversized_link_error.to_string().contains("8 KiB"));
        assert!(database
            .recent_capture_status_events()
            .unwrap()
            .iter()
            .any(|event| event.reason_type == CaptureStatusReason::ContentTooLarge));
        assert_eq!(database.list("", false, 100, 0).unwrap().len(), 1);
    }

    #[test]
    fn uses_a_thirty_day_default_and_exempts_pinned_items_from_automatic_retention() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        assert_eq!(database.capture_preferences().unwrap().retention_days, 30);
        let expired = database
            .capture_text("remove by retention", Some("Notes"))
            .unwrap()
            .unwrap();
        let pinned = database
            .capture_text("retain while pinned", Some("Notes"))
            .unwrap()
            .unwrap();
        database.toggle_pin(&pinned.id).unwrap();
        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute(
                    "UPDATE clipboard_items
                     SET retention_until = '2000-01-01T00:00:00.000Z'
                     WHERE id IN (?1, ?2)",
                    rusqlite::params![&expired.id, &pinned.id],
                )
                .unwrap();
        }

        let active = database.list("", false, 20, 0).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, pinned.id);
        assert!(active[0].is_pinned);
        assert!(database.get_by_id(&expired.id).is_err());
        assert_eq!(
            database.recycle_bin_items(20).unwrap()[0].item.id,
            expired.id
        );
    }

    #[test]
    fn persists_core_and_labs_preferences_with_safe_defaults() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let defaults = database.capture_preferences().unwrap();
        assert_eq!(
            defaults.sensitive_content_policy,
            SensitiveContentPolicy::Default
        );
        assert!(!defaults.sensitive_pause_enabled);
        assert!(!defaults.labs_enabled);
        assert_eq!(
            defaults.paste_behavior,
            crate::clipboard::policy::PasteBehavior::Restore
        );
        assert!(!defaults.onboarding_completed);

        let updated = crate::clipboard::policy::CapturePreferences {
            labs_enabled: true,
            paste_behavior: crate::clipboard::policy::PasteBehavior::AutoPaste,
            quick_paste_shortcut: "CommandOrControl+Shift+V".to_owned(),
            onboarding_completed: true,
            ..defaults
        };
        database.update_capture_preferences(&updated).unwrap();

        assert_eq!(database.capture_preferences().unwrap(), updated);
    }

    #[test]
    fn records_only_opted_in_anonymous_local_diagnostics() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let first_recovery_started = LocalDiagnosticEventInput {
            event_type: LocalDiagnosticEventType::FirstRecovery,
            outcome: LocalDiagnosticOutcome::Started,
        };
        assert!(
            !database
                .record_local_diagnostic_event(&first_recovery_started)
                .unwrap()
                .recorded
        );
        let disabled_summary = database.local_diagnostics_summary().unwrap();
        assert!(!disabled_summary.enabled);
        assert_eq!(disabled_summary.stored_event_count, 0);

        let mut preferences = database.capture_preferences().unwrap();
        preferences.diagnostics_enabled = true;
        database.update_capture_preferences(&preferences).unwrap();

        let events = [
            first_recovery_started,
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::FirstRecovery,
                outcome: LocalDiagnosticOutcome::Completed,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::DirectPaste,
                outcome: LocalDiagnosticOutcome::Started,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::DirectPaste,
                outcome: LocalDiagnosticOutcome::Degraded,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::DirectPaste,
                outcome: LocalDiagnosticOutcome::Restricted,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::RecycleBinRestore,
                outcome: LocalDiagnosticOutcome::Restored,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::DuplicateGroupExpand,
                outcome: LocalDiagnosticOutcome::Expanded,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::ClipboardRestoreError,
                outcome: LocalDiagnosticOutcome::Failed,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::CapturePermissionRestricted,
                outcome: LocalDiagnosticOutcome::Restricted,
            },
            LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::SystemPolicyRestricted,
                outcome: LocalDiagnosticOutcome::Restricted,
            },
        ];
        for event in events {
            assert!(
                database
                    .record_local_diagnostic_event(&event)
                    .unwrap()
                    .recorded
            );
        }

        let summary = database.local_diagnostics_summary().unwrap();
        assert!(summary.enabled);
        assert_eq!(summary.stored_event_count, 10);
        assert_eq!(summary.first_recovery.attempts, 1);
        assert_eq!(summary.first_recovery.completed_or_degraded, 1);
        assert_eq!(summary.first_recovery.percent, Some(100));
        assert_eq!(summary.direct_paste.attempts, 1);
        assert_eq!(summary.direct_paste.completed_or_degraded, 1);
        assert_eq!(summary.direct_paste.percent, Some(100));
        assert_eq!(summary.recycle_bin_restore_count, 1);
        assert_eq!(summary.duplicate_group_expand_count, 1);
        assert_eq!(summary.critical_error_categories.len(), 4);

        let export = database.export_local_diagnostics().unwrap();
        assert_eq!(export.schema_version, 1);
        assert_eq!(export.events.len(), 10);
        assert!(export.events.iter().all(|event| {
            event.occurred_day.len() == 10
                && !event.occurred_day.contains('T')
                && !event.app_version.is_empty()
        }));
        let serialized = serde_json::to_string(&export).unwrap();
        for prohibited in [
            "clipboard secret",
            "sourceApp",
            "fileName",
            "documentName",
            "networkId",
            "contentHash",
            "storageKey",
            "occurredAt",
        ] {
            assert!(
                !serialized.contains(prohibited),
                "export contains {prohibited}"
            );
        }

        let connection = database.connection.lock().unwrap();
        let columns = connection
            .prepare("PRAGMA table_info(local_diagnostic_events)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            ["id", "event_type", "outcome", "occurred_day", "app_version"]
        );
        drop(connection);

        database.clear_local_diagnostics().unwrap();
        assert_eq!(
            database
                .local_diagnostics_summary()
                .unwrap()
                .stored_event_count,
            0
        );
    }

    #[test]
    fn rejects_diagnostic_event_pairs_that_cannot_describe_a_supported_flow() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut preferences = database.capture_preferences().unwrap();
        preferences.diagnostics_enabled = true;
        database.update_capture_preferences(&preferences).unwrap();

        assert!(database
            .record_local_diagnostic_event(&LocalDiagnosticEventInput {
                event_type: LocalDiagnosticEventType::DuplicateGroupExpand,
                outcome: LocalDiagnosticOutcome::Completed,
            })
            .is_err());
    }

    #[test]
    fn migrates_legacy_sensitive_pause_preferences_to_strict_policy() {
        let connection = Connection::open_in_memory().unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "UPDATE capture_preferences
                 SET capture_paused = 1,
                     sensitive_pause_enabled = 1,
                     retention_days = 30,
                     denied_apps_json = '[\"KeePassXC\"]'
                 WHERE singleton = 1",
                [],
            )
            .unwrap();

        connection.execute_batch(migrations::MIGRATION_6).unwrap();
        connection.execute_batch(migrations::MIGRATION_7).unwrap();
        connection.execute_batch(migrations::MIGRATION_8).unwrap();
        connection.execute_batch(migrations::MIGRATION_9).unwrap();
        connection.execute_batch(migrations::MIGRATION_10).unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_items
                 (id, content, content_hash, kind, source_app, created_at, updated_at, is_pinned, copy_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 0, 0)",
                rusqlite::params![
                    "legacy-text",
                    "Legacy release note",
                    "legacy-hash",
                    "text",
                    "Notes",
                    "2026-01-01T00:00:00.000Z",
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_representations
                 (clipboard_item_id, storage_key, content_hash, kind, mime_type, display_name,
                  image_width, image_height, byte_size, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, ?6, ?7)",
                rusqlite::params![
                    "legacy-text",
                    "legacy-file-reference",
                    "legacy-file-hash",
                    "file",
                    "application/x-clipriva-file-list+json",
                    1_i64,
                    "2026-01-01T00:00:00.000Z",
                ],
            )
            .unwrap();
        connection.execute_batch(migrations::MIGRATION_11).unwrap();
        connection.execute_batch(migrations::MIGRATION_12).unwrap();
        for migration in [
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
            migrations::MIGRATION_16,
            migrations::MIGRATION_17,
            migrations::MIGRATION_18,
            migrations::MIGRATION_19,
            migrations::MIGRATION_20,
            migrations::MIGRATION_21,
            migrations::MIGRATION_22,
            migrations::MIGRATION_23,
            migrations::MIGRATION_24,
            migrations::MIGRATION_25,
            migrations::MIGRATION_26,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        let preferences = Database::capture_preferences_with_connection(&connection).unwrap();
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();

        assert_eq!(version, 26);
        assert!(preferences.capture_paused);
        assert_eq!(
            preferences.sensitive_content_policy,
            SensitiveContentPolicy::Strict
        );
        assert!(preferences.sensitive_pause_enabled);
        assert_eq!(preferences.retention_days, 30);
        assert_eq!(preferences.denied_apps, ["KeePassXC"]);
        assert!(!preferences.labs_enabled);
        assert!(!preferences.diagnostics_enabled);
        assert_eq!(preferences.paste_behavior, PasteBehavior::Restore);
        assert_eq!(
            preferences.quick_paste_shortcut,
            DEFAULT_QUICK_PASTE_SHORTCUT
        );
        assert_eq!(
            preferences.stack_shortcut,
            crate::clipboard::policy::DEFAULT_STACK_SHORTCUT
        );
        assert!(!preferences.onboarding_completed);

        let legacy = Database::get_by_id_with_connection(&connection, "legacy-text").unwrap();
        assert_eq!(legacy.global_id, "legacy-text");
        assert_eq!(legacy.group_id, None);
        assert_eq!(legacy.version, 1);
        assert_eq!(legacy.occurrence_count, 1);
        assert!(!legacy.device_id.is_empty());
        assert!(!legacy.retention_until.is_empty());
        assert_eq!(legacy.representations.len(), 1);
        let foreign_key_errors: u32 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_errors, 0);
        let fts_matches: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items_fts WHERE clipboard_items_fts MATCH 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_matches, 1);
    }

    #[test]
    fn migration_12_folds_v11_duplicates_and_rich_variants_transactionally() {
        let root = std::env::temp_dir().join(format!(
            "clipriva-occurrence-migration-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let database_path = root.join("clipriva.sqlite3");
        let connection = Connection::open(&database_path).unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        let device_id: String = connection
            .query_row(
                "SELECT device_id FROM clipriva_local_identity WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        for (id, hash, group_id, kind, source, created_at, pinned, copies, deleted_at) in [
            (
                "canonical-text",
                "plain-hash",
                Some("exact-group"),
                "text",
                "Notes",
                "2026-01-01T00:00:00.000Z",
                0_i64,
                1_i64,
                Some("2026-02-01T00:00:00.000Z"),
            ),
            (
                "duplicate-text",
                "plain-hash",
                Some("exact-group"),
                "text",
                "Terminal",
                "2026-01-02T00:00:00.000Z",
                1_i64,
                2_i64,
                None,
            ),
            (
                "rich-variant",
                "legacy-represented-hash",
                None,
                "richText",
                "TextEdit",
                "2026-01-03T00:00:00.000Z",
                0_i64,
                3_i64,
                None,
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO clipboard_items
                     (id, global_id, device_id, content, content_hash, group_id, version, kind,
                      source_app, created_at, updated_at, retention_until, is_pinned, copy_count,
                      deleted_at, recycle_expires_at)
                     VALUES (?1, ?1, ?2, 'Same visible text', ?3, ?4, 1, ?5, ?6, ?7, ?7,
                             '2026-12-31T00:00:00.000Z', ?8, ?9, ?10,
                             CASE WHEN ?10 IS NULL THEN NULL ELSE '2099-01-01T00:00:00.000Z' END)",
                    rusqlite::params![
                        id, device_id, hash, group_id, kind, source, created_at, pinned, copies,
                        deleted_at,
                    ],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO clipboard_representations
                 (clipboard_item_id, storage_key, content_hash, kind, mime_type, display_name,
                  image_width, image_height, byte_size, created_at)
                 VALUES ('rich-variant', 'rich-storage', 'rich-bytes', 'richText', 'text/rtf',
                         NULL, NULL, NULL, 12, '2026-01-03T00:00:00.000Z')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO context_enrichments
                 (clipboard_item_id, schema_version, enricher_id, enricher_version,
                  enrichment_json, updated_at)
                 VALUES ('duplicate-text', 1, 'local', '1', '{}',
                         '2026-01-02T00:00:00.000Z')",
                [],
            )
            .unwrap();
        drop(connection);

        let database = Database::open(&database_path).unwrap();
        let migrated = database.get_by_id("canonical-text").unwrap();
        assert_eq!(migrated.kind, "richText");
        assert_eq!(migrated.source_app.as_deref(), Some("TextEdit"));
        assert!(migrated.is_pinned);
        assert_eq!(migrated.copy_count, 6);
        assert_eq!(migrated.occurrence_count, 3);
        assert_eq!(migrated.representations.len(), 1);
        assert!(database.get_by_id("duplicate-text").is_err());
        assert!(database.get_by_id("rich-variant").is_err());
        {
            let connection = database.connection.lock().unwrap();
            let version: u32 = connection
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            // Opening a v11 fixture also applies every later local schema
            // migration after the v12 canonical-item migration completes.
            assert_eq!(version, 26);
            let foreign_key_errors: u32 = connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(foreign_key_errors, 0);
            let enrichment_item: String = connection
                .query_row(
                    "SELECT clipboard_item_id FROM context_enrichments",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(enrichment_item, "canonical-text");
        }
        drop(database);

        let reopened = Database::open(&database_path).unwrap();
        assert_eq!(
            reopened
                .get_by_id("canonical-text")
                .unwrap()
                .occurrence_count,
            3
        );
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_labs_storage_and_capabilities_off_until_enabled() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("A normal clipboard note", Some("Notes"))
            .unwrap()
            .unwrap();
        {
            let connection = database.connection.lock().unwrap();
            let enrichment_count: u32 = connection
                .query_row("SELECT COUNT(*) FROM context_enrichments", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(enrichment_count, 0);
        }
        assert!(database.context_enrichment(&item.id).is_err());
        assert!(database
            .search_local_semantic("normal", false, 10, 0)
            .is_err());
        assert!(database.list_local_text_actions().is_err());

        enable_labs(&database);
        assert!(database.context_enrichment(&item.id).is_ok());
        assert_eq!(
            database
                .search_local_semantic("normal", false, 10, 0)
                .unwrap()
                .len(),
            1
        );
        assert!(!database.list_local_text_actions().unwrap().is_empty());
    }

    #[test]
    fn persists_a_versioned_local_enrichment_with_the_clip() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let item = database
            .capture_text("Discuss launch at alice@example.com", Some("Notes"))
            .unwrap()
            .unwrap();

        let enrichment = database.context_enrichment(&item.id).unwrap();
        assert_eq!(enrichment.metadata.schema_version, 1);
        assert!(enrichment.redaction.requires_explicit_consent);
        assert!(enrichment.summary.text.contains("[REDACTED:email_address]"));
    }

    #[test]
    fn ranks_content_matches_ahead_of_source_only_matches() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        database
            .capture_text("Prepare the release notes", Some("Notes"))
            .unwrap();
        database
            .capture_text("Review the deployment checklist", Some("Release Manager"))
            .unwrap();

        let items = database.list("release", false, 100, 0).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "Prepare the release notes");
        assert_eq!(items[1].content, "Review the deployment checklist");
    }

    #[test]
    fn quick_paste_orders_explicit_match_tiers_with_case_and_whitespace_normalization() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let exact = database
            .capture_text("  ReLeAsE\n  ", Some("Notes"))
            .unwrap()
            .unwrap();
        let prefix = database
            .capture_text("release checklist", Some("Notes"))
            .unwrap()
            .unwrap();
        let word = database
            .capture_text("https://docs.example/release-notes", Some("Arc"))
            .unwrap()
            .unwrap();
        database.toggle_pin(&word.id).unwrap();
        let contains = database
            .capture_text("unreleased status", Some("Notes"))
            .unwrap()
            .unwrap();

        let first = database
            .quick_paste_search(" RELEASE ", false, 10, 0)
            .unwrap();
        let second = database
            .quick_paste_search(" RELEASE ", false, 10, 0)
            .unwrap();

        assert_eq!(
            first
                .iter()
                .map(|result| result.item.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                exact.id.as_str(),
                prefix.id.as_str(),
                word.id.as_str(),
                contains.id.as_str(),
            ]
        );
        assert_eq!(
            first
                .iter()
                .map(|result| result.match_kind)
                .collect::<Vec<_>>(),
            vec![
                QuickPasteMatchKind::Exact,
                QuickPasteMatchKind::Prefix,
                QuickPasteMatchKind::Contains,
                QuickPasteMatchKind::Contains,
            ]
        );
        assert_eq!(
            first
                .iter()
                .map(|result| result.item.id.as_str())
                .collect::<Vec<_>>(),
            second
                .iter()
                .map(|result| result.item.id.as_str())
                .collect::<Vec<_>>()
        );

        let whitespace_exact = database
            .capture_text("A   MIXED\nvalue", Some("Notes"))
            .unwrap()
            .unwrap();
        let result = database
            .quick_paste_search(" a mixed    VALUE ", false, 10, 0)
            .unwrap();
        assert_eq!(result[0].item.id, whitespace_exact.id);
        assert_eq!(result[0].match_kind, QuickPasteMatchKind::Exact);
        assert_eq!(result[0].match_field, Some(QuickPasteMatchField::Content));
    }

    #[test]
    fn quick_paste_finds_chinese_substrings_and_literal_code_url_symbols() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let chinese = database
            .capture_text("版本规划与体验升级", Some("Notes"))
            .unwrap()
            .unwrap();
        let code = database
            .capture_text("const user_id = \"sample-42\";", Some("Editor"))
            .unwrap()
            .unwrap();
        let url = database
            .capture_text("https://example.test/a/b?x=1", Some("Browser"))
            .unwrap()
            .unwrap();

        for query in ["版本规划", "规划"] {
            assert_eq!(
                database.quick_paste_search(query, false, 9, 0).unwrap()[0]
                    .item
                    .id,
                chinese.id
            );
        }
        for query in ["user_id", "sample-42", "\""] {
            assert!(database
                .quick_paste_search(query, false, 9, 0)
                .unwrap()
                .iter()
                .any(|result| result.item.id == code.id));
        }
        for query in ["a/b?x=1", "b?x", "/"] {
            assert!(database
                .quick_paste_search(query, false, 9, 0)
                .unwrap()
                .iter()
                .any(|result| result.item.id == url.id));
        }
    }

    #[test]
    fn quick_paste_reaches_an_older_match_beyond_50k_newer_items() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let mut preferences = database.capture_preferences().unwrap();
        preferences.max_history_items = 100_000;
        database.update_capture_preferences(&preferences).unwrap();
        {
            let connection = database.connection.lock().unwrap();
            let device_id = Database::local_device_id_with_connection(&connection).unwrap();
            connection.execute_batch("BEGIN IMMEDIATE").unwrap();
            connection
                .execute(
                    "INSERT INTO clipboard_items
                     (id, global_id, device_id, content, content_hash, version, kind,
                      created_at, updated_at, retention_until, search_text)
                     VALUES ('older-target', 'older-target', ?1, 'older unique sample',
                             'older-target', 1, 'text', '2026-01-01T00:00:00.000Z',
                             '2026-01-01T00:00:00.000Z', '2099-01-01T00:00:00.000Z',
                             'older unique sample')",
                    [&device_id],
                )
                .unwrap();
            for index in 0..50_000 {
                let id = format!("newer-{index:05}");
                let content = format!("synthetic filler {index:05}");
                connection
                    .execute(
                        "INSERT INTO clipboard_items
                         (id, global_id, device_id, content, content_hash, version, kind,
                          created_at, updated_at, retention_until, search_text)
                         VALUES (?1, ?1, ?2, ?3, ?1, 1, 'text',
                                 '2026-09-26T00:00:00.000Z', '2026-09-26T00:00:00.000Z',
                                 '2099-01-01T00:00:00.000Z', ?3)",
                        rusqlite::params![id, device_id, content],
                    )
                    .unwrap();
            }
            connection.execute_batch("COMMIT").unwrap();
        }

        let started = std::time::Instant::now();
        let result = database
            .quick_paste_search("unique sample", false, 9, 0)
            .unwrap();
        eprintln!("50k older-item Quick Paste query: {:?}", started.elapsed());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].item.id, "older-target");

        let common_started = std::time::Instant::now();
        let common = database.quick_paste_search("filler", false, 9, 0).unwrap();
        eprintln!(
            "50k broad Quick Paste query: {:?}",
            common_started.elapsed()
        );
        assert_eq!(common.len(), 9);

        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute(
                    "UPDATE clipboard_items SET last_used_at = '2026-09-27T00:00:00.000Z'
                     WHERE id = 'older-target'",
                    [],
                )
                .unwrap();
        }
        let recent = database.quick_paste_search("", false, 9, 0).unwrap();
        assert_eq!(recent[0].item.id, "older-target");
    }

    #[test]
    fn quick_paste_searches_all_file_basenames_and_historical_sources() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let file = database
            .capture_file_references(
                &[
                    "/private/example/one.txt".to_owned(),
                    "/private/example/two.txt".to_owned(),
                    "/private/example/three.txt".to_owned(),
                    "/private/example/four.txt".to_owned(),
                ],
                Some("Finder"),
            )
            .unwrap()
            .unwrap();
        let note = database
            .capture_text("shared source note", Some("Notes"))
            .unwrap()
            .unwrap();
        let repeated = database
            .capture_text("shared source note", Some("Terminal"))
            .unwrap()
            .unwrap();

        assert_eq!(note.id, repeated.id);
        let filename_match = database
            .quick_paste_search("four.txt", false, 10, 0)
            .unwrap();
        assert_eq!(filename_match[0].item.id, file.id);
        assert_eq!(
            filename_match[0].match_field,
            Some(QuickPasteMatchField::DisplayName)
        );
        let source_match = database.quick_paste_search("Notes", false, 10, 0).unwrap();
        assert_eq!(source_match[0].item.id, note.id);
        assert_eq!(
            source_match[0].match_field,
            Some(QuickPasteMatchField::SourceApp)
        );
        let serialized = serde_json::to_string(&file).unwrap();
        assert!(!serialized.contains("/private/example"));
        assert!(!serialized.contains("searchText"));
    }

    #[test]
    fn quick_paste_zero_input_keeps_recent_work_first_and_exposes_one_pin() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let pinned = database
            .capture_text("Reference clip", None)
            .unwrap()
            .unwrap();
        database.toggle_pin(&pinned.id).unwrap();
        let recent = database.capture_text("Recent clip", None).unwrap().unwrap();
        let older = database.capture_text("Older clip", None).unwrap().unwrap();

        let connection = database.connection.lock().unwrap();
        connection
            .execute(
                "UPDATE clipboard_items SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params!["2026-01-01T00:00:00.000Z", &pinned.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE clipboard_occurrences SET occurred_at = ?1 WHERE clipboard_item_id = ?2",
                rusqlite::params!["2026-01-01T00:00:00.000Z", &pinned.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE clipboard_items SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params!["2026-03-01T00:00:00.000Z", &recent.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE clipboard_occurrences SET occurred_at = ?1 WHERE clipboard_item_id = ?2",
                rusqlite::params!["2026-03-01T00:00:00.000Z", &recent.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE clipboard_items SET created_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params!["2026-02-01T00:00:00.000Z", &older.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE clipboard_occurrences SET occurred_at = ?1 WHERE clipboard_item_id = ?2",
                rusqlite::params!["2026-02-01T00:00:00.000Z", &older.id],
            )
            .unwrap();
        drop(connection);

        let suggestions = database.quick_paste_search("   ", false, 2, 0).unwrap();
        assert_eq!(suggestions[0].item.id, recent.id);
        assert!(suggestions.iter().any(|result| result.item.id == pinned.id));
        assert!(suggestions
            .iter()
            .all(|result| result.match_kind == QuickPasteMatchKind::Suggestion));
    }

    #[test]
    fn repeated_quick_paste_retries_update_one_existing_history_item() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_text("retry-safe clipboard item", Some("Notes"))
            .unwrap()
            .unwrap();

        let first_retry = database.record_copy(&item.id).unwrap();
        let second_retry = database.record_copy(&item.id).unwrap();
        let history = database.list("", false, 10, 0).unwrap();

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, item.id);
        assert_eq!(first_retry.copy_count, item.copy_count + 1);
        assert_eq!(second_retry.copy_count, item.copy_count + 2);
    }

    #[test]
    #[ignore = "release-mode acceptance benchmark; run with cargo test --release quick_paste_searches_10k_items_within_target -- --ignored --nocapture"]
    fn quick_paste_searches_10k_items_within_target() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        {
            let connection = database.connection.lock().unwrap();
            let device_id = Database::local_device_id_with_connection(&connection).unwrap();
            connection.execute_batch("BEGIN IMMEDIATE").unwrap();
            for index in 0..10_000 {
                let id = format!("performance-{index:05}");
                let content = format!("synthetic quick paste value {index:05}");
                connection
                    .execute(
                        "INSERT INTO clipboard_items
                         (id, global_id, device_id, content, content_hash, version, kind,
                          created_at, updated_at, retention_until, search_text)
                         VALUES (?1, ?1, ?2, ?3, ?1, 1, 'text', ?4, ?4,
                                 '2099-01-01T00:00:00.000Z', ?3)",
                        rusqlite::params![
                            id,
                            device_id,
                            content,
                            format!("2026-01-01T00:00:{:02}.000Z", index % 60),
                        ],
                    )
                    .unwrap();
                connection
                    .execute(
                        "INSERT INTO clipboard_occurrences
                         (id, clipboard_item_id, device_id, occurred_at)
                         VALUES ('occurrence-' || ?1, ?1, ?2, ?3)",
                        rusqlite::params![
                            id,
                            device_id,
                            format!("2026-01-01T00:00:{:02}.000Z", index % 60),
                        ],
                    )
                    .unwrap();
            }
            connection.execute_batch("COMMIT").unwrap();
        }

        let started = std::time::Instant::now();
        let results = database
            .quick_paste_search("value 09999", false, 10, 0)
            .unwrap();
        let elapsed = started.elapsed();
        eprintln!("10k Quick Paste search completed in {elapsed:?}");
        assert_eq!(results.len(), 1);
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "10k Quick Paste search took {elapsed:?}"
        );
    }

    #[test]
    fn turns_plain_text_into_safe_fts_prefix_terms() {
        assert_eq!(
            fts_prefix_query("release: \"notes\" *"),
            Some("\"release\"* AND \"notes\"*".to_owned())
        );
    }

    #[test]
    fn special_characters_do_not_cause_fts_syntax_errors() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        database
            .capture_text("Release notes for the desktop application", Some("Notes"))
            .unwrap();

        for query in ["release:*", "\"release\" OR *", "()[]{}", "C++"] {
            assert!(
                database.list(query, false, 100, 0).is_ok(),
                "query {query:?} should be safe for FTS"
            );
        }
    }

    #[test]
    fn local_semantic_search_returns_deterministic_scores_and_highlights() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let exact = database
            .capture_text("ClipRiva release notes", Some("Notes"))
            .unwrap()
            .unwrap();
        database
            .capture_text("ClipRiva deployment checklist", Some("Terminal"))
            .unwrap();
        database
            .capture_text("Release notes for another product", Some("Notes"))
            .unwrap();

        let first = database
            .search_local_semantic("clipriva release notes", false, 10, 0)
            .unwrap();
        let second = database
            .search_local_semantic("clipriva release notes", false, 10, 0)
            .unwrap();

        assert_eq!(first.len(), 3);
        assert_eq!(
            first
                .iter()
                .map(|result| &result.item.id)
                .collect::<Vec<_>>(),
            second
                .iter()
                .map(|result| &result.item.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(first[0].item.id, exact.id);
        assert_eq!(first[0].score, 1.0);
        assert_eq!(first[0].matched_terms, ["clipriva", "notes", "release"]);
        assert_eq!(first[0].highlight_ranges.len(), 3);
        assert!(first[0]
            .highlight_ranges
            .iter()
            .all(|range| range.end > range.start));
        assert!(first[0].score > first[1].score);
    }

    #[test]
    fn local_semantic_search_respects_pins_and_empty_queries() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let pinned = database
            .capture_text("ClipRiva release notes", None)
            .unwrap()
            .unwrap();
        database.toggle_pin(&pinned.id).unwrap();
        database
            .capture_text("ClipRiva release checklist", None)
            .unwrap();

        assert!(database
            .search_local_semantic("   ", false, 10, 0)
            .unwrap()
            .is_empty());
        let results = database
            .search_local_semantic("clipriva release", true, 10, 0)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.id, pinned.id);
    }

    #[test]
    fn local_actions_build_content_free_audits_and_persist_only_after_the_effect_boundary() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let item = database
            .capture_text("  alice@example.com  ", Some("Notes"))
            .unwrap()
            .unwrap();
        let trim = database
            .list_local_text_actions()
            .unwrap()
            .into_iter()
            .find(|action| action.transform == crate::actions::LocalTextTransform::TrimWhitespace)
            .unwrap();

        let preview = database
            .preview_local_text_action(&trim.id, &item.id)
            .unwrap();
        let execution = database.confirm_local_text_action(&preview).unwrap();

        assert_eq!(execution.output, "alice@example.com");
        assert_eq!(execution.audit.input_content_hash, "not-stored");
        assert!(execution.audit.input_preview.is_empty());
        assert!(execution.audit.output_content_hash.is_none());
        assert!(execution.audit.output_preview.is_none());
        assert!(database
            .list_action_audit(Some(&item.id), 10)
            .unwrap()
            .is_empty());

        database.record_action_audit(&execution.audit).unwrap();
        assert_eq!(
            database
                .list_action_audit(Some(&item.id), 10)
                .unwrap()
                .len(),
            1
        );

        let connection = database.connection.lock().unwrap();
        assert!(connection
            .execute(
                "UPDATE action_audit_log SET action_name = 'changed' WHERE id = ?1",
                [&execution.audit.id],
            )
            .is_err());
    }

    #[test]
    fn local_filter_confirmation_rejects_changed_item_or_rule_without_audit() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let item = database.capture_text("  sample  ", None).unwrap().unwrap();
        let action = database
            .list_local_text_actions()
            .unwrap()
            .into_iter()
            .find(|action| action.transform == crate::actions::LocalTextTransform::TrimWhitespace)
            .unwrap();
        let preview = database
            .preview_local_text_action(&action.id, &item.id)
            .unwrap();
        assert_eq!(preview.input, item.content);
        assert_eq!(preview.output, "sample");
        assert!(database
            .list_action_audit(Some(&item.id), 10)
            .unwrap()
            .is_empty());

        let execution = database.confirm_local_text_action(&preview).unwrap();
        assert_eq!(execution.output, "sample");
        assert!(database
            .list_action_audit(Some(&item.id), 10)
            .unwrap()
            .is_empty());
        assert_eq!(database.get_by_id(&item.id).unwrap().content, item.content);

        let mut changed_rule = preview.clone();
        changed_rule.action.name = "Forged rule".to_owned();
        assert!(database.confirm_local_text_action(&changed_rule).is_err());
        let mut changed_result = preview.clone();
        changed_result.output = "forged result".to_owned();
        assert!(database.confirm_local_text_action(&changed_result).is_err());
        database
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE clipboard_items SET version = version + 1 WHERE id = ?1",
                [&item.id],
            )
            .unwrap();
        assert!(database.confirm_local_text_action(&preview).is_err());
        assert!(database
            .list_action_audit(Some(&item.id), 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn custom_filter_shortcut_slots_are_unique_editable_and_builtin_safe() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let draft: CreateLocalTextAction = serde_json::from_value(serde_json::json!({
            "name": "Clean lines",
            "transform": "trim_whitespace",
            "steps": [{ "transform": "trim_whitespace" }],
            "shortcutSlot": 1
        }))
        .unwrap();
        let created = database.create_local_text_action(&draft).unwrap();
        assert_eq!(created.shortcut_slot, Some(1));

        let duplicate: CreateLocalTextAction = serde_json::from_value(serde_json::json!({
            "name": "Other clean lines",
            "transform": "uppercase",
            "shortcutSlot": 1
        }))
        .unwrap();
        assert!(database.create_local_text_action(&duplicate).is_err());

        let updated: CreateLocalTextAction = serde_json::from_value(serde_json::json!({
            "name": "Clean and sort",
            "transform": "trim_whitespace",
            "steps": [
                { "transform": "trim_whitespace" },
                { "transform": "sort_lines_asc" }
            ],
            "shortcutSlot": 2
        }))
        .unwrap();
        let updated = database
            .update_local_text_action(&created.id, &updated)
            .unwrap();
        assert_eq!(updated.name, "Clean and sort");
        assert_eq!(updated.steps.len(), 2);
        assert_eq!(updated.shortcut_slot, Some(2));

        let builtin = database
            .list_local_text_actions()
            .unwrap()
            .into_iter()
            .find(|action| action.is_builtin)
            .unwrap();
        assert!(database
            .update_local_text_action(&builtin.id, &draft)
            .is_err());
        assert!(database.delete_local_text_action(&builtin.id).is_err());
        assert!(database.delete_local_text_action(&created.id).unwrap());
    }

    #[test]
    fn cloud_boundary_records_a_preview_without_an_execution_path() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        enable_labs(&database);
        let item = database
            .capture_text("Contact alice@example.com", Some("Notes"))
            .unwrap()
            .unwrap();

        let preview = database.preview_cloud_action(&item.id).unwrap();

        assert!(!preview.is_executable);
        assert_eq!(preview.delivery, "preview_only");
        assert!(preview
            .redaction
            .redacted_content
            .contains("[REDACTED:email_address]"));
        let audits = database.list_action_audit(Some(&item.id), 10).unwrap();
        assert_eq!(
            audits[0].status,
            crate::actions::ActionAuditStatus::PreviewOnlyBlocked
        );
    }

    #[test]
    fn migration_15_disables_network_consent_and_downgrades_preview_trust() {
        let connection = Connection::open_in_memory().unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "UPDATE local_link_preferences SET enabled = 1, discovery_enabled = 1",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO local_link_devices
                 (device_id, display_name, peer_public_key, public_key_fingerprint,
                  trust_status, pairing_code_hash, paired_at, trusted_at)
                 VALUES ('preview-peer', 'Preview Mac', '', '', 'trusted', 'fixture-hash',
                         '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')",
                [],
            )
            .unwrap();

        connection.execute_batch(migrations::MIGRATION_15).unwrap();

        let (enabled, discovery): (i64, i64) = connection
            .query_row(
                "SELECT enabled, discovery_enabled FROM local_link_preferences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((enabled, discovery), (0, 0));
        let (status, key): (String, Vec<u8>) = connection
            .query_row(
                "SELECT trust_status, peer_public_key FROM local_link_devices
                 WHERE device_id = 'preview-peer'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "needsRePairing");
        assert!(key.is_empty());
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 15);
    }

    #[test]
    fn migration_16_preserves_existing_retention_and_accepts_size_statuses() {
        let connection = Connection::open_in_memory().unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "UPDATE capture_preferences SET retention_days = 90 WHERE singleton = 1",
                [],
            )
            .unwrap();

        connection.execute_batch(migrations::MIGRATION_16).unwrap();

        let retention_days: u32 = connection
            .query_row(
                "SELECT retention_days FROM capture_preferences WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retention_days, 90);
        connection
            .execute(
                "INSERT INTO uncaptured_clipboard_events
                 (id, reason_type, source_app, occurred_at, expires_at)
                 VALUES ('size-boundary', 'contentTooLarge', 'Notes',
                         '2026-01-01T00:00:00.000Z', '2026-01-08T00:00:00.000Z')",
                [],
            )
            .unwrap();
    }

    #[test]
    fn migration_17_defaults_existing_local_link_devices_to_thirty_days() {
        let connection = Connection::open_in_memory().unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
            migrations::MIGRATION_16,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO local_link_devices
                 (device_id, display_name, peer_public_key, public_key_fingerprint,
                  trust_status, paired_at, protocol_min, protocol_max)
                 VALUES ('pre-v17-peer', 'Peer Mac', X'01', 'A1B2', 'trusted',
                         '2026-07-01T00:00:00.000Z', 1, 1)",
                [],
            )
            .unwrap();

        connection.execute_batch(migrations::MIGRATION_17).unwrap();

        let duration: String = connection
            .query_row(
                "SELECT trust_duration FROM local_link_devices WHERE device_id = 'pre-v17-peer'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(duration, "thirtyDays");
        assert!(connection
            .execute(
                "UPDATE local_link_devices SET trust_duration = 'thisSession'
                 WHERE device_id = 'pre-v17-peer'",
                [],
            )
            .is_err());
    }

    #[test]
    fn migration_20_preserves_v19_transfers_and_adds_content_free_copy_claims() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
            migrations::MIGRATION_16,
            migrations::MIGRATION_17,
            migrations::MIGRATION_18,
            migrations::MIGRATION_19,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        let pre_migration_version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(pre_migration_version, 19);
        connection
            .execute(
                "INSERT INTO local_link_devices
                 (device_id, display_name, peer_public_key, public_key_fingerprint,
                  trust_status, paired_at, trusted_at, protocol_min, protocol_max,
                  trust_duration)
                 VALUES ('migration-peer', 'Migration Peer',
                         X'000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F',
                         'A1B2C3',
                         'trusted', '2026-07-01T00:00:00.000Z',
                         '2026-07-01T00:01:00.000Z', 2, 2, 'thirtyDays')",
                [],
            )
            .unwrap();
        connection
            .execute_batch(
                "INSERT INTO local_link_transfers
                 (id, device_id, direction, status, item_kind, byte_size, created_at,
                  updated_at, completed_at, expires_at, failure_reason, receiver_action)
                 VALUES
                 ('incoming-active', 'migration-peer', 'incoming', 'awaitingReceiver',
                  'text', 11, '2026-07-01T01:00:00.000Z', '2026-07-01T01:00:01.000Z',
                  NULL, '2026-07-01T01:01:00.000Z', NULL, NULL),
                 ('outgoing-active', 'migration-peer', 'outgoing', 'viewed',
                  'text', 12, '2026-07-01T02:00:00.000Z', '2026-07-01T02:00:01.000Z',
                  NULL, '2026-07-01T02:01:00.000Z', NULL, NULL),
                 ('incoming-terminal', 'migration-peer', 'incoming', 'copied',
                  'text', 13, '2026-07-01T03:00:00.000Z', '2026-07-01T03:00:01.000Z',
                  '2026-07-01T03:00:01.000Z', NULL, NULL, 'copy'),
                 ('outgoing-terminal', 'migration-peer', 'outgoing', 'failed',
                  'text', 14, '2026-07-01T04:00:00.000Z', '2026-07-01T04:00:01.000Z',
                  '2026-07-01T04:00:01.000Z', NULL, 'payloadUnavailable', NULL);",
            )
            .unwrap();

        connection.execute_batch(migrations::MIGRATION_20).unwrap();
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 20);

        let device: (String, String, Vec<u8>, String, String, String) = connection
            .query_row(
                "SELECT device_id, display_name, peer_public_key, public_key_fingerprint,
                        trust_status, trust_duration
                 FROM local_link_devices WHERE device_id = 'migration-peer'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            device,
            (
                "migration-peer".to_owned(),
                "Migration Peer".to_owned(),
                (0_u8..32).collect::<Vec<_>>(),
                "A1B2C3".to_owned(),
                "trusted".to_owned(),
                "thirtyDays".to_owned(),
            )
        );
        let mut statement = connection
            .prepare(
                "SELECT id, direction, status, byte_size, created_at, updated_at,
                        completed_at, expires_at, failure_reason, receiver_action
                 FROM local_link_transfers ORDER BY id ASC",
            )
            .unwrap();
        let transfers = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(transfers.len(), 4);
        assert_eq!(
            transfers[0],
            (
                "incoming-active".to_owned(),
                "incoming".to_owned(),
                "awaitingReceiver".to_owned(),
                11,
                "2026-07-01T01:00:00.000Z".to_owned(),
                "2026-07-01T01:00:01.000Z".to_owned(),
                None,
                Some("2026-07-01T01:01:00.000Z".to_owned()),
                None,
                None,
            )
        );
        assert_eq!(transfers[1].0, "incoming-terminal");
        assert_eq!(transfers[1].2, "copied");
        assert_eq!(transfers[1].9.as_deref(), Some("copy"));
        assert_eq!(transfers[2].0, "outgoing-active");
        assert_eq!(transfers[2].2, "viewed");
        assert_eq!(transfers[3].0, "outgoing-terminal");
        assert_eq!(transfers[3].2, "failed");
        assert_eq!(transfers[3].8.as_deref(), Some("payloadUnavailable"));
        drop(statement);

        let mut statement = connection
            .prepare("PRAGMA table_info(local_link_effect_claims)")
            .unwrap();
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns,
            vec![
                "transfer_id",
                "action",
                "state",
                "effect_token",
                "pasteboard_change_count",
                "created_at",
                "updated_at",
            ]
        );
        assert!(columns.iter().all(|column| !matches!(
            column.as_str(),
            "body" | "content" | "preview" | "digest" | "peer" | "device_id" | "endpoint"
        )));

        let transfer_sql: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master
                 WHERE type = 'table' AND name = 'local_link_transfers'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(transfer_sql.contains("'reconciling'"));
        assert!(transfer_sql.contains("'outcomeUnknown'"));

        assert_eq!(
            connection
                .execute(
                    "UPDATE local_link_transfers SET status = 'reconciling'
                     WHERE id = 'outgoing-active'",
                    [],
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .execute(
                    "UPDATE local_link_transfers SET failure_reason = 'outcomeUnknown'
                     WHERE id = 'outgoing-terminal'",
                    [],
                )
                .unwrap(),
            1
        );
        assert!(connection
            .execute(
                "UPDATE local_link_transfers SET status = 'payloadCommitted'
                 WHERE id = 'outgoing-active'",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "UPDATE local_link_transfers SET failure_reason = 'bodyLost'
                 WHERE id = 'outgoing-terminal'",
                [],
            )
            .is_err());

        connection
            .execute(
                "INSERT INTO local_link_effect_claims
                 (transfer_id, action, state, effect_token, pasteboard_change_count,
                  created_at, updated_at)
                 VALUES ('incoming-active', 'copy', 'prepared',
                         '0123456789abcdef0123456789abcdef', NULL,
                         '2026-07-01T05:00:00.000Z', '2026-07-01T05:00:00.000Z')",
                [],
            )
            .unwrap();
        let foreign_keys_enabled: bool = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert!(foreign_keys_enabled);
        connection
            .execute(
                "DELETE FROM local_link_transfers WHERE id = 'incoming-active'",
                [],
            )
            .unwrap();
        let remaining_claims: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM local_link_effect_claims
                 WHERE transfer_id = 'incoming-active'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_claims, 0);
        let foreign_key_errors: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_errors, 0);
    }

    #[test]
    fn copy_effect_claim_is_single_winner_recoverable_and_idempotent() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        seed_local_link_transfer(&database, "copy-once", "incoming", "awaitingReceiver");
        let original_token = "0123456789abcdef0123456789abcdef";

        let claimed = database
            .prepare_local_link_copy_effect("copy-once", original_token)
            .unwrap();
        assert!(matches!(claimed, LocalLinkCopyClaimOutcome::Claimed(_)));
        let existing = database
            .prepare_local_link_copy_effect("copy-once", "fedcba9876543210fedcba9876543210")
            .unwrap();
        let LocalLinkCopyClaimOutcome::Existing(existing) = existing else {
            panic!("the durable claim must win a repeated preparation");
        };
        assert_eq!(existing.effect_token, original_token);
        assert_eq!(existing.state, LocalLinkCopyEffectState::Prepared);
        assert_eq!(
            database
                .local_link_copy_effect_claims_for_recovery(10)
                .unwrap()
                .len(),
            1
        );

        assert!(database.finalize_local_link_reject("copy-once").is_err());
        assert_eq!(
            database
                .complete_local_link_copy_effect("copy-once", original_token, 42)
                .unwrap(),
            LocalLinkTerminalCasOutcome::Applied
        );
        assert_eq!(
            database
                .complete_local_link_copy_effect("copy-once", original_token, 42)
                .unwrap(),
            LocalLinkTerminalCasOutcome::AlreadyApplied
        );
        let claim = database
            .local_link_copy_effect_claim("copy-once")
            .unwrap()
            .unwrap();
        assert_eq!(claim.state, LocalLinkCopyEffectState::Completed);
        assert_eq!(claim.pasteboard_change_count, Some(42));
        assert!(database
            .local_link_copy_effect_claims_for_recovery(10)
            .unwrap()
            .is_empty());

        let connection = database.connection.lock().unwrap();
        let terminal: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, failure_reason, receiver_action
                 FROM local_link_transfers WHERE id = 'copy-once'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            terminal,
            ("copied".to_owned(), None, Some("copy".to_owned()))
        );
    }

    #[test]
    fn copy_recovery_reports_uncertainty_without_replaying_effect() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        seed_local_link_transfer(&database, "copy-unknown", "incoming", "viewed");
        let token = "copy-unknown-marker-0123456789";
        database
            .prepare_local_link_copy_effect("copy-unknown", token)
            .unwrap();

        assert_eq!(
            database
                .mark_local_link_copy_effect_uncertain("copy-unknown", token)
                .unwrap(),
            LocalLinkTerminalCasOutcome::Applied
        );
        assert_eq!(
            database
                .mark_local_link_copy_effect_uncertain("copy-unknown", token)
                .unwrap(),
            LocalLinkTerminalCasOutcome::AlreadyApplied
        );
        let claim = database
            .local_link_copy_effect_claim("copy-unknown")
            .unwrap()
            .unwrap();
        assert_eq!(claim.state, LocalLinkCopyEffectState::Uncertain);

        let connection = database.connection.lock().unwrap();
        let terminal: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, failure_reason, receiver_action
                 FROM local_link_transfers WHERE id = 'copy-unknown'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            terminal,
            (
                "failed".to_owned(),
                Some("outcomeUnknown".to_owned()),
                Some("copy".to_owned())
            )
        );
    }

    #[test]
    fn local_link_save_and_terminal_transition_commit_together_once() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        seed_local_link_transfer(&database, "save-once", "incoming", "viewed");
        let marker = "atomic save marker 4c936de7";

        let saved = database
            .finalize_local_link_save("save-once", "atomic-peer", marker)
            .unwrap();
        let LocalLinkSaveFinalization::Saved(item) = saved else {
            panic!("the first finalization must save the item");
        };
        assert_eq!(item.content, marker);
        assert!(matches!(
            database
                .finalize_local_link_save("save-once", "atomic-peer", marker)
                .unwrap(),
            LocalLinkSaveFinalization::AlreadySaved
        ));

        let connection = database.connection.lock().unwrap();
        let occurrence_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_occurrences WHERE clipboard_item_id = ?1",
                [&item.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(occurrence_count, 1);
        let terminal: (String, Option<String>) = connection
            .query_row(
                "SELECT status, receiver_action FROM local_link_transfers
                 WHERE id = 'save-once'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(terminal, ("saved".to_owned(), Some("save".to_owned())));
    }

    #[test]
    fn local_link_save_rolls_back_history_when_terminal_write_fails() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        seed_local_link_transfer(&database, "save-rollback", "incoming", "awaitingReceiver");
        {
            let connection = database.connection.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_atomic_local_link_save
                     BEFORE UPDATE OF status ON local_link_transfers
                     WHEN NEW.id = 'save-rollback' AND NEW.status = 'saved'
                     BEGIN
                         SELECT RAISE(ABORT, 'synthetic terminal failure');
                     END;",
                )
                .unwrap();
        }
        let marker = "must roll back 6e9e020b";
        assert!(database
            .finalize_local_link_save("save-rollback", "atomic-peer", marker)
            .is_err());

        let connection = database.connection.lock().unwrap();
        let stored: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE content = ?1",
                [marker],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, 0);
        let status: String = connection
            .query_row(
                "SELECT status FROM local_link_transfers WHERE id = 'save-rollback'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "awaitingReceiver");
    }

    #[test]
    fn reject_and_sender_reconciliation_transitions_are_idempotent_cas() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        seed_local_link_transfer(&database, "reject-once", "incoming", "awaitingReceiver");
        assert_eq!(
            database.finalize_local_link_reject("reject-once").unwrap(),
            LocalLinkTerminalCasOutcome::Applied
        );
        assert_eq!(
            database.finalize_local_link_reject("reject-once").unwrap(),
            LocalLinkTerminalCasOutcome::AlreadyApplied
        );

        seed_local_link_transfer(&database, "reconcile-once", "outgoing", "encrypted");
        let deadline = "2099-01-01T00:00:00.000Z";
        assert_eq!(
            database
                .mark_local_link_transfer_reconciling("reconcile-once", deadline)
                .unwrap(),
            LocalLinkTerminalCasOutcome::Applied
        );
        assert_eq!(
            database
                .mark_local_link_transfer_reconciling("reconcile-once", deadline)
                .unwrap(),
            LocalLinkTerminalCasOutcome::AlreadyApplied
        );
        assert_eq!(
            database
                .finalize_local_link_outcome_unknown("reconcile-once")
                .unwrap(),
            LocalLinkTerminalCasOutcome::Applied
        );
        assert_eq!(
            database
                .finalize_local_link_outcome_unknown("reconcile-once")
                .unwrap(),
            LocalLinkTerminalCasOutcome::AlreadyApplied
        );
    }

    #[test]
    fn migration_21_upgrades_v20_with_item_owned_image_text_fts() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
            migrations::MIGRATION_16,
            migrations::MIGRATION_17,
            migrations::MIGRATION_18,
            migrations::MIGRATION_19,
            migrations::MIGRATION_20,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        let before: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(before, 20);

        connection.execute_batch(migrations::MIGRATION_21).unwrap();
        let after: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(after, 21);
        let objects: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE name IN (
                    'clipboard_image_text_extractions',
                    'clipboard_image_text_extractions_fts',
                    'clipboard_image_text_extractions_ai',
                    'clipboard_image_text_extractions_ad',
                    'clipboard_image_text_extractions_au'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(objects, 5);
    }

    #[test]
    fn migration_26_retries_after_interruption_without_losing_local_facts() {
        let root = std::env::temp_dir().join(format!(
            "clipriva-alpha2-upgrade-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("clipriva.sqlite3");
        let image_bytes = encode_rgba_png(&[24, 96, 160, 255], 1, 1).unwrap();
        let source_blob_store = BlobStore::open(root.join("blobs")).unwrap();
        let shared_blob = source_blob_store
            .store(BlobInput {
                kind: BlobKind::Image,
                mime_type: "image/png",
                display_name: Some("synthetic-upgrade.png"),
                image_dimensions: Some(ImageDimensions {
                    width: 1,
                    height: 1,
                }),
                bytes: &image_bytes,
            })
            .unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        for migration in [
            migrations::MIGRATION_1,
            migrations::MIGRATION_2,
            migrations::MIGRATION_3,
            migrations::MIGRATION_4,
            migrations::MIGRATION_5,
            migrations::MIGRATION_6,
            migrations::MIGRATION_7,
            migrations::MIGRATION_8,
            migrations::MIGRATION_9,
            migrations::MIGRATION_10,
            migrations::MIGRATION_11,
            migrations::MIGRATION_12,
            migrations::MIGRATION_13,
            migrations::MIGRATION_14,
            migrations::MIGRATION_15,
            migrations::MIGRATION_16,
            migrations::MIGRATION_17,
            migrations::MIGRATION_18,
            migrations::MIGRATION_19,
            migrations::MIGRATION_20,
            migrations::MIGRATION_21,
            migrations::MIGRATION_22,
            migrations::MIGRATION_23,
            migrations::MIGRATION_24,
            migrations::MIGRATION_25,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        let now = super::timestamp();
        connection
            .execute(
                "INSERT INTO clipboard_items
             (id, global_id, device_id, content, content_hash, kind, source_app,
              created_at, updated_at, retention_until, is_pinned, copy_count, search_text)
             VALUES ('saved-text', 'saved-text', 'synthetic-device', 'Synthetic retained text',
                     'synthetic-hash', 'text', 'Notes', ?1, ?1, '2099-01-01T00:00:00Z', 1, 2,
                     'Synthetic retained text')",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_items
             (id, global_id, device_id, content, content_hash, kind, source_app,
              created_at, updated_at, retention_until, is_pinned, copy_count, search_text)
             VALUES ('image-item', 'image-item', 'synthetic-device', 'Synthetic image',
                     'synthetic-image-hash', 'image', 'Preview', ?1, ?1,
                     '2099-01-01T00:00:00Z', 0, 1, 'Synthetic image')",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_items
             (id, global_id, device_id, content, content_hash, kind, source_app,
              created_at, updated_at, retention_until, is_pinned, copy_count, search_text,
              deleted_at, recycle_expires_at)
             VALUES ('image-recycled', 'image-recycled', 'synthetic-device',
                     'Synthetic recycled image', 'synthetic-recycled-hash', 'image', 'Preview',
                     ?1, ?1, '2099-01-01T00:00:00Z', 0, 0, 'Synthetic recycled image',
                     ?1, '2099-01-01T00:00:00Z')",
                [&now],
            )
            .unwrap();
        for item_id in ["image-item", "image-recycled"] {
            connection
                .execute(
                    "INSERT INTO clipboard_representations
                 (clipboard_item_id, storage_key, content_hash, kind, mime_type,
                  display_name, image_width, image_height, byte_size, created_at)
                 VALUES (?1, ?2, ?3, 'image', 'image/png', 'synthetic-upgrade.png',
                         1, 1, ?4, ?5)",
                    params![
                        item_id,
                        &shared_blob.storage_key,
                        &shared_blob.content_hash,
                        shared_blob.byte_size,
                        &now
                    ],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO clipboard_item_tags
             (clipboard_item_id, label, position, created_at)
             VALUES ('saved-text', 'Reference', 0, ?1)",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_item_notes
             (clipboard_item_id, text, created_at, updated_at)
             VALUES ('saved-text', 'Synthetic searchable Note', ?1, ?1)",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_image_text_extractions
             (clipboard_item_id, text, extracted_at, updated_at)
             VALUES ('image-item', 'Synthetic manual OCR', ?1, ?1)",
                [&now],
            )
            .unwrap();
        connection.execute("INSERT INTO clipboard_smart_collections
             (id, name, rule_schema_version, rule_json, created_at, updated_at)
             VALUES ('smart-reference', 'Saved references', 1,
                     '{\"kinds\":[\"text\"],\"sourceApp\":null,\"pinFilter\":\"pinned\",\"timeFilter\":\"all\",\"recentlyUsedOnly\":false,\"localLinkOnly\":false}',
                     ?1, ?1)", [&now]).unwrap();
        connection
            .execute(
                "INSERT INTO local_text_actions
             (id, name, transform, pipeline_json, is_builtin, created_at, updated_at)
             VALUES ('legacy-filter', 'Legacy uppercase', 'uppercase', NULL, 0, ?1, ?1)",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE capture_preferences SET retention_days = 60,
             labs_enabled = 1, stack_shortcut = 'CommandOrControl+Shift+S'
             WHERE singleton = 1",
                [],
            )
            .unwrap();

        // A quiescent v25 profile can be snapshotted without copying a live WAL.
        // Media is copied through its verified, content-addressed store.
        let backup_root = root.join("synthetic-backup");
        std::fs::create_dir_all(&backup_root).unwrap();
        let backup_path = backup_root.join("clipriva.sqlite3");
        connection
            .execute("VACUUM INTO ?1", [backup_path.to_str().unwrap()])
            .unwrap();
        let backed_up_bytes = source_blob_store.read(&shared_blob.storage_key).unwrap();
        let backed_up_blob = BlobStore::open(backup_root.join("blobs"))
            .unwrap()
            .store(BlobInput {
                kind: BlobKind::Image,
                mime_type: "image/png",
                display_name: Some("synthetic-upgrade.png"),
                image_dimensions: Some(ImageDimensions {
                    width: 1,
                    height: 1,
                }),
                bytes: &backed_up_bytes,
            })
            .unwrap();
        assert_eq!(backed_up_blob.storage_key, shared_blob.storage_key);

        let interrupted = migrations::MIGRATION_26.replace(
            "PRAGMA user_version = 26;",
            "SELECT missing_column FROM local_text_actions; PRAGMA user_version = 26;",
        );
        assert!(connection.execute_batch(&interrupted).is_err());
        connection.execute_batch("ROLLBACK;").unwrap();
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 25);
        let shortcut_column_count: u32 = connection.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('local_text_actions') WHERE name = 'shortcut_slot'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(shortcut_column_count, 0);
        drop(connection);

        let database = Database::open(&path).unwrap();
        let saved = database.get_by_id("saved-text").unwrap();
        assert!(saved.is_pinned);
        assert_eq!(saved.tags, ["Reference"]);
        assert_eq!(
            database.item_note("saved-text").unwrap().unwrap().text,
            "Synthetic searchable Note"
        );
        assert_eq!(
            database
                .image_text_extraction("image-item")
                .unwrap()
                .unwrap()
                .text,
            "Synthetic manual OCR"
        );
        let active_image = database.get_by_id("image-item").unwrap();
        assert_eq!(active_image.representations.len(), 1);
        assert_eq!(
            active_image.representations[0].storage_key,
            shared_blob.storage_key
        );
        assert_eq!(
            database.read_blob(&shared_blob.storage_key).unwrap(),
            image_bytes
        );
        let recycled = database.recycle_bin_items(10).unwrap();
        assert_eq!(recycled.len(), 1);
        assert_eq!(recycled[0].item.id, "image-recycled");
        assert_eq!(
            recycled[0].item.representations[0].storage_key,
            shared_blob.storage_key
        );
        let restored = database.restore_from_recycle_bin("image-recycled").unwrap();
        assert_eq!(
            restored.representations[0].storage_key,
            shared_blob.storage_key
        );
        assert!(database.recycle_bin_items(10).unwrap().is_empty());
        assert!(database
            .list_local_text_actions()
            .unwrap()
            .iter()
            .any(|action| action.id == "legacy-filter"));
        let connection = database.connection.lock().unwrap();
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 26);
        let references: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM clipboard_smart_collections WHERE id = 'smart-reference'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(references, 1);
        let foreign_key_errors: u32 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_errors, 0);
        drop(connection);
        let preferences = database.capture_preferences().unwrap();
        assert_eq!(preferences.retention_days, 60);
        assert!(preferences.labs_enabled);
        assert_eq!(preferences.stack_shortcut, "CommandOrControl+Shift+S");
        drop(database);

        let reopened = Database::open(&path).unwrap();
        assert_eq!(
            reopened
                .get_by_id("image-recycled")
                .unwrap()
                .representations[0]
                .storage_key,
            shared_blob.storage_key
        );
        reopened.delete("image-item").unwrap();
        reopened
            .permanently_delete_recycled_items(&["image-item".to_owned()])
            .unwrap();
        assert_eq!(
            reopened.read_blob(&shared_blob.storage_key).unwrap(),
            image_bytes
        );
        reopened.delete("image-recycled").unwrap();
        reopened
            .permanently_delete_recycled_items(&["image-recycled".to_owned()])
            .unwrap();
        assert!(reopened.read_blob(&shared_blob.storage_key).is_err());
        drop(reopened);

        let restored_backup = Database::open(&backup_path).unwrap();
        assert!(restored_backup.get_by_id("saved-text").unwrap().is_pinned);
        assert_eq!(
            restored_backup
                .item_note("saved-text")
                .unwrap()
                .unwrap()
                .text,
            "Synthetic searchable Note"
        );
        assert_eq!(
            restored_backup
                .get_by_id("image-item")
                .unwrap()
                .representations[0]
                .storage_key,
            shared_blob.storage_key
        );
        assert_eq!(
            restored_backup.recycle_bin_items(10).unwrap()[0].item.id,
            "image-recycled"
        );
        assert_eq!(
            restored_backup.read_blob(&shared_blob.storage_key).unwrap(),
            image_bytes
        );
        drop(restored_backup);
        std::fs::remove_dir_all(root).unwrap();
    }
}
