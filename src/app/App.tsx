import { PhysicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArchiveRestore,
  Clipboard,
  Command,
  Filter,
  Folder,
  HardDrive,
  History,
  Inbox,
  ListChecks,
  ListPlus,
  MonitorUp,
  MoreHorizontal,
  Pencil,
  Pin,
  Search,
  Settings,
  Shield,
  ShieldCheck,
  Sparkles,
  Waves,
  WifiOff,
  X,
} from "lucide-react";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { CaptureSettingsDialog } from "../features/clipboard/CaptureSettingsDialog";
import { CleanupPreviewDialog } from "../features/clipboard/CleanupPreviewDialog";
import { ClipboardCard } from "../features/clipboard/ClipboardCard";
import { ClipboardInspector } from "../features/clipboard/ClipboardInspector";
import { ClipboardQueuePanel } from "../features/clipboard/ClipboardQueuePanel";
import { useSyncedClipboardQueue } from "../features/clipboard/clipboardQueueSync";
import { useClipboardStackBridge } from "../features/clipboard/clipboardStackBridge";
import { DeleteUndoToast } from "../features/clipboard/DeleteUndoToast";
import { kindLabel } from "../features/clipboard/format";
import { LocalLinkDevicesCenter } from "../features/clipboard/LocalLinkDevicesCenter";
import { LocalLinkIncomingCenter } from "../features/clipboard/LocalLinkIncomingCenter";
import {
  normalizedLocalLinkStatus,
  presentOperationalLocalLinkDevice,
} from "../features/clipboard/LocalLinkPresentation";
import { LocalLinkReceiveCard } from "../features/clipboard/LocalLinkReceiveCard";
import { LocalLinkSendDialog } from "../features/clipboard/LocalLinkSendDialog";
import { LocalLinkTransfersPopover } from "../features/clipboard/LocalLinkTransfersPopover";
import { QuickPasteOverlay } from "../features/clipboard/QuickPasteOverlay";
import {
  useAutomationPreferences,
  useCapturePreferences,
  useClipboardActions,
  useClipboardCleanupPreview,
  useClipboardFilterOptions,
  useClipboardItems,
  useLocalDiagnostics,
  useLocalDiagnosticsActions,
  useLocalLinkDevices,
  useLocalLinkPreferences,
  useLocalLinkTransfers,
  usePastePermissionActions,
  usePastePermissionStatus,
  useRecentCaptureStatusEvents,
  useRecycleBinItems,
} from "../features/clipboard/queries";
import { RecycleBinDialog } from "../features/clipboard/RecycleBinDialog";
import { SmartCollectionDialog } from "../features/clipboard/SmartCollectionDialog";
import type {
  CapturePreferences,
  ClipboardCleanupRequest,
  ClipboardItem,
  ClipboardItemKind,
  ClipboardSmartCollectionRule,
  ClipboardTimeFilter,
} from "../features/clipboard/types";
import { isDesktopRuntime } from "../lib/runtime";
import { isQuickPasteWindow } from "./windowMode";
import {
  initialWorkspaceUiState,
  reduceWorkspaceUiState,
  type WorkspaceUiAction,
} from "./workspaceState";

/**
 * A small frontend bridge until first-value completion becomes a native
 * CapturePreferences field. It keeps the teaching state shared between the
 * main window and Quick Paste without changing the persisted Rust schema.
 */
export const FIRST_VALUE_GUIDE_PENDING_KEY = "clipriva.first-value-guide-pending";
export const FIRST_VALUE_COMPLETED_KEY = "clipriva.first-value-completed";
export const MAIN_WINDOW_SIZE_KEY = "clipriva.main-window-size";
export const PRIVACY_COVER_KEY = "clipriva.privacy-cover-enabled";
export const PRIVACY_COVER_CHANGED_EVENT = "clipriva:privacy-cover-changed";

const MAIN_WINDOW_MINIMUM = { width: 720, height: 480 };

const clipboardKinds: ClipboardItemKind[] = [
  "text",
  "code",
  "command",
  "url",
  "color",
  "image",
  "richText",
  "file",
];

const timeFilterLabel: Record<Exclude<ClipboardTimeFilter, "all">, string> = {
  past24Hours: "24 hours",
  past7Days: "7 days",
  past30Days: "30 days",
};

export function App() {
  const quickPasteWindow = isQuickPasteWindow();
  const [privacyCoverEnabled, setPrivacyCoverEnabled] = useState(() =>
    readStorageFlag(PRIVACY_COVER_KEY),
  );

  useEffect(() => {
    document.documentElement.dataset.windowMode = quickPasteWindow ? "quick-paste" : "main";
    return () => {
      delete document.documentElement.dataset.windowMode;
    };
  }, [quickPasteWindow]);

  useEffect(() => {
    const syncPrivacyCover = () => setPrivacyCoverEnabled(readStorageFlag(PRIVACY_COVER_KEY));
    window.addEventListener("storage", syncPrivacyCover);
    window.addEventListener(PRIVACY_COVER_CHANGED_EVENT, syncPrivacyCover);
    return () => {
      window.removeEventListener("storage", syncPrivacyCover);
      window.removeEventListener(PRIVACY_COVER_CHANGED_EVENT, syncPrivacyCover);
    };
  }, []);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    void getCurrentWindow()
      .setContentProtected(privacyCoverEnabled)
      .catch(() => undefined);
  }, [privacyCoverEnabled]);

  const setPrivacyCover = (enabled: boolean) => {
    if (enabled) {
      window.localStorage.setItem(PRIVACY_COVER_KEY, "true");
    } else {
      window.localStorage.removeItem(PRIVACY_COVER_KEY);
    }
    setPrivacyCoverEnabled(enabled);
    window.dispatchEvent(new CustomEvent(PRIVACY_COVER_CHANGED_EVENT));
  };

  if (privacyCoverEnabled) {
    return <PrivacyCoverScreen onReveal={() => setPrivacyCover(false)} />;
  }

  if (quickPasteWindow) {
    return <QuickPasteOverlay />;
  }

  return <ClipboardWorkspace onEnablePrivacyCover={() => setPrivacyCover(true)} />;
}

function ClipboardWorkspace({ onEnablePrivacyCover }: { onEnablePrivacyCover: () => void }) {
  const [workspaceUi, dispatchWorkspaceUiRaw] = useReducer(
    reduceWorkspaceUiState,
    initialWorkspaceUiState,
  );
  const workspaceView = workspaceUi.view;
  const workspaceSurface = workspaceUi.surface;
  const workspaceSurfaceRef = useRef(workspaceSurface);
  workspaceSurfaceRef.current = workspaceSurface;
  const detailDirtyRef = useRef(false);
  const [pendingDetailNavigation, setPendingDetailNavigation] = useState<
    { kind: "action"; action: WorkspaceUiAction } | { kind: "item"; id: string } | null
  >(null);
  const [detailItemId, setDetailItemId] = useState<string | null>(null);
  const [detailItemSnapshot, setDetailItemSnapshot] = useState<ClipboardItem | null>(null);
  const [sendItemSnapshot, setSendItemSnapshot] = useState<ClipboardItem | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const detailReturnRef = useRef<HTMLElement | null>(null);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const restoreDetailFocus = useCallback(() => {
    window.requestAnimationFrame(() => {
      if (detailReturnRef.current?.isConnected) detailReturnRef.current.focus();
      else listRef.current?.focus();
    });
  }, []);
  const keepDetailEditing = useCallback(() => {
    setPendingDetailNavigation(null);
    setSendItemSnapshot(null);
    window.requestAnimationFrame(() => {
      document
        .querySelector<HTMLTextAreaElement>(".stable-inspector .clip-note-editor textarea")
        ?.focus();
    });
  }, []);
  const dispatchWorkspaceUi = useCallback(
    (action: WorkspaceUiAction) => {
      const leavingDetails =
        workspaceSurfaceRef.current.kind === "details" &&
        !(action.type === "openSurface" && action.surface.kind === "details");
      if (leavingDetails && detailDirtyRef.current) {
        setPendingDetailNavigation({ kind: "action", action });
        return;
      }
      dispatchWorkspaceUiRaw(action);
      if (leavingDetails && action.type === "closeSurface") {
        restoreDetailFocus();
      }
    },
    [restoreDetailFocus],
  );
  useEffect(() => {
    if (!pendingDetailNavigation) return;
    const frame = window.requestAnimationFrame(() => keepEditingRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [pendingDetailNavigation]);
  const [query, setQuery] = useState("");
  const pinnedOnly = workspaceView === "pinned";
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [kindFilters, setKindFilters] = useState<ClipboardItemKind[]>([]);
  const [sourceAppFilter, setSourceAppFilter] = useState("");
  const [tagFilter, setTagFilter] = useState("");
  const [smartCollectionId, setSmartCollectionId] = useState<string | null>(null);
  const [smartCollectionDialogOpen, setSmartCollectionDialogOpen] = useState(false);
  const [smartCollectionEditingId, setSmartCollectionEditingId] = useState<string | null>(null);
  const [pinFilter, setPinFilter] = useState<"all" | "pinned" | "unpinned">("all");
  const [timeFilter, setTimeFilter] = useState<ClipboardTimeFilter>("all");
  const [recentlyUsedOnly, setRecentlyUsedOnly] = useState(false);
  const [localLinkOnly, setLocalLinkOnly] = useState(false);
  const [bulkMode, setBulkMode] = useState(false);
  const [queueOpen, setQueueOpen] = useState(false);
  const [queueFailure, setQueueFailure] = useState<string | null>(null);
  const [clipboardQueue, dispatchClipboardQueue] = useSyncedClipboardQueue();
  const clipboardStack = useClipboardStackBridge(dispatchClipboardQueue);
  const [bulkSelectedIds, setBulkSelectedIds] = useState<Set<string>>(new Set());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [productToolsOpen, setProductToolsOpen] = useState(false);
  const [cleanupMode, setCleanupMode] = useState<"closed" | "menu">("closed");
  const [undoDelete, setUndoDelete] = useState<{ id: string; label: string } | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [copyNotice, setCopyNotice] = useState<string | null>(null);
  const [copyErrorItemId, setCopyErrorItemId] = useState<string | null>(null);
  const [firstValueGuideArmed, setFirstValueGuideArmed] = useState(() =>
    readStorageFlag(FIRST_VALUE_GUIDE_PENDING_KEY),
  );
  const [firstValueCompleted, setFirstValueCompleted] = useState(() =>
    readStorageFlag(FIRST_VALUE_COMPLETED_KEY),
  );
  const cleanupRequest = workspaceSurface.kind === "cleanup" ? workspaceSurface.request : null;
  const recycleBinOpen = workspaceSurface.kind === "recycleBin";
  const searchRef = useRef<HTMLInputElement>(null);
  const ftsItemsQuery = useClipboardItems({
    query,
    pinnedOnly,
    filters: {
      kinds: kindFilters,
      sourceApp: sourceAppFilter || null,
      collection: tagFilter || null,
      smartCollectionId,
      pinFilter: pinnedOnly ? "pinned" : pinFilter,
      timeFilter,
      recentlyUsedOnly,
      localLinkOnly,
    },
  });
  const historyInventoryQuery = useClipboardItems({ query: "", pinnedOnly: false });
  const clipboardFilterOptionsQuery = useClipboardFilterOptions(
    workspaceView !== "localLinkPreview",
  );
  const preferencesQuery = useCapturePreferences();
  const automationPreferences = useAutomationPreferences();
  const pastePermissionQuery = usePastePermissionStatus();
  const pastePermissionActions = usePastePermissionActions();
  const captureStatusEventsQuery = useRecentCaptureStatusEvents();
  const cleanupPreviewQuery = useClipboardCleanupPreview(cleanupRequest);
  const recycleBinQuery = useRecycleBinItems(recycleBinOpen);
  const localDiagnosticsQuery = useLocalDiagnostics();
  const localDiagnosticsActions = useLocalDiagnosticsActions();
  const localLinkPreferencesQuery = useLocalLinkPreferences(true);
  const localLinkDevicesQuery = useLocalLinkDevices(true);
  const localLinkTransfersQuery = useLocalLinkTransfers(true);
  const labsEnabled = preferencesQuery.data?.labsEnabled ?? false;
  const actions = useClipboardActions();
  const items = ftsItemsQuery.data ?? [];
  const operationalLocalLinkDevices = (localLinkDevicesQuery.data ?? []).map((device) => ({
    device,
    presentation: presentOperationalLocalLinkDevice(localLinkPreferencesQuery.data, device),
  }));
  const onlineLocalLinkDevices = operationalLocalLinkDevices.filter(
    ({ presentation }) => presentation.sendable,
  ).length;
  const localLinkAttentionCount = (localLinkDevicesQuery.data ?? []).filter(
    (device) => device.trustStatus === "needsRePairing" || device.trustStatus === "pairing",
  ).length;
  const pendingIncomingCount = (localLinkTransfersQuery.data ?? []).filter(
    (transfer) =>
      transfer.direction === "incoming" &&
      ["awaitingReceiver", "viewed"].includes(normalizedLocalLinkStatus(transfer)),
  ).length;
  const sourceApps = clipboardFilterOptionsQuery.data?.sourceApps ?? [];
  const collections = clipboardFilterOptionsQuery.data?.collections ?? [];
  const availableTags = collections
    .filter((collection) => collection.kind === "manual")
    .map((collection) => collection.name);
  const activeSmartCollection = collections.find(
    (collection) => collection.kind === "smart" && collection.id === smartCollectionId,
  );
  const editingSmartCollection = collections.find(
    (collection) => collection.kind === "smart" && collection.id === smartCollectionEditingId,
  );
  const collectionScopeName = activeSmartCollection?.name ?? tagFilter;
  const visibleItems = items;
  const activeFilterCount =
    kindFilters.length +
    Number(Boolean(sourceAppFilter)) +
    Number(Boolean(tagFilter)) +
    Number(pinFilter !== "all") +
    Number(timeFilter !== "all") +
    Number(recentlyUsedOnly) +
    Number(localLinkOnly);
  const smartRuleDraft: ClipboardSmartCollectionRule = {
    kinds: kindFilters,
    sourceApp: sourceAppFilter || null,
    pinFilter: pinnedOnly ? "pinned" : pinFilter,
    timeFilter,
    recentlyUsedOnly,
    localLinkOnly,
  };
  const canSaveSmartCollection =
    kindFilters.length > 0 ||
    Boolean(sourceAppFilter) ||
    pinnedOnly ||
    pinFilter !== "all" ||
    timeFilter !== "all" ||
    recentlyUsedOnly ||
    localLinkOnly;
  const unpinnedCount = clipboardFilterOptionsQuery.data?.unpinnedCount ?? 0;
  const onboardingVisible = Boolean(
    preferencesQuery.data && !preferencesQuery.data.onboardingCompleted,
  );
  const firstValueTaskPending = Boolean(
    preferencesQuery.data?.onboardingCompleted && firstValueGuideArmed && !firstValueCompleted,
  );
  const firstValueGuideVisible = Boolean(
    firstValueTaskPending &&
      workspaceView === "history" &&
      workspaceSurface.kind === "none" &&
      !productToolsOpen,
  );

  useEffect(() => {
    if (!isDesktopRuntime()) return;

    const currentWindow = getCurrentWebviewWindow();
    const savedSize = readSavedMainWindowSize();
    if (savedSize) {
      void currentWindow.setSize(new PhysicalSize(savedSize.width, savedSize.height));
    }
    const resizeListener = currentWindow.onResized(({ payload: size }) => {
      writeSavedMainWindowSize(size.width, size.height);
    });
    const stackShortcutListener = currentWindow.listen("clipriva://clipboard-stack-open-v1", () =>
      setQueueOpen(true),
    );

    return () => {
      void resizeListener.then((unlisten) => unlisten());
      void stackShortcutListener.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
        return;
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        event.shiftKey &&
        event.key.toLocaleLowerCase() === "l"
      ) {
        if (workspaceSurface.kind === "settings" || workspaceSurface.kind === "send") return;
        event.preventDefault();
        setProductToolsOpen(false);
        dispatchWorkspaceUi({ type: "toggleSurface", kind: "transfers" });
        return;
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        event.shiftKey &&
        event.key.toLocaleLowerCase() === "i"
      ) {
        if (workspaceSurface.kind === "settings" || workspaceSurface.kind === "send") return;
        event.preventDefault();
        setProductToolsOpen(false);
        dispatchWorkspaceUi({ type: "toggleSurface", kind: "incoming" });
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [dispatchWorkspaceUi, workspaceSurface.kind]);

  useEffect(() => {
    if (selectedId && visibleItems.some((item) => item.id === selectedId)) return;
    setSelectedId(visibleItems[0]?.id ?? null);
  }, [selectedId, visibleItems]);

  useEffect(() => {
    if (!bulkMode) return;

    const visibleItemIds = new Set(visibleItems.map((item) => item.id));
    setBulkSelectedIds((current) => {
      const next = new Set([...current].filter((id) => visibleItemIds.has(id)));
      return next.size === current.size ? current : next;
    });
  }, [bulkMode, visibleItems]);

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;

      if (workspaceSurface.kind === "send") return;
      if (workspaceSurface.kind !== "none") {
        event.preventDefault();
        dispatchWorkspaceUi({ type: "closeSurface" });
        return;
      }
      if (cleanupMode === "menu") {
        event.preventDefault();
        setCleanupMode("closed");
        return;
      }
      if (filtersOpen) {
        event.preventDefault();
        setFiltersOpen(false);
        return;
      }
      if (productToolsOpen) {
        event.preventDefault();
        setProductToolsOpen(false);
        return;
      }
      if (bulkMode) {
        event.preventDefault();
        setBulkMode(false);
        setBulkSelectedIds(new Set());
      }
    };

    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [
    bulkMode,
    cleanupMode,
    dispatchWorkspaceUi,
    filtersOpen,
    productToolsOpen,
    workspaceSurface.kind,
  ]);

  useEffect(() => {
    const syncFirstValueGuide = () => {
      setFirstValueGuideArmed(readStorageFlag(FIRST_VALUE_GUIDE_PENDING_KEY));
      setFirstValueCompleted(readStorageFlag(FIRST_VALUE_COMPLETED_KEY));
    };

    window.addEventListener("storage", syncFirstValueGuide);
    window.addEventListener("clipriva:first-value-completed", syncFirstValueGuide);
    return () => {
      window.removeEventListener("storage", syncFirstValueGuide);
      window.removeEventListener("clipriva:first-value-completed", syncFirstValueGuide);
    };
  }, []);

  const selectedItem = useMemo(
    () => visibleItems.find((item) => item.id === selectedId) ?? null,
    [selectedId, visibleItems],
  );
  const detailItem =
    visibleItems.find((item) => item.id === detailItemId) ??
    (historyInventoryQuery.data ?? []).find((item) => item.id === detailItemId) ??
    detailItemSnapshot;

  const pauseCapture = () => {
    const preferences = preferencesQuery.data;
    if (!preferences) return;

    actions.updatePreferences.mutate({
      ...preferences,
      capturePaused: true,
      pauseReason: "Paused manually",
    });
  };

  const copyItem = useCallback(
    async (id: string, reportError = true) => {
      try {
        const copied = await actions.copy.mutateAsync(id);
        setCopyErrorItemId(null);
        setCopyNotice(`${kindLabel[copied.kind]} copied to clipboard`);
        return copied;
      } catch (error) {
        setCopyNotice(null);
        if (reportError) setCopyErrorItemId(id);
        throw error;
      }
    },
    [actions.copy],
  );

  const activateQueueItem = useCallback(
    async (id: string) => {
      setQueueFailure(null);
      let token: string | null = null;
      try {
        token = await clipboardStack.begin(id);
        await copyItem(id, false);
        await clipboardStack.finish(token, true);
      } catch {
        if (token !== null) {
          await clipboardStack.finish(token, false).catch(() => undefined);
        }
        setQueueFailure("Could not copy this clip. Retry Copy or skip it.");
      }
    },
    [clipboardStack, copyItem],
  );

  const enqueueItems = (ids: readonly string[]) => {
    setQueueFailure(null);
    setQueueOpen(true);
    void clipboardStack.add(ids).catch(() => {
      setQueueFailure("Could not add these clips to the Stack.");
    });
  };

  const toggleSavedItem = (item: ClipboardItem) => {
    if (item.isPinned) {
      actions.removeSnippet.mutate(item.id);
    } else {
      actions.saveSnippet.mutate({ id: item.id });
    }
  };

  useEffect(() => {
    if (!copyNotice) return;
    const timeout = window.setTimeout(() => setCopyNotice(null), 1_500);
    return () => window.clearTimeout(timeout);
  }, [copyNotice]);

  const requestResumeCapture = () => {
    if (preferencesQuery.data?.sensitiveContentPolicy === "strict") {
      dispatchWorkspaceUi({ type: "openSurface", surface: { kind: "settings" } });
      return;
    }
    actions.resumeCapture.mutate();
  };

  const requestCleanup = (request: ClipboardCleanupRequest) => {
    setCleanupMode("closed");
    dispatchWorkspaceUi({
      type: "openSurface",
      surface: {
        kind: "cleanup",
        request: { ...request, itemIds: request.itemIds ?? [] },
      },
    });
  };

  const confirmCleanup = async () => {
    if (!cleanupRequest) return;
    await actions.moveToRecycleBin.mutateAsync(cleanupRequest);
    for (const itemId of cleanupRequest.itemIds ?? []) {
      await clipboardStack.markUnavailable(itemId, "recycled");
    }
    dispatchWorkspaceUi({ type: "closeSurface" });
    if (bulkMode) exitBulkMode();
  };

  const includePinnedInCleanup = () => {
    if (!cleanupRequest) return;
    dispatchWorkspaceUi({
      type: "updateCleanup",
      request: { ...cleanupRequest, includePinned: true },
    });
  };

  const saveCapturePreferences = async (preferences: CapturePreferences) => {
    const previous = preferencesQuery.data;
    const saved = await actions.updatePreferences.mutateAsync(preferences);
    if (
      previous &&
      (previous.retentionDays !== saved.retentionDays ||
        previous.maxHistoryItems !== saved.maxHistoryItems)
    ) {
      // Settings closes after onSave resolves. Open the mutually-exclusive
      // cleanup review on the next task so that close cannot dismiss it.
      window.setTimeout(
        () => requestCleanup({ scope: "retentionPolicy", includePinned: false }),
        0,
      );
    }
    return saved;
  };

  const clearFilters = () => {
    setKindFilters([]);
    setSourceAppFilter("");
    setTagFilter("");
    setPinFilter("all");
    setTimeFilter("all");
    setRecentlyUsedOnly(false);
    setLocalLinkOnly(false);
  };

  const toggleKindFilter = (kind: ClipboardItemKind) => {
    setKindFilters((current) =>
      current.includes(kind)
        ? current.filter((candidate) => candidate !== kind)
        : [...current, kind],
    );
  };

  const exitBulkMode = () => {
    setBulkMode(false);
    setBulkSelectedIds(new Set());
  };

  const toggleBulkSelection = (id: string) => {
    setBulkSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const requestSelectedClipsCleanup = () => {
    const selectedIds = [...bulkSelectedIds];
    if (selectedIds.length === 0) return;
    requestCleanup({ scope: "selected", itemIds: selectedIds, includePinned: false });
  };

  const openDetails = (id: string) => {
    if (workspaceSurface.kind === "details" && detailDirtyRef.current && detailItemId !== id) {
      setPendingDetailNavigation({ kind: "item", id });
      return;
    }
    setDetailItemId(id);
    setDetailItemSnapshot(visibleItems.find((item) => item.id === id) ?? null);
    detailReturnRef.current =
      document.getElementById(historyOptionId(id))?.querySelector<HTMLElement>(".clip-select") ??
      null;
    setSelectedId(id);
    dispatchWorkspaceUi({ type: "openSurface", surface: { kind: "details" } });
  };

  const openSettings = () => {
    setProductToolsOpen(false);
    dispatchWorkspaceUi({ type: "openSurface", surface: { kind: "settings" } });
  };

  const restoreFromRecycleBin = useCallback(
    async (id: string) => {
      try {
        const restored = await actions.restoreFromRecycleBin.mutateAsync(id);
        void localDiagnosticsActions.record
          .mutateAsync({
            eventType: "recycleBinRestore",
            outcome: "restored",
          })
          .catch(() => undefined);
        return restored;
      } catch (error) {
        void localDiagnosticsActions.record
          .mutateAsync({
            eventType: "recycleBinRestore",
            outcome: "failed",
          })
          .catch(() => undefined);
        throw error;
      }
    },
    [actions.restoreFromRecycleBin, localDiagnosticsActions.record],
  );

  const dismissUndoDelete = useCallback(() => setUndoDelete(null), []);

  const requestItemDelete = (item: ClipboardItem) => {
    setDeleteError(null);
    if (item.isPinned) {
      requestCleanup({
        scope: "selected",
        itemIds: [item.id],
        includePinned: false,
      });
      return;
    }

    void actions.moveToRecycleBin
      .mutateAsync({
        scope: "selected",
        itemIds: [item.id],
        includePinned: false,
      })
      .then((result) => {
        if (result.movedToRecycleBinCount > 0) {
          void clipboardStack.markUnavailable(item.id, "recycled");
          setUndoDelete({ id: item.id, label: `${kindLabel[item.kind]} clip` });
        }
      })
      .catch((error) => {
        setDeleteError(String(error).replace(/^Error:\s*/, ""));
      });
  };

  const undoLastDelete = useCallback(async () => {
    if (!undoDelete) return;
    setDeleteError(null);
    try {
      await restoreFromRecycleBin(undoDelete.id);
      setUndoDelete(null);
    } catch (error) {
      setDeleteError(String(error).replace(/^Error:\s*/, ""));
    }
  }, [restoreFromRecycleBin, undoDelete]);

  useEffect(() => {
    if (!undoDelete) return;
    const handleUndoShortcut = (event: KeyboardEvent) => {
      if (
        !event.metaKey ||
        event.shiftKey ||
        event.key.toLocaleLowerCase() !== "z" ||
        isEditableTarget(event.target)
      ) {
        return;
      }
      event.preventDefault();
      void undoLastDelete();
    };
    window.addEventListener("keydown", handleUndoShortcut);
    return () => window.removeEventListener("keydown", handleUndoShortcut);
  }, [undoDelete, undoLastDelete]);

  const resumeCaptureFromSettings = async () => {
    await actions.resumeCapture.mutateAsync();
  };

  const finishOnboarding = async () => {
    const preferences = preferencesQuery.data;
    if (!preferences) return;

    await actions.updatePreferences.mutateAsync({
      ...preferences,
      onboardingCompleted: true,
    });
    writeStorageFlag(FIRST_VALUE_GUIDE_PENDING_KEY, true);
    setFirstValueGuideArmed(true);
  };

  const handleHistoryKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (isEditableTarget(event.target) || isHistoryActionTarget(event.target)) return;

    const selectedIndex = visibleItems.findIndex((item) => item.id === selectedId);
    if (selectedIndex < 0) return;
    const activeItem = visibleItems[selectedIndex];
    if (!activeItem) return;

    let nextIndex: number | null = null;
    if (event.key === "ArrowDown") {
      nextIndex = Math.min(selectedIndex + 1, visibleItems.length - 1);
    } else if (event.key === "ArrowUp") {
      nextIndex = Math.max(selectedIndex - 1, 0);
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (bulkMode) {
        toggleBulkSelection(activeItem.id);
      } else {
        void copyItem(activeItem.id).catch(() => undefined);
      }
      return;
    } else {
      return;
    }

    event.preventDefault();
    const nextItem = visibleItems[nextIndex];
    if (!nextItem) return;
    setSelectedId(nextItem.id);
    event.currentTarget.focus();
  };

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="ClipRiva sources">
        <div className="brand">
          <span className="brand-mark">
            <Waves size={18} strokeWidth={2.4} />
          </span>
          <div>
            <strong>ClipRiva</strong>
            <span>Local-first clipboard</span>
          </div>
        </div>

        <nav className="primary-nav" aria-label="ClipRiva workspace">
          <button
            type="button"
            className="nav-item"
            data-active={workspaceView === "history" && !tagFilter && !smartCollectionId}
            aria-current={
              workspaceView === "history" && !tagFilter && !smartCollectionId ? "page" : undefined
            }
            onClick={() => {
              setProductToolsOpen(false);
              setTagFilter("");
              setSmartCollectionId(null);
              setPinFilter("all");
              dispatchWorkspaceUi({ type: "switchView", view: "history" });
            }}
          >
            <History size={16} aria-hidden="true" />
            History
            <span className="nav-count">
              {clipboardFilterOptionsQuery.data?.totalCount ??
                historyInventoryQuery.data?.length ??
                0}
            </span>
          </button>
          <button
            type="button"
            className="nav-item"
            data-active={workspaceView === "pinned"}
            aria-current={workspaceView === "pinned" ? "page" : undefined}
            onClick={() => {
              setProductToolsOpen(false);
              setTagFilter("");
              setSmartCollectionId(null);
              setPinFilter("all");
              dispatchWorkspaceUi({ type: "switchView", view: "pinned" });
            }}
          >
            <Pin size={16} aria-hidden="true" />
            Saved
            <span className="nav-count">
              {clipboardFilterOptionsQuery.data
                ? clipboardFilterOptionsQuery.data.totalCount -
                  clipboardFilterOptionsQuery.data.unpinnedCount
                : (historyInventoryQuery.data ?? []).filter((item) => item.isPinned).length}
            </span>
          </button>
        </nav>

        <section className="sidebar-collections" aria-labelledby="collections-heading">
          <div className="sidebar-section-heading">
            <span id="collections-heading">Collections</span>
          </div>
          {collections.length > 0 ? (
            <div className="collection-nav-list">
              {collections.map((collection) => (
                <button
                  key={collection.name}
                  type="button"
                  className="nav-item"
                  data-active={
                    workspaceView === "history" &&
                    (collection.kind === "smart"
                      ? smartCollectionId === collection.id
                      : tagFilter === collection.name)
                  }
                  aria-current={
                    workspaceView === "history" &&
                    (collection.kind === "smart"
                      ? smartCollectionId === collection.id
                      : tagFilter === collection.name)
                      ? "page"
                      : undefined
                  }
                  onClick={() => {
                    setProductToolsOpen(false);
                    setTagFilter(collection.kind === "manual" ? collection.name : "");
                    setSmartCollectionId(collection.kind === "smart" ? collection.id : null);
                    setPinFilter("all");
                    dispatchWorkspaceUi({ type: "switchView", view: "history" });
                  }}
                >
                  {collection.kind === "smart" ? (
                    <Sparkles size={15} aria-hidden="true" />
                  ) : (
                    <Folder size={15} aria-hidden="true" />
                  )}
                  <span>{collection.name}</span>
                  <span className="nav-count">{collection.itemCount}</span>
                </button>
              ))}
            </div>
          ) : (
            <p className="sidebar-empty-copy">Save a clip to a Collection to see it here.</p>
          )}
        </section>

        <div className="sidebar-bottom">
          <button
            type="button"
            className="sidebar-tool"
            data-active={workspaceView === "localLinkPreview"}
            onClick={() => {
              setProductToolsOpen(false);
              setQuery("");
              clearFilters();
              setFiltersOpen(false);
              dispatchWorkspaceUi({ type: "switchView", view: "localLinkPreview" });
            }}
          >
            <MonitorUp size={16} aria-hidden="true" />
            <span>
              <strong>Local Link</strong>
              <small>Preview · explicit handoff</small>
            </span>
          </button>
          <button type="button" className="sidebar-tool" onClick={openSettings}>
            <Shield size={16} aria-hidden="true" />
            <span>
              <strong>Privacy</strong>
              <small>Capture and local data</small>
            </span>
          </button>
          <button type="button" className="sidebar-tool" onClick={onEnablePrivacyCover}>
            <ShieldCheck size={16} aria-hidden="true" />
            <span>
              <strong>Privacy Cover</strong>
              <small>Hide clipboard content now</small>
            </span>
          </button>
          <div className="local-status" role="status">
            <span className="status-dot" />
            <div>
              <strong>Local-first · This Mac</strong>
              <span>No account or telemetry required</span>
            </div>
            <button
              type="button"
              className="icon-button"
              onClick={openSettings}
              aria-label="Open settings"
              title="Settings"
            >
              <Settings size={15} />
            </button>
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          {workspaceView === "localLinkPreview" ? (
            <div className="preview-context" role="status" aria-label="Current workspace">
              <MonitorUp size={16} aria-hidden="true" />
              Local Link <span>Preview</span>
            </div>
          ) : (
            <div className="search-controls">
              <label className="search-box">
                <Search size={17} aria-hidden="true" />
                <span className="search-scope" aria-hidden="true">
                  {pinnedOnly ? "Saved" : collectionScopeName || "History"}
                </span>
                <input
                  ref={searchRef}
                  type="search"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={
                    pinnedOnly
                      ? "Search saved clips…"
                      : collectionScopeName
                        ? `Search ${collectionScopeName}…`
                        : "Search history…"
                  }
                  aria-label={
                    pinnedOnly
                      ? "Search saved clipboard clips"
                      : collectionScopeName
                        ? `Search ${collectionScopeName} Collection`
                        : "Search clipboard history"
                  }
                />
                <kbd>
                  <Command size={12} /> K
                </kbd>
              </label>
            </div>
          )}
          <div className="topbar-actions">
            <div className="product-tools-control">
              <button
                type="button"
                className="icon-button"
                onClick={() => setProductToolsOpen((current) => !current)}
                aria-label="Open product tools"
                aria-haspopup="menu"
                aria-expanded={productToolsOpen}
                title="Product tools"
              >
                <MoreHorizontal size={17} />
                {pendingIncomingCount > 0 ? (
                  <span className="product-tools-attention" aria-hidden="true" />
                ) : null}
              </button>
              {productToolsOpen ? (
                <div className="product-tools-menu" role="menu" aria-label="Product tools">
                  <button
                    type="button"
                    role="menuitem"
                    aria-current={workspaceView === "localLinkPreview" ? "page" : undefined}
                    onClick={() => {
                      setProductToolsOpen(false);
                      setQuery("");
                      clearFilters();
                      setFiltersOpen(false);
                      dispatchWorkspaceUi({ type: "switchView", view: "localLinkPreview" });
                    }}
                  >
                    <MonitorUp size={15} aria-hidden="true" />
                    <span>
                      <strong>Local Link</strong>
                      <small>Preview · device handoff</small>
                    </span>
                    {onlineLocalLinkDevices > 0 || localLinkAttentionCount > 0 ? (
                      <span
                        className="devices-nav-count"
                        data-tone={localLinkAttentionCount > 0 ? "attention" : "healthy"}
                        aria-hidden="true"
                      >
                        {localLinkAttentionCount > 0 ? "!" : onlineLocalLinkDevices}
                      </span>
                    ) : null}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    aria-label={
                      pendingIncomingCount === 0
                        ? "Open Inbox"
                        : `Open Inbox, ${pendingIncomingCount} pending ${pendingIncomingCount === 1 ? "request" : "requests"}`
                    }
                    onClick={() => {
                      setProductToolsOpen(false);
                      dispatchWorkspaceUi({
                        type: "openSurface",
                        surface: { kind: "incoming" },
                      });
                    }}
                  >
                    <Inbox size={15} aria-hidden="true" />
                    <span>
                      <strong>Incoming</strong>
                      <small>Explicit Copy, Save or Reject</small>
                    </span>
                    {pendingIncomingCount > 0 ? (
                      <span className="devices-nav-count" data-tone="attention" aria-hidden="true">
                        {pendingIncomingCount}
                      </span>
                    ) : null}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setProductToolsOpen(false);
                      dispatchWorkspaceUi({
                        type: "openSurface",
                        surface: { kind: "transfers" },
                      });
                    }}
                  >
                    <History size={15} aria-hidden="true" />
                    <span>
                      <strong>Transfer activity</strong>
                      <small>Metadata only</small>
                    </span>
                  </button>
                </div>
              ) : null}
            </div>
            {preferencesQuery.data?.capturePaused ? (
              <button
                type="button"
                className="capture-status capture-paused"
                onClick={requestResumeCapture}
                disabled={actions.resumeCapture.isPending}
              >
                <span className="pulse" />
                {actions.resumeCapture.isPending
                  ? "Resuming…"
                  : `Capture paused · ${preferencesQuery.data.pauseReason ?? "Paused manually"} · ${preferencesQuery.data.sensitiveContentPolicy === "strict" ? "Review" : "Resume"}`}
              </button>
            ) : (
              <button
                type="button"
                className="capture-status capture-active"
                onClick={pauseCapture}
                disabled={!preferencesQuery.data || actions.updatePreferences.isPending}
              >
                <span className="pulse" />
                {actions.updatePreferences.isPending ? "Pausing…" : "Capture active · Pause"}
              </button>
            )}
          </div>
        </header>

        <div
          className="workspace-grid"
          data-workspace-view={workspaceView}
          data-details-open={workspaceSurface.kind === "details"}
        >
          {workspaceView === "localLinkPreview" ? (
            <LocalLinkDevicesCenter
              onOpenTransfers={() =>
                dispatchWorkspaceUi({
                  type: "openSurface",
                  surface: { kind: "transfers" },
                })
              }
            />
          ) : (
            <section className="history-pane">
              <div className="section-heading">
                <div>
                  <span className="eyebrow">
                    {pinnedOnly
                      ? "Reusable snippets"
                      : collectionScopeName
                        ? activeSmartCollection
                          ? "Smart Collection"
                          : "Collection"
                        : "Your local history"}
                  </span>
                  <h1>{pinnedOnly ? "Saved" : collectionScopeName || "History"}</h1>
                </div>
                <div className="history-toolbar">
                  {bulkMode ? (
                    <>
                      <span className="bulk-selection-count" aria-live="polite">
                        {bulkSelectedIds.size} selected
                      </span>
                      <button
                        type="button"
                        className="text-button danger-confirmation"
                        onClick={requestSelectedClipsCleanup}
                        disabled={bulkSelectedIds.size === 0}
                        aria-label={`Move ${bulkSelectedIds.size} selected clipboard clips to Recycle Bin`}
                      >
                        <ArchiveRestore size={15} />
                        Remove
                      </button>
                      <button
                        type="button"
                        className="text-button"
                        onClick={exitBulkMode}
                        aria-label="Exit selection mode"
                      >
                        Done
                      </button>
                    </>
                  ) : (
                    <>
                      <div className="history-filter-control">
                        <button
                          type="button"
                          className="text-button"
                          onClick={() => setFiltersOpen((current) => !current)}
                          aria-label="Filter clipboard history"
                          aria-expanded={filtersOpen}
                          aria-controls="history-filter-panel"
                        >
                          <Filter size={15} />
                          Filters{activeFilterCount > 0 ? ` (${activeFilterCount})` : ""}
                        </button>
                        {filtersOpen ? (
                          <section
                            id="history-filter-panel"
                            className="history-filter-panel"
                            aria-label="Clipboard history filters"
                          >
                            <div className="history-filter-heading">
                              <strong>Filter history</strong>
                              <button
                                type="button"
                                className="text-button"
                                onClick={clearFilters}
                                disabled={activeFilterCount === 0}
                              >
                                Clear
                              </button>
                            </div>
                            <fieldset className="history-filter-kinds">
                              <legend>Type</legend>
                              {clipboardKinds.map((kind) => (
                                <label key={kind}>
                                  <input
                                    type="checkbox"
                                    checked={kindFilters.includes(kind)}
                                    onChange={() => toggleKindFilter(kind)}
                                  />
                                  {kindLabel[kind]}
                                </label>
                              ))}
                            </fieldset>
                            <label className="history-filter-field">
                              <span>Source app</span>
                              <select
                                value={sourceAppFilter}
                                onChange={(event) => setSourceAppFilter(event.target.value)}
                              >
                                <option value="">All apps</option>
                                {sourceApps.map((sourceApp) => (
                                  <option key={sourceApp} value={sourceApp}>
                                    {sourceApp}
                                  </option>
                                ))}
                              </select>
                            </label>
                            <label className="history-filter-field">
                              <span>Collection</span>
                              <select
                                value={tagFilter}
                                onChange={(event) => setTagFilter(event.target.value)}
                              >
                                <option value="">All Collections</option>
                                {availableTags.map((collection) => (
                                  <option key={collection} value={collection}>
                                    {collection}
                                  </option>
                                ))}
                              </select>
                            </label>
                            <label className="history-filter-field">
                              <span>Saved status</span>
                              <select
                                value={pinFilter}
                                onChange={(event) => {
                                  const value = event.target.value as "all" | "pinned" | "unpinned";
                                  setPinFilter(value);
                                  if (value !== "all") {
                                    dispatchWorkspaceUi({ type: "switchView", view: "history" });
                                  }
                                }}
                              >
                                <option value="all">All clips</option>
                                <option value="pinned">Saved</option>
                                <option value="unpinned">Not saved</option>
                              </select>
                            </label>
                            <label className="history-filter-field">
                              <span>Captured</span>
                              <select
                                value={timeFilter}
                                onChange={(event) =>
                                  setTimeFilter(event.target.value as ClipboardTimeFilter)
                                }
                              >
                                <option value="all">Any time</option>
                                <option value="past24Hours">Past 24 hours</option>
                                <option value="past7Days">Past 7 days</option>
                                <option value="past30Days">Past 30 days</option>
                              </select>
                            </label>
                            <label className="history-filter-checkbox">
                              <input
                                type="checkbox"
                                checked={recentlyUsedOnly}
                                onChange={(event) => setRecentlyUsedOnly(event.target.checked)}
                              />
                              Recently used
                            </label>
                            <label className="history-filter-checkbox">
                              <input
                                type="checkbox"
                                checked={localLinkOnly}
                                onChange={(event) => setLocalLinkOnly(event.target.checked)}
                              />
                              Received with Local Link
                            </label>
                            {canSaveSmartCollection ? (
                              <button
                                type="button"
                                className="text-button"
                                onClick={() => {
                                  setSmartCollectionEditingId(null);
                                  setSmartCollectionDialogOpen(true);
                                }}
                              >
                                <Sparkles size={14} aria-hidden="true" />
                                Save filters as Smart Collection
                              </button>
                            ) : null}
                          </section>
                        ) : null}
                      </div>
                      <button
                        type="button"
                        className="text-button"
                        onClick={() =>
                          dispatchWorkspaceUi({
                            type: "openSurface",
                            surface: { kind: "recycleBin" },
                          })
                        }
                        aria-label="Open Recycle Bin"
                      >
                        <ArchiveRestore size={15} />
                        Recycle Bin
                      </button>
                      <button
                        type="button"
                        className="text-button"
                        onClick={() => setQueueOpen((open) => !open)}
                        aria-expanded={queueOpen}
                        aria-controls="clipboard-queue-workspace"
                      >
                        <ListPlus size={15} />
                        Stack
                        {clipboardQueue.order.length > 0 ? ` (${clipboardQueue.order.length})` : ""}
                      </button>
                      <div className="history-overflow">
                        <button
                          type="button"
                          className="icon-button"
                          aria-label="More history actions"
                          aria-haspopup="menu"
                          aria-expanded={cleanupMode !== "closed"}
                          onClick={() =>
                            setCleanupMode((current) => (current === "closed" ? "menu" : "closed"))
                          }
                        >
                          <MoreHorizontal size={17} />
                        </button>
                        {cleanupMode === "menu" ? (
                          <div className="history-overflow-menu" role="menu">
                            <button
                              type="button"
                              role="menuitem"
                              className="history-menu-action"
                              onClick={() => {
                                setCleanupMode("closed");
                                setBulkSelectedIds(new Set());
                                setBulkMode(true);
                              }}
                            >
                              <ListChecks size={14} />
                              Select clips
                            </button>
                            <button
                              type="button"
                              role="menuitem"
                              className="history-menu-action"
                              onClick={() =>
                                requestCleanup({ scope: "retentionPolicy", includePinned: false })
                              }
                            >
                              <History size={14} />
                              Review retention cleanup
                            </button>
                            <button
                              type="button"
                              role="menuitem"
                              className="history-danger-action"
                              disabled={unpinnedCount === 0}
                              onClick={() =>
                                requestCleanup({ scope: "allUnpinned", includePinned: false })
                              }
                              aria-label={`Move ${unpinnedCount} unpinned clipboard items to Recycle Bin`}
                            >
                              <ArchiveRestore size={14} />
                              Clear {unpinnedCount} unpinned{" "}
                              {unpinnedCount === 1 ? "clip" : "clips"}
                            </button>
                          </div>
                        ) : null}
                      </div>
                    </>
                  )}
                </div>
              </div>

              {queueOpen ? (
                <div id="clipboard-queue-workspace" className="workspace-queue">
                  <div
                    className="workspace-stack-toolbar"
                    role="toolbar"
                    aria-label="Stack collection controls"
                  >
                    <button
                      type="button"
                      onClick={() => enqueueItems(visibleItems.map((item) => item.id))}
                      disabled={visibleItems.length === 0}
                    >
                      Add visible clips
                    </button>
                    <button
                      type="button"
                      aria-pressed={clipboardStack.collecting}
                      onClick={() =>
                        void clipboardStack
                          .setCollecting(!clipboardStack.collecting)
                          .catch(() => setQueueFailure("Could not change Stack collection mode."))
                      }
                    >
                      {clipboardStack.collecting ? "Stop collecting" : "Start collecting"}
                    </button>
                  </div>
                  <ClipboardQueuePanel
                    state={clipboardQueue}
                    activationMode="copy"
                    failureMessage={queueFailure}
                    getItemLabel={(id) => {
                      const item = (historyInventoryQuery.data ?? []).find(
                        (candidate) => candidate.id === id,
                      );
                      return item
                        ? `${kindLabel[item.kind]} · ${item.sourceApp ?? "Unknown app"}`
                        : "Clip";
                    }}
                    onActivate={(id) => void activateQueueItem(id)}
                    onRetry={(id) => void activateQueueItem(id)}
                    onCopyAdvance={(id) => void activateQueueItem(id)}
                    onMove={(from, to) => {
                      void clipboardStack
                        .move(from, to)
                        .catch(() => setQueueFailure("Could not reorder the Stack."));
                    }}
                    onRemove={(index) => {
                      void clipboardStack
                        .remove(index)
                        .catch(() => setQueueFailure("Could not remove this Stack item."));
                    }}
                    onSkip={() => {
                      setQueueFailure(null);
                      void clipboardStack
                        .advance()
                        .catch(() => setQueueFailure("Could not advance the Stack."));
                    }}
                    onReset={() => {
                      setQueueFailure(null);
                      void clipboardStack
                        .reset()
                        .catch(() => setQueueFailure("Could not reset the Stack."));
                    }}
                  />
                </div>
              ) : null}

              {activeFilterCount > 0 || activeSmartCollection ? (
                <fieldset className="active-filter-chips">
                  <legend>Active clipboard filters</legend>
                  {activeSmartCollection ? (
                    <>
                      <span className="filter-token">
                        <Sparkles size={12} aria-hidden="true" /> Smart ·{" "}
                        {activeSmartCollection.name}
                      </span>
                      <button
                        type="button"
                        onClick={() => {
                          setSmartCollectionEditingId(activeSmartCollection.id);
                          setSmartCollectionDialogOpen(true);
                        }}
                        aria-label={`Edit Smart Collection ${activeSmartCollection.name}`}
                      >
                        Edit <Pencil size={12} aria-hidden="true" />
                      </button>
                    </>
                  ) : null}
                  {kindFilters.map((kind) => (
                    <button
                      key={kind}
                      type="button"
                      onClick={() => toggleKindFilter(kind)}
                      aria-label={`Clear ${kindLabel[kind]} filter`}
                    >
                      Type · {kindLabel[kind]} <X size={12} aria-hidden="true" />
                    </button>
                  ))}
                  {sourceAppFilter ? (
                    <button type="button" onClick={() => setSourceAppFilter("")}>
                      Source · {sourceAppFilter} <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  {tagFilter ? (
                    <button type="button" onClick={() => setTagFilter("")}>
                      Collection · {tagFilter} <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  {pinFilter !== "all" ? (
                    <button type="button" onClick={() => setPinFilter("all")}>
                      Saved · {pinFilter === "pinned" ? "Only" : "Excluded"}{" "}
                      <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  {timeFilter !== "all" ? (
                    <button type="button" onClick={() => setTimeFilter("all")}>
                      Captured · {timeFilterLabel[timeFilter]} <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  {recentlyUsedOnly ? (
                    <button type="button" onClick={() => setRecentlyUsedOnly(false)}>
                      Recently used <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  {localLinkOnly ? (
                    <button type="button" onClick={() => setLocalLinkOnly(false)}>
                      Local Link <X size={12} aria-hidden="true" />
                    </button>
                  ) : null}
                  <button type="button" className="active-filter-clear" onClick={clearFilters}>
                    Clear all
                  </button>
                </fieldset>
              ) : null}

              {ftsItemsQuery.isError ? (
                <div className="state-panel error-state">
                  <strong>Could not load clipboard history</strong>
                  <p>{String(ftsItemsQuery.error)}</p>
                  <button type="button" onClick={() => ftsItemsQuery.refetch()}>
                    Try again
                  </button>
                </div>
              ) : visibleItems.length === 0 ? (
                <div className="state-panel">
                  <Clipboard size={24} />
                  <strong>
                    {pinnedOnly
                      ? "No saved clips yet"
                      : query.trim()
                        ? "No matching clips"
                        : "No clips yet"}
                  </strong>
                  <p>
                    {pinnedOnly
                      ? "Save a clip from History to keep it easy to find."
                      : query.trim()
                        ? "Try another search or clear the current query."
                        : "Copy something to start building your private local history."}
                  </p>
                </div>
              ) : (
                <div
                  ref={listRef}
                  className="clip-list"
                  role="listbox"
                  tabIndex={0}
                  aria-label="Clipboard history"
                  aria-keyshortcuts={
                    bulkMode ? "ArrowUp ArrowDown Enter Escape" : "ArrowUp ArrowDown Enter"
                  }
                  aria-activedescendant={selectedId ? historyOptionId(selectedId) : undefined}
                  onKeyDown={handleHistoryKeyDown}
                >
                  {visibleItems.map((item) => {
                    const selected = item.id === selectedId;
                    return (
                      <div
                        key={item.id}
                        id={historyOptionId(item.id)}
                        className="clip-history-option"
                        role="option"
                        tabIndex={-1}
                        aria-selected={selected}
                        aria-current={selected ? "true" : undefined}
                      >
                        <ClipboardCard
                          item={item}
                          selected={selected}
                          selectionMode={bulkMode}
                          bulkSelected={bulkSelectedIds.has(item.id)}
                          onSelect={() => {
                            if (bulkMode) {
                              toggleBulkSelection(item.id);
                              return;
                            }
                            openDetails(item.id);
                          }}
                          onCopy={() => copyItem(item.id).catch(() => undefined)}
                          onEnqueue={() => enqueueItems([item.id])}
                          onSend={() => {
                            setSendItemSnapshot(item);
                            setSelectedId(item.id);
                            dispatchWorkspaceUi({
                              type: "openSurface",
                              surface: { kind: "send" },
                            });
                          }}
                          onTogglePin={() => toggleSavedItem(item)}
                          onDelete={() => requestItemDelete(item)}
                        />
                      </div>
                    );
                  })}
                </div>
              )}
            </section>
          )}

          {workspaceView !== "localLinkPreview" && workspaceSurface.kind === "details" ? (
            <aside className="stable-inspector" aria-label="Clip details">
              <button
                type="button"
                className="inspector-return-button"
                onClick={() => dispatchWorkspaceUi({ type: "closeSurface" })}
              >
                Back to results
              </button>
              {detailItem ? (
                <ClipboardInspector
                  key={detailItem.id}
                  item={detailItem}
                  labsEnabled={labsEnabled}
                  onCopy={(id) => void copyItem(id).catch(() => undefined)}
                  onEnqueue={(item) => enqueueItems([item.id])}
                  onTogglePin={() => detailItem && toggleSavedItem(detailItem)}
                  onReplaceTags={(id, tags) => actions.replaceTags.mutateAsync({ id, tags })}
                  onNoteDirtyChange={(dirty) => {
                    detailDirtyRef.current = dirty;
                  }}
                  onSend={() => {
                    setSendItemSnapshot(detailItem);
                    setSelectedId(detailItem.id);
                    dispatchWorkspaceUi({ type: "openSurface", surface: { kind: "send" } });
                  }}
                />
              ) : (
                <div className="stable-inspector-empty">
                  <Clipboard size={22} aria-hidden="true" />
                  <strong>Select a clip to inspect</strong>
                  <p>Preview, copy, save and organize it without opening another window.</p>
                </div>
              )}
            </aside>
          ) : null}
        </div>
      </main>

      {smartCollectionDialogOpen ? (
        <SmartCollectionDialog
          rule={editingSmartCollection?.rule ?? smartRuleDraft}
          initialName={editingSmartCollection?.name}
          searchExcluded={!editingSmartCollection && Boolean(query.trim())}
          manualCollectionExcluded={!editingSmartCollection && Boolean(tagFilter)}
          onSave={(input) =>
            editingSmartCollection?.id
              ? actions.updateSmartCollection.mutateAsync({
                  id: editingSmartCollection.id,
                  input,
                })
              : actions.createSmartCollection.mutateAsync(input)
          }
          onDelete={
            editingSmartCollection?.id
              ? async () => {
                  await actions.deleteSmartCollection.mutateAsync(editingSmartCollection.id ?? "");
                  setSmartCollectionId(null);
                }
              : undefined
          }
          onClose={() => {
            setSmartCollectionDialogOpen(false);
            setSmartCollectionEditingId(null);
          }}
        />
      ) : null}

      {undoDelete ? (
        <DeleteUndoToast
          itemLabel={undoDelete.label}
          isUndoing={
            actions.restoreFromRecycleBin.isPending &&
            actions.restoreFromRecycleBin.variables === undoDelete.id
          }
          onUndo={undoLastDelete}
          onDismiss={dismissUndoDelete}
        />
      ) : null}
      {copyNotice ? (
        <aside className="copy-result-toast" role="status" aria-label="Copy result">
          {copyNotice}
        </aside>
      ) : null}
      {copyErrorItemId ? (
        <aside className="copy-error-toast" role="alert" aria-label="Copy failed">
          <span>Could not copy this Item. It remains in History.</span>
          <button
            type="button"
            onClick={() => void copyItem(copyErrorItemId).catch(() => undefined)}
          >
            Retry Copy
          </button>
          <button
            type="button"
            aria-label="Dismiss copy error"
            onClick={() => setCopyErrorItemId(null)}
          >
            Dismiss
          </button>
        </aside>
      ) : null}
      {deleteError ? (
        <aside className="delete-error-toast" role="alert">
          {deleteError}
          <button
            type="button"
            className="icon-button"
            onClick={() => setDeleteError(null)}
            aria-label="Dismiss delete error"
          >
            <X size={13} />
          </button>
        </aside>
      ) : null}

      {pendingDetailNavigation ? (
        <div className="inspector-draft-backdrop">
          <section
            className="inspector-draft-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="inspector-draft-title"
            onKeyDown={(event) => {
              event.stopPropagation();
              if (event.key === "Escape") {
                event.preventDefault();
                keepDetailEditing();
              }
              if (event.key === "Tab") {
                const controls = Array.from(event.currentTarget.querySelectorAll("button"));
                const first = controls[0];
                const last = controls.at(-1);
                if (event.shiftKey && document.activeElement === first) {
                  event.preventDefault();
                  last?.focus();
                } else if (!event.shiftKey && document.activeElement === last) {
                  event.preventDefault();
                  first?.focus();
                }
              }
            }}
          >
            <h2 id="inspector-draft-title">Unsaved Note</h2>
            <p>Save your Note in Clip details, or discard this draft before leaving.</p>
            <div>
              <button type="button" ref={keepEditingRef} onClick={keepDetailEditing}>
                Keep editing
              </button>
              <button
                type="button"
                onClick={() => {
                  const pending = pendingDetailNavigation;
                  setPendingDetailNavigation(null);
                  detailDirtyRef.current = false;
                  if (pending.kind === "item") {
                    openDetails(pending.id);
                  } else {
                    dispatchWorkspaceUiRaw(pending.action);
                    if (pending.action.type === "closeSurface") {
                      restoreDetailFocus();
                    }
                  }
                }}
              >
                Discard draft
              </button>
            </div>
          </section>
        </div>
      ) : null}

      <LocalLinkTransfersPopover
        isOpen={workspaceSurface.kind === "transfers"}
        onClose={() => dispatchWorkspaceUi({ type: "closeSurface" })}
      />
      <LocalLinkIncomingCenter
        isOpen={workspaceSurface.kind === "incoming"}
        onClose={() => dispatchWorkspaceUi({ type: "closeSurface" })}
      />
      {!onboardingVisible &&
      !firstValueTaskPending &&
      workspaceView !== "localLinkPreview" &&
      workspaceSurface.kind !== "incoming" ? (
        <LocalLinkReceiveCard
          isSuppressed={workspaceSurface.kind !== "none"}
          onReveal={() => {
            dispatchWorkspaceUi({ type: "closeSurface" });
          }}
        />
      ) : null}
      <LocalLinkSendDialog
        item={sendItemSnapshot ?? selectedItem}
        isOpen={workspaceSurface.kind === "send"}
        onClose={() => {
          setSendItemSnapshot(null);
          dispatchWorkspaceUi({ type: "closeSurface" });
        }}
        onBackToItem={() => {
          const item = sendItemSnapshot ?? selectedItem;
          if (item) openDetails(item.id);
          setSendItemSnapshot(null);
        }}
      />

      <CaptureSettingsDialog
        isOpen={workspaceSurface.kind === "settings"}
        preferences={preferencesQuery.data}
        automationPreferences={automationPreferences.query.data}
        captureStatusEvents={captureStatusEventsQuery.data}
        retentionPreviewItems={historyInventoryQuery.data}
        isSaving={actions.updatePreferences.isPending || automationPreferences.update.isPending}
        isClearingStatusEvents={actions.clearCaptureStatus.isPending}
        localDiagnosticsSummary={localDiagnosticsQuery.data}
        isClearingLocalDiagnostics={localDiagnosticsActions.clear.isPending}
        isExportingLocalDiagnostics={localDiagnosticsActions.exportData.isPending}
        isResumingCapture={actions.resumeCapture.isPending}
        pastePermissionStatus={pastePermissionQuery.data}
        isCheckingPastePermission={pastePermissionQuery.isFetching}
        isOpeningPastePermissionSettings={pastePermissionActions.openSettings.isPending}
        onClose={() => dispatchWorkspaceUi({ type: "closeSurface" })}
        onSave={saveCapturePreferences}
        onSaveAutomation={(preferences) => automationPreferences.update.mutateAsync(preferences)}
        onClearStatusEvents={() => actions.clearCaptureStatus.mutate()}
        onResumeCapture={resumeCaptureFromSettings}
        onClearLocalDiagnostics={() => localDiagnosticsActions.clear.mutate()}
        onExportLocalDiagnostics={() => localDiagnosticsActions.exportData.mutateAsync()}
        onRecheckPastePermission={() => void pastePermissionQuery.refetch()}
        onOpenPastePermissionSettings={() => pastePermissionActions.openSettings.mutate()}
      />
      {onboardingVisible ? (
        <PrivacyOnboarding
          isSaving={actions.updatePreferences.isPending}
          onComplete={finishOnboarding}
        />
      ) : null}
      {firstValueGuideVisible ? <FirstValueGuide /> : null}
      <CleanupPreviewDialog
        isOpen={Boolean(cleanupRequest && cleanupPreviewQuery.data)}
        preview={cleanupPreviewQuery.data ?? null}
        isSubmitting={actions.moveToRecycleBin.isPending}
        errorMessage={
          actions.moveToRecycleBin.isError
            ? String(actions.moveToRecycleBin.error).replace(/^Error:\s*/, "")
            : null
        }
        onCancel={() => dispatchWorkspaceUi({ type: "closeSurface" })}
        onConfirm={() => void confirmCleanup().catch(() => undefined)}
        onIncludePinned={
          cleanupRequest && !cleanupRequest.includePinned ? includePinnedInCleanup : undefined
        }
      />
      <RecycleBinDialog
        isOpen={recycleBinOpen}
        items={recycleBinQuery.data ?? []}
        isLoading={recycleBinQuery.isLoading}
        isRestoringId={
          actions.restoreFromRecycleBin.isPending ? actions.restoreFromRecycleBin.variables : null
        }
        isDeletingPermanently={actions.permanentlyDeleteFromRecycleBin.isPending}
        isEmptying={actions.emptyRecycleBin.isPending}
        onClose={() => dispatchWorkspaceUi({ type: "closeSurface" })}
        onRestore={restoreFromRecycleBin}
        onPermanentlyDelete={(itemIds) =>
          actions.permanentlyDeleteFromRecycleBin.mutateAsync(itemIds)
        }
        onEmpty={() => actions.emptyRecycleBin.mutateAsync()}
      />
    </div>
  );
}

function FirstValueGuide() {
  return (
    <aside className="first-value-guide" aria-label="Your first Quick Paste">
      <span className="eyebrow">One quick task</span>
      <strong>Recover something you copied earlier.</strong>
      <ol>
        <li>Copy two different non-sensitive things.</li>
        <li>Open Quick Paste, select the first one, then choose Copy.</li>
      </ol>
      <p>It is now on your clipboard. Return to the other app and press ⌘V.</p>
    </aside>
  );
}

function PrivacyCoverScreen({ onReveal }: { onReveal: () => void }) {
  return (
    <main className="privacy-cover-screen" aria-labelledby="privacy-cover-title">
      <span className="brand-mark" aria-hidden="true">
        <ShieldCheck size={21} />
      </span>
      <span className="eyebrow">Presentation privacy</span>
      <h1 id="privacy-cover-title">Privacy Cover is on</h1>
      <p>
        Clipboard content, sources, Collections, images and filenames are not rendered while this
        cover is active.
      </p>
      <p>Capture continues unless paused. Existing History stays on this Mac.</p>
      <button type="button" className="settings-save" onClick={onReveal}>
        Reveal ClipRiva
      </button>
      <small>
        Window content protection is requested, but Cover is not disk encryption or a system-wide
        recording block.
      </small>
    </main>
  );
}

function PrivacyOnboarding({
  isSaving,
  onComplete,
}: {
  isSaving: boolean;
  onComplete: () => Promise<unknown>;
}) {
  const continueRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    previousFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusFirstAction = window.requestAnimationFrame(() => continueRef.current?.focus());

    const keepFocusInDialog = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      event.preventDefault();
      continueRef.current?.focus();
    };

    window.addEventListener("keydown", keepFocusInDialog);
    return () => {
      window.cancelAnimationFrame(focusFirstAction);
      window.removeEventListener("keydown", keepFocusInDialog);
      previousFocusRef.current?.focus();
    };
  }, []);

  return (
    <div className="onboarding-backdrop">
      <section
        className="onboarding-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
        aria-describedby="onboarding-description"
      >
        <span className="brand-mark onboarding-mark">
          <Waves size={20} strokeWidth={2.4} />
        </span>
        <span className="eyebrow">Welcome to ClipRiva</span>
        <h2 id="onboarding-title" className="onboarding-title">
          Your clipboard stays under your control
        </h2>
        <p id="onboarding-description" className="onboarding-copy">
          ClipRiva keeps a searchable history on this Mac so copied text and images are easy to
          recover. Start with two non-sensitive examples, then use Quick Paste to copy the earlier
          one back to your clipboard. Paste it yourself with ⌘V.
        </p>
        <ul>
          <li>
            <HardDrive className="onboarding-list-icon" size={16} aria-hidden="true" />
            History is stored in ClipRiva&apos;s local application data directory.
          </li>
          <li>
            <WifiOff className="onboarding-list-icon" size={16} aria-hidden="true" />
            Core features need no account, analytics or network connection.
          </li>
          <li>
            <ShieldCheck className="onboarding-list-icon" size={16} aria-hidden="true" />
            Excluded apps and detected high-confidence secrets are not stored.
          </li>
        </ul>
        <button
          ref={continueRef}
          type="button"
          className="settings-save onboarding-continue"
          onClick={() => void onComplete()}
          disabled={isSaving}
        >
          {isSaving ? "Saving…" : "Start using ClipRiva"}
        </button>
        <small>
          Pause stops future capture; it does not erase History. Copy needs no Accessibility
          permission.
        </small>
      </section>
    </div>
  );
}

function historyOptionId(id: string) {
  return `history-clip-${id}`;
}

function readStorageFlag(key: string) {
  try {
    return window.localStorage.getItem(key) === "true";
  } catch {
    return false;
  }
}

function writeStorageFlag(key: string, value: boolean) {
  try {
    if (value) {
      window.localStorage.setItem(key, "true");
      return;
    }
    window.localStorage.removeItem(key);
  } catch {
    // Local onboarding guidance should never interfere with core clipboard use.
  }
}

function isEditableTarget(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

function isHistoryActionTarget(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    target.closest(".clip-actions, .duplicate-group-header") !== null
  );
}

function readSavedMainWindowSize() {
  try {
    const raw = window.localStorage.getItem(MAIN_WINDOW_SIZE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { width?: unknown; height?: unknown };
    if (
      typeof parsed.width !== "number" ||
      typeof parsed.height !== "number" ||
      !Number.isFinite(parsed.width) ||
      !Number.isFinite(parsed.height)
    ) {
      return null;
    }
    return {
      width: Math.max(MAIN_WINDOW_MINIMUM.width, Math.round(parsed.width)),
      height: Math.max(MAIN_WINDOW_MINIMUM.height, Math.round(parsed.height)),
    };
  } catch {
    return null;
  }
}

function writeSavedMainWindowSize(width: number, height: number) {
  try {
    window.localStorage.setItem(
      MAIN_WINDOW_SIZE_KEY,
      JSON.stringify({
        width: Math.max(MAIN_WINDOW_MINIMUM.width, Math.round(width)),
        height: Math.max(MAIN_WINDOW_MINIMUM.height, Math.round(height)),
      }),
    );
  } catch {
    // Window-size convenience must never block clipboard history.
  }
}
