import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";
import {
  acceptLocalLinkTransfer,
  beginLocalLinkPairing,
  cancelClipboardImageTextExtraction,
  cancelLocalLinkPairing,
  cancelLocalLinkTransfer,
  clearCaptureStatusEvents,
  clearClipboardHistory,
  clearLocalDiagnostics,
  clearLocalLinkTransfers,
  confirmLocalLinkPairing,
  confirmLocalTextAction,
  copyClipboardItem,
  createClipboardSmartCollection,
  createLocalTextAction,
  deleteClipboardCollection,
  deleteClipboardImageText,
  deleteClipboardItem,
  deleteClipboardItemNote,
  deleteClipboardSmartCollection,
  deleteLocalTextAction,
  emptyClipboardRecycleBin,
  exportLocalDiagnostics,
  extractClipboardImageText,
  getAutomationPreferences,
  getCapturePreferences,
  getClipboardEnrichment,
  getClipboardFilterOptions,
  getClipboardImageText,
  getClipboardItemNote,
  getLocalDiagnosticsSummary,
  getLocalLinkPairing,
  getLocalLinkPreferences,
  getPastePermissionStatus,
  listActionAudit,
  listClipboardCollections,
  listClipboardItems,
  listLocalLinkDevices,
  listLocalLinkTransfers,
  listLocalTextActions,
  listRecentCaptureStatusEvents,
  listRecycleBinItems,
  markLocalLinkTransferViewed,
  moveClipboardItemsToRecycleBin,
  openPastePermissionSettings,
  permanentlyDeleteRecycleBinItems,
  previewClipboardCleanup,
  previewLocalTextAction,
  recordLocalDiagnosticEvent,
  rejectLocalLinkTransfer,
  removeClipboardSnippet,
  renameClipboardCollection,
  replaceClipboardTags,
  requestPastePermission,
  resetLocalLinkIdentity,
  restoreClipboardItemFromRecycleBin,
  resumeClipboardCapture,
  revealLocalLinkTransfer,
  revokeLocalLinkDevice,
  saveClipboardItemNote,
  saveClipboardSnippet,
  searchLocalSemanticClipboardItems,
  searchQuickPasteItems,
  sendClipboardItemToLocalLinkDevice,
  toggleClipboardPin,
  updateAutomationPreferences,
  updateCapturePreferences,
  updateClipboardSmartCollection,
  updateLocalLinkPreferences,
  updateLocalTextAction,
} from "./api";
import type {
  AutomationPreferences,
  CapturePreferences,
  ClipboardCleanupRequest,
  ClipboardListOptions,
  ClipboardSmartCollectionInput,
  CreateLocalTextAction,
  LocalDiagnosticEventInput,
  LocalLinkAcceptAction,
  LocalLinkPreferences,
  LocalLinkTrustDuration,
  LocalTextActionPreview,
} from "./types";

const historyKey = ["clipboard-history"] as const;
const clipboardFilterOptionsKey = ["clipboard-filter-options"] as const;
const clipboardCollectionsKey = ["clipboard-collections"] as const;
const clipboardImageTextKey = ["clipboard-image-text"] as const;
const clipboardItemNoteKey = ["clipboard-item-note"] as const;
const cleanupPreviewKey = ["clipboard-cleanup-preview"] as const;
const recycleBinKey = ["clipboard-recycle-bin"] as const;
const capturePreferencesKey = ["capture-preferences"] as const;
const automationPreferencesKey = ["automation-preferences"] as const;
const captureStatusEventsKey = ["capture-status-events"] as const;
const localDiagnosticsKey = ["local-diagnostics"] as const;
const enrichmentKey = ["clipboard-enrichment"] as const;
const localTextActionsKey = ["local-text-actions"] as const;
const actionAuditKey = ["action-audit"] as const;
const localSemanticSearchKey = ["local-semantic-search"] as const;
const quickPasteSearchKey = ["quick-paste-search"] as const;
const pastePermissionKey = ["paste-permission"] as const;
const localLinkPreferencesKey = ["local-link-preferences"] as const;
const localLinkDevicesKey = ["local-link-devices"] as const;
const localLinkPairingKey = ["local-link-pairing"] as const;
const localLinkTransfersKey = ["local-link-transfers"] as const;

export function useClipboardItems(options: ClipboardListOptions) {
  return useQuery({
    queryKey: [...historyKey, options],
    queryFn: () => listClipboardItems(options),
    refetchInterval: 2_000,
  });
}

export function useClipboardFilterOptions(enabled = true) {
  return useQuery({
    queryKey: clipboardFilterOptionsKey,
    queryFn: getClipboardFilterOptions,
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useClipboardCollections(enabled = true) {
  return useQuery({
    queryKey: clipboardCollectionsKey,
    queryFn: listClipboardCollections,
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useClipboardImageText(id: string | null, enabled: boolean) {
  const queryClient = useQueryClient();
  const activeRequestId = useRef<string | null>(null);
  const query = useQuery({
    queryKey: [...clipboardImageTextKey, id],
    queryFn: () => getClipboardImageText(id ?? ""),
    enabled: enabled && Boolean(id),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: [...clipboardImageTextKey, id] });
    void queryClient.invalidateQueries({ queryKey: historyKey });
  };
  const extract = useMutation({
    mutationFn: () => {
      const requestId = crypto.randomUUID();
      activeRequestId.current = requestId;
      return extractClipboardImageText(id ?? "", requestId);
    },
    onSettled: () => {
      activeRequestId.current = null;
    },
    onSuccess: refresh,
  });
  const cancel = useMutation({
    mutationFn: () =>
      activeRequestId.current
        ? cancelClipboardImageTextExtraction(activeRequestId.current)
        : Promise.resolve(false),
  });
  const remove = useMutation({
    mutationFn: () => deleteClipboardImageText(id ?? ""),
    onSuccess: refresh,
  });
  return { query, extract, cancel, remove };
}

export function useClipboardItemNote(id: string | null, enabled = true) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: [...clipboardItemNoteKey, id],
    queryFn: () => getClipboardItemNote(id ?? ""),
    enabled: enabled && Boolean(id),
  });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: [...clipboardItemNoteKey, id] });
    void queryClient.invalidateQueries({ queryKey: historyKey });
  };
  const save = useMutation({
    mutationFn: (text: string) => saveClipboardItemNote(id ?? "", text),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: () => deleteClipboardItemNote(id ?? ""),
    onSuccess: refresh,
  });
  return { query, save, remove };
}

export function useQuickPasteItems(options: ClipboardListOptions, enabled = true) {
  return useQuery({
    queryKey: [...quickPasteSearchKey, options],
    queryFn: () => searchQuickPasteItems(options),
    enabled,
    refetchInterval: enabled ? 5_000 : false,
  });
}

export function useLocalSemanticClipboardItems(options: ClipboardListOptions, enabled: boolean) {
  return useQuery({
    queryKey: [...localSemanticSearchKey, options],
    queryFn: () => searchLocalSemanticClipboardItems(options),
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

export function useClipboardActions() {
  const queryClient = useQueryClient();
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: historyKey });
    void queryClient.invalidateQueries({ queryKey: quickPasteSearchKey });
    void queryClient.invalidateQueries({ queryKey: clipboardFilterOptionsKey });
    void queryClient.invalidateQueries({ queryKey: clipboardCollectionsKey });
  };
  const refreshRecycleBin = () => void queryClient.invalidateQueries({ queryKey: recycleBinKey });
  const cacheCapturePreferences = (preferences: CapturePreferences) =>
    queryClient.setQueryData(capturePreferencesKey, preferences);
  const refreshCaptureStatusEvents = () =>
    queryClient.invalidateQueries({ queryKey: captureStatusEventsKey });

  const copy = useMutation({
    mutationFn: copyClipboardItem,
    onSuccess: refresh,
  });
  const togglePin = useMutation({
    mutationFn: toggleClipboardPin,
    onSuccess: refresh,
  });
  const replaceTags = useMutation({
    mutationFn: ({ id, tags }: { id: string; tags: string[] }) => replaceClipboardTags(id, tags),
    onSuccess: refresh,
  });
  const saveSnippet = useMutation({
    mutationFn: ({ id, collections }: { id: string; collections?: string[] }) =>
      saveClipboardSnippet(id, collections),
    onSuccess: refresh,
  });
  const removeSnippet = useMutation({
    mutationFn: removeClipboardSnippet,
    onSuccess: refresh,
  });
  const renameCollection = useMutation({
    mutationFn: ({ from, to }: { from: string; to: string }) => renameClipboardCollection(from, to),
    onSuccess: refresh,
  });
  const deleteCollection = useMutation({
    mutationFn: deleteClipboardCollection,
    onSuccess: refresh,
  });
  const createSmartCollection = useMutation({
    mutationFn: (input: ClipboardSmartCollectionInput) => createClipboardSmartCollection(input),
    onSuccess: refresh,
  });
  const updateSmartCollection = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ClipboardSmartCollectionInput }) =>
      updateClipboardSmartCollection(id, input),
    onSuccess: refresh,
  });
  const deleteSmartCollection = useMutation({
    mutationFn: deleteClipboardSmartCollection,
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: deleteClipboardItem,
    onSuccess: refresh,
  });
  const clear = useMutation({
    mutationFn: clearClipboardHistory,
    onSuccess: refresh,
  });
  const moveToRecycleBin = useMutation({
    mutationFn: moveClipboardItemsToRecycleBin,
    onSuccess: () => {
      refresh();
      refreshRecycleBin();
    },
  });
  const restoreFromRecycleBin = useMutation({
    mutationFn: restoreClipboardItemFromRecycleBin,
    onSuccess: () => {
      refresh();
      refreshRecycleBin();
    },
  });
  const permanentlyDeleteFromRecycleBin = useMutation({
    mutationFn: permanentlyDeleteRecycleBinItems,
    onSuccess: refreshRecycleBin,
  });
  const emptyRecycleBin = useMutation({
    mutationFn: emptyClipboardRecycleBin,
    onSuccess: refreshRecycleBin,
  });
  const resumeCapture = useMutation({
    mutationFn: resumeClipboardCapture,
    onSuccess: (preferences) => {
      cacheCapturePreferences(preferences);
      refreshCaptureStatusEvents();
    },
  });
  const updatePreferences = useMutation({
    mutationFn: (preferences: CapturePreferences) => updateCapturePreferences(preferences),
    onSuccess: cacheCapturePreferences,
  });
  const clearCaptureStatus = useMutation({
    mutationFn: clearCaptureStatusEvents,
    onSuccess: refreshCaptureStatusEvents,
  });

  return {
    copy,
    togglePin,
    replaceTags,
    saveSnippet,
    removeSnippet,
    renameCollection,
    deleteCollection,
    createSmartCollection,
    updateSmartCollection,
    deleteSmartCollection,
    remove,
    clear,
    moveToRecycleBin,
    restoreFromRecycleBin,
    permanentlyDeleteFromRecycleBin,
    emptyRecycleBin,
    resumeCapture,
    updatePreferences,
    clearCaptureStatus,
  };
}

export function useCapturePreferences(enabled = true) {
  return useQuery({
    queryKey: capturePreferencesKey,
    queryFn: getCapturePreferences,
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

export function useAutomationPreferences(enabled = true) {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: automationPreferencesKey,
    queryFn: getAutomationPreferences,
    enabled,
  });
  const update = useMutation({
    mutationFn: (preferences: AutomationPreferences) => updateAutomationPreferences(preferences),
    onSuccess: (preferences) => queryClient.setQueryData(automationPreferencesKey, preferences),
  });
  return { query, update };
}

export function useRecentCaptureStatusEvents(enabled = true) {
  return useQuery({
    queryKey: captureStatusEventsKey,
    queryFn: listRecentCaptureStatusEvents,
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

export function usePastePermissionStatus(enabled = true) {
  return useQuery({
    queryKey: pastePermissionKey,
    queryFn: getPastePermissionStatus,
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

export function usePastePermissionActions() {
  const queryClient = useQueryClient();
  const request = useMutation({
    mutationFn: requestPastePermission,
    onSuccess: (status) => queryClient.setQueryData(pastePermissionKey, status),
  });
  const openSettings = useMutation({
    mutationFn: openPastePermissionSettings,
  });
  return { request, openSettings };
}

/** Fetches a preview from the same scoped request that will be confirmed. */
export function useClipboardCleanupPreview(request: ClipboardCleanupRequest | null) {
  return useQuery({
    queryKey: [...cleanupPreviewKey, request],
    queryFn: () => previewClipboardCleanup(request as ClipboardCleanupRequest),
    enabled: request !== null,
    staleTime: 0,
  });
}

/** Active only when the local Recycle Bin is open in the workspace. */
export function useRecycleBinItems(enabled: boolean) {
  return useQuery({
    queryKey: recycleBinKey,
    queryFn: () => listRecycleBinItems(),
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

/** Reads only the aggregate, local opt-in diagnostics summary. */
export function useLocalDiagnostics() {
  return useQuery({
    queryKey: localDiagnosticsKey,
    queryFn: getLocalDiagnosticsSummary,
    refetchInterval: 30_000,
  });
}

/**
 * Exposes typed event recording for feature flows plus clear/export controls
 * for Settings. Successful writes refresh the aggregate-only Settings view.
 */
export function useLocalDiagnosticsActions() {
  const queryClient = useQueryClient();
  const refresh = () => queryClient.invalidateQueries({ queryKey: localDiagnosticsKey });
  const record = useMutation({
    mutationFn: (event: LocalDiagnosticEventInput) => recordLocalDiagnosticEvent(event),
    onSuccess: refresh,
  });
  const clear = useMutation({
    mutationFn: clearLocalDiagnostics,
    onSuccess: refresh,
  });
  const exportData = useMutation({ mutationFn: exportLocalDiagnostics });
  return { record, clear, exportData };
}

export function useClipboardEnrichment(id: string | null, enabled = true) {
  return useQuery({
    queryKey: [...enrichmentKey, id],
    queryFn: () => getClipboardEnrichment(id ?? ""),
    enabled: enabled && id !== null,
    staleTime: Infinity,
  });
}

export function useLocalTextActions(enabled = true) {
  return useQuery({
    queryKey: localTextActionsKey,
    queryFn: listLocalTextActions,
    enabled,
    staleTime: Infinity,
  });
}

export function useActionAudit(clipboardItemId: string | null) {
  return useQuery({
    queryKey: [...actionAuditKey, clipboardItemId],
    queryFn: () => listActionAudit(clipboardItemId),
    enabled: clipboardItemId !== null,
    staleTime: 5_000,
  });
}

export function useLocalTextActionMutations(clipboardItemId: string) {
  const queryClient = useQueryClient();
  const refreshActions = () => queryClient.invalidateQueries({ queryKey: localTextActionsKey });
  const refreshAudit = () =>
    queryClient.invalidateQueries({ queryKey: [...actionAuditKey, clipboardItemId] });
  const refreshHistory = () => queryClient.invalidateQueries({ queryKey: historyKey });

  const create = useMutation({
    mutationFn: (draft: CreateLocalTextAction) => createLocalTextAction(draft),
    onSuccess: refreshActions,
  });
  const update = useMutation({
    mutationFn: ({ id, draft }: { id: string; draft: CreateLocalTextAction }) =>
      updateLocalTextAction(id, draft),
    onSuccess: refreshActions,
  });
  const remove = useMutation({
    mutationFn: deleteLocalTextAction,
    onSuccess: refreshActions,
  });
  const preview = useMutation({
    mutationFn: (actionId: string) => previewLocalTextAction(actionId, clipboardItemId),
  });
  const confirm = useMutation({
    mutationFn: (snapshot: LocalTextActionPreview) => confirmLocalTextAction(snapshot),
    onSuccess: () => {
      refreshAudit();
      refreshHistory();
    },
  });
  return { create, update, remove, preview, confirm };
}

/** Local Link starts disabled and every incoming transfer still needs a user action. */
export function useLocalLinkPreferences(enabled = true) {
  return useQuery({
    queryKey: localLinkPreferencesKey,
    queryFn: getLocalLinkPreferences,
    enabled,
    refetchInterval: enabled ? 3_000 : false,
  });
}

export function useLocalLinkDevices(enabled = true) {
  return useQuery({
    queryKey: localLinkDevicesKey,
    queryFn: listLocalLinkDevices,
    enabled,
    refetchInterval: enabled ? 3_000 : false,
  });
}

/** Polls backend-owned pairing state so inbound and outbound flows share one SAS dialog. */
export function useLocalLinkPairing(enabled = true) {
  return useQuery({
    queryKey: localLinkPairingKey,
    queryFn: getLocalLinkPairing,
    enabled,
    refetchInterval: enabled ? 1_500 : false,
  });
}

export function useLocalLinkTransfers(enabled = true) {
  return useQuery({
    queryKey: localLinkTransfersKey,
    queryFn: () => listLocalLinkTransfers(),
    enabled,
    refetchInterval: enabled ? 2_000 : false,
  });
}

/** Mutations invalidate only local device/transfer state, never Core search. */
export function useLocalLinkActions() {
  const queryClient = useQueryClient();
  const refreshPreferences = () =>
    queryClient.invalidateQueries({ queryKey: localLinkPreferencesKey });
  const refreshDevices = () => queryClient.invalidateQueries({ queryKey: localLinkDevicesKey });
  const refreshPairing = () => queryClient.invalidateQueries({ queryKey: localLinkPairingKey });
  const refreshTransfers = () => queryClient.invalidateQueries({ queryKey: localLinkTransfersKey });
  const refreshHistory = () => queryClient.invalidateQueries({ queryKey: historyKey });

  const updatePreferences = useMutation({
    mutationFn: (preferences: LocalLinkPreferences) => updateLocalLinkPreferences(preferences),
    onSuccess: (preferences) => {
      queryClient.setQueryData(localLinkPreferencesKey, preferences);
      refreshDevices();
      refreshPairing();
    },
  });
  const beginPairing = useMutation({
    mutationFn: beginLocalLinkPairing,
    onSuccess: () => {
      refreshDevices();
      refreshPairing();
    },
  });
  const confirmPairing = useMutation({
    mutationFn: ({
      deviceId,
      trustDuration,
    }: {
      deviceId: string;
      trustDuration: LocalLinkTrustDuration;
    }) => confirmLocalLinkPairing(deviceId, trustDuration),
    onSuccess: () => {
      refreshDevices();
      refreshPairing();
    },
  });
  const cancelPairing = useMutation({
    mutationFn: cancelLocalLinkPairing,
    onSuccess: () => {
      refreshDevices();
      refreshPairing();
    },
  });
  const revokeDevice = useMutation({
    mutationFn: revokeLocalLinkDevice,
    onSuccess: refreshDevices,
  });
  const send = useMutation({
    mutationFn: ({ clipboardItemId, deviceId }: { clipboardItemId: string; deviceId: string }) =>
      sendClipboardItemToLocalLinkDevice(clipboardItemId, deviceId),
    onSuccess: refreshTransfers,
  });
  const accept = useMutation({
    mutationFn: ({ transferId, action }: { transferId: string; action: LocalLinkAcceptAction }) =>
      acceptLocalLinkTransfer(transferId, action),
    onSuccess: () => {
      refreshTransfers();
      refreshHistory();
    },
  });
  const reject = useMutation({
    mutationFn: rejectLocalLinkTransfer,
    onSuccess: refreshTransfers,
  });
  const cancelTransfer = useMutation({
    mutationFn: cancelLocalLinkTransfer,
    onSuccess: refreshTransfers,
  });
  const markViewed = useMutation({
    mutationFn: markLocalLinkTransferViewed,
    onSuccess: refreshTransfers,
  });
  const revealTransfer = useMutation({
    mutationFn: revealLocalLinkTransfer,
  });
  const resetIdentity = useMutation({
    mutationFn: resetLocalLinkIdentity,
    onSuccess: () => {
      refreshPreferences();
      refreshDevices();
      refreshTransfers();
    },
  });
  const clearTransfers = useMutation({
    mutationFn: clearLocalLinkTransfers,
    onSuccess: refreshTransfers,
  });
  return {
    updatePreferences,
    beginPairing,
    confirmPairing,
    cancelPairing,
    revokeDevice,
    send,
    accept,
    reject,
    cancelTransfer,
    markViewed,
    revealTransfer,
    resetIdentity,
    clearTransfers,
    refreshPreferences,
    refreshPairing,
  };
}
