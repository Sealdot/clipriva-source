import { invoke } from "@tauri-apps/api/core";
import { isDesktopRuntime } from "../../lib/runtime";
import {
  acceptBrowserLocalLinkTransfer,
  activateBrowserItem,
  beginBrowserLocalLinkPairing,
  cancelBrowserClipboardImageTextExtraction,
  cancelBrowserLocalLinkPairing,
  cancelBrowserLocalLinkTransfer,
  clearBrowserCaptureStatusEvents,
  clearBrowserHistory,
  clearBrowserLocalDiagnostics,
  clearBrowserLocalLinkTransfers,
  confirmBrowserLocalLinkPairing,
  confirmBrowserLocalTextAction,
  copyBrowserItem,
  createBrowserClipboardSmartCollection,
  createBrowserLocalTextAction,
  deleteBrowserClipboardCollection,
  deleteBrowserClipboardImageText,
  deleteBrowserClipboardItemNote,
  deleteBrowserClipboardSmartCollection,
  deleteBrowserItem,
  deleteBrowserLocalTextAction,
  emptyBrowserRecycleBin,
  exportBrowserLocalDiagnostics,
  extractBrowserClipboardImageText,
  getBrowserAutomationPreferences,
  getBrowserCapturePreferences,
  getBrowserClipboardFilterOptions,
  getBrowserClipboardImageText,
  getBrowserClipboardItemNote,
  getBrowserContextEnrichment,
  getBrowserItem,
  getBrowserLocalDiagnosticsSummary,
  getBrowserLocalLinkPreferences,
  getBrowserPastePermissionStatus,
  listBrowserActionAudit,
  listBrowserCaptureStatusEvents,
  listBrowserClipboardCollections,
  listBrowserItems,
  listBrowserLocalLinkDevices,
  listBrowserLocalLinkTransfers,
  listBrowserLocalTextActions,
  listBrowserRecycleBinItems,
  markBrowserLocalLinkTransferViewed,
  moveBrowserItemsToRecycleBin,
  openBrowserPastePermissionSettings,
  permanentlyDeleteBrowserRecycleBinItems,
  previewBrowserClipboardCleanup,
  previewBrowserCloudAction,
  previewBrowserLocalTextAction,
  recordBrowserLocalDiagnosticEvent,
  rejectBrowserLocalLinkTransfer,
  removeBrowserClipboardSnippet,
  renameBrowserClipboardCollection,
  replaceBrowserClipboardTags,
  requestBrowserPastePermission,
  resetBrowserLocalLinkIdentity,
  restoreBrowserRecycleBinItem,
  resumeBrowserCapture,
  revealBrowserLocalLinkTransfer,
  revokeBrowserLocalLinkDevice,
  saveBrowserClipboardItemNote,
  saveBrowserClipboardSnippet,
  searchBrowserLocalSemanticItems,
  searchBrowserQuickPasteItems,
  sendBrowserClipboardItemToLocalLinkDevice,
  toggleBrowserPin,
  updateBrowserAutomationPreferences,
  updateBrowserCapturePreferences,
  updateBrowserClipboardSmartCollection,
  updateBrowserLocalLinkPreferences,
  updateBrowserLocalTextAction,
} from "./browserRepository";
import type {
  ActionAuditRecord,
  AutomationPreferences,
  CapturePreferences,
  CaptureStatusEvent,
  ClipboardActivationMode,
  ClipboardCleanupPreview,
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
  ClipboardListOptions,
  ClipboardSmartCollectionInput,
  ClipboardUseResult,
  CloudActionPreview,
  ContextEnrichment,
  CreateLocalTextAction,
  LocalDiagnosticEventInput,
  LocalDiagnosticRecordResult,
  LocalDiagnosticsExport,
  LocalDiagnosticsSummary,
  LocalLinkAcceptAction,
  LocalLinkDevice,
  LocalLinkDiagnostics,
  LocalLinkPairing,
  LocalLinkPreferences,
  LocalLinkTransfer,
  LocalLinkTrustDuration,
  LocalSemanticSearchResult,
  LocalTextAction,
  LocalTextActionPreview,
  NativeLocalLinkReadinessSnapshot,
  PastePermissionStatus,
  QuickPasteSearchResult,
  RecycleBinItem,
  TextActionExecution,
} from "./types";

export async function listClipboardItems(options: ClipboardListOptions) {
  if (!isDesktopRuntime()) {
    return listBrowserItems(options);
  }

  return invoke<ClipboardItem[]>("list_clipboard_items", {
    query: options.query,
    pinnedOnly: options.pinnedOnly,
    limit: options.limit ?? 100,
    offset: options.offset ?? 0,
    filters: options.filters ?? null,
  });
}

export async function getClipboardItem(id: string): Promise<ClipboardItem | null> {
  if (!isDesktopRuntime()) return getBrowserItem(id);
  return invoke<ClipboardItem | null>("get_clipboard_item", { id });
}

export async function getClipboardFilterOptions() {
  if (!isDesktopRuntime()) {
    return getBrowserClipboardFilterOptions();
  }
  return invoke<ClipboardFilterOptions>("get_clipboard_filter_options");
}

export async function extractClipboardImageText(id: string, requestId: string) {
  if (!isDesktopRuntime()) return extractBrowserClipboardImageText(id, requestId);
  return invoke<ClipboardImageTextExtractionResult>("extract_clipboard_image_text", {
    id,
    requestId,
  });
}

export async function cancelClipboardImageTextExtraction(requestId: string) {
  if (!isDesktopRuntime()) return cancelBrowserClipboardImageTextExtraction(requestId);
  return invoke<boolean>("cancel_clipboard_image_text_extraction", { requestId });
}

export async function getClipboardImageText(id: string) {
  if (!isDesktopRuntime()) return getBrowserClipboardImageText(id);
  return invoke<ClipboardImageTextExtraction | null>("get_clipboard_image_text", { id });
}

export async function deleteClipboardImageText(id: string) {
  if (!isDesktopRuntime()) return deleteBrowserClipboardImageText(id);
  return invoke<boolean>("delete_clipboard_image_text", { id });
}

export async function listClipboardCollections() {
  if (!isDesktopRuntime()) {
    return listBrowserClipboardCollections();
  }
  return invoke<ClipboardCollection[]>("list_clipboard_collections");
}

export async function createClipboardSmartCollection(input: ClipboardSmartCollectionInput) {
  if (!isDesktopRuntime()) return createBrowserClipboardSmartCollection(input);
  return invoke<ClipboardCollection>("create_clipboard_smart_collection", { input });
}

export async function updateClipboardSmartCollection(
  id: string,
  input: ClipboardSmartCollectionInput,
) {
  if (!isDesktopRuntime()) return updateBrowserClipboardSmartCollection(id, input);
  return invoke<ClipboardCollection>("update_clipboard_smart_collection", { id, input });
}

export async function deleteClipboardSmartCollection(id: string) {
  if (!isDesktopRuntime()) return deleteBrowserClipboardSmartCollection(id);
  return invoke<boolean>("delete_clipboard_smart_collection", { id });
}

export async function getClipboardItemNote(id: string) {
  if (!isDesktopRuntime()) return getBrowserClipboardItemNote(id);
  return invoke<ClipboardItemNote | null>("get_clipboard_item_note", { id });
}

export async function saveClipboardItemNote(id: string, text: string) {
  if (!isDesktopRuntime()) return saveBrowserClipboardItemNote(id, text);
  return invoke<ClipboardItemNoteMutationResult>("save_clipboard_item_note", { id, text });
}

export async function deleteClipboardItemNote(id: string) {
  if (!isDesktopRuntime()) return deleteBrowserClipboardItemNote(id);
  return invoke<boolean>("delete_clipboard_item_note", { id });
}

/**
 * Searches only the local history with Quick Paste's explicit lexical tiers.
 * It has no network, embedding, or Labs dependency.
 */
export async function searchQuickPasteItems(options: ClipboardListOptions) {
  if (!isDesktopRuntime()) {
    return searchBrowserQuickPasteItems(options);
  }

  return invoke<QuickPasteSearchResult[]>("search_quick_paste_items", {
    query: options.query,
    pinnedOnly: options.pinnedOnly,
    limit: options.limit ?? 9,
    offset: options.offset ?? 0,
  });
}

/**
 * Uses only ClipRiva's local lexical fallback. The desktop command has no
 * network or embedding-provider dependency; browser previews mirror it.
 */
export async function searchLocalSemanticClipboardItems(options: ClipboardListOptions) {
  if (!isDesktopRuntime()) {
    return searchBrowserLocalSemanticItems(options);
  }

  return invoke<LocalSemanticSearchResult[]>("search_local_semantic_clipboard_items", {
    query: options.query,
    pinnedOnly: options.pinnedOnly,
    limit: options.limit ?? 100,
    offset: options.offset ?? 0,
  });
}

export async function copyClipboardItem(id: string) {
  if (!isDesktopRuntime()) {
    return copyBrowserItem(id);
  }

  return invoke<ClipboardItem>("copy_clipboard_item", { id });
}

export async function activateClipboardItem(id: string, mode: ClipboardActivationMode = "copy") {
  if (!isDesktopRuntime()) {
    return activateBrowserItem(id, mode);
  }

  return invoke<ClipboardUseResult>("use_clipboard_item", { id, mode });
}

export async function getPastePermissionStatus() {
  if (!isDesktopRuntime()) {
    return getBrowserPastePermissionStatus();
  }

  return invoke<PastePermissionStatus>("get_paste_permission_status");
}

export async function requestPastePermission() {
  if (!isDesktopRuntime()) {
    return requestBrowserPastePermission();
  }

  return invoke<PastePermissionStatus>("request_paste_permission");
}

export async function openPastePermissionSettings() {
  if (!isDesktopRuntime()) {
    return openBrowserPastePermissionSettings();
  }

  return invoke<void>("open_paste_permission_settings");
}

export async function toggleClipboardPin(id: string) {
  if (!isDesktopRuntime()) {
    return toggleBrowserPin(id);
  }

  return invoke<ClipboardItem>("toggle_clipboard_pin", { id });
}

export async function replaceClipboardTags(id: string, tags: string[]) {
  if (!isDesktopRuntime()) {
    return replaceBrowserClipboardTags(id, tags);
  }

  return invoke<ClipboardItem>("replace_clipboard_tags", { id, tags });
}

export async function saveClipboardSnippet(id: string, collections?: string[]) {
  if (!isDesktopRuntime()) {
    return saveBrowserClipboardSnippet(id, collections);
  }
  return invoke<ClipboardItem>("save_clipboard_snippet", {
    id,
    collections: collections ?? null,
  });
}

export async function removeClipboardSnippet(id: string) {
  if (!isDesktopRuntime()) {
    return removeBrowserClipboardSnippet(id);
  }
  return invoke<ClipboardItem>("remove_clipboard_snippet", { id });
}

export async function renameClipboardCollection(from: string, to: string) {
  if (!isDesktopRuntime()) {
    return renameBrowserClipboardCollection(from, to);
  }
  return invoke<ClipboardCollectionMutationResult>("rename_clipboard_collection", { from, to });
}

export async function deleteClipboardCollection(name: string) {
  if (!isDesktopRuntime()) {
    return deleteBrowserClipboardCollection(name);
  }
  return invoke<ClipboardCollectionMutationResult>("delete_clipboard_collection", { name });
}

export async function deleteClipboardItem(id: string) {
  if (!isDesktopRuntime()) {
    return deleteBrowserItem(id);
  }

  return invoke<void>("delete_clipboard_item", { id });
}

export async function clearClipboardHistory() {
  if (!isDesktopRuntime()) {
    return clearBrowserHistory();
  }

  return invoke<void>("clear_clipboard_history");
}

export async function previewClipboardCleanup(request: ClipboardCleanupRequest) {
  if (!isDesktopRuntime()) {
    return previewBrowserClipboardCleanup(request);
  }

  return invoke<ClipboardCleanupPreview>("preview_clipboard_cleanup", { request });
}

export async function moveClipboardItemsToRecycleBin(request: ClipboardCleanupRequest) {
  if (!isDesktopRuntime()) {
    return moveBrowserItemsToRecycleBin(request);
  }

  return invoke<ClipboardCleanupResult>("move_clipboard_items_to_recycle_bin", { request });
}

export async function listRecycleBinItems(limit = 100) {
  if (!isDesktopRuntime()) {
    return listBrowserRecycleBinItems(limit);
  }

  return invoke<RecycleBinItem[]>("list_recycle_bin_items", { limit });
}

export async function restoreClipboardItemFromRecycleBin(id: string) {
  if (!isDesktopRuntime()) {
    return restoreBrowserRecycleBinItem(id);
  }

  return invoke<ClipboardItem>("restore_clipboard_item_from_recycle_bin", { id });
}

export async function permanentlyDeleteRecycleBinItems(itemIds: string[]) {
  if (!isDesktopRuntime()) {
    return permanentlyDeleteBrowserRecycleBinItems(itemIds);
  }

  return invoke<number>("permanently_delete_recycle_bin_items", { itemIds });
}

export async function emptyClipboardRecycleBin() {
  if (!isDesktopRuntime()) {
    return emptyBrowserRecycleBin();
  }

  return invoke<number>("empty_clipboard_recycle_bin");
}

export async function getCapturePreferences() {
  if (!isDesktopRuntime()) {
    return getBrowserCapturePreferences();
  }

  return invoke<CapturePreferences>("get_capture_preferences");
}

export async function updateCapturePreferences(preferences: CapturePreferences) {
  if (!isDesktopRuntime()) {
    return updateBrowserCapturePreferences(preferences);
  }

  return invoke<CapturePreferences>("update_capture_preferences", { preferences });
}

export async function getAutomationPreferences() {
  if (!isDesktopRuntime()) return getBrowserAutomationPreferences();
  return invoke<AutomationPreferences>("get_automation_preferences");
}

export async function updateAutomationPreferences(preferences: AutomationPreferences) {
  if (!isDesktopRuntime()) return updateBrowserAutomationPreferences(preferences);
  return invoke<AutomationPreferences>("update_automation_preferences", { preferences });
}

export async function resumeClipboardCapture() {
  if (!isDesktopRuntime()) {
    return resumeBrowserCapture();
  }

  return invoke<CapturePreferences>("resume_clipboard_capture");
}

export async function listRecentCaptureStatusEvents() {
  if (!isDesktopRuntime()) {
    return listBrowserCaptureStatusEvents();
  }

  return invoke<CaptureStatusEvent[]>("list_recent_capture_status_events");
}

export async function clearCaptureStatusEvents() {
  if (!isDesktopRuntime()) {
    return clearBrowserCaptureStatusEvents();
  }

  return invoke<void>("clear_capture_status_events");
}

/**
 * Records a locally opt-in diagnostic event. The typed payload has only an
 * event and outcome enum; clipboard content and identifying metadata are not
 * accepted by either runtime.
 */
export async function recordLocalDiagnosticEvent(event: LocalDiagnosticEventInput) {
  if (!isDesktopRuntime()) {
    return recordBrowserLocalDiagnosticEvent(event);
  }

  return invoke<LocalDiagnosticRecordResult>("record_local_diagnostic_event", { event });
}

export async function getLocalDiagnosticsSummary() {
  if (!isDesktopRuntime()) {
    return getBrowserLocalDiagnosticsSummary();
  }

  return invoke<LocalDiagnosticsSummary>("get_local_diagnostics_summary");
}

export async function clearLocalDiagnostics() {
  if (!isDesktopRuntime()) {
    return clearBrowserLocalDiagnostics();
  }

  return invoke<void>("clear_local_diagnostics");
}

export async function exportLocalDiagnostics() {
  if (!isDesktopRuntime()) {
    return exportBrowserLocalDiagnostics();
  }

  return invoke<LocalDiagnosticsExport>("export_local_diagnostics");
}

export async function getClipboardEnrichment(id: string) {
  if (!isDesktopRuntime()) {
    return getBrowserContextEnrichment(id);
  }

  return invoke<ContextEnrichment>("get_clipboard_enrichment", { id });
}

export async function listLocalTextActions() {
  if (!isDesktopRuntime()) {
    return listBrowserLocalTextActions();
  }

  return invoke<LocalTextAction[]>("list_local_text_actions");
}

export async function createLocalTextAction(draft: CreateLocalTextAction) {
  if (!isDesktopRuntime()) {
    return createBrowserLocalTextAction(draft);
  }

  return invoke<LocalTextAction>("create_local_text_action", { draft });
}

export async function updateLocalTextAction(id: string, draft: CreateLocalTextAction) {
  if (!isDesktopRuntime()) return updateBrowserLocalTextAction(id, draft);
  return invoke<LocalTextAction>("update_local_text_action", { id, draft });
}

export async function deleteLocalTextAction(id: string) {
  if (!isDesktopRuntime()) return deleteBrowserLocalTextAction(id);
  return invoke<boolean>("delete_local_text_action", { id });
}

export async function previewLocalTextAction(actionId: string, clipboardItemId: string) {
  if (!isDesktopRuntime()) return previewBrowserLocalTextAction(actionId, clipboardItemId);
  return invoke<LocalTextActionPreview>("preview_local_text_action", { actionId, clipboardItemId });
}

export async function confirmLocalTextAction(preview: LocalTextActionPreview) {
  if (!isDesktopRuntime()) return confirmBrowserLocalTextAction(preview);
  return invoke<TextActionExecution>("confirm_local_text_action", { preview });
}

export async function listActionAudit(clipboardItemId: string | null, limit = 8) {
  if (!isDesktopRuntime()) {
    return listBrowserActionAudit(clipboardItemId, limit);
  }

  return invoke<ActionAuditRecord[]>("list_action_audit", { clipboardItemId, limit });
}

export async function previewCloudAction(clipboardItemId: string) {
  if (!isDesktopRuntime()) {
    return previewBrowserCloudAction(clipboardItemId);
  }

  return invoke<CloudActionPreview>("preview_cloud_action", { clipboardItemId });
}

/**
 * Local Link commands address only a nearby or paired Mac. They never accept
 * an account, URL, or cloud delivery queue as a destination.
 */
export async function getLocalLinkPreferences() {
  if (!isDesktopRuntime()) return getBrowserLocalLinkPreferences();
  const preferences = await invoke<LocalLinkPreferences>("get_local_link_preferences");
  if (!preferences.enabled) {
    return { ...preferences, identityFingerprint: null, protocolVersion: null };
  }
  const identityFingerprint = await invoke<string>("get_local_link_identity_fingerprint");
  return { ...preferences, identityFingerprint, protocolVersion: "v2" };
}

export async function updateLocalLinkPreferences(preferences: LocalLinkPreferences) {
  if (!isDesktopRuntime()) return updateBrowserLocalLinkPreferences(preferences);
  await invoke<LocalLinkPreferences>("update_local_link_preferences", {
    preferences: {
      enabled: preferences.enabled,
      discoveryEnabled: preferences.discoveryEnabled,
      deviceName: preferences.deviceName,
    },
  });
  return getLocalLinkPreferences();
}

export async function listLocalLinkDevices() {
  if (!isDesktopRuntime()) return listBrowserLocalLinkDevices();
  return invoke<LocalLinkDevice[]>("list_local_link_devices");
}

export async function getLocalLinkReadinessSnapshot(): Promise<NativeLocalLinkReadinessSnapshot> {
  if (isDesktopRuntime()) {
    return invoke<NativeLocalLinkReadinessSnapshot>("get_local_link_readiness_snapshot");
  }
  const preferences = await getBrowserLocalLinkPreferences();
  const devices = preferences.enabled ? await listBrowserLocalLinkDevices() : [];
  return buildBrowserReadiness(preferences, devices);
}

export async function buildLocalLinkDiagnostics(): Promise<LocalLinkDiagnostics> {
  if (isDesktopRuntime()) {
    return invoke<LocalLinkDiagnostics>("build_local_link_diagnostics");
  }
  const preferences = await getBrowserLocalLinkPreferences();
  const devices = await listBrowserLocalLinkDevices();
  const transfers = (await listBrowserLocalLinkTransfers()).slice(0, 50);
  const peerRefs = new Map<string, string>();
  const peerRef = (deviceId: string) => {
    const existing = peerRefs.get(deviceId);
    if (existing) return existing;
    const value = `preview-peer-${peerRefs.size + 1}`;
    peerRefs.set(deviceId, value);
    return value;
  };
  const diagnosticTransfers = transfers.map((transfer) => ({
    peerRef: peerRef(transfer.deviceId),
    direction: transfer.direction,
    status: transfer.status,
    failureReason: transfer.failureReason ?? null,
    recoveryAction: transfer.recoveryAction ?? null,
    createdAt: transfer.createdAt,
    updatedAt: transfer.updatedAt,
    completedAt: transfer.completedAt ?? null,
    durationMs: Math.max(0, Date.parse(transfer.updatedAt) - Date.parse(transfer.createdAt)),
  }));
  const diagnosticPeers = devices
    .slice(0, Math.max(0, 50 - diagnosticTransfers.length))
    .map((device) => ({
      peerRef: peerRef(device.deviceId),
      trustStatus: device.trustStatus,
      trustDuration: device.trustDuration ?? ("thirtyDays" as const),
      availability: device.online ? ("online" as const) : ("offline" as const),
    }));
  return {
    schemaVersion: 1,
    appVersion: "browser-preview",
    osFamily: "other",
    architecture: "other",
    generatedAt: new Date().toISOString(),
    localLinkEnabled: preferences.enabled,
    discoveryEnabled: preferences.discoveryEnabled,
    readiness: buildBrowserReadiness(preferences, devices),
    recordCount: diagnosticPeers.length + diagnosticTransfers.length,
    peers: diagnosticPeers,
    transfers: diagnosticTransfers,
  };
}

/**
 * The browser fixture predates the native pairing DTO and still asks its
 * adapter for a code. Keep that compatibility detail at the boundary: the UI
 * only ever reads the backend-owned SAS through `getLocalLinkPairing`.
 */
const browserFixturePairingCode = "482915";
let browserFixturePairing: LocalLinkPairing | null = null;

export async function beginLocalLinkPairing(request: {
  deviceId: string;
  displayName: string;
}): Promise<LocalLinkPairing> {
  if (!isDesktopRuntime()) {
    const device = await beginBrowserLocalLinkPairing(
      request.deviceId,
      request.displayName,
      browserFixturePairingCode,
    );
    const preferences = await getBrowserLocalLinkPreferences();
    browserFixturePairing = {
      device,
      verificationCode: browserFixturePairingCode,
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      localFingerprint: preferences.identityFingerprint ?? null,
      peerFingerprint: device.publicKeyFingerprint ?? null,
      localConfirmed: false,
      peerConfirmed: false,
    };
    return browserFixturePairing;
  }
  await invoke<LocalLinkDevice>("begin_local_link_pairing", { request });
  return getLocalLinkPairingOrThrow();
}

/** Reads the active backend-owned pairing flow, including its SAS and expiry. */
export async function getLocalLinkPairing(): Promise<LocalLinkPairing | null> {
  if (!isDesktopRuntime()) return browserFixturePairing;
  return invoke<LocalLinkPairing | null>("get_local_link_pairing");
}

async function getLocalLinkPairingOrThrow(): Promise<LocalLinkPairing> {
  const pairing = await getLocalLinkPairing();
  if (!pairing) throw new Error("The pairing request was not available.");
  return pairing;
}

export async function confirmLocalLinkPairing(
  deviceId: string,
  trustDuration: LocalLinkTrustDuration = "thirtyDays",
) {
  if (!isDesktopRuntime()) {
    const device = await confirmBrowserLocalLinkPairing(
      deviceId,
      browserFixturePairingCode,
      trustDuration,
    );
    browserFixturePairing = null;
    return device;
  }
  return invoke<LocalLinkDevice>("confirm_local_link_pairing", {
    request: { deviceId, trustDuration },
  });
}

export async function cancelLocalLinkPairing(deviceId: string) {
  if (!isDesktopRuntime()) {
    browserFixturePairing = null;
    return cancelBrowserLocalLinkPairing(deviceId);
  }
  return invoke<void>("cancel_local_link_pairing", { deviceId });
}

export async function revokeLocalLinkDevice(deviceId: string) {
  if (!isDesktopRuntime()) return revokeBrowserLocalLinkDevice(deviceId);
  return invoke<LocalLinkDevice>("revoke_local_link_device", { deviceId });
}

export async function sendClipboardItemToLocalLinkDevice(
  clipboardItemId: string,
  deviceId: string,
) {
  if (!isDesktopRuntime()) {
    return sendBrowserClipboardItemToLocalLinkDevice(clipboardItemId, deviceId);
  }
  return invoke<LocalLinkTransfer>("send_clipboard_item_to_device", { clipboardItemId, deviceId });
}

export async function listLocalLinkTransfers(limit = 30) {
  if (!isDesktopRuntime()) return listBrowserLocalLinkTransfers();
  return invoke<LocalLinkTransfer[]>("list_local_link_transfers", { limit });
}

/** Records the receiver opening Incoming Center without returning plaintext. */
export async function markLocalLinkTransferViewed(transferId: string) {
  if (!isDesktopRuntime()) return markBrowserLocalLinkTransferViewed(transferId);
  return invoke<LocalLinkTransfer>("mark_local_link_transfer_viewed", { transferId });
}

/** The native side applies the destination action without exposing its payload. */
export async function acceptLocalLinkTransfer(transferId: string, action: LocalLinkAcceptAction) {
  if (!isDesktopRuntime()) return acceptBrowserLocalLinkTransfer(transferId, action);
  return invoke<LocalLinkTransfer>("accept_local_link_transfer", { transferId, action });
}

export async function rejectLocalLinkTransfer(transferId: string) {
  if (!isDesktopRuntime()) return rejectBrowserLocalLinkTransfer(transferId);
  return invoke<LocalLinkTransfer>("reject_local_link_transfer", { transferId });
}

export async function cancelLocalLinkTransfer(transferId: string) {
  if (!isDesktopRuntime()) return cancelBrowserLocalLinkTransfer(transferId);
  return invoke<LocalLinkTransfer>("cancel_local_link_transfer", { transferId });
}

/** Opens native secure presentation without returning plaintext to the WebView. */
export async function revealLocalLinkTransfer(transferId: string) {
  if (!isDesktopRuntime()) return revealBrowserLocalLinkTransfer(transferId);
  return invoke<void>("reveal_local_link_transfer", { transferId });
}

export async function resetLocalLinkIdentity() {
  if (!isDesktopRuntime()) return resetBrowserLocalLinkIdentity();
  return invoke<void>("reset_local_link_identity");
}

export async function clearLocalLinkTransfers() {
  if (!isDesktopRuntime()) return clearBrowserLocalLinkTransfers();
  return invoke<number>("clear_local_link_transfers");
}

function buildBrowserReadiness(
  preferences: LocalLinkPreferences,
  devices: LocalLinkDevice[],
): NativeLocalLinkReadinessSnapshot {
  const stages: NativeLocalLinkReadinessSnapshot["stages"] = [
    { stage: "localNetwork", state: preferences.enabled ? "passed" : "notChecked" },
    { stage: "enabled", state: preferences.enabled ? "passed" : "blocked" },
    {
      stage: "discovery",
      state: !preferences.enabled
        ? "notChecked"
        : preferences.discoveryEnabled
          ? "passed"
          : "blocked",
    },
    { stage: "trust", state: "notChecked" },
    { stage: "authenticatedPresence", state: "notChecked" },
  ];
  if (!preferences.enabled) {
    return {
      state: "actionRequired",
      stages,
      firstBlocker: "localLinkDisabled",
      primaryAction: "enableLocalLink",
      actionDeviceId: null,
      readyDeviceCount: 0,
      pendingRequestCapacity: 3,
    };
  }
  if (!preferences.discoveryEnabled) {
    return {
      state: "actionRequired",
      stages,
      firstBlocker: "discoveryDisabled",
      primaryAction: "enableDiscovery",
      actionDeviceId: null,
      readyDeviceCount: 0,
      pendingRequestCapacity: 3,
    };
  }
  const trusted = devices.filter((device) => device.trustStatus === "trusted");
  const rePair = devices.find((device) => device.trustStatus === "needsRePairing");
  const trustStage = stages[3];
  if (trustStage) trustStage.state = trusted.length > 0 ? "passed" : "blocked";
  if (trusted.length === 0) {
    return {
      state: "actionRequired",
      stages,
      firstBlocker: rePair ? "trustRequiresRePairing" : "noTrustedDevice",
      primaryAction: rePair ? "rePairDevice" : "pairDevice",
      actionDeviceId: rePair?.deviceId ?? null,
      readyDeviceCount: 0,
      pendingRequestCapacity: 3,
    };
  }
  const readyDeviceCount = trusted.filter((device) => device.online).length;
  const presenceStage = stages[4];
  if (presenceStage) presenceStage.state = readyDeviceCount > 0 ? "passed" : "blocked";
  return {
    state: readyDeviceCount > 0 ? "ready" : "offline",
    stages,
    firstBlocker: readyDeviceCount > 0 ? null : "noAuthenticatedPresence",
    primaryAction: readyDeviceCount > 0 ? null : "refreshDevices",
    actionDeviceId: null,
    readyDeviceCount,
    pendingRequestCapacity: 3,
  };
}
