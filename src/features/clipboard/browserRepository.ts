import { evaluateFilterPipeline } from "./filterComposer";
import type {
  ActionAuditRecord,
  AutomationPreferences,
  CapturePreferences,
  CaptureStatusEvent,
  ClipboardActivationMode,
  ClipboardCleanupPreview,
  ClipboardCleanupRepresentative,
  ClipboardCleanupRequest,
  ClipboardCleanupResult,
  ClipboardCollection,
  ClipboardCollectionMutationResult,
  ClipboardFilterOptions,
  ClipboardImageTextExtraction,
  ClipboardImageTextExtractionResult,
  ClipboardItem,
  ClipboardItemNote,
  ClipboardItemNoteMutationResult,
  ClipboardListFilters,
  ClipboardListOptions,
  ClipboardSmartCollectionInput,
  ClipboardSmartCollectionRule,
  CloudActionPreview,
  ContextEnrichment,
  CreateLocalTextAction,
  LocalDiagnosticErrorCategory,
  LocalDiagnosticEventInput,
  LocalDiagnosticEventType,
  LocalDiagnosticOutcome,
  LocalDiagnosticRate,
  LocalDiagnosticRecordResult,
  LocalDiagnosticsExport,
  LocalDiagnosticsExportEvent,
  LocalDiagnosticsSummary,
  LocalLinkAcceptAction,
  LocalLinkContentKind,
  LocalLinkDevice,
  LocalLinkPairing,
  LocalLinkPreferences,
  LocalLinkTransfer,
  LocalLinkTrustDuration,
  LocalSemanticSearchResult,
  LocalTextAction,
  LocalTextActionPreview,
  LocalTextTransform,
  PasteFailureReason,
  PastePermissionStatus,
  QuickPasteMatchField,
  QuickPasteMatchKind,
  QuickPasteSearchResult,
  RecycleBinItem,
  TextActionExecution,
} from "./types";

const now = Date.now();
const browserLocalLinkHistorySourcePrefix = "Local Link from ";

let items: ClipboardItem[] = [
  {
    id: "demo-code",
    content: "export function cleanClip(input: string) {\n  return input.trim();\n}",
    kind: "code",
    sourceApp: "Visual Studio Code",
    createdAt: new Date(now - 1000 * 60 * 2).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 2).toISOString(),
    isPinned: true,
    copyCount: 4,
  },
  {
    id: "demo-url",
    content: "https://v2.tauri.app/plugin/clipboard/",
    kind: "url",
    sourceApp: "Arc",
    createdAt: new Date(now - 1000 * 60 * 18).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 18).toISOString(),
    isPinned: false,
    copyCount: 1,
  },
  {
    id: "demo-text",
    content: "ClipRiva keeps searchable clipboard history private and available on this Mac.",
    kind: "text",
    sourceApp: "Notes",
    createdAt: new Date(now - 1000 * 60 * 55).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 55).toISOString(),
    isPinned: true,
    copyCount: 2,
    tags: ["Reference"],
  },
  {
    id: "demo-chinese",
    content: "版本规划与体验升级",
    kind: "text",
    sourceApp: "Notes",
    createdAt: new Date(now - 1000 * 60 * 60 * 9).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 9).toISOString(),
    isPinned: false,
    copyCount: 1,
  },
  {
    id: "demo-color",
    content: "#6EE7B7",
    kind: "color",
    sourceApp: "Figma",
    createdAt: new Date(now - 1000 * 60 * 60 * 3).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 3).toISOString(),
    isPinned: false,
    copyCount: 1,
  },
  {
    id: "demo-command",
    content: "pnpm test -- --runInBand",
    kind: "command",
    sourceApp: "Terminal",
    createdAt: new Date(now - 1000 * 60 * 60 * 4).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 4).toISOString(),
    isPinned: false,
    copyCount: 1,
  },
  {
    id: "demo-image",
    content: "Product capture · release-dashboard.png",
    kind: "image",
    sourceApp: "Figma",
    createdAt: new Date(now - 1000 * 60 * 60 * 5).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 5).toISOString(),
    isPinned: false,
    copyCount: 0,
    representations: [
      {
        storageKey: "sha256/e02f4dc3b6ca9e0a7ad47590bc73c46ae6f796b5ca84b065e6fd812ce0bb7820",
        contentHash: "e02f4dc3b6ca9e0a7ad47590bc73c46ae6f796b5ca84b065e6fd812ce0bb7820",
        kind: "image",
        mimeType: "image/png",
        displayName: "release-dashboard.png",
        imageDimensions: { width: 1440, height: 900 },
        byteSize: 1_684_291,
      },
    ],
  },
  {
    id: "demo-rich-text",
    content: "Launch plan — bold headings, links and checklist preserved locally.",
    kind: "richText",
    sourceApp: "Notion",
    createdAt: new Date(now - 1000 * 60 * 60 * 7).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 7).toISOString(),
    isPinned: false,
    copyCount: 0,
    representations: [
      {
        storageKey: "sha256/31f139de8ffaf5b5daff89d52d3b09fb663f8949778edf45dbd497e6ee385c6b",
        contentHash: "31f139de8ffaf5b5daff89d52d3b09fb663f8949778edf45dbd497e6ee385c6b",
        kind: "richText",
        mimeType: "text/html",
        displayName: null,
        byteSize: 4_938,
      },
    ],
  },
  {
    id: "demo-file",
    content: "File reference · ClipRiva-M1-brief.pdf",
    kind: "file",
    sourceApp: "Finder",
    createdAt: new Date(now - 1000 * 60 * 60 * 9).toISOString(),
    updatedAt: new Date(now - 1000 * 60 * 60 * 9).toISOString(),
    isPinned: false,
    copyCount: 0,
    representations: [
      {
        storageKey: "sha256/b6a3e9e309a6f0d79580d2d7982c19d8121e8779aab1a025e8c63cd6dd3b8c17",
        contentHash: "b6a3e9e309a6f0d79580d2d7982c19d8121e8779aab1a025e8c63cd6dd3b8c17",
        kind: "file",
        mimeType: "application/pdf",
        displayName: "ClipRiva-M1-brief.pdf",
        byteSize: 385_714,
      },
    ],
  },
];

const browserImageTextExtractions = new Map<string, ClipboardImageTextExtraction>();
const browserItemNotes = new Map<string, ClipboardItemNote>();
let browserSmartCollections: ClipboardCollection[] = [];
let browserImageTextJob: { requestId: string; cancelled: boolean } | null = null;

// Browser-only preview data mirrors the 0.5 Item/Occurrence contract. Usage
// (`copyCount`) is deliberately separate, so trying or copying an Item does
// not make the UI claim that the source clipboard event happened again.
items = items.map((item) => {
  const occurrenceCount = Math.max(1, item.copyCount);
  return {
    ...item,
    occurrenceCount,
    occurrences: Array.from({ length: occurrenceCount }, (_, index) => ({
      id: `${item.id}-occurrence-${index + 1}`,
      sourceApp: item.sourceApp,
      deviceId: "browser-preview-device",
      occurredAt: new Date(new Date(item.createdAt).getTime() - index * 60_000).toISOString(),
    })),
  };
});

let capturePreferences: CapturePreferences = {
  capturePaused: false,
  sensitivePauseEnabled: false,
  sensitiveContentPolicy: "default",
  retentionDays: 30,
  maxHistoryItems: 5000,
  deniedApps: [],
  pauseReason: null,
  labsEnabled: false,
  pasteBehavior: "restore",
  quickPasteShortcut: "CommandOrControl+Shift+Space",
  stackShortcut: "CommandOrControl+Alt+S",
  onboardingCompleted: false,
  diagnosticsEnabled: false,
};
let captureStatusEvents: CaptureStatusEvent[] = [];
let automationPreferences: AutomationPreferences = {
  enabled: false,
  historyMetadata: false,
  historyContent: false,
  clipboardWrite: false,
  libraryWrite: false,
  filtersRun: false,
};
let localDiagnosticEvents: LocalDiagnosticsExportEvent[] = [];
let recycleBinItems: RecycleBinItem[] = [];
type BrowserDirectPasteSimulation = "sent" | PasteFailureReason;
let browserDirectPasteSimulation: BrowserDirectPasteSimulation = "focus";
let browserPastePermissionStatus: PastePermissionStatus = "granted";

const browserLocalLinkDefaultPreferences: LocalLinkPreferences = {
  enabled: false,
  discoveryEnabled: false,
  deviceName: "This Mac",
  identityFingerprint: "A3F7 · 91C2",
  protocolVersion: "v2 UI contract",
  // Browser previews follow the production safety default: never discover or
  // receive automatically until the person explicitly enables a control.
};
let localLinkPreferences: LocalLinkPreferences = { ...browserLocalLinkDefaultPreferences };
const browserLocalLinkDevices: LocalLinkDevice[] = [
  {
    deviceId: "browser-nearby-macbook-air",
    displayName: "Meeting MacBook Air",
    trustStatus: "revoked",
    pairedAt: null,
    publicKeyFingerprint: "B72D · 40EF",
    online: true,
    availability: "online",
    lastSeenAt: new Date(now - 8_000).toISOString(),
    protocolVersion: "v2",
    protocolMin: 2,
    protocolMax: 2,
  },
  {
    deviceId: "browser-trusted-mac-mini",
    displayName: "Studio Mac mini",
    trustStatus: "trusted",
    pairedAt: new Date(now - 86_400_000).toISOString(),
    trustedAt: new Date(now - 86_400_000).toISOString(),
    trustDuration: "thirtyDays",
    publicKeyFingerprint: "4CD8 · A177",
    online: true,
    availability: "online",
    lastSeenAt: new Date(now - 12_000).toISOString(),
    protocolVersion: "v2",
    protocolMin: 2,
    protocolMax: 2,
  },
  {
    deviceId: "browser-offline-macbook-pro",
    displayName: "Travel MacBook Pro",
    trustStatus: "trusted",
    pairedAt: new Date(now - 604_800_000).toISOString(),
    trustedAt: new Date(now - 604_800_000).toISOString(),
    trustDuration: "thirtyDays",
    publicKeyFingerprint: "98B2 · 6F0A",
    online: false,
    availability: "offline",
    lastSeenAt: new Date(now - 18 * 60_000).toISOString(),
    protocolVersion: "v2",
    protocolMin: 2,
    protocolMax: 2,
  },
];
let localLinkDevices = browserLocalLinkDevices.map((device) => ({ ...device }));
let localLinkPairing: LocalLinkPairing | null = null;
const browserLocalLinkPayloads = new Map<string, string>();
let localLinkTransfers: LocalLinkTransfer[] = [
  {
    id: "browser-incoming-review-text",
    direction: "incoming",
    deviceId: "browser-trusted-mac-mini",
    itemKind: "text",
    byteSize: 58,
    status: "awaitingReceiver",
    createdAt: new Date(now - 45_000).toISOString(),
    updatedAt: new Date(now - 45_000).toISOString(),
    fileName: null,
    failureReason: null,
    expiresAt: new Date(now + 60_000).toISOString(),
    isUiFixture: true,
  },
  {
    id: "browser-failed-text-transfer",
    direction: "outgoing",
    deviceId: "browser-trusted-mac-mini",
    clipboardItemId: "demo-text",
    itemKind: "text",
    byteSize: 76,
    status: "failed",
    createdAt: new Date(now - 180_000).toISOString(),
    updatedAt: new Date(now - 180_000).toISOString(),
    failureReason: "transportUnavailable",
    isUiFixture: true,
  },
  {
    id: "browser-copied-text-transfer",
    direction: "incoming",
    deviceId: "browser-trusted-mac-mini",
    itemKind: "text",
    byteSize: 42,
    status: "copied",
    receiverAction: "copy",
    createdAt: new Date(now - 75_000).toISOString(),
    updatedAt: new Date(now - 72_000).toISOString(),
    completedAt: new Date(now - 72_000).toISOString(),
    failureReason: null,
    isUiFixture: true,
  },
  {
    id: "browser-rejected-text-transfer",
    direction: "outgoing",
    deviceId: "browser-offline-macbook-pro",
    itemKind: "text",
    byteSize: 31,
    status: "rejected",
    createdAt: new Date(now - 240_000).toISOString(),
    updatedAt: new Date(now - 235_000).toISOString(),
    completedAt: new Date(now - 235_000).toISOString(),
    failureReason: null,
    isUiFixture: true,
  },
];
browserLocalLinkPayloads.set(
  "browser-incoming-review-text",
  "Review the agenda before the meeting.",
);

const createdAt = new Date(now).toISOString();
const builtinActionDefinitions: Array<[string, string, LocalTextTransform]> = [
  ["builtin-uppercase", "Uppercase", "uppercase"],
  ["builtin-lowercase", "Lowercase", "lowercase"],
  ["builtin-trim-whitespace", "Trim whitespace", "trim_whitespace"],
  ["builtin-normalize-whitespace", "Normalize whitespace", "normalize_whitespace"],
  ["builtin-format-json", "Format JSON", "format_json"],
];
let localTextActions: LocalTextAction[] = builtinActionDefinitions.map(([id, name, transform]) => ({
  id,
  name,
  transform,
  steps: [{ transform }],
  shortcutSlot: null,
  isBuiltin: true,
  createdAt,
  updatedAt: createdAt,
}));
let actionAudit: ActionAuditRecord[] = [];

export async function listBrowserItems(options: ClipboardListOptions) {
  const needle = options.query.trim().toLocaleLowerCase();
  return items
    .filter((item) => matchesBrowserListFilters(item, options.filters, options.pinnedOnly))
    .filter(
      (item) =>
        !needle ||
        item.content.toLocaleLowerCase().includes(needle) ||
        item.sourceApp?.toLocaleLowerCase().includes(needle) ||
        item.tags?.some((tag) => tag.toLocaleLowerCase().includes(needle)) ||
        browserImageTextExtractions.get(item.id)?.text.toLocaleLowerCase().includes(needle) ||
        browserItemNotes.get(item.id)?.text.toLocaleLowerCase().includes(needle),
    )
    .map((item) => ({
      ...item,
      textInImageMatch: Boolean(
        needle &&
          browserImageTextExtractions.get(item.id)?.text.toLocaleLowerCase().includes(needle),
      ),
      noteMatch: Boolean(
        needle && browserItemNotes.get(item.id)?.text.toLocaleLowerCase().includes(needle),
      ),
    }))
    .slice(options.offset ?? 0, (options.offset ?? 0) + (options.limit ?? 100));
}

export async function getBrowserItem(id: string): Promise<ClipboardItem | null> {
  return items.find((item) => item.id === id) ?? null;
}

/** Deterministic, synthetic browser preview for the native manual Vision action. */
export async function extractBrowserClipboardImageText(
  id: string,
  requestId: string,
): Promise<ClipboardImageTextExtractionResult> {
  const item = items.find((candidate) => candidate.id === id);
  if (item?.kind !== "image") throw new Error("Clipboard image was not found");
  if (browserImageTextJob)
    throw new Error("Another local image text extraction is already running.");
  browserImageTextJob = { requestId, cancelled: false };
  // Keep the synthetic worker briefly pending so browser previews and tests
  // exercise the same explicit cancellation control as native Vision.
  await new Promise((resolve) => setTimeout(resolve, 25));
  if (browserImageTextJob?.requestId !== requestId || browserImageTextJob.cancelled) {
    if (browserImageTextJob?.requestId === requestId) browserImageTextJob = null;
    return { status: "cancelled", extraction: null };
  }
  const extractedAt = new Date().toISOString();
  const extraction: ClipboardImageTextExtraction = {
    clipboardItemId: id,
    text: "Release dashboard synthetic browser preview",
    extractedAt,
    updatedAt: extractedAt,
  };
  browserImageTextExtractions.set(id, extraction);
  browserImageTextJob = null;
  return { status: "extracted", extraction };
}

export async function cancelBrowserClipboardImageTextExtraction(requestId: string) {
  if (browserImageTextJob?.requestId !== requestId || browserImageTextJob.cancelled) return false;
  browserImageTextJob.cancelled = true;
  return true;
}

export async function getBrowserClipboardImageText(id: string) {
  const item = items.find((candidate) => candidate.id === id);
  if (item?.kind !== "image") throw new Error("Clipboard image was not found");
  return browserImageTextExtractions.get(id) ?? null;
}

export async function deleteBrowserClipboardImageText(id: string) {
  const item = items.find((candidate) => candidate.id === id);
  if (item?.kind !== "image") throw new Error("Clipboard image was not found");
  return browserImageTextExtractions.delete(id);
}

export async function getBrowserClipboardItemNote(id: string) {
  if (!items.some((item) => item.id === id)) throw new Error("Clipboard item not found.");
  return browserItemNotes.get(id) ?? null;
}

export async function saveBrowserClipboardItemNote(
  id: string,
  rawText: string,
): Promise<ClipboardItemNoteMutationResult> {
  if (!items.some((item) => item.id === id)) throw new Error("Clipboard item not found.");
  const text = rawText.trim();
  if (!text) {
    browserItemNotes.delete(id);
    return { status: "deleted", note: null };
  }
  if (new TextEncoder().encode(text).byteLength > 16 * 1024) {
    throw new Error("Notes must be at most 16 KiB of UTF-8 text.");
  }
  if (/BEGIN (?:OPENSSH |RSA )?PRIVATE KEY|AKIA[0-9A-Z]{16}/u.test(text)) {
    return { status: "sensitiveBlocked", note: null };
  }
  const previous = browserItemNotes.get(id);
  const timestamp = new Date().toISOString();
  const note: ClipboardItemNote = {
    clipboardItemId: id,
    text,
    createdAt: previous?.createdAt ?? timestamp,
    updatedAt: timestamp,
  };
  browserItemNotes.set(id, note);
  return { status: "saved", note };
}

export async function deleteBrowserClipboardItemNote(id: string) {
  if (!items.some((item) => item.id === id)) throw new Error("Clipboard item not found.");
  return browserItemNotes.delete(id);
}

export async function createBrowserClipboardSmartCollection(input: ClipboardSmartCollectionInput) {
  if (browserSmartCollections.length >= 20) {
    throw new Error("ClipRiva supports at most 20 Smart Collections.");
  }
  const name = normalizeBrowserCollectionName(input.name);
  validateBrowserSmartRule(input.rule);
  ensureBrowserCollectionNameAvailable(name);
  const collection: ClipboardCollection = {
    id: crypto.randomUUID(),
    name,
    kind: "smart",
    rule: { ...input.rule },
    itemCount: 0,
    savedItemCount: 0,
  };
  browserSmartCollections = [...browserSmartCollections, collection];
  return (
    (await listBrowserClipboardCollections()).find(({ id }) => id === collection.id) ?? collection
  );
}

export async function updateBrowserClipboardSmartCollection(
  id: string,
  input: ClipboardSmartCollectionInput,
) {
  const index = browserSmartCollections.findIndex((collection) => collection.id === id);
  if (index < 0) throw new Error("Smart Collection not found.");
  const name = normalizeBrowserCollectionName(input.name);
  validateBrowserSmartRule(input.rule);
  ensureBrowserCollectionNameAvailable(name, id);
  const current = browserSmartCollections[index];
  if (!current) throw new Error("Smart Collection not found.");
  browserSmartCollections[index] = {
    ...current,
    name,
    rule: { ...input.rule },
  };
  const updated = (await listBrowserClipboardCollections()).find(
    (collection) => collection.id === id,
  );
  if (!updated) throw new Error("Smart Collection not found.");
  return updated;
}

export async function deleteBrowserClipboardSmartCollection(id: string) {
  const before = browserSmartCollections.length;
  browserSmartCollections = browserSmartCollections.filter((collection) => collection.id !== id);
  return browserSmartCollections.length !== before;
}

export async function getBrowserClipboardFilterOptions(): Promise<ClipboardFilterOptions> {
  const sourceApps = Array.from(
    new Set(
      items.flatMap((item) => [
        item.sourceApp,
        ...(item.occurrences ?? []).map((item) => item.sourceApp),
      ]),
    ),
  )
    .filter((sourceApp): sourceApp is string => Boolean(sourceApp?.trim()))
    .sort((left, right) => left.localeCompare(right));
  return {
    sourceApps,
    collections: await listBrowserClipboardCollections(),
    totalCount: items.length,
    unpinnedCount: items.filter((item) => !item.isPinned).length,
  };
}

export async function listBrowserClipboardCollections(): Promise<ClipboardCollection[]> {
  const collections = new Map<string, ClipboardCollection>();
  for (const item of items) {
    for (const collectionName of item.tags ?? []) {
      const key = collectionName.toLocaleLowerCase();
      const collection = collections.get(key) ?? {
        id: null,
        name: collectionName,
        kind: "manual" as const,
        rule: null,
        itemCount: 0,
        savedItemCount: 0,
      };
      collection.itemCount += 1;
      collection.savedItemCount += Number(item.isPinned);
      collections.set(key, collection);
    }
  }
  return [...collections.values(), ...browserSmartCollections]
    .map((collection) =>
      collection.kind === "smart" && collection.rule
        ? {
            ...collection,
            itemCount: items.filter((item) => matchesBrowserRule(item, collection.rule ?? {}))
              .length,
            savedItemCount: items.filter(
              (item) => item.isPinned && matchesBrowserRule(item, collection.rule ?? {}),
            ).length,
          }
        : collection,
    )
    .sort((left, right) => left.name.localeCompare(right.name));
}

function matchesBrowserListFilters(
  item: ClipboardItem,
  filters: ClipboardListFilters | undefined,
  pinnedOnly: boolean,
) {
  if (filters?.smartCollectionId) {
    const smart = browserSmartCollections.find(
      (collection) => collection.id === filters.smartCollectionId,
    );
    if (!smart?.rule || !matchesBrowserRule(item, smart.rule)) return false;
  }
  const pinFilter = pinnedOnly ? "pinned" : (filters?.pinFilter ?? "all");
  if (pinFilter === "pinned" && !item.isPinned) return false;
  if (pinFilter === "unpinned" && item.isPinned) return false;
  if (filters?.kinds?.length && !filters.kinds.includes(item.kind)) return false;
  if (filters?.sourceApp) {
    const source = filters.sourceApp.toLocaleLowerCase();
    const sources = [item.sourceApp, ...(item.occurrences ?? []).map((item) => item.sourceApp)];
    if (!sources.some((candidate) => candidate?.toLocaleLowerCase() === source)) return false;
  }
  if (
    filters?.collection &&
    !(item.tags ?? []).some(
      (collection) => collection.toLocaleLowerCase() === filters.collection?.toLocaleLowerCase(),
    )
  ) {
    return false;
  }
  const maximumAge = {
    all: null,
    past24Hours: 24 * 60 * 60 * 1_000,
    past7Days: 7 * 24 * 60 * 60 * 1_000,
    past30Days: 30 * 24 * 60 * 60 * 1_000,
  }[filters?.timeFilter ?? "all"];
  const latestOccurrence = item.occurrences?.[0]?.occurredAt ?? item.createdAt;
  if (maximumAge !== null && Date.parse(latestOccurrence) < Date.now() - maximumAge) return false;
  if (filters?.recentlyUsedOnly && item.copyCount === 0) return false;
  if (filters?.localLinkOnly && !item.sourceApp?.startsWith(browserLocalLinkHistorySourcePrefix)) {
    return false;
  }
  return true;
}

function matchesBrowserRule(item: ClipboardItem, rule: ClipboardSmartCollectionRule) {
  return matchesBrowserListFilters(
    item,
    {
      kinds: rule.kinds,
      sourceApp: rule.sourceApp,
      pinFilter: rule.pinFilter,
      timeFilter: rule.timeFilter,
      recentlyUsedOnly: rule.recentlyUsedOnly,
      localLinkOnly: rule.localLinkOnly,
    },
    false,
  );
}

/** Mirrors the desktop's deterministic, local Quick Paste ranking. */
export async function searchBrowserQuickPasteItems(
  options: ClipboardListOptions,
): Promise<QuickPasteSearchResult[]> {
  const query = normalizeQuickPasteText(options.query);
  const offset = options.offset ?? 0;
  const limit = Math.min(options.limit ?? 9, 20);
  if (limit <= 0) return [];

  const candidates = items.filter((item) => !options.pinnedOnly || item.isPinned);
  const results = query
    ? candidates
        .flatMap((item) => {
          const match = quickPasteMatch(
            item,
            query,
            browserItemNotes.get(item.id)?.text,
            browserImageTextExtractions.get(item.id)?.text,
          );
          return match ? [{ item, ...match }] : [];
        })
        .sort(compareQuickPasteResults)
    : zeroInputQuickPasteSuggestions(candidates, offset + limit);

  return results.slice(offset, offset + limit);
}

function normalizeQuickPasteText(value: string) {
  return value.trim().split(/\s+/u).filter(Boolean).join(" ").toLowerCase();
}

function quickPasteMatch(
  item: ClipboardItem,
  query: string,
  note?: string,
  imageText?: string,
): Pick<QuickPasteSearchResult, "matchKind" | "matchField" | "highlightRanges"> | null {
  const fields: Array<{ field: QuickPasteMatchField; value: string }> = [
    { field: "content", value: item.content },
    ...(item.representations ?? []).flatMap((representation) =>
      representation.displayName
        ? [{ field: "displayName" as const, value: representation.displayName }]
        : [],
    ),
    ...(item.sourceApp ? [{ field: "sourceApp" as const, value: item.sourceApp }] : []),
    ...(item.tags ?? []).map((tag) => ({ field: "tag" as const, value: tag })),
    ...(note ? [{ field: "note" as const, value: note }] : []),
    ...(imageText ? [{ field: "imageText" as const, value: imageText }] : []),
  ];
  const matches = fields.flatMap(({ field, value }) => {
    const matchKind = quickPasteTextMatchKind(normalizeQuickPasteText(value), query);
    return matchKind
      ? [
          {
            matchKind,
            matchField: field,
            highlightRanges: findQuickPasteHighlightRanges(value, query),
          },
        ]
      : [];
  });

  return (
    matches.sort(
      (left, right) =>
        quickPasteMatchRank(left.matchKind) - quickPasteMatchRank(right.matchKind) ||
        quickPasteFieldRank(left.matchField) - quickPasteFieldRank(right.matchField),
    )[0] ?? null
  );
}

function quickPasteTextMatchKind(value: string, query: string): QuickPasteMatchKind | null {
  if (value === query) return "exact";
  if (value.startsWith(query)) return "prefix";
  return value.includes(query) ? "contains" : null;
}

function quickPasteFieldRank(field: QuickPasteMatchField) {
  return {
    content: 0,
    displayName: 1,
    sourceApp: 2,
    tag: 3,
    note: 4,
    imageText: 5,
  }[field];
}

function quickPasteMatchRank(matchKind: QuickPasteMatchKind) {
  return {
    exact: 0,
    prefix: 1,
    word: 2,
    contains: 3,
    suggestion: 4,
  }[matchKind];
}

function compareQuickPasteResults(left: QuickPasteSearchResult, right: QuickPasteSearchResult) {
  return (
    quickPasteMatchRank(left.matchKind) - quickPasteMatchRank(right.matchKind) ||
    Number(right.item.isPinned) - Number(left.item.isPinned) ||
    browserQuickPasteActivityAt(right.item).localeCompare(browserQuickPasteActivityAt(left.item)) ||
    right.item.copyCount - left.item.copyCount ||
    left.item.id.localeCompare(right.item.id)
  );
}

function zeroInputQuickPasteSuggestions(
  candidates: ClipboardItem[],
  visibleWindow: number,
): QuickPasteSearchResult[] {
  const recent = [...candidates].sort(
    (left, right) =>
      browserQuickPasteActivityAt(right).localeCompare(browserQuickPasteActivityAt(left)) ||
      right.copyCount - left.copyCount ||
      left.id.localeCompare(right.id),
  );

  if (visibleWindow > 0 && !recent.slice(0, visibleWindow).some((item) => item.isPinned)) {
    const pinnedIndex = recent.findIndex((item) => item.isPinned);
    if (pinnedIndex >= 0) {
      const [pinned] = recent.splice(pinnedIndex, 1);
      if (pinned) {
        recent.splice(Math.min(visibleWindow - 1, recent.length), 0, pinned);
      }
    }
  }

  return recent.map((item) => ({ item, matchKind: "suggestion" }));
}

function browserQuickPasteActivityAt(item: ClipboardItem) {
  const lastOccurrenceAt = item.occurrences?.[0]?.occurredAt ?? item.createdAt;
  return item.lastUsedAt && item.lastUsedAt > lastOccurrenceAt ? item.lastUsedAt : lastOccurrenceAt;
}

function findQuickPasteHighlightRanges(value: string, normalizedQuery: string) {
  const pattern = normalizedQuery
    .split(" ")
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&"))
    .join("\\s+");
  if (!pattern) return [];

  return Array.from(value.matchAll(new RegExp(pattern, "giu")), (match) => {
    const start = match.index ?? 0;
    return { start, end: start + match[0].length };
  });
}

/** Mirrors the desktop's local lexical fallback for browser-only previews. */
export async function searchBrowserLocalSemanticItems(
  options: ClipboardListOptions,
): Promise<LocalSemanticSearchResult[]> {
  const queryTerms = tokenSet(options.query);
  if (queryTerms.size === 0) return [];

  const results = items
    .filter((item) => !options.pinnedOnly || item.isPinned)
    .flatMap((item) => {
      const documentTerms = tokenSet(item.content);
      const matchedTerms = [...queryTerms].filter((term) => documentTerms.has(term)).sort();
      if (matchedTerms.length === 0) return [];

      const unionSize = queryTerms.size + documentTerms.size - matchedTerms.length;
      const score = matchedTerms.length / unionSize;
      return [
        {
          item,
          score,
          matchedTerms,
          highlightRanges: findHighlightRanges(item.content, new Set(matchedTerms)),
        },
      ];
    })
    .sort((left, right) => right.score - left.score || left.item.id.localeCompare(right.item.id));

  const offset = options.offset ?? 0;
  return results.slice(offset, offset + (options.limit ?? 100));
}

function tokenSet(value: string) {
  return new Set(
    Array.from(value.matchAll(/[\p{L}\p{N}]+/gu), (match) => match[0].toLocaleLowerCase()),
  );
}

function findHighlightRanges(value: string, matchedTerms: Set<string>) {
  return Array.from(value.matchAll(/[\p{L}\p{N}]+/gu), (match) => {
    const token = match[0];
    const start = match.index ?? 0;
    return matchedTerms.has(token.toLocaleLowerCase())
      ? { start, end: start + token.length }
      : null;
  }).filter((range): range is { start: number; end: number } => range !== null);
}

export async function copyBrowserItem(id: string) {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }

  await navigator.clipboard?.writeText(item.content);
  const usedAt = new Date().toISOString();
  item.copyCount += 1;
  item.updatedAt = usedAt;
  item.lastUsedAt = usedAt;
  return item;
}

/**
 * Browser previews cannot focus a native predecessor or send Command-V. They
 * therefore safely model the focus failure by default after restoring the
 * selected item. Tests may select another bounded result category.
 */
export async function activateBrowserItem(id: string, mode: ClipboardActivationMode = "copy") {
  if (!items.some((item) => item.id === id)) {
    return { outcome: "invalidItem" as const, failureReason: null };
  }
  try {
    await copyBrowserItem(id);
  } catch {
    return { outcome: "writeFailed" as const, failureReason: null };
  }
  if (mode === "copy") {
    return { outcome: "clipboardRestored" as const, failureReason: null };
  }

  if (browserDirectPasteSimulation === "sent") {
    return { outcome: "pasteSent" as const, failureReason: null };
  }
  return {
    outcome: "pasteNotSent" as const,
    failureReason: browserDirectPasteSimulation,
  };
}

/** Test-only fixture control for the browser's safe Direct Paste simulation. */
export function setBrowserDirectPasteSimulationForTests(simulation: BrowserDirectPasteSimulation) {
  browserDirectPasteSimulation = simulation;
}

export async function getBrowserPastePermissionStatus() {
  return browserPastePermissionStatus;
}

export async function requestBrowserPastePermission() {
  browserPastePermissionStatus = "granted";
  return browserPastePermissionStatus;
}

export async function openBrowserPastePermissionSettings() {
  return undefined;
}

/** Test-only fixture control for the browser's permission education flow. */
export function setBrowserPastePermissionStatusForTests(status: PastePermissionStatus) {
  browserPastePermissionStatus = status;
}

export async function toggleBrowserPin(id: string) {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }

  item.isPinned = !item.isPinned;
  item.updatedAt = new Date().toISOString();
  return item;
}

export async function replaceBrowserClipboardTags(id: string, tags: string[]) {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }
  const normalizedTags = normalizeBrowserClipboardTags(tags);
  ensureManualTagsDoNotConflict(normalizedTags);
  item.tags = normalizedTags;
  return item;
}

export async function saveBrowserClipboardSnippet(id: string, collections?: string[]) {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }
  const normalizedCollections = collections
    ? normalizeBrowserClipboardTags(collections)
    : undefined;
  if (normalizedCollections) ensureManualTagsDoNotConflict(normalizedCollections);
  item.isPinned = true;
  if (normalizedCollections) item.tags = normalizedCollections;
  item.updatedAt = new Date().toISOString();
  return item;
}

export async function removeBrowserClipboardSnippet(id: string) {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }
  item.isPinned = false;
  item.updatedAt = new Date().toISOString();
  return item;
}

export async function renameBrowserClipboardCollection(
  from: string,
  to: string,
): Promise<ClipboardCollectionMutationResult> {
  const [normalizedFrom] = normalizeBrowserClipboardTags([from]);
  const [normalizedTo] = normalizeBrowserClipboardTags([to]);
  if (!normalizedFrom || !normalizedTo) throw new Error("Collection names cannot be empty.");
  ensureManualTagsDoNotConflict([normalizedTo]);
  const affected = new Set<string>();
  const allItems = [...items, ...recycleBinItems.map(({ item }) => item)];
  for (const item of allItems) {
    if (!(item.tags ?? []).some((tag) => sameBrowserCollection(tag, normalizedFrom))) continue;
    item.tags = normalizeBrowserClipboardTags(
      (item.tags ?? []).map((tag) =>
        sameBrowserCollection(tag, normalizedFrom) ? normalizedTo : tag,
      ),
    );
    affected.add(item.id);
  }
  return { affectedItemCount: affected.size };
}

export async function deleteBrowserClipboardCollection(
  name: string,
): Promise<ClipboardCollectionMutationResult> {
  const [normalizedName] = normalizeBrowserClipboardTags([name]);
  if (!normalizedName) throw new Error("Collection names cannot be empty.");
  const affected = new Set<string>();
  const allItems = [...items, ...recycleBinItems.map(({ item }) => item)];
  for (const item of allItems) {
    const nextTags = (item.tags ?? []).filter((tag) => !sameBrowserCollection(tag, normalizedName));
    if (nextTags.length === (item.tags ?? []).length) continue;
    item.tags = nextTags;
    affected.add(item.id);
  }
  return { affectedItemCount: affected.size };
}

function sameBrowserCollection(left: string, right: string) {
  return left.toLocaleLowerCase() === right.toLocaleLowerCase();
}

function normalizeBrowserClipboardTags(tags: string[]) {
  const normalized: string[] = [];
  const seen = new Set<string>();
  for (const rawTag of tags) {
    const tag = rawTag.trim().split(/\s+/u).filter(Boolean).join(" ");
    if (!tag) continue;
    if ([...tag].length > 24 || /\p{Cc}/u.test(tag)) {
      throw new Error("Clipboard tags must contain 1 to 24 visible characters.");
    }
    const comparisonKey = tag.toLocaleLowerCase();
    if (!seen.has(comparisonKey)) {
      normalized.push(tag);
      seen.add(comparisonKey);
    }
    if (normalized.length > 8) {
      throw new Error("A clipboard item can have at most 8 tags.");
    }
  }
  return normalized;
}

function ensureManualTagsDoNotConflict(tags: string[]) {
  if (
    tags.some((tag) =>
      browserSmartCollections.some(
        (collection) => collection.name.toLocaleLowerCase() === tag.toLocaleLowerCase(),
      ),
    )
  ) {
    throw new Error("A Smart Collection already uses this name.");
  }
}

function normalizeBrowserCollectionName(rawName: string) {
  const name = rawName.trim().split(/\s+/u).filter(Boolean).join(" ");
  if (!name || [...name].length > 24 || /\p{Cc}/u.test(name)) {
    throw new Error("Collection names must contain 1 to 24 visible characters.");
  }
  return name;
}

function ensureBrowserCollectionNameAvailable(name: string, excludingSmartId?: string) {
  const comparison = name.toLocaleLowerCase();
  const manualExists = [...items, ...recycleBinItems.map(({ item }) => item)].some((item) =>
    (item.tags ?? []).some((tag) => tag.toLocaleLowerCase() === comparison),
  );
  const smartExists = browserSmartCollections.some(
    (collection) =>
      collection.id !== excludingSmartId && collection.name.toLocaleLowerCase() === comparison,
  );
  if (manualExists || smartExists) throw new Error("A Collection with this name already exists.");
}

function validateBrowserSmartRule(rule: ClipboardSmartCollectionRule) {
  const supportedKinds = new Set([
    "text",
    "code",
    "command",
    "url",
    "color",
    "image",
    "richText",
    "file",
  ]);
  if (rule.kinds?.some((kind) => !supportedKinds.has(kind))) {
    throw new Error("Smart Collection contains an unsupported clipboard kind.");
  }
  const hasRule = Boolean(
    rule.kinds?.length ||
      rule.sourceApp?.trim() ||
      (rule.pinFilter && rule.pinFilter !== "all") ||
      (rule.timeFilter && rule.timeFilter !== "all") ||
      rule.recentlyUsedOnly ||
      rule.localLinkOnly,
  );
  if (!hasRule) throw new Error("A Smart Collection needs at least one supported filter.");
}

export async function deleteBrowserItem(id: string) {
  await moveBrowserItemsToRecycleBin({ scope: "selected", itemIds: [id] });
}

export async function clearBrowserHistory() {
  await moveBrowserItemsToRecycleBin({ scope: "allUnpinned" });
}

export async function previewBrowserClipboardCleanup(
  request: ClipboardCleanupRequest,
): Promise<ClipboardCleanupPreview> {
  purgeExpiredBrowserRecycleBin();
  const candidates = cleanupCandidates(request);
  const eligible = eligibleCleanupCandidates(candidates, request);
  const deletionTime = new Date();
  const expiryTimes = eligible.map((item) => recycleExpiryFor(item, deletionTime));
  const representativeItems: ClipboardCleanupRepresentative[] = eligible
    .slice(0, 3)
    .map((item) => ({
      id: item.id,
      kind: item.kind,
      sourceApp: item.sourceApp,
      capturedAt: item.createdAt,
      isPinned: item.isPinned,
    }));

  return {
    scope: request.scope,
    affectedCount: eligible.length,
    pinnedSkippedCount:
      candidates.filter((item) => item.isPinned).length -
      eligible.filter((item) => item.isPinned).length,
    ruleConditions: cleanupRuleConditions(request),
    representativeItems,
    recycleBinRetention: {
      maximumDays: recycleBinMaximumDays(),
      earliestExpiresAt:
        expiryTimes.length > 0
          ? new Date(Math.min(...expiryTimes.map(Number))).toISOString()
          : null,
      latestExpiresAt:
        expiryTimes.length > 0
          ? new Date(Math.max(...expiryTimes.map(Number))).toISOString()
          : null,
    },
  };
}

export async function moveBrowserItemsToRecycleBin(
  request: ClipboardCleanupRequest,
): Promise<ClipboardCleanupResult> {
  const preview = await previewBrowserClipboardCleanup(request);
  const eligible = eligibleCleanupCandidates(cleanupCandidates(request), request);
  const deletedAt = new Date();
  const eligibleIds = new Set(eligible.map((item) => item.id));
  const moved = items.filter((item) => eligibleIds.has(item.id));

  items = items.filter((item) => !eligibleIds.has(item.id));
  recycleBinItems = [
    ...moved.map((item) => ({
      item,
      deletedAt: deletedAt.toISOString(),
      recycleExpiresAt: recycleExpiryFor(item, deletedAt).toISOString(),
    })),
    ...recycleBinItems,
  ];

  return { preview, movedToRecycleBinCount: moved.length };
}

export async function listBrowserRecycleBinItems(limit = 100): Promise<RecycleBinItem[]> {
  purgeExpiredBrowserRecycleBin();
  return recycleBinItems.slice(0, Math.min(limit, 500));
}

export async function restoreBrowserRecycleBinItem(id: string): Promise<ClipboardItem> {
  purgeExpiredBrowserRecycleBin();
  const recycled = recycleBinItems.find((entry) => entry.item.id === id);
  if (!recycled) throw new Error("Clipboard item not found in recycle bin.");

  recycleBinItems = recycleBinItems.filter((entry) => entry.item.id !== id);
  const item = { ...recycled.item, restoredAt: new Date().toISOString() };
  items = [item, ...items];
  return item;
}

export async function permanentlyDeleteBrowserRecycleBinItems(itemIds: string[]): Promise<number> {
  const ids = new Set(itemIds);
  const before = recycleBinItems.length;
  recycleBinItems = recycleBinItems.filter((entry) => !ids.has(entry.item.id));
  for (const id of ids) {
    browserImageTextExtractions.delete(id);
    browserItemNotes.delete(id);
  }
  return before - recycleBinItems.length;
}

export async function emptyBrowserRecycleBin(): Promise<number> {
  const count = recycleBinItems.length;
  for (const entry of recycleBinItems) {
    browserImageTextExtractions.delete(entry.item.id);
    browserItemNotes.delete(entry.item.id);
  }
  recycleBinItems = [];
  return count;
}

function cleanupCandidates(request: ClipboardCleanupRequest) {
  switch (request.scope) {
    case "selected": {
      const selected = new Set(request.itemIds ?? []);
      return items.filter((item) => selected.has(item.id));
    }
    case "allUnpinned":
      return items.filter((item) => !item.isPinned);
    case "retentionPolicy": {
      const retentionCutoff = Date.now() - capturePreferences.retentionDays * 24 * 60 * 60 * 1000;
      let unpinnedSeen = 0;
      return items.filter((item) => {
        if (item.isPinned) return false;
        unpinnedSeen += 1;
        return (
          Date.parse(item.createdAt) < retentionCutoff ||
          unpinnedSeen > capturePreferences.maxHistoryItems
        );
      });
    }
  }
}

function eligibleCleanupCandidates(
  itemsToClean: ClipboardItem[],
  request: ClipboardCleanupRequest,
) {
  if (request.scope === "retentionPolicy") return itemsToClean.filter((item) => !item.isPinned);
  return itemsToClean.filter((item) => !item.isPinned || request.includePinned === true);
}

function cleanupRuleConditions(request: ClipboardCleanupRequest) {
  const conditions =
    request.scope === "selected"
      ? ["Only the selected clips are included."]
      : request.scope === "allUnpinned"
        ? ["All unpinned active clips are included."]
        : [
            `Unpinned clips older than ${capturePreferences.retentionDays} days or beyond the ${capturePreferences.maxHistoryItems}-clip limit are included.`,
          ];
  conditions.push(
    request.scope === "retentionPolicy" || !request.includePinned
      ? "Pinned clips are skipped by default."
      : "Pinned clips are included after explicit confirmation.",
  );
  conditions.push(
    `Recovery is available for the shorter of ${recycleBinMaximumDays()} ${
      recycleBinMaximumDays() === 1 ? "day" : "days"
    } or the clip's remaining retention time.`,
  );
  return conditions;
}

function recycleBinMaximumDays() {
  return capturePreferences.sensitiveContentPolicy === "strict" ? 1 : 7;
}

function recycleExpiryFor(item: ClipboardItem, deletedAt: Date) {
  const originalExpiry =
    item.retentionUntil && Number.isFinite(Date.parse(item.retentionUntil))
      ? Date.parse(item.retentionUntil)
      : Date.parse(item.createdAt) + capturePreferences.retentionDays * 24 * 60 * 60 * 1000;
  const remaining = Math.max(0, originalExpiry - deletedAt.getTime());
  return new Date(
    deletedAt.getTime() + Math.min(recycleBinMaximumDays() * 24 * 60 * 60 * 1000, remaining),
  );
}

function purgeExpiredBrowserRecycleBin() {
  const now = Date.now();
  for (const entry of recycleBinItems) {
    if (Date.parse(entry.recycleExpiresAt) <= now) {
      browserImageTextExtractions.delete(entry.item.id);
      browserItemNotes.delete(entry.item.id);
    }
  }
  recycleBinItems = recycleBinItems.filter((entry) => Date.parse(entry.recycleExpiresAt) > now);
}

export async function getBrowserCapturePreferences() {
  return capturePreferences;
}

export async function updateBrowserCapturePreferences(preferences: CapturePreferences) {
  const sensitiveContentPolicy =
    preferences.sensitiveContentPolicy ??
    (preferences.sensitivePauseEnabled ? "strict" : "default");
  capturePreferences = {
    ...preferences,
    sensitiveContentPolicy,
    // Keep browser previews faithful to the legacy field exposed by the
    // native command, even if a caller sends conflicting values.
    sensitivePauseEnabled: sensitiveContentPolicy === "strict",
    retentionDays: Math.min(Math.max(preferences.retentionDays, 1), 3650),
    maxHistoryItems: Math.min(Math.max(preferences.maxHistoryItems, 100), 100000),
  };
  return capturePreferences;
}

export async function getBrowserAutomationPreferences() {
  return { ...automationPreferences };
}

export async function updateBrowserAutomationPreferences(preferences: AutomationPreferences) {
  automationPreferences = { ...preferences };
  return { ...automationPreferences };
}

export async function resumeBrowserCapture() {
  capturePreferences = { ...capturePreferences, capturePaused: false, pauseReason: null };
  return capturePreferences;
}

/** Browser previews do not observe the host clipboard, so this stays empty
 * unless a future preview fixture explicitly supplies privacy-safe events. */
export async function listBrowserCaptureStatusEvents() {
  return [...captureStatusEvents]
    .filter((event) => Date.parse(event.expiresAt) > Date.now())
    .sort((left, right) => right.occurredAt.localeCompare(left.occurredAt))
    .slice(0, 20);
}

export async function clearBrowserCaptureStatusEvents() {
  captureStatusEvents = [];
}

/**
 * Browser previews keep the same opt-in, bounded schema as the desktop
 * database.  The input carries only a fixed event/result pair; no clipboard
 * content, clip identity, application name, path, filename, timestamp, or
 * network identifier can enter this store.
 */
export async function recordBrowserLocalDiagnosticEvent(
  event: LocalDiagnosticEventInput,
): Promise<LocalDiagnosticRecordResult> {
  if (!isValidBrowserLocalDiagnosticEvent(event)) {
    throw new Error("This diagnostic event/result pair is not supported.");
  }

  if (!capturePreferences.diagnosticsEnabled) {
    return { recorded: false };
  }

  pruneBrowserLocalDiagnosticEvents();
  localDiagnosticEvents = [
    ...localDiagnosticEvents,
    {
      eventType: event.eventType,
      outcome: event.outcome,
      occurredDay: browserDiagnosticDay(),
      appVersion: "browser-preview",
    },
  ].slice(-2_000);
  return { recorded: true };
}

export async function getBrowserLocalDiagnosticsSummary(): Promise<LocalDiagnosticsSummary> {
  pruneBrowserLocalDiagnosticEvents();
  return browserLocalDiagnosticsSummary();
}

export async function clearBrowserLocalDiagnostics() {
  localDiagnosticEvents = [];
}

export async function exportBrowserLocalDiagnostics(): Promise<LocalDiagnosticsExport> {
  pruneBrowserLocalDiagnosticEvents();
  return {
    schemaVersion: 2,
    exportedAtDay: browserDiagnosticDay(),
    appVersion: "browser-preview",
    summary: browserLocalDiagnosticsSummary(),
    events: [...localDiagnosticEvents],
  };
}

function isValidBrowserLocalDiagnosticEvent({ eventType, outcome }: LocalDiagnosticEventInput) {
  const allowedOutcomes: Record<LocalDiagnosticEventType, LocalDiagnosticOutcome[]> = {
    firstRecovery: ["started", "completed"],
    directPaste: ["started", "completed", "degraded", "failed", "restricted"],
    recycleBinRestore: ["restored", "failed"],
    duplicateGroupExpand: ["expanded"],
    clipboardRestoreError: ["failed"],
    capturePermissionRestricted: ["restricted"],
    systemPolicyRestricted: ["restricted"],
  };
  return allowedOutcomes[eventType].includes(outcome);
}

function browserDiagnosticDay() {
  return new Date().toISOString().slice(0, 10);
}

function pruneBrowserLocalDiagnosticEvents() {
  const cutoff = new Date(Date.now() - 90 * 24 * 60 * 60 * 1_000).toISOString().slice(0, 10);
  localDiagnosticEvents = localDiagnosticEvents
    .filter((event) => event.occurredDay >= cutoff)
    .slice(-2_000);
}

function browserLocalDiagnosticsSummary(): LocalDiagnosticsSummary {
  const firstRecovery = browserLocalDiagnosticRate("firstRecovery");
  const directPaste = browserLocalDiagnosticRate("directPaste");
  const criticalErrorCategories: LocalDiagnosticErrorCategory[] = [];

  for (const event of localDiagnosticEvents) {
    if (event.outcome !== "failed" && event.outcome !== "restricted") continue;
    const existing = criticalErrorCategories.find(
      (category) => category.eventType === event.eventType && category.outcome === event.outcome,
    );
    if (existing) {
      existing.count += 1;
    } else {
      criticalErrorCategories.push({
        eventType: event.eventType,
        outcome: event.outcome,
        count: 1,
      });
    }
  }

  return {
    enabled: capturePreferences.diagnosticsEnabled ?? false,
    storedEventCount: localDiagnosticEvents.length,
    firstRecovery,
    directPaste,
    recycleBinRestoreCount: browserLocalDiagnosticCount("recycleBinRestore", "restored"),
    duplicateGroupExpandCount: browserLocalDiagnosticCount("duplicateGroupExpand", "expanded"),
    criticalErrorCategories: criticalErrorCategories.sort(
      (left, right) =>
        right.count - left.count ||
        `${left.eventType}:${left.outcome}`.localeCompare(`${right.eventType}:${right.outcome}`),
    ),
  };
}

function browserLocalDiagnosticRate(eventType: LocalDiagnosticEventType): LocalDiagnosticRate {
  const attempts = browserLocalDiagnosticCount(eventType, "started");
  const completedOrDegraded = browserLocalDiagnosticCount(
    eventType,
    eventType === "firstRecovery" ? "completed" : "degraded",
  );
  return {
    attempts,
    completedOrDegraded,
    percent:
      attempts === 0 ? null : Math.min(100, Math.round((completedOrDegraded / attempts) * 100)),
  };
}

function browserLocalDiagnosticCount(
  eventType: LocalDiagnosticEventType,
  outcome: LocalDiagnosticOutcome,
) {
  return localDiagnosticEvents.filter(
    (event) => event.eventType === eventType && event.outcome === outcome,
  ).length;
}

export async function getBrowserContextEnrichment(id: string): Promise<ContextEnrichment> {
  const item = items.find((candidate) => candidate.id === id);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }

  const contentKind = item.kind === "url" ? "url" : item.kind;
  return {
    metadata: {
      schemaVersion: 1,
      enricherId: "clipriva.browser-context",
      enricherVersion: "1.0.0",
    },
    classification: { kind: contentKind, matchedRules: [`content.${contentKind}.demo`] },
    summary: {
      text: `Local ${contentKind} context captured from ${item.sourceApp ?? "an unknown app"}.`,
      isTruncated: false,
    },
    entities: [],
    tags: [
      {
        id: `content:${contentKind}`,
        label: contentKind.slice(0, 1).toUpperCase() + contentKind.slice(1),
      },
      ...(item.sourceApp ? [{ id: "source:captured", label: "Source attributed" }] : []),
    ],
    redaction: { redactedContent: item.content, requiresExplicitConsent: false },
  };
}

export async function listBrowserLocalTextActions() {
  return [...localTextActions].sort(
    (left, right) =>
      Number(right.isBuiltin) - Number(left.isBuiltin) || left.name.localeCompare(right.name),
  );
}

export async function createBrowserLocalTextAction(draft: CreateLocalTextAction) {
  const name = draft.name.trim();
  if (!name || name.length > 64) {
    throw new Error("Action names must contain 1 to 64 characters.");
  }
  if (
    localTextActions.some((action) => action.name.toLocaleLowerCase() === name.toLocaleLowerCase())
  ) {
    throw new Error("An action with this name already exists.");
  }
  if (
    draft.shortcutSlot !== null &&
    draft.shortcutSlot !== undefined &&
    (!Number.isInteger(draft.shortcutSlot) ||
      draft.shortcutSlot < 1 ||
      draft.shortcutSlot > 9 ||
      localTextActions.some((action) => action.shortcutSlot === draft.shortcutSlot))
  ) {
    throw new Error("A local filter shortcut slot must be a unique value from 1 to 9.");
  }

  const timestamp = new Date().toISOString();
  const action: LocalTextAction = {
    id: `local-action-${crypto.randomUUID()}`,
    name,
    transform: draft.transform,
    steps: draft.steps?.length ? draft.steps : [{ transform: draft.transform }],
    shortcutSlot: draft.shortcutSlot ?? null,
    isBuiltin: false,
    createdAt: timestamp,
    updatedAt: timestamp,
  };
  localTextActions = [...localTextActions, action];
  return action;
}

export async function updateBrowserLocalTextAction(id: string, draft: CreateLocalTextAction) {
  const existing = localTextActions.find((action) => action.id === id);
  if (!existing) throw new Error("Local filter not found.");
  if (existing.isBuiltin) throw new Error("Built-in local filters cannot be changed.");
  const name = draft.name.trim();
  if (!name || name.length > 64) throw new Error("Action names must contain 1 to 64 characters.");
  if (
    localTextActions.some(
      (action) => action.id !== id && action.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
    )
  ) {
    throw new Error("An action with this name already exists.");
  }
  const slot = draft.shortcutSlot ?? null;
  if (
    (slot !== null && (!Number.isInteger(slot) || slot < 1 || slot > 9)) ||
    localTextActions.some(
      (action) => action.id !== id && action.shortcutSlot === slot && slot !== null,
    )
  ) {
    throw new Error("A local filter shortcut slot must be a unique value from 1 to 9.");
  }
  const timestamp = new Date().toISOString();
  const updated: LocalTextAction = {
    ...existing,
    name,
    transform: draft.transform,
    steps: draft.steps?.length ? draft.steps : [{ transform: draft.transform }],
    shortcutSlot: slot,
    updatedAt: timestamp,
  };
  localTextActions = localTextActions.map((action) => (action.id === id ? updated : action));
  return updated;
}

export async function deleteBrowserLocalTextAction(id: string) {
  const existing = localTextActions.find((action) => action.id === id);
  if (!existing) throw new Error("Local filter not found.");
  if (existing.isBuiltin) throw new Error("Built-in local filters cannot be deleted.");
  localTextActions = localTextActions.filter((action) => action.id !== id);
  return true;
}

export async function previewBrowserLocalTextAction(
  actionId: string,
  clipboardItemId: string,
): Promise<LocalTextActionPreview> {
  if (!capturePreferences.labsEnabled) throw new Error("Labs is disabled.");
  const action = localTextActions.find((candidate) => candidate.id === actionId);
  const item = items.find((candidate) => candidate.id === clipboardItemId);
  if (!action || !item) {
    throw new Error("Clipboard item or local action not found.");
  }
  if (item.representations?.length) {
    throw new Error("Local text actions are available for text clips only.");
  }

  const output = evaluateFilterPipeline(
    item.content,
    action.steps.map((step) => ({ kind: step.transform })),
  );
  return {
    action: structuredClone(action),
    clipboardItemId: item.id,
    itemVersion: item.version ?? 1,
    input: item.content,
    output,
  };
}

export async function confirmBrowserLocalTextAction(
  preview: LocalTextActionPreview,
): Promise<TextActionExecution> {
  if (!capturePreferences.labsEnabled) throw new Error("Labs is disabled.");
  const action = localTextActions.find((candidate) => candidate.id === preview.action.id);
  const item = items.find((candidate) => candidate.id === preview.clipboardItemId);
  if (!action || !item)
    throw new Error("Filter preview is out of date. Preview again before copying.");
  const currentOutput = evaluateFilterPipeline(
    item.content,
    action.steps.map((step) => ({ kind: step.transform })),
  );
  if (
    JSON.stringify(action) !== JSON.stringify(preview.action) ||
    (item.version ?? 1) !== preview.itemVersion ||
    item.content !== preview.input ||
    currentOutput !== preview.output
  ) {
    throw new Error("Filter preview is out of date. Preview again before copying.");
  }
  const timestamp = new Date().toISOString();
  const audit: ActionAuditRecord = {
    id: `audit-${crypto.randomUUID()}`,
    schemaVersion: 2,
    actionId: action.id,
    actionName: action.name,
    transform: action.transform,
    executionMode: "local",
    status: "completed",
    clipboardItemId: item.id,
    inputContentHash: "not-stored",
    inputPreview: "",
    outputContentHash: null,
    outputPreview: null,
    createdAt: timestamp,
  };
  await navigator.clipboard.writeText(currentOutput);
  actionAudit = [audit, ...actionAudit];
  return { action, output: currentOutput, audit };
}

export async function listBrowserActionAudit(clipboardItemId: string | null, limit: number) {
  return actionAudit
    .filter((entry) => !clipboardItemId || entry.clipboardItemId === clipboardItemId)
    .slice(0, Math.min(Math.max(limit, 1), 100));
}

export async function previewBrowserCloudAction(
  clipboardItemId: string,
): Promise<CloudActionPreview> {
  const item = items.find((candidate) => candidate.id === clipboardItemId);
  if (!item) {
    throw new Error("Clipboard item not found.");
  }

  const timestamp = new Date().toISOString();
  const redaction = redactionPreview(item.content);
  const audit: ActionAuditRecord = {
    id: `audit-${crypto.randomUUID()}`,
    schemaVersion: 1,
    actionId: null,
    actionName: "Future cloud action",
    transform: null,
    executionMode: "cloud_preview",
    status: "preview_only_blocked",
    clipboardItemId: item.id,
    inputContentHash: demoHash(item.content),
    inputPreview: redaction.redactedContent,
    outputContentHash: null,
    outputPreview: null,
    createdAt: timestamp,
  };
  actionAudit = [audit, ...actionAudit];
  return {
    delivery: "preview_only",
    isExecutable: false,
    message: "Cloud actions are disabled. This is a redacted local preview only.",
    redaction,
    audit,
  };
}

/** Browser-only Local Link fixture for UI development and automated flows. */
export async function getBrowserLocalLinkPreferences(): Promise<LocalLinkPreferences> {
  // A URL-scoped visual fixture makes the ready sidecar reviewable without
  // weakening the production/browser default-off contract or claiming LAN evidence.
  if (new URLSearchParams(window.location.search).has("local-link-ready")) {
    localLinkPreferences = {
      ...localLinkPreferences,
      enabled: true,
      discoveryEnabled: true,
    };
  }
  return { ...localLinkPreferences };
}

export async function updateBrowserLocalLinkPreferences(
  preferences: LocalLinkPreferences,
): Promise<LocalLinkPreferences> {
  const deviceName = preferences.deviceName.trim();
  if (!deviceName || deviceName.length > 80) {
    throw new Error("Device name must contain 1 to 80 characters.");
  }
  localLinkPreferences = {
    enabled: preferences.enabled,
    deviceName,
    discoveryEnabled: preferences.discoveryEnabled,
    identityFingerprint: preferences.identityFingerprint ?? null,
    protocolVersion: preferences.protocolVersion ?? null,
  };
  if (!preferences.enabled) {
    // Match the native rule: a session-only verification does not survive
    // turning Local Link off.
    localLinkDevices = localLinkDevices.map((device) =>
      device.trustDuration === "thisSession" && device.trustStatus === "trusted"
        ? {
            ...device,
            trustStatus: "needsRePairing" as const,
            pairedAt: null,
            trustedAt: null,
            trustDuration: "thirtyDays" as const,
          }
        : device,
    );
  }
  return getBrowserLocalLinkPreferences();
}

export async function listBrowserLocalLinkDevices(): Promise<LocalLinkDevice[]> {
  return localLinkDevices
    .filter(
      (device) =>
        device.trustStatus === "trusted" ||
        device.trustStatus === "needsRePairing" ||
        device.trustStatus === "pairing" ||
        Boolean(device.revokedAt) ||
        localLinkPreferences.discoveryEnabled,
    )
    .map((device) => ({ ...device }))
    .sort((left, right) => {
      const trustOrder = { trusted: 0, needsRePairing: 1, pairing: 2, revoked: 3 } as const;
      return (
        trustOrder[left.trustStatus] - trustOrder[right.trustStatus] ||
        left.displayName.localeCompare(right.displayName)
      );
    });
}

export async function beginBrowserLocalLinkPairing(
  deviceId: string,
  displayName: string,
  pairingCode: string,
): Promise<LocalLinkDevice> {
  if (!localLinkPreferences.enabled || !localLinkPreferences.discoveryEnabled) {
    throw new Error("Turn on device discovery before pairing a nearby device.");
  }
  if (!/^\d{6}$/u.test(pairingCode)) {
    throw new Error("Enter the same six-digit code shown on both devices.");
  }
  const device = localLinkDevices.find((candidate) => candidate.deviceId === deviceId);
  if (!device || device.trustStatus === "trusted") {
    throw new Error("This nearby device is no longer available for pairing.");
  }

  const pairingDevice = {
    ...device,
    displayName: displayName.trim() || device.displayName,
    trustStatus: "pairing" as const,
    pairedAt: new Date().toISOString(),
  };
  localLinkDevices = localLinkDevices.map((candidate) =>
    candidate.deviceId === deviceId ? pairingDevice : candidate,
  );
  localLinkPairing = {
    device: pairingDevice,
    verificationCode: pairingCode,
    expiresAt: new Date(Date.now() + 60_000).toISOString(),
  };
  return { ...pairingDevice };
}

export async function confirmBrowserLocalLinkPairing(
  deviceId: string,
  verificationCode: string,
  trustDuration: LocalLinkTrustDuration = "thirtyDays",
): Promise<LocalLinkDevice> {
  if (!localLinkPairing || localLinkPairing.device.deviceId !== deviceId) {
    throw new Error("This pairing request has expired. Start again from nearby devices.");
  }
  if (
    localLinkPairing.expiresAt !== null &&
    new Date(localLinkPairing.expiresAt).getTime() <= Date.now()
  ) {
    await cancelBrowserLocalLinkPairing(deviceId);
    throw new Error("This pairing request has expired. Start again from nearby devices.");
  }
  if (verificationCode !== localLinkPairing.verificationCode) {
    throw new Error("The six-digit code does not match on both devices.");
  }

  const pairedAt = new Date().toISOString();
  localLinkDevices = localLinkDevices.map((device) =>
    device.deviceId === deviceId
      ? {
          ...device,
          trustStatus: "trusted",
          pairedAt,
          trustedAt: pairedAt,
          trustDuration,
          trustExpiresAt:
            trustDuration === "thirtyDays"
              ? new Date(Date.now() + 30 * 24 * 60 * 60 * 1_000).toISOString()
              : null,
          revokedAt: null,
        }
      : device,
  );
  localLinkPairing = null;
  const trusted = localLinkDevices.find((device) => device.deviceId === deviceId);
  if (!trusted) throw new Error("Nearby device could not be found.");
  return { ...trusted };
}

export async function cancelBrowserLocalLinkPairing(deviceId: string): Promise<void> {
  if (localLinkPairing?.device.deviceId === deviceId) {
    localLinkDevices = localLinkDevices.map((device) =>
      device.deviceId === deviceId ? { ...device, trustStatus: "revoked", pairedAt: null } : device,
    );
    localLinkPairing = null;
  }
}

export async function revokeBrowserLocalLinkDevice(deviceId: string): Promise<LocalLinkDevice> {
  const device = localLinkDevices.find((candidate) => candidate.deviceId === deviceId);
  if (device?.trustStatus !== "trusted") {
    throw new Error("Trusted device not found.");
  }
  // Removing trust is immediate. It can reappear only as a nearby device when
  // the user has explicitly enabled discovery; it never retains a pairing key.
  const revokedAt = new Date().toISOString();
  localLinkDevices = localLinkDevices.map((candidate) =>
    candidate.deviceId === deviceId
      ? { ...candidate, trustStatus: "revoked", pairedAt: null, revokedAt }
      : candidate,
  );
  const revoked = localLinkDevices.find((candidate) => candidate.deviceId === deviceId);
  if (!revoked) throw new Error("Trusted device not found.");
  return { ...revoked };
}

export async function sendBrowserClipboardItemToLocalLinkDevice(
  clipboardItemId: string,
  deviceId: string,
): Promise<LocalLinkTransfer> {
  const item = items.find((candidate) => candidate.id === clipboardItemId);
  const device = localLinkDevices.find((candidate) => candidate.deviceId === deviceId);
  if (!item) throw new Error("Clipboard item not found.");
  if (!localLinkPreferences.enabled) {
    throw new Error("Enable Local Link in Settings → Devices before sending.");
  }
  if (device?.trustStatus !== "trusted") {
    throw new Error("Choose a trusted device before sending.");
  }
  if (!isBrowserLocalLinkTextItem(item)) {
    throw new Error("Local Link Text Beta can send text clips only.");
  }
  const byteSize = localLinkBrowserItemByteSize(item);
  if (byteSize > 256 * 1024) {
    throw new Error("This text clip exceeds Local Link's 256 KiB beta limit.");
  }

  const kind = localLinkContentKindForBrowserItem(item);
  const timestamp = new Date().toISOString();
  const transfer: LocalLinkTransfer = {
    id: `browser-transfer-${crypto.randomUUID()}`,
    direction: "outgoing",
    deviceId: device.deviceId,
    clipboardItemId,
    itemKind: kind,
    byteSize,
    status: "connecting",
    createdAt: timestamp,
    updatedAt: timestamp,
    failureReason: null,
    expiresAt: new Date(Date.now() + 60_000).toISOString(),
    isUiFixture: true,
  };

  const sourceBlocked = Boolean(
    capturePreferences.capturePaused ||
      (item.sourceApp && capturePreferences.deniedApps.includes(item.sourceApp)),
  );
  if (sourceBlocked) {
    transfer.status = "failed";
    transfer.failureReason = "captureBlocked";
  }

  localLinkTransfers = [transfer, ...localLinkTransfers];
  return { ...transfer };
}

export async function listBrowserLocalLinkTransfers(): Promise<LocalLinkTransfer[]> {
  expireBrowserLocalLinkTransfers();
  return [...localLinkTransfers]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .map((transfer) => ({ ...transfer }));
}

/**
 * Records the receiver opening Incoming Center without reading, copying, or
 * returning the memory-only payload to the browser fixture.
 */
export async function markBrowserLocalLinkTransferViewed(
  transferId: string,
): Promise<LocalLinkTransfer> {
  const transfer = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (transfer?.direction !== "incoming") {
    throw new Error("Only an incoming transfer can be marked as viewed.");
  }
  if (transfer.status === "viewed") return { ...transfer };
  if (transfer.status !== "awaitingReceiver" || !browserLocalLinkPayloads.has(transfer.id)) {
    throw new Error("This incoming transfer is no longer waiting for your decision.");
  }
  return updateBrowserLocalLinkTransfer(transfer.id, {
    status: "viewed",
    updatedAt: new Date().toISOString(),
  });
}

export async function acceptBrowserLocalLinkTransfer(
  transferId: string,
  action: LocalLinkAcceptAction,
): Promise<LocalLinkTransfer> {
  const transfer = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (
    transfer?.direction !== "incoming" ||
    !["awaitingReceiver", "viewed"].includes(transfer.status)
  ) {
    throw new Error("This incoming transfer is no longer waiting for your confirmation.");
  }

  const payload = browserLocalLinkPayloads.get(transfer.id);
  if (!payload) {
    throw new Error("This incoming transfer is no longer available.");
  }
  if (action === "copy") {
    await navigator.clipboard?.writeText(payload);
  } else {
    const timestamp = new Date().toISOString();
    items = [
      {
        id: `browser-received-${crypto.randomUUID()}`,
        content: payload,
        kind: "text",
        sourceApp: `${browserLocalLinkHistorySourcePrefix}${localLinkDeviceName(transfer.deviceId)}`,
        createdAt: timestamp,
        updatedAt: timestamp,
        isPinned: false,
        copyCount: 0,
        occurrenceCount: 1,
        occurrences: [
          {
            id: `browser-received-occurrence-${crypto.randomUUID()}`,
            sourceApp: `${browserLocalLinkHistorySourcePrefix}${localLinkDeviceName(transfer.deviceId)}`,
            deviceId: transfer.deviceId,
            occurredAt: timestamp,
          },
        ],
      },
      ...items,
    ];
  }

  const completed = updateBrowserLocalLinkTransfer(transfer.id, {
    status: action === "save" ? "saved" : "copied",
    updatedAt: new Date().toISOString(),
    completedAt: new Date().toISOString(),
    failureReason: null,
    receiverAction: action,
  });
  browserLocalLinkPayloads.delete(transfer.id);
  return completed;
}

export async function rejectBrowserLocalLinkTransfer(
  transferId: string,
): Promise<LocalLinkTransfer> {
  const transfer = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (
    transfer?.direction !== "incoming" ||
    !["awaitingReceiver", "viewed"].includes(transfer.status)
  ) {
    throw new Error("This incoming transfer is no longer waiting for your confirmation.");
  }
  const rejected = updateBrowserLocalLinkTransfer(transfer.id, {
    status: "rejected",
    updatedAt: new Date().toISOString(),
    completedAt: new Date().toISOString(),
    failureReason: null,
    receiverAction: "reject",
  });
  browserLocalLinkPayloads.delete(transfer.id);
  return rejected;
}

export async function cancelBrowserLocalLinkTransfer(
  transferId: string,
): Promise<LocalLinkTransfer> {
  const transfer = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (transfer?.direction !== "outgoing") {
    throw new Error("Only an outgoing request can be cancelled.");
  }
  if (["copied", "saved", "rejected", "cancelled", "expired", "failed"].includes(transfer.status)) {
    throw new Error("This request has already reached a final state.");
  }
  return updateBrowserLocalLinkTransfer(transfer.id, {
    status: "cancelled",
    updatedAt: new Date().toISOString(),
    completedAt: new Date().toISOString(),
    failureReason: null,
  });
}

/** Browser previews acknowledge reveal intent without exposing the payload. */
export async function revealBrowserLocalLinkTransfer(transferId: string): Promise<void> {
  const transfer = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (
    transfer?.direction !== "incoming" ||
    !["awaitingReceiver", "viewed"].includes(transfer.status) ||
    !browserLocalLinkPayloads.has(transfer.id)
  ) {
    throw new Error("This incoming transfer is no longer available for secure viewing.");
  }
}

export async function resetBrowserLocalLinkIdentity(): Promise<void> {
  localLinkDevices = localLinkDevices.map((device) =>
    device.trustStatus === "trusted"
      ? { ...device, trustStatus: "needsRePairing" as const, pairedAt: null }
      : device,
  );
  localLinkPreferences = {
    ...localLinkPreferences,
    identityFingerprint: `RESET · ${String(Date.now()).slice(-4)}`,
  };
}

export async function clearBrowserLocalLinkTransfers(): Promise<number> {
  const terminalIds = new Set(
    localLinkTransfers
      .filter((transfer) =>
        ["copied", "saved", "rejected", "cancelled", "expired", "failed"].includes(transfer.status),
      )
      .map((transfer) => transfer.id),
  );
  localLinkTransfers = localLinkTransfers.filter((transfer) => !terminalIds.has(transfer.id));
  for (const transferId of terminalIds) browserLocalLinkPayloads.delete(transferId);
  return terminalIds.size;
}

/** Resettable fixture control so browser fallback tests never leak trust state. */
export function resetBrowserLocalLinkForTests() {
  localLinkPreferences = { ...browserLocalLinkDefaultPreferences };
  localLinkDevices = browserLocalLinkDevices.map((device) => ({ ...device }));
  localLinkPairing = null;
  items = items.filter((item) => !item.sourceApp?.startsWith(browserLocalLinkHistorySourcePrefix));
  localLinkTransfers = [
    {
      id: "browser-incoming-review-text",
      direction: "incoming",
      deviceId: "browser-trusted-mac-mini",
      itemKind: "text",
      byteSize: 58,
      status: "awaitingReceiver",
      createdAt: new Date(now - 45_000).toISOString(),
      updatedAt: new Date(now - 45_000).toISOString(),
      fileName: null,
      failureReason: null,
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      isUiFixture: true,
    },
    {
      id: "browser-failed-text-transfer",
      direction: "outgoing",
      deviceId: "browser-trusted-mac-mini",
      clipboardItemId: "demo-text",
      itemKind: "text",
      byteSize: 76,
      status: "failed",
      createdAt: new Date(now - 180_000).toISOString(),
      updatedAt: new Date(now - 180_000).toISOString(),
      failureReason: "transportUnavailable",
      isUiFixture: true,
    },
    {
      id: "browser-copied-text-transfer",
      direction: "incoming",
      deviceId: "browser-trusted-mac-mini",
      itemKind: "text",
      byteSize: 42,
      status: "copied",
      receiverAction: "copy",
      createdAt: new Date(now - 75_000).toISOString(),
      updatedAt: new Date(now - 72_000).toISOString(),
      completedAt: new Date(now - 72_000).toISOString(),
      failureReason: null,
      isUiFixture: true,
    },
    {
      id: "browser-rejected-text-transfer",
      direction: "outgoing",
      deviceId: "browser-offline-macbook-pro",
      itemKind: "text",
      byteSize: 31,
      status: "rejected",
      createdAt: new Date(now - 240_000).toISOString(),
      updatedAt: new Date(now - 235_000).toISOString(),
      completedAt: new Date(now - 235_000).toISOString(),
      failureReason: null,
      isUiFixture: true,
    },
  ];
  browserLocalLinkPayloads.clear();
  browserLocalLinkPayloads.set(
    "browser-incoming-review-text",
    "Review the agenda before the meeting.",
  );
}

function updateBrowserLocalLinkTransfer(
  transferId: string,
  changes: Partial<LocalLinkTransfer>,
): LocalLinkTransfer {
  const current = localLinkTransfers.find((candidate) => candidate.id === transferId);
  if (!current) throw new Error("Transfer not found.");
  const next = { ...current, ...changes };
  localLinkTransfers = localLinkTransfers.map((transfer) =>
    transfer.id === transferId ? next : transfer,
  );
  return { ...next };
}

function expireBrowserLocalLinkTransfers() {
  const currentTime = Date.now();
  for (const transfer of localLinkTransfers) {
    if (
      transfer.direction === "incoming" &&
      ["awaitingReceiver", "viewed"].includes(transfer.status) &&
      transfer.expiresAt &&
      new Date(transfer.expiresAt).getTime() <= currentTime
    ) {
      updateBrowserLocalLinkTransfer(transfer.id, {
        status: "expired",
        updatedAt: new Date(currentTime).toISOString(),
        completedAt: new Date(currentTime).toISOString(),
        failureReason: null,
      });
      browserLocalLinkPayloads.delete(transfer.id);
    }
  }
}

function localLinkContentKindForBrowserItem(item: ClipboardItem): LocalLinkContentKind {
  if (!isBrowserLocalLinkTextItem(item)) {
    throw new Error("Local Link Text Beta can send text clips only.");
  }
  return "text";
}

function isBrowserLocalLinkTextItem(item: ClipboardItem) {
  return (
    item.kind === "text" ||
    item.kind === "code" ||
    item.kind === "command" ||
    item.kind === "url" ||
    item.kind === "color"
  );
}

function localLinkBrowserItemByteSize(item: ClipboardItem) {
  return item.representations?.[0]?.byteSize ?? new TextEncoder().encode(item.content).byteLength;
}

function localLinkDeviceName(deviceId: string) {
  return (
    localLinkDevices.find((device) => device.deviceId === deviceId)?.displayName ?? "another Mac"
  );
}

function redactionPreview(content: string) {
  const email = /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi;
  const matches = [...content.matchAll(email)];
  return {
    redactedContent: content.replace(email, "[REDACTED:email_address]"),
    requiresExplicitConsent: matches.length > 0,
    matches: matches.map(() => ({
      kind: "email_address",
      replacement: "[REDACTED:email_address]",
    })),
  };
}

function demoHash(content: string) {
  let value = 2166136261;
  for (let index = 0; index < content.length; index += 1) {
    value ^= content.charCodeAt(index);
    value = Math.imul(value, 16777619);
  }
  return `demo-${(value >>> 0).toString(16).padStart(8, "0")}`;
}
