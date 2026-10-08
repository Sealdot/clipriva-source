export type ClipboardItemKind =
  | "text"
  | "code"
  | "command"
  | "url"
  | "color"
  | "image"
  | "richText"
  | "file";

export type ClipboardRepresentationKind = "image" | "richText" | "file";

/**
 * Metadata for a non-text representation stored locally by the native runtime.
 * `storageKey` is content-addressed and relative to ClipRiva's private blob
 * directory; it is not a user filesystem path or a URL exposed to the webview.
 */
export interface ClipboardRepresentation {
  storageKey: string;
  contentHash: string;
  kind: ClipboardRepresentationKind;
  mimeType: string;
  displayName: string | null;
  imageDimensions?: {
    width: number;
    height: number;
  } | null;
  byteSize: number;
}

export interface ClipboardOccurrence {
  id: string;
  sourceApp: string | null;
  deviceId: string;
  occurredAt: string;
}

export interface ClipboardItem {
  id: string;
  /** Stable record identifier reserved for a future sync protocol. */
  globalId?: string;
  /** Locally generated origin device identifier; never used as an account. */
  deviceId?: string;
  /** Opaque repository-issued ID for an exact local duplicate group. */
  groupId?: string | null;
  /** Monotonic sequence within an exact duplicate group. */
  version?: number;
  content: string;
  kind: ClipboardItemKind;
  sourceApp: string | null;
  createdAt: string;
  updatedAt: string;
  /** Original retention deadline; recycle-bin expiry can never exceed it. */
  retentionUntil?: string | null;
  isPinned: boolean;
  copyCount: number;
  restoredAt?: string | null;
  lastUsedAt?: string | null;
  representations?: ClipboardRepresentation[];
  /** Bounded user-authored labels stored only with this local History Item. */
  tags?: string[];
  /**
   * Number of times this exact item entered local history. Optional while
   * upgrading from the legacy group/version model.
   */
  occurrenceCount?: number;
  /** Newest-first local capture history for this item. */
  occurrences?: ClipboardOccurrence[];
  /** Set only when the current History search matched manually extracted image text. */
  textInImageMatch?: boolean;
  /** Set only when the current local search matched an Item-owned Note. */
  noteMatch?: boolean;
}

export interface ClipboardItemNote {
  clipboardItemId: string;
  text: string;
  createdAt: string;
  updatedAt: string;
}

export type ClipboardItemNoteMutationStatus = "saved" | "deleted" | "sensitiveBlocked";

export interface ClipboardItemNoteMutationResult {
  status: ClipboardItemNoteMutationStatus;
  note: ClipboardItemNote | null;
}

export interface ClipboardImageTextExtraction {
  clipboardItemId: string;
  text: string;
  extractedAt: string;
  updatedAt: string;
}

export type ClipboardImageTextExtractionStatus =
  | "extracted"
  | "noText"
  | "sensitiveBlocked"
  | "cancelled";

export interface ClipboardImageTextExtractionResult {
  status: ClipboardImageTextExtractionStatus;
  extraction: ClipboardImageTextExtraction | null;
}

export type ClipboardCleanupScope = "selected" | "allUnpinned" | "retentionPolicy";

/**
 * The same explicit request powers a preview and its confirmed execution.
 * `includePinned` must only be set after a dedicated confirmation for any
 * manually selected pinned clips.
 */
export interface ClipboardCleanupRequest {
  scope: ClipboardCleanupScope;
  itemIds?: string[];
  includePinned?: boolean;
}

export interface ClipboardCleanupRepresentative {
  id: string;
  kind: ClipboardItemKind;
  sourceApp: string | null;
  capturedAt: string;
  isPinned: boolean;
}

export interface RecycleBinRetention {
  maximumDays: number;
  earliestExpiresAt: string | null;
  latestExpiresAt: string | null;
}

export interface ClipboardCleanupPreview {
  scope: ClipboardCleanupScope;
  affectedCount: number;
  pinnedSkippedCount: number;
  ruleConditions: string[];
  representativeItems: ClipboardCleanupRepresentative[];
  recycleBinRetention: RecycleBinRetention;
}

export interface ClipboardCleanupResult {
  preview: ClipboardCleanupPreview;
  movedToRecycleBinCount: number;
}

export interface RecycleBinItem {
  item: ClipboardItem;
  deletedAt: string;
  recycleExpiresAt: string;
}

export interface ClipboardListOptions {
  query: string;
  pinnedOnly: boolean;
  limit?: number;
  offset?: number;
  filters?: ClipboardListFilters;
}

export type ClipboardPinFilter = "all" | "pinned" | "unpinned";
export type ClipboardTimeFilter = "all" | "past24Hours" | "past7Days" | "past30Days";

/** Structured filters are applied by the native repository before paging. */
export interface ClipboardListFilters {
  kinds?: ClipboardItemKind[];
  sourceApp?: string | null;
  /** Collections reuse the existing bounded, local Item tag storage. */
  collection?: string | null;
  smartCollectionId?: string | null;
  pinFilter?: ClipboardPinFilter;
  timeFilter?: ClipboardTimeFilter;
  recentlyUsedOnly?: boolean;
  localLinkOnly?: boolean;
}

export type ClipboardCollectionKind = "manual" | "smart";

export interface ClipboardSmartCollectionRule {
  kinds?: ClipboardItemKind[];
  sourceApp?: string | null;
  pinFilter?: ClipboardPinFilter;
  timeFilter?: ClipboardTimeFilter;
  recentlyUsedOnly?: boolean;
  localLinkOnly?: boolean;
}

export interface ClipboardSmartCollectionInput {
  name: string;
  rule: ClipboardSmartCollectionRule;
}

export interface ClipboardCollection {
  id: string | null;
  name: string;
  kind: ClipboardCollectionKind;
  rule: ClipboardSmartCollectionRule | null;
  itemCount: number;
  savedItemCount: number;
}

export interface ClipboardFilterOptions {
  sourceApps: string[];
  collections: ClipboardCollection[];
  totalCount: number;
  unpinnedCount: number;
}

export interface ClipboardCollectionMutationResult {
  affectedItemCount: number;
}

/** Evidence behind Quick Paste's local, deterministic result order. */
export type QuickPasteMatchKind = "suggestion" | "exact" | "prefix" | "word" | "contains";
export type QuickPasteMatchField =
  | "content"
  | "displayName"
  | "sourceApp"
  | "tag"
  | "note"
  | "imageText";

export interface TextHighlightRange {
  start: number;
  end: number;
}

export interface QuickPasteSearchResult {
  item: ClipboardItem;
  matchKind: QuickPasteMatchKind;
  /** Optional during migration from the pre-0.5 search command. */
  matchField?: QuickPasteMatchField;
  /** UTF-16 ranges into the field identified by `matchField`. */
  highlightRanges?: TextHighlightRange[];
}

export type WorkspaceSearchMode = "fts" | "localSemantic";
export type PasteBehavior = "restore" | "autoPaste";
export type ClipboardActivationMode = "directPaste" | "plainTextPaste" | "copy";
export type PastePermissionStatus = "granted" | "notGranted";
/**
 * How ClipRiva responds when its local high-confidence detector sees a secret.
 * The desktop always returns this field; it is optional here so an older
 * desktop build can still be opened safely by a newer webview during rollout.
 */
export type SensitiveContentPolicy = "default" | "strict";
/** A truthful result state for a clipboard recovery attempt. */
export type PasteOutcome = "clipboardRestored" | "pasteSent" | "pasteNotSent";
/** Native recovery stops before input dispatch on these bounded failures. */
export type ClipboardUseOutcome =
  | PasteOutcome
  | "invalidItem"
  | "writeFailed"
  | "historyRecordFailed"
  | "busy";

/**
 * Direct Paste failure reasons are intentionally broad. They never contain a
 * platform error, clipboard value, preview, or other sensitive fragment.
 */
export type PasteFailureReason = "permission" | "focus" | "compatibility";

/**
 * Evidence returned by ClipRiva's offline lexical fallback. This is not a
 * vector score and no embedding provider is contacted for it.
 */
export interface LocalSemanticSearchResult {
  item: ClipboardItem;
  score: number;
  matchedTerms: string[];
  highlightRanges: Array<{
    start: number;
    end: number;
  }>;
}

export interface CapturePreferences {
  capturePaused: boolean;
  /** @deprecated Use sensitiveContentPolicy when it is available. */
  sensitivePauseEnabled: boolean;
  sensitiveContentPolicy?: SensitiveContentPolicy;
  retentionDays: number;
  maxHistoryItems: number;
  deniedApps: string[];
  pauseReason: string | null;
  labsEnabled: boolean;
  /** Optional local-only reliability measurement; defaults to off. */
  diagnosticsEnabled?: boolean;
  pasteBehavior: PasteBehavior;
  quickPasteShortcut: string;
  stackShortcut: string;
  onboardingCompleted: boolean;
}

/**
 * A short-lived, local explanation for a clipboard change ClipRiva did not
 * retain. It intentionally has no content, preview, representation or hash.
 */
export type CaptureStatusReason =
  | "manualPause"
  | "sensitiveContentDefault"
  | "sensitiveContentStrict"
  | "excludedApplication"
  | "unsupportedFormat"
  | "contentTooLarge";

export interface CaptureStatusEvent {
  id: string;
  reasonType: CaptureStatusReason;
  sourceApp: string | null;
  occurredAt: string;
  expiresAt: string;
}

/**
 * Opt-in local Diagnostics accepts only these finite event/result values.
 * There is intentionally no text, clip ID, source app, path, filename,
 * document name, timestamp, fingerprint, account or network field.
 */
export type LocalDiagnosticEventType =
  | "firstRecovery"
  | "directPaste"
  | "recycleBinRestore"
  | "duplicateGroupExpand"
  | "clipboardRestoreError"
  | "capturePermissionRestricted"
  | "systemPolicyRestricted";

export type LocalDiagnosticOutcome =
  | "started"
  | "completed"
  | "degraded"
  | "failed"
  | "restricted"
  | "restored"
  | "expanded";

export interface LocalDiagnosticEventInput {
  eventType: LocalDiagnosticEventType;
  outcome: LocalDiagnosticOutcome;
}

export interface LocalDiagnosticRecordResult {
  recorded: boolean;
}

export interface LocalDiagnosticRate {
  attempts: number;
  completedOrDegraded: number;
  percent: number | null;
}

export interface LocalDiagnosticErrorCategory {
  eventType: LocalDiagnosticEventType;
  outcome: LocalDiagnosticOutcome;
  count: number;
}

export interface LocalDiagnosticsSummary {
  enabled: boolean;
  storedEventCount: number;
  firstRecovery: LocalDiagnosticRate;
  directPaste: LocalDiagnosticRate;
  recycleBinRestoreCount: number;
  duplicateGroupExpandCount: number;
  criticalErrorCategories: LocalDiagnosticErrorCategory[];
}

export interface LocalDiagnosticsExportEvent {
  eventType: LocalDiagnosticEventType;
  outcome: LocalDiagnosticOutcome;
  occurredDay: string;
  appVersion: string;
}

export interface LocalDiagnosticsExport {
  schemaVersion: number;
  exportedAtDay: string;
  appVersion: string;
  summary: LocalDiagnosticsSummary;
  events: LocalDiagnosticsExportEvent[];
}

export interface ClipboardUseResult {
  outcome: ClipboardUseOutcome;
  failureReason: PasteFailureReason | null;
}

export interface ContextEnrichment {
  metadata: {
    schemaVersion: number;
    enricherId: string;
    enricherVersion: string;
  };
  classification: {
    kind: "text" | "url" | "code" | "command" | "color" | "json" | "image" | "richText" | "file";
    matchedRules: string[];
  };
  summary: {
    text: string;
    isTruncated: boolean;
  };
  entities: Array<{
    kind: string;
    preview: string;
    isRedacted: boolean;
  }>;
  tags: Array<{
    id: string;
    label: string;
  }>;
  redaction: {
    redactedContent: string;
    requiresExplicitConsent: boolean;
  };
}

export type LocalTextTransform =
  | "uppercase"
  | "lowercase"
  | "trim_whitespace"
  | "normalize_whitespace"
  | "format_json"
  | "remove_blank_lines"
  | "deduplicate_lines"
  | "sort_lines_asc"
  | "sort_lines_desc";

export interface LocalTextPipelineStep {
  transform: LocalTextTransform;
}

export interface LocalTextAction {
  id: string;
  name: string;
  transform: LocalTextTransform;
  steps: LocalTextPipelineStep[];
  shortcutSlot: number | null;
  createdAt: string;
  updatedAt: string;
  isBuiltin: boolean;
}

export interface CreateLocalTextAction {
  name: string;
  transform: LocalTextTransform;
  steps?: LocalTextPipelineStep[];
  shortcutSlot?: number | null;
}

export interface AutomationPreferences {
  enabled: boolean;
  historyMetadata: boolean;
  historyContent: boolean;
  clipboardWrite: boolean;
  libraryWrite: boolean;
  filtersRun: boolean;
}

export interface ActionAuditRecord {
  id: string;
  schemaVersion: number;
  actionId: string | null;
  actionName: string;
  transform: LocalTextTransform | null;
  executionMode: "local" | "cloud_preview";
  status: "completed" | "preview_only_blocked";
  clipboardItemId: string;
  inputContentHash: string;
  inputPreview: string;
  outputContentHash: string | null;
  outputPreview: string | null;
  createdAt: string;
}

export interface TextActionExecution {
  action: LocalTextAction;
  output: string;
  audit: ActionAuditRecord;
}

/** Ephemeral local preview; confirmation rechecks Item and rule snapshots. */
export interface LocalTextActionPreview {
  action: LocalTextAction;
  clipboardItemId: string;
  itemVersion: number;
  input: string;
  output: string;
}

export interface CloudActionPreview {
  delivery: "preview_only";
  isExecutable: false;
  message: string;
  redaction: {
    redactedContent: string;
    requiresExplicitConsent: boolean;
    matches: Array<{
      kind: string;
      replacement: string;
    }>;
  };
  audit: ActionAuditRecord;
}

/**
 * Local Link sends one deliberately selected local clip to a paired device.
 * It is not a history-sync protocol and these values never describe an
 * account, cloud service, or remote delivery queue.
 */
/** v0.7 freezes Local Link to a single selected text clip. */
export type LocalLinkContentKind = "text";
export type LocalLinkDeviceTrust = "pairing" | "trusted" | "needsRePairing" | "revoked";
/** The user-selected lifetime for a verified Local Link peer key. */
export type LocalLinkTrustDuration = "thisSession" | "thirtyDays" | "always";
export type LocalLinkDeviceAvailability = "online" | "offline" | "unknown";
export type LocalLinkTransferDirection = "incoming" | "outgoing";
export type LocalLinkTransferStatus =
  | "connecting"
  | "encrypted"
  | "awaitingReceiver"
  /** The receiver opened Incoming Center; this has no body or preview. */
  | "viewed"
  /** Receipt recovery is querying metadata only; plaintext is never retried. */
  | "reconciling"
  | "copied"
  | "saved"
  | "rejected"
  | "cancelled"
  | "expired"
  | "failed";
export type LocalLinkReceiveAction = "copy" | "save" | "reject";
export type LocalLinkAcceptAction = Exclude<LocalLinkReceiveAction, "reject">;
export type LocalLinkTransferFilter = "all" | "incoming" | "outgoing" | "failed";

/**
 * Local Link is intentionally opt-in at both discovery and receipt. The
 * defaults ensure a new desktop neither broadcasts itself nor overwrites the
 * local clipboard from a trusted device without a visible user action.
 */
export interface LocalLinkPreferences {
  /** Enables the Local Link feature surface. It defaults to false. */
  enabled: boolean;
  /** Enables LAN discovery. It defaults to false and is never account-based. */
  discoveryEnabled: boolean;
  deviceName: string;
  /** Populated only when the native v0.8 identity contract is available. */
  identityFingerprint?: string | null;
  protocolVersion?: string | null;
}

export interface LocalLinkDevice {
  deviceId: string;
  displayName: string;
  trustStatus: LocalLinkDeviceTrust;
  pairedAt: string | null;
  trustedAt?: string | null;
  /** Defaults to thirty days when an older native runtime omits this metadata. */
  trustDuration?: LocalLinkTrustDuration | null;
  /** Derived native metadata; a null value means the device is not currently trusted. */
  trustExpiresAt?: string | null;
  /** Safe reason for the required pairing ceremony, never a key or endpoint. */
  rePairingReason?: "trustExpired" | "identityInvalidated" | null;
  revokedAt?: string | null;
  lastTransferAt?: string | null;
  publicKeyFingerprint?: string | null;
  lastSeenAt?: string | null;
  /** Native runtime reachability. It is runtime-only and is not an endpoint. */
  online?: boolean;
  /** Browser-preview compatibility until every fixture uses the native DTO. */
  availability?: LocalLinkDeviceAvailability;
  protocolMin?: number | null;
  protocolMax?: number | null;
  protocolVersion?: string | null;
}

/** A short-lived, native-owned pairing request with a shared safety code (SAS). */
export interface LocalLinkPairing {
  device: LocalLinkDevice;
  verificationCode: string;
  expiresAt: string | null;
  localFingerprint?: string | null;
  peerFingerprint?: string | null;
  localConfirmed?: boolean;
  peerConfirmed?: boolean;
}

/**
 * Transfer records deliberately expose only UI-safe metadata. Pending text is
 * never part of this IPC/query DTO; reveal requests open a native secure view
 * without returning plaintext to the WebView.
 */
export interface LocalLinkTransfer {
  id: string;
  direction: LocalLinkTransferDirection;
  deviceId: string;
  peerDisplayName?: string;
  clipboardItemId?: string | null;
  receivedClipboardItemId?: string | null;
  itemKind?: ClipboardItemKind | null;
  status: LocalLinkTransferStatus;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
  retryCount?: number;
  /** The completed receiver decision, never a payload or preview. */
  receiverAction?: LocalLinkReceiveAction | null;
  failureReason?:
    | "transportUnavailable"
    | "deviceUnavailable"
    | "authFailed"
    | "versionMismatch"
    | "receiverLocked"
    | "receiverBusy"
    | "rateLimited"
    | "duplicate"
    | "protocolViolation"
    | "captureBlocked"
    | "itemUnavailable"
    | "unsupportedItem"
    | "deviceNotTrusted"
    | "deviceRevoked"
    | "payloadUnavailable"
    | "outcomeUnknown"
    | null;
  /** Deterministic, metadata-only next step for a terminal transfer. */
  recoveryAction?:
    | "startNewTransferFromOriginalItem"
    | "rePairDevice"
    | "updateClipRiva"
    | "reviewLocalCapturePolicy"
    | "selectSupportedItem"
    | "checkTransferHistory"
    | null;
  fileName?: string | null;
  byteSize?: number | null;
  expiresAt?: string | null;
  /** True only for local browser fixtures used to develop the UI. */
  isUiFixture?: boolean;
}

export type NativeLocalLinkReadinessStage =
  | "localNetwork"
  | "enabled"
  | "discovery"
  | "trust"
  | "authenticatedPresence";
export type NativeLocalLinkReadinessStageState = "passed" | "blocked" | "notChecked";

export interface NativeLocalLinkReadinessSnapshot {
  state: "ready" | "actionRequired" | "offline";
  stages: Array<{
    stage: NativeLocalLinkReadinessStage;
    state: NativeLocalLinkReadinessStageState;
  }>;
  firstBlocker:
    | "localNetworkUnavailable"
    | "localLinkDisabled"
    | "discoveryDisabled"
    | "pairingIncomplete"
    | "trustRequiresRePairing"
    | "noTrustedDevice"
    | "noAuthenticatedPresence"
    | null;
  primaryAction:
    | "restartLocalLink"
    | "enableLocalLink"
    | "enableDiscovery"
    | "completePairing"
    | "pairDevice"
    | "rePairDevice"
    | "refreshDevices"
    | null;
  actionDeviceId: string | null;
  readyDeviceCount: number;
  pendingRequestCapacity: number;
}

export interface LocalLinkDiagnostics {
  schemaVersion: number;
  appVersion: string;
  osFamily: "macos" | "other";
  architecture: "arm64" | "x86_64" | "other";
  generatedAt: string;
  localLinkEnabled: boolean;
  discoveryEnabled: boolean;
  readiness: NativeLocalLinkReadinessSnapshot;
  recordCount: number;
  peers: Array<{
    peerRef: string;
    trustStatus: LocalLinkDeviceTrust;
    trustDuration: LocalLinkTrustDuration;
    availability: "online" | "offline";
  }>;
  transfers: Array<{
    peerRef: string;
    direction: LocalLinkTransferDirection;
    status: LocalLinkTransferStatus;
    failureReason: LocalLinkTransfer["failureReason"];
    recoveryAction: LocalLinkTransfer["recoveryAction"];
    createdAt: string;
    updatedAt: string;
    completedAt: string | null;
    durationMs: number | null;
  }>;
}
