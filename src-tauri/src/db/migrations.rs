pub const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS clipboard_items (
    id TEXT PRIMARY KEY NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'text',
    source_app TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    is_pinned INTEGER NOT NULL DEFAULT 0,
    copy_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at
    ON clipboard_items(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_clipboard_items_pinned_created
    ON clipboard_items(is_pinned DESC, created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_items_fts USING fts5(
    content,
    content='clipboard_items',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS clipboard_items_after_insert
AFTER INSERT ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_items_after_delete
AFTER DELETE ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER IF NOT EXISTS clipboard_items_after_update
AFTER UPDATE OF content ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO clipboard_items_fts(rowid, content) VALUES (new.rowid, new.content);
END;

PRAGMA user_version = 1;
"#;

pub const MIGRATION_2: &str = r#"
CREATE TABLE IF NOT EXISTS capture_preferences (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    capture_paused INTEGER NOT NULL DEFAULT 0,
    -- New installs use the low-interruption policy introduced in migration 7.
    -- Existing installs keep their historical `1` value and are mapped to
    -- the Strict policy during that migration.
    sensitive_pause_enabled INTEGER NOT NULL DEFAULT 0,
    retention_days INTEGER NOT NULL DEFAULT 30,
    max_history_items INTEGER NOT NULL DEFAULT 5000,
    denied_apps_json TEXT NOT NULL DEFAULT '[]',
    pause_reason TEXT,
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO capture_preferences
    (singleton, updated_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

PRAGMA user_version = 2;
"#;

pub const MIGRATION_3: &str = r#"
CREATE TABLE IF NOT EXISTS context_enrichments (
    clipboard_item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL,
    enricher_id TEXT NOT NULL,
    enricher_version TEXT NOT NULL,
    enrichment_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_context_enrichments_enricher
    ON context_enrichments(enricher_id, enricher_version, schema_version);

PRAGMA user_version = 3;
"#;

pub const MIGRATION_4: &str = r#"
CREATE TABLE IF NOT EXISTS local_text_actions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    transform TEXT NOT NULL CHECK(transform IN (
        'uppercase', 'lowercase', 'trim_whitespace', 'normalize_whitespace', 'format_json'
    )),
    is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS action_audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    schema_version INTEGER NOT NULL,
    action_id TEXT,
    action_name TEXT NOT NULL,
    transform TEXT,
    execution_mode TEXT NOT NULL CHECK(execution_mode IN ('local', 'cloud_preview')),
    status TEXT NOT NULL CHECK(status IN ('completed', 'preview_only_blocked')),
    clipboard_item_id TEXT NOT NULL,
    input_content_hash TEXT NOT NULL,
    input_preview TEXT NOT NULL,
    output_content_hash TEXT,
    output_preview TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_action_audit_clip_created
    ON action_audit_log(clipboard_item_id, created_at DESC);

CREATE TRIGGER IF NOT EXISTS action_audit_log_prevent_update
BEFORE UPDATE ON action_audit_log
BEGIN
    SELECT RAISE(ABORT, 'action audit records are immutable');
END;

CREATE TRIGGER IF NOT EXISTS action_audit_log_prevent_delete
BEFORE DELETE ON action_audit_log
BEGIN
    SELECT RAISE(ABORT, 'action audit records are immutable');
END;

INSERT OR IGNORE INTO local_text_actions
    (id, name, transform, is_builtin, created_at, updated_at)
VALUES
    ('builtin-uppercase', 'Uppercase', 'uppercase', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('builtin-lowercase', 'Lowercase', 'lowercase', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('builtin-trim-whitespace', 'Trim whitespace', 'trim_whitespace', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('builtin-normalize-whitespace', 'Normalize whitespace', 'normalize_whitespace', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('builtin-format-json', 'Format JSON', 'format_json', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

PRAGMA user_version = 4;
"#;

pub const MIGRATION_5: &str = r#"
CREATE TABLE IF NOT EXISTS clipboard_representations (
    clipboard_item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    storage_key TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('image', 'richText', 'file')),
    mime_type TEXT NOT NULL,
    display_name TEXT,
    image_width INTEGER,
    image_height INTEGER,
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (clipboard_item_id, storage_key),
    CHECK(
        (image_width IS NULL AND image_height IS NULL)
        OR (image_width > 0 AND image_height > 0)
    )
);

CREATE INDEX IF NOT EXISTS idx_clipboard_representations_storage_key
    ON clipboard_representations(storage_key);

PRAGMA user_version = 5;
"#;

pub const MIGRATION_6: &str = r#"
ALTER TABLE capture_preferences
    ADD COLUMN labs_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE capture_preferences
    ADD COLUMN paste_behavior TEXT NOT NULL DEFAULT 'restore'
        CHECK(paste_behavior IN ('restore', 'autoPaste'));
ALTER TABLE capture_preferences
    ADD COLUMN quick_paste_shortcut TEXT NOT NULL DEFAULT 'CommandOrControl+Shift+Space';
ALTER TABLE capture_preferences
    ADD COLUMN onboarding_completed INTEGER NOT NULL DEFAULT 0;

PRAGMA user_version = 6;
"#;

pub const MIGRATION_7: &str = r#"
ALTER TABLE capture_preferences
    ADD COLUMN sensitive_content_policy TEXT NOT NULL DEFAULT 'default'
        CHECK(sensitive_content_policy IN ('default', 'strict'));

-- `sensitive_pause_enabled = 1` was the prior safe behavior: a sensitive
-- value stopped subsequent capture. Preserve that behavior for existing
-- installations by making it explicit rather than silently weakening it.
UPDATE capture_preferences
SET sensitive_content_policy = CASE sensitive_pause_enabled
    WHEN 1 THEN 'strict'
    ELSE 'default'
END;

PRAGMA user_version = 7;
"#;

// Capture-trust evidence intentionally has no clipboard-content column. It
// answers why a recent clipboard change was not saved without creating a new
// path for sensitive data to persist locally.
pub const MIGRATION_8: &str = r#"
CREATE TABLE IF NOT EXISTS uncaptured_clipboard_events (
    id TEXT PRIMARY KEY NOT NULL,
    reason_type TEXT NOT NULL CHECK(reason_type IN (
        'manualPause',
        'sensitiveContentDefault',
        'sensitiveContentStrict',
        'excludedApplication',
        'unsupportedFormat',
        'contentTooLarge'
    )),
    source_app TEXT,
    occurred_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_uncaptured_clipboard_events_recent
    ON uncaptured_clipboard_events(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_uncaptured_clipboard_events_expiry
    ON uncaptured_clipboard_events(expires_at);

PRAGMA user_version = 8;
"#;

// Recycle-bin state stays on the original clip row. That retains local media
// references and all capture metadata without copying sensitive clipboard
// content into a second table. Active-history queries always require
// `deleted_at IS NULL`; expired recycle-bin rows are permanently purged.
pub const MIGRATION_9: &str = r#"
ALTER TABLE clipboard_items
    ADD COLUMN deleted_at TEXT;
ALTER TABLE clipboard_items
    ADD COLUMN recycle_expires_at TEXT;
ALTER TABLE clipboard_items
    ADD COLUMN restored_at TEXT;

CREATE INDEX IF NOT EXISTS idx_clipboard_items_recycle_expiry
    ON clipboard_items(recycle_expires_at)
    WHERE deleted_at IS NOT NULL;

PRAGMA user_version = 9;
"#;

// Local diagnostics are opt-in and structurally incapable of retaining
// clipboard content or other reconstructable context. The only persisted
// event dimensions are allow-listed category/result values, a date-only UTC
// bucket, and the app version that produced the event.
pub const MIGRATION_10: &str = r#"
ALTER TABLE capture_preferences
    ADD COLUMN diagnostics_enabled INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS local_diagnostic_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK(event_type IN (
        'firstRecovery',
        'directPaste',
        'recycleBinRestore',
        'duplicateGroupExpand',
        'clipboardRestoreError',
        'capturePermissionRestricted',
        'systemPolicyRestricted'
    )),
    outcome TEXT NOT NULL CHECK(outcome IN (
        'started', 'completed', 'degraded', 'failed', 'restricted', 'restored', 'expanded'
    )),
    occurred_day TEXT NOT NULL,
    app_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_local_diagnostic_events_day
    ON local_diagnostic_events(occurred_day DESC);

PRAGMA user_version = 10;
"#;

// 0.4 keeps every capture as an independent local record so exact duplicates
// can be presented as a reversible version group. The original v1 table made
// `content_hash` unique, so rebuilding the table is the only safe way to
// remove that constraint while preserving existing IDs and child references.
// This migration is atomic: an interruption leaves the prior v10 database
// untouched and the next open can retry from the same version.
pub const MIGRATION_11: &str = r#"
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

DROP TRIGGER IF EXISTS clipboard_items_after_insert;
DROP TRIGGER IF EXISTS clipboard_items_after_delete;
DROP TRIGGER IF EXISTS clipboard_items_after_update;
DROP TABLE IF EXISTS clipboard_items_fts;

CREATE TABLE IF NOT EXISTS clipriva_local_identity (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    device_id TEXT NOT NULL
);
INSERT OR IGNORE INTO clipriva_local_identity (singleton, device_id)
VALUES (1, lower(hex(randomblob(16))));

CREATE TABLE clipboard_items_0_4 (
    id TEXT PRIMARY KEY NOT NULL,
    global_id TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    group_id TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version > 0),
    kind TEXT NOT NULL DEFAULT 'text',
    source_app TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    retention_until TEXT NOT NULL,
    is_pinned INTEGER NOT NULL DEFAULT 0,
    copy_count INTEGER NOT NULL DEFAULT 0,
    deleted_at TEXT,
    recycle_expires_at TEXT,
    restored_at TEXT
);

INSERT INTO clipboard_items_0_4 (
    id, global_id, device_id, content, content_hash, group_id, version, kind,
    source_app, created_at, updated_at, retention_until, is_pinned, copy_count,
    deleted_at, recycle_expires_at, restored_at
)
SELECT
    legacy.id,
    legacy.id,
    identity.device_id,
    legacy.content,
    legacy.content_hash,
    CASE
        WHEN legacy.kind IN ('text', 'code', 'url', 'color', 'file') THEN legacy.id
        ELSE NULL
    END,
    1,
    legacy.kind,
    legacy.source_app,
    legacy.created_at,
    legacy.updated_at,
    COALESCE(
        strftime(
            '%Y-%m-%dT%H:%M:%fZ',
            julianday(legacy.created_at) + (
                SELECT retention_days FROM capture_preferences WHERE singleton = 1
            )
        ),
        legacy.created_at
    ),
    legacy.is_pinned,
    legacy.copy_count,
    legacy.deleted_at,
    legacy.recycle_expires_at,
    legacy.restored_at
FROM clipboard_items AS legacy
CROSS JOIN clipriva_local_identity AS identity
WHERE identity.singleton = 1;

DROP TABLE clipboard_items;
ALTER TABLE clipboard_items_0_4 RENAME TO clipboard_items;

CREATE INDEX idx_clipboard_items_created_at
    ON clipboard_items(created_at DESC);
CREATE INDEX idx_clipboard_items_pinned_created
    ON clipboard_items(is_pinned DESC, created_at DESC);
CREATE INDEX idx_clipboard_items_group_version
    ON clipboard_items(group_id, version DESC, created_at DESC)
    WHERE group_id IS NOT NULL;
CREATE INDEX idx_clipboard_items_recycle_expiry
    ON clipboard_items(recycle_expires_at)
    WHERE deleted_at IS NOT NULL;

CREATE VIRTUAL TABLE clipboard_items_fts USING fts5(
    content,
    content='clipboard_items',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER clipboard_items_after_insert
AFTER INSERT ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER clipboard_items_after_delete
AFTER DELETE ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER clipboard_items_after_update
AFTER UPDATE OF content ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO clipboard_items_fts(rowid, content) VALUES (new.rowid, new.content);
END;

INSERT INTO clipboard_items_fts(clipboard_items_fts) VALUES ('rebuild');

PRAGMA user_version = 11;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// 0.5 promotes an exact clipboard value to one canonical Item and records
// each capture separately as an Occurrence. The oldest row in every v11
// exact-duplicate group keeps its stable IDs; child metadata is remapped
// before duplicate rows are removed. Existing representation rows remain the
// local Variant storage boundary.
pub const MIGRATION_12: &str = r#"
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

DROP TRIGGER IF EXISTS clipboard_items_after_insert;
DROP TRIGGER IF EXISTS clipboard_items_after_delete;
DROP TRIGGER IF EXISTS clipboard_items_after_update;
DROP TABLE IF EXISTS clipboard_items_fts;

ALTER TABLE clipboard_items
    ADD COLUMN last_used_at TEXT;
ALTER TABLE clipboard_items
    ADD COLUMN search_text TEXT NOT NULL DEFAULT '';
UPDATE clipboard_items SET search_text = content;

CREATE TEMP TABLE clipriva_v12_canonical_map (
    old_id TEXT PRIMARY KEY NOT NULL,
    canonical_id TEXT NOT NULL
);

INSERT INTO clipriva_v12_canonical_map (old_id, canonical_id)
SELECT
    item.id,
    CASE
        WHEN item.kind IN ('text', 'code', 'url', 'color', 'richText') THEN (
            SELECT candidate.id
            FROM clipboard_items AS candidate
            WHERE candidate.kind IN ('text', 'code', 'url', 'color', 'richText')
              AND candidate.content = item.content COLLATE BINARY
            ORDER BY candidate.created_at ASC, candidate.id ASC
            LIMIT 1
        )
        WHEN item.group_id IS NULL THEN item.id
        ELSE (
            SELECT candidate.id
            FROM clipboard_items AS candidate
            WHERE candidate.group_id = item.group_id
            ORDER BY candidate.created_at ASC, candidate.id ASC
            LIMIT 1
        )
    END
FROM clipboard_items AS item;

CREATE TABLE clipboard_occurrences (
    id TEXT PRIMARY KEY NOT NULL,
    clipboard_item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    source_app TEXT,
    occurred_at TEXT NOT NULL
);

INSERT INTO clipboard_occurrences
    (id, clipboard_item_id, device_id, source_app, occurred_at)
SELECT item.id, mapping.canonical_id, item.device_id, item.source_app, item.created_at
FROM clipboard_items AS item
INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = item.id;

INSERT OR IGNORE INTO clipboard_representations (
    clipboard_item_id, storage_key, content_hash, kind, mime_type, display_name,
    image_width, image_height, byte_size, created_at
)
SELECT
    mapping.canonical_id, representation.storage_key, representation.content_hash,
    representation.kind, representation.mime_type, representation.display_name,
    representation.image_width, representation.image_height, representation.byte_size,
    representation.created_at
FROM clipboard_representations AS representation
INNER JOIN clipriva_v12_canonical_map AS mapping
    ON mapping.old_id = representation.clipboard_item_id
WHERE mapping.old_id <> mapping.canonical_id
ORDER BY representation.created_at ASC, representation.storage_key ASC;

DELETE FROM clipboard_representations
WHERE clipboard_item_id IN (
    SELECT old_id FROM clipriva_v12_canonical_map WHERE old_id <> canonical_id
);

CREATE TEMP TABLE clipriva_v12_context_backup AS
SELECT canonical_id, schema_version, enricher_id, enricher_version, enrichment_json, updated_at
FROM (
    SELECT
        mapping.canonical_id,
        enrichment.schema_version,
        enrichment.enricher_id,
        enrichment.enricher_version,
        enrichment.enrichment_json,
        enrichment.updated_at,
        ROW_NUMBER() OVER (
            PARTITION BY mapping.canonical_id
            ORDER BY enrichment.updated_at DESC, enrichment.clipboard_item_id ASC
        ) AS preference_rank
    FROM context_enrichments AS enrichment
    INNER JOIN clipriva_v12_canonical_map AS mapping
        ON mapping.old_id = enrichment.clipboard_item_id
)
WHERE preference_rank = 1;

DELETE FROM context_enrichments
WHERE clipboard_item_id IN (SELECT old_id FROM clipriva_v12_canonical_map);

INSERT INTO context_enrichments (
    clipboard_item_id, schema_version, enricher_id, enricher_version,
    enrichment_json, updated_at
)
SELECT canonical_id, schema_version, enricher_id, enricher_version, enrichment_json, updated_at
FROM clipriva_v12_context_backup;

DROP TABLE clipriva_v12_context_backup;

DROP TRIGGER IF EXISTS action_audit_log_prevent_update;
DROP TRIGGER IF EXISTS action_audit_log_prevent_delete;
UPDATE action_audit_log
SET clipboard_item_id = (
    SELECT mapping.canonical_id
    FROM clipriva_v12_canonical_map AS mapping
    WHERE mapping.old_id = action_audit_log.clipboard_item_id
)
WHERE clipboard_item_id IN (
    SELECT old_id FROM clipriva_v12_canonical_map WHERE old_id <> canonical_id
);
CREATE TRIGGER action_audit_log_prevent_update
BEFORE UPDATE ON action_audit_log
BEGIN
    SELECT RAISE(ABORT, 'action audit records are immutable');
END;
CREATE TRIGGER action_audit_log_prevent_delete
BEFORE DELETE ON action_audit_log
BEGIN
    SELECT RAISE(ABORT, 'action audit records are immutable');
END;

UPDATE clipboard_items AS canonical
SET kind = CASE
        WHEN EXISTS (
            SELECT 1
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id AND candidate.kind = 'richText'
        ) THEN 'richText'
        ELSE canonical.kind
    END,
    content_hash = COALESCE(
        (
            SELECT candidate.content_hash
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id AND candidate.kind <> 'richText'
            ORDER BY candidate.created_at ASC, candidate.id ASC
            LIMIT 1
        ),
        canonical.content_hash
    ),
    source_app = (
        SELECT candidate.source_app
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id AND candidate.source_app IS NOT NULL
        ORDER BY candidate.created_at DESC, candidate.id DESC
        LIMIT 1
    ),
    updated_at = (
        SELECT MAX(candidate.updated_at)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id
    ),
    retention_until = (
        SELECT MAX(candidate.retention_until)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id
    ),
    is_pinned = (
        SELECT MAX(candidate.is_pinned)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id
    ),
    copy_count = (
        SELECT SUM(candidate.copy_count)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id
    ),
    deleted_at = CASE
        WHEN EXISTS (
            SELECT 1
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id AND candidate.deleted_at IS NULL
        ) THEN NULL
        ELSE (
            SELECT MAX(candidate.deleted_at)
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id
        )
    END,
    recycle_expires_at = CASE
        WHEN EXISTS (
            SELECT 1
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id AND candidate.deleted_at IS NULL
        ) THEN NULL
        ELSE (
            SELECT MAX(candidate.recycle_expires_at)
            FROM clipboard_items AS candidate
            INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
            WHERE mapping.canonical_id = canonical.id
        )
    END,
    restored_at = (
        SELECT MAX(candidate.restored_at)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id
    ),
    last_used_at = (
        SELECT MAX(candidate.updated_at)
        FROM clipboard_items AS candidate
        INNER JOIN clipriva_v12_canonical_map AS mapping ON mapping.old_id = candidate.id
        WHERE mapping.canonical_id = canonical.id AND candidate.copy_count > 0
    ),
    group_id = NULL,
    version = 1
WHERE canonical.id IN (
    SELECT canonical_id FROM clipriva_v12_canonical_map
);

DELETE FROM clipboard_items
WHERE id IN (
    SELECT old_id FROM clipriva_v12_canonical_map WHERE old_id <> canonical_id
);

DROP INDEX IF EXISTS idx_clipboard_items_group_version;
CREATE INDEX idx_clipboard_items_exact_content
    ON clipboard_items(kind, content_hash);
CREATE INDEX idx_clipboard_occurrences_item_recent
    ON clipboard_occurrences(clipboard_item_id, occurred_at DESC, id DESC);
CREATE INDEX idx_clipboard_occurrences_source
    ON clipboard_occurrences(source_app COLLATE NOCASE, occurred_at DESC)
    WHERE source_app IS NOT NULL;

CREATE VIRTUAL TABLE clipboard_items_fts USING fts5(
    search_text,
    content='clipboard_items',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER clipboard_items_after_insert
AFTER INSERT ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(rowid, search_text) VALUES (new.rowid, new.search_text);
END;

CREATE TRIGGER clipboard_items_after_delete
AFTER DELETE ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, search_text)
    VALUES ('delete', old.rowid, old.search_text);
END;

CREATE TRIGGER clipboard_items_after_update
AFTER UPDATE OF search_text ON clipboard_items BEGIN
    INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, search_text)
    VALUES ('delete', old.rowid, old.search_text);
    INSERT INTO clipboard_items_fts(rowid, search_text) VALUES (new.rowid, new.search_text);
END;

INSERT INTO clipboard_items_fts(clipboard_items_fts) VALUES ('rebuild');

DROP TABLE clipriva_v12_canonical_map;
PRAGMA user_version = 12;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// Local Link is intentionally an opt-in experiment. These tables store only
// device display metadata and local transfer state. No endpoint, account,
// discovery address, or remote service configuration is persisted here.
pub const MIGRATION_13: &str = r#"
BEGIN;
PRAGMA foreign_keys = OFF;

CREATE TABLE local_link_preferences (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0, 1)),
    discovery_enabled INTEGER NOT NULL DEFAULT 0 CHECK(discovery_enabled IN (0, 1)),
    device_name TEXT NOT NULL DEFAULT 'This device',
    updated_at TEXT NOT NULL
);

INSERT INTO local_link_preferences
    (singleton, enabled, discovery_enabled, device_name, updated_at)
VALUES (1, 0, 0, 'This device', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TABLE local_link_devices (
    device_id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    -- The peer public key is opaque local handshake material. It is neither
    -- an account identifier nor a network endpoint.
    peer_public_key TEXT NOT NULL DEFAULT '',
    public_key_fingerprint TEXT NOT NULL DEFAULT '',
    trust_status TEXT NOT NULL CHECK(trust_status IN ('pairing', 'trusted', 'revoked')),
    pairing_code_hash TEXT NOT NULL,
    paired_at TEXT NOT NULL,
    trusted_at TEXT,
    revoked_at TEXT,
    last_seen_at TEXT,
    last_transfer_at TEXT
);

CREATE TABLE local_link_transfers (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES local_link_devices(device_id) ON DELETE RESTRICT,
    direction TEXT NOT NULL CHECK(direction IN ('outgoing', 'incoming')),
    status TEXT NOT NULL CHECK(status IN (
        'pendingAcceptance', 'sent', 'accepted', 'rejected', 'failed'
    )),
    clipboard_item_id TEXT REFERENCES clipboard_items(id) ON DELETE SET NULL,
    received_clipboard_item_id TEXT REFERENCES clipboard_items(id) ON DELETE SET NULL,
    item_kind TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    failure_reason TEXT CHECK(failure_reason IN (
        'transportUnavailable', 'captureBlocked', 'itemUnavailable',
        'unsupportedItem', 'deviceNotTrusted', 'deviceRevoked', 'payloadUnavailable'
    )),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK(retry_count >= 0)
);

CREATE INDEX idx_local_link_devices_status
    ON local_link_devices(trust_status, paired_at DESC);
CREATE INDEX idx_local_link_transfers_recent
    ON local_link_transfers(created_at DESC, id DESC);
CREATE INDEX idx_local_link_transfers_device
    ON local_link_transfers(device_id, updated_at DESC);

PRAGMA user_version = 13;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// v0.7 records only which explicit receiver action completed an incoming
// transfer. The column deliberately has no plaintext, preview, endpoint,
// session, or key material.
pub const MIGRATION_14: &str = r#"
BEGIN;

ALTER TABLE local_link_transfers
    ADD COLUMN receiver_action TEXT CHECK(receiver_action IN ('copy', 'save'));

PRAGMA user_version = 14;
COMMIT;
"#;

// v0.8 replaces the local control-plane preview schema with a content-free
// authenticated-transport schema. Preview trust rows never contained a peer
// identity, so they are deliberately downgraded instead of being treated as
// real network trust. Enabling is also reset once on upgrade so installing the
// new listener can never start network activity without a fresh user action.
pub const MIGRATION_15: &str = r#"
BEGIN;
PRAGMA foreign_keys = OFF;

UPDATE local_link_preferences
SET enabled = 0, discovery_enabled = 0,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE singleton = 1;

DROP INDEX IF EXISTS idx_local_link_devices_status;
DROP INDEX IF EXISTS idx_local_link_transfers_recent;
DROP INDEX IF EXISTS idx_local_link_transfers_device;

ALTER TABLE local_link_devices RENAME TO local_link_devices_v07;
ALTER TABLE local_link_transfers RENAME TO local_link_transfers_v07;

CREATE TABLE local_link_devices (
    device_id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    peer_public_key BLOB NOT NULL DEFAULT X'',
    public_key_fingerprint TEXT NOT NULL DEFAULT '',
    trust_status TEXT NOT NULL CHECK(trust_status IN (
        'pairing', 'trusted', 'needsRePairing', 'revoked'
    )),
    paired_at TEXT NOT NULL,
    trusted_at TEXT,
    revoked_at TEXT,
    last_seen_at TEXT,
    last_transfer_at TEXT,
    protocol_min INTEGER NOT NULL DEFAULT 1 CHECK(protocol_min BETWEEN 1 AND 65535),
    protocol_max INTEGER NOT NULL DEFAULT 1 CHECK(protocol_max BETWEEN protocol_min AND 65535)
);

INSERT INTO local_link_devices
    (device_id, display_name, peer_public_key, public_key_fingerprint, trust_status,
     paired_at, trusted_at, revoked_at, last_seen_at, last_transfer_at,
     protocol_min, protocol_max)
SELECT
    device_id,
    display_name,
    CASE
        WHEN trust_status = 'trusted' AND public_key_fingerprint <> ''
            THEN CAST(peer_public_key AS BLOB)
        ELSE X''
    END,
    public_key_fingerprint,
    CASE
        WHEN trust_status = 'revoked' THEN 'revoked'
        ELSE 'needsRePairing'
    END,
    paired_at,
    CASE WHEN trust_status = 'trusted' AND public_key_fingerprint <> '' THEN trusted_at ELSE NULL END,
    revoked_at,
    last_seen_at,
    last_transfer_at,
    1,
    1
FROM local_link_devices_v07;

CREATE TABLE local_link_transfers (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES local_link_devices(device_id) ON DELETE RESTRICT,
    direction TEXT NOT NULL CHECK(direction IN ('outgoing', 'incoming')),
    status TEXT NOT NULL CHECK(status IN (
        'connecting', 'encrypted', 'awaitingReceiver', 'copied', 'saved',
        'rejected', 'cancelled', 'expired', 'failed'
    )),
    item_kind TEXT NOT NULL CHECK(item_kind = 'text'),
    byte_size INTEGER NOT NULL DEFAULT 0 CHECK(byte_size BETWEEN 0 AND 262144),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    expires_at TEXT,
    failure_reason TEXT CHECK(failure_reason IN (
        'transportUnavailable', 'deviceUnavailable', 'authFailed', 'versionMismatch',
        'receiverLocked', 'receiverBusy', 'rateLimited', 'duplicate',
        'protocolViolation', 'captureBlocked', 'itemUnavailable', 'unsupportedItem',
        'deviceNotTrusted', 'deviceRevoked', 'payloadUnavailable'
    )),
    receiver_action TEXT CHECK(receiver_action IN ('copy', 'save', 'reject'))
);

INSERT INTO local_link_transfers
    (id, device_id, direction, status, item_kind, byte_size, created_at,
     updated_at, completed_at, expires_at, failure_reason, receiver_action)
SELECT
    id,
    device_id,
    direction,
    CASE
        WHEN status = 'accepted' AND receiver_action = 'copy' THEN 'copied'
        WHEN status = 'accepted' AND receiver_action = 'save' THEN 'saved'
        WHEN status = 'rejected' THEN 'rejected'
        ELSE 'failed'
    END,
    'text',
    0,
    created_at,
    updated_at,
    CASE WHEN status IN ('accepted', 'rejected', 'failed', 'sent', 'pendingAcceptance')
        THEN COALESCE(completed_at, updated_at) ELSE completed_at END,
    NULL,
    CASE
        WHEN status IN ('pendingAcceptance', 'sent') THEN 'payloadUnavailable'
        WHEN status = 'failed' THEN COALESCE(failure_reason, 'transportUnavailable')
        WHEN status = 'accepted' AND receiver_action IS NULL THEN 'payloadUnavailable'
        ELSE NULL
    END,
    CASE WHEN status = 'rejected' THEN 'reject' ELSE receiver_action END
FROM local_link_transfers_v07;

CREATE TABLE local_link_replay_tombstones (
    peer_device_id TEXT NOT NULL REFERENCES local_link_devices(device_id) ON DELETE CASCADE,
    transfer_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    PRIMARY KEY(peer_device_id, transfer_id),
    UNIQUE(peer_device_id, session_id, nonce)
);

CREATE INDEX idx_local_link_devices_status
    ON local_link_devices(trust_status, paired_at DESC);
CREATE INDEX idx_local_link_transfers_recent
    ON local_link_transfers(updated_at DESC, id DESC);
CREATE INDEX idx_local_link_transfers_device
    ON local_link_transfers(device_id, updated_at DESC);
CREATE INDEX idx_local_link_replay_expiry
    ON local_link_replay_tombstones(expires_at);

DROP TABLE local_link_transfers_v07;
DROP TABLE local_link_devices_v07;

PRAGMA user_version = 15;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// v1.1 adds a content-free reason for a rejected size boundary. New installs
// receive the 30-day retention default from migration 2; existing settings
// are deliberately preserved. SQLite cannot alter a CHECK constraint in
// place, so the bounded status table is rebuilt without adding a column or
// any clipboard-content storage.
pub const MIGRATION_16: &str = r#"
BEGIN;

DROP INDEX IF EXISTS idx_uncaptured_clipboard_events_recent;
DROP INDEX IF EXISTS idx_uncaptured_clipboard_events_expiry;
ALTER TABLE uncaptured_clipboard_events RENAME TO uncaptured_clipboard_events_v11;

CREATE TABLE uncaptured_clipboard_events (
    id TEXT PRIMARY KEY NOT NULL,
    reason_type TEXT NOT NULL CHECK(reason_type IN (
        'manualPause',
        'sensitiveContentDefault',
        'sensitiveContentStrict',
        'excludedApplication',
        'unsupportedFormat',
        'contentTooLarge'
    )),
    source_app TEXT,
    occurred_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

INSERT INTO uncaptured_clipboard_events
    (id, reason_type, source_app, occurred_at, expires_at)
SELECT id, reason_type, source_app, occurred_at, expires_at
FROM uncaptured_clipboard_events_v11;

DROP TABLE uncaptured_clipboard_events_v11;

CREATE INDEX idx_uncaptured_clipboard_events_recent
    ON uncaptured_clipboard_events(occurred_at DESC);
CREATE INDEX idx_uncaptured_clipboard_events_expiry
    ON uncaptured_clipboard_events(expires_at);

PRAGMA user_version = 16;
COMMIT;
"#;

// The Local Link trust lifetime is metadata only: 30 days stays the default
// for upgraded and new pairings, while permanent trust is an explicit local
// choice. Session-only grants retain their peer key only in runtime memory and
// use the existing `needsRePairing` row for display and transfer metadata.
pub const MIGRATION_17: &str = r#"
BEGIN;

ALTER TABLE local_link_devices
    ADD COLUMN trust_duration TEXT NOT NULL DEFAULT 'thirtyDays'
    CHECK(trust_duration IN ('thirtyDays', 'always'));

PRAGMA user_version = 17;
COMMIT;
"#;

// v1.4 records whether the receiving person opened a pending Local Link
// request. The state is deliberately metadata-only: this rebuild adds no body,
// preview, digest, nonce, endpoint, or device-name column.
pub const MIGRATION_18: &str = r#"
BEGIN;
PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS idx_local_link_transfers_recent;
DROP INDEX IF EXISTS idx_local_link_transfers_device;

ALTER TABLE local_link_transfers RENAME TO local_link_transfers_v14;

CREATE TABLE local_link_transfers (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES local_link_devices(device_id) ON DELETE RESTRICT,
    direction TEXT NOT NULL CHECK(direction IN ('outgoing', 'incoming')),
    status TEXT NOT NULL CHECK(status IN (
        'connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'copied', 'saved',
        'rejected', 'cancelled', 'expired', 'failed'
    )),
    item_kind TEXT NOT NULL CHECK(item_kind = 'text'),
    byte_size INTEGER NOT NULL DEFAULT 0 CHECK(byte_size BETWEEN 0 AND 262144),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    expires_at TEXT,
    failure_reason TEXT CHECK(failure_reason IN (
        'transportUnavailable', 'deviceUnavailable', 'authFailed', 'versionMismatch',
        'receiverLocked', 'receiverBusy', 'rateLimited', 'duplicate',
        'protocolViolation', 'captureBlocked', 'itemUnavailable', 'unsupportedItem',
        'deviceNotTrusted', 'deviceRevoked', 'payloadUnavailable'
    )),
    receiver_action TEXT CHECK(receiver_action IN ('copy', 'save', 'reject'))
);

INSERT INTO local_link_transfers
    (id, device_id, direction, status, item_kind, byte_size, created_at,
     updated_at, completed_at, expires_at, failure_reason, receiver_action)
SELECT id, device_id, direction, status, item_kind, byte_size, created_at,
       updated_at, completed_at, expires_at, failure_reason, receiver_action
FROM local_link_transfers_v14;

CREATE INDEX idx_local_link_transfers_recent
    ON local_link_transfers(updated_at DESC, id DESC);
CREATE INDEX idx_local_link_transfers_device
    ON local_link_transfers(device_id, updated_at DESC);

DROP TABLE local_link_transfers_v14;

PRAGMA user_version = 18;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// v1.5 adds bounded, user-authored labels for local organization. A tag is
// attached to the existing clip row and follows that row through recycle-bin
// restore; permanent deletion cascades to the tag rows. Tags are never copied
// into Local Link transfer or diagnostic tables.
pub const MIGRATION_19: &str = r#"
BEGIN;

CREATE TABLE clipboard_item_tags (
    clipboard_item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    label TEXT NOT NULL COLLATE NOCASE,
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 7),
    created_at TEXT NOT NULL,
    PRIMARY KEY (clipboard_item_id, label),
    CHECK(length(label) BETWEEN 1 AND 24)
);

CREATE INDEX idx_clipboard_item_tags_label
    ON clipboard_item_tags(label COLLATE NOCASE, clipboard_item_id);

PRAGMA user_version = 19;
COMMIT;
"#;

// v1.5.1 makes Local Link's post-payload uncertainty explicit and adds a
// content-free claim used to arbitrate a receiver Copy side effect. The claim
// stores only a caller-generated random operation marker and pasteboard
// generation metadata; it deliberately has no body, preview, digest, peer,
// endpoint, clipboard-item reference, or other replay material.
pub const MIGRATION_20: &str = r#"
BEGIN;
PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS idx_local_link_transfers_recent;
DROP INDEX IF EXISTS idx_local_link_transfers_device;

ALTER TABLE local_link_transfers RENAME TO local_link_transfers_v15;

CREATE TABLE local_link_transfers (
    id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL REFERENCES local_link_devices(device_id) ON DELETE RESTRICT,
    direction TEXT NOT NULL CHECK(direction IN ('outgoing', 'incoming')),
    status TEXT NOT NULL CHECK(status IN (
        'connecting', 'encrypted', 'awaitingReceiver', 'viewed', 'reconciling',
        'copied', 'saved', 'rejected', 'cancelled', 'expired', 'failed'
    )),
    item_kind TEXT NOT NULL CHECK(item_kind = 'text'),
    byte_size INTEGER NOT NULL DEFAULT 0 CHECK(byte_size BETWEEN 0 AND 262144),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    expires_at TEXT,
    failure_reason TEXT CHECK(failure_reason IN (
        'transportUnavailable', 'deviceUnavailable', 'authFailed', 'versionMismatch',
        'receiverLocked', 'receiverBusy', 'rateLimited', 'duplicate',
        'protocolViolation', 'captureBlocked', 'itemUnavailable', 'unsupportedItem',
        'deviceNotTrusted', 'deviceRevoked', 'payloadUnavailable', 'outcomeUnknown'
    )),
    receiver_action TEXT CHECK(receiver_action IN ('copy', 'save', 'reject'))
);

INSERT INTO local_link_transfers
    (id, device_id, direction, status, item_kind, byte_size, created_at,
     updated_at, completed_at, expires_at, failure_reason, receiver_action)
SELECT id, device_id, direction, status, item_kind, byte_size, created_at,
       updated_at, completed_at, expires_at, failure_reason, receiver_action
FROM local_link_transfers_v15;

CREATE INDEX idx_local_link_transfers_recent
    ON local_link_transfers(updated_at DESC, id DESC);
CREATE INDEX idx_local_link_transfers_device
    ON local_link_transfers(device_id, updated_at DESC);

DROP TABLE local_link_transfers_v15;

CREATE TABLE local_link_effect_claims (
    transfer_id TEXT PRIMARY KEY NOT NULL
        REFERENCES local_link_transfers(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK(action = 'copy'),
    state TEXT NOT NULL CHECK(state IN ('prepared', 'completed', 'uncertain')),
    effect_token TEXT NOT NULL UNIQUE CHECK(length(effect_token) BETWEEN 16 AND 128),
    pasteboard_change_count INTEGER CHECK(pasteboard_change_count >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_local_link_effect_claims_recovery
    ON local_link_effect_claims(state, updated_at ASC);

PRAGMA user_version = 20;
COMMIT;
PRAGMA foreign_keys = ON;
"#;

// v2.0 stores user-requested, on-device image text separately from the
// clipboard Item body. The child row follows the Item lifecycle and the FTS
// index is maintained only while an extraction exists.
pub const MIGRATION_21: &str = r#"
BEGIN;

CREATE TABLE clipboard_image_text_extractions (
    clipboard_item_id TEXT PRIMARY KEY NOT NULL
        REFERENCES clipboard_items(id) ON DELETE CASCADE,
    text TEXT NOT NULL CHECK(length(CAST(text AS BLOB)) BETWEEN 1 AND 262144),
    extracted_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE clipboard_image_text_extractions_fts USING fts5(
    text,
    content='clipboard_image_text_extractions',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER clipboard_image_text_extractions_ai AFTER INSERT
ON clipboard_image_text_extractions BEGIN
    INSERT INTO clipboard_image_text_extractions_fts(rowid, text)
    VALUES (new.rowid, new.text);
END;

CREATE TRIGGER clipboard_image_text_extractions_ad AFTER DELETE
ON clipboard_image_text_extractions BEGIN
    INSERT INTO clipboard_image_text_extractions_fts(
        clipboard_image_text_extractions_fts, rowid, text
    ) VALUES ('delete', old.rowid, old.text);
END;

CREATE TRIGGER clipboard_image_text_extractions_au AFTER UPDATE
ON clipboard_image_text_extractions BEGIN
    INSERT INTO clipboard_image_text_extractions_fts(
        clipboard_image_text_extractions_fts, rowid, text
    ) VALUES ('delete', old.rowid, old.text);
    INSERT INTO clipboard_image_text_extractions_fts(rowid, text)
    VALUES (new.rowid, new.text);
END;

PRAGMA user_version = 21;
COMMIT;
"#;

// The 2.0 alpha keeps Smart Collections as bounded saved filter definitions,
// separate from manual tag-backed memberships. Notes are Item-owned and use a
// dedicated FTS index so their plaintext never has to be added to list DTOs.
pub const MIGRATION_22: &str = r#"
BEGIN;

CREATE TABLE clipboard_smart_collections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    rule_schema_version INTEGER NOT NULL CHECK(rule_schema_version = 1),
    rule_json TEXT NOT NULL CHECK(length(CAST(rule_json AS BLOB)) BETWEEN 2 AND 4096),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK(length(name) BETWEEN 1 AND 24)
);

CREATE TABLE clipboard_item_notes (
    clipboard_item_id TEXT PRIMARY KEY NOT NULL
        REFERENCES clipboard_items(id) ON DELETE CASCADE,
    text TEXT NOT NULL CHECK(length(CAST(text AS BLOB)) BETWEEN 1 AND 16384),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE VIRTUAL TABLE clipboard_item_notes_fts USING fts5(
    text,
    content='clipboard_item_notes',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER clipboard_item_notes_ai AFTER INSERT ON clipboard_item_notes BEGIN
    INSERT INTO clipboard_item_notes_fts(rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TRIGGER clipboard_item_notes_ad AFTER DELETE ON clipboard_item_notes BEGIN
    INSERT INTO clipboard_item_notes_fts(clipboard_item_notes_fts, rowid, text)
    VALUES ('delete', old.rowid, old.text);
END;

CREATE TRIGGER clipboard_item_notes_au AFTER UPDATE ON clipboard_item_notes BEGIN
    INSERT INTO clipboard_item_notes_fts(clipboard_item_notes_fts, rowid, text)
    VALUES ('delete', old.rowid, old.text);
    INSERT INTO clipboard_item_notes_fts(rowid, text) VALUES (new.rowid, new.text);
END;

PRAGMA user_version = 22;
COMMIT;
"#;

// v2.0 alpha upgrades single-step Local Actions to bounded Filter Composer
// pipelines while preserving every built-in and user-created action.
pub const MIGRATION_23: &str = r#"
BEGIN;

ALTER TABLE local_text_actions RENAME TO local_text_actions_single_step;

CREATE TABLE local_text_actions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    transform TEXT NOT NULL CHECK(transform IN (
        'uppercase', 'lowercase', 'trim_whitespace', 'normalize_whitespace', 'format_json',
        'remove_blank_lines', 'deduplicate_lines', 'sort_lines_asc', 'sort_lines_desc'
    )),
    pipeline_json TEXT
        CHECK(pipeline_json IS NULL OR length(CAST(pipeline_json AS BLOB)) BETWEEN 2 AND 2048),
    is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO local_text_actions
    (id, name, transform, pipeline_json, is_builtin, created_at, updated_at)
SELECT id, name, transform, NULL, is_builtin, created_at, updated_at
FROM local_text_actions_single_step;

DROP TABLE local_text_actions_single_step;

PRAGMA user_version = 23;
COMMIT;
"#;

// Local automation is a separate default-off integration. Capabilities are
// persisted individually so enabling the socket never grants content access
// or a write capability implicitly.
pub const MIGRATION_24: &str = r#"
BEGIN;

CREATE TABLE automation_preferences (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK(singleton = 1),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0, 1)),
    history_metadata INTEGER NOT NULL DEFAULT 0 CHECK(history_metadata IN (0, 1)),
    history_content INTEGER NOT NULL DEFAULT 0 CHECK(history_content IN (0, 1)),
    clipboard_write INTEGER NOT NULL DEFAULT 0 CHECK(clipboard_write IN (0, 1)),
    library_write INTEGER NOT NULL DEFAULT 0 CHECK(library_write IN (0, 1)),
    filters_run INTEGER NOT NULL DEFAULT 0 CHECK(filters_run IN (0, 1)),
    updated_at TEXT NOT NULL
);

INSERT INTO automation_preferences
    (singleton, enabled, history_metadata, history_content, clipboard_write,
     library_write, filters_run, updated_at)
VALUES (1, 0, 0, 0, 0, 0, 0, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

PRAGMA user_version = 24;
COMMIT;
"#;

// Stack uses a distinct configurable global shortcut. Keeping the preference
// beside Quick Paste makes conflict validation and rollback one transaction.
pub const MIGRATION_25: &str = r#"
BEGIN;

ALTER TABLE capture_preferences
    ADD COLUMN stack_shortcut TEXT NOT NULL DEFAULT 'CommandOrControl+Alt+S';

PRAGMA user_version = 25;
COMMIT;
"#;

// Custom Filters may claim one of nine contextual shortcut slots. The slot is
// never global and built-ins keep no assignment.
pub const MIGRATION_26: &str = r#"
BEGIN;

ALTER TABLE local_text_actions
    ADD COLUMN shortcut_slot INTEGER CHECK(shortcut_slot BETWEEN 1 AND 9);

CREATE UNIQUE INDEX idx_local_text_actions_shortcut_slot
    ON local_text_actions(shortcut_slot)
    WHERE shortcut_slot IS NOT NULL;

PRAGMA user_version = 26;
COMMIT;
"#;
