import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  ArrowDown,
  ArrowUp,
  Check,
  Code2,
  Command,
  CornerDownLeft,
  File,
  FileText,
  Image,
  Link2,
  Palette,
  Search,
  Terminal,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { isDesktopRuntime } from "../../lib/runtime";
import { activateClipboardItem, getClipboardItem } from "./api";
import { ClipboardActionPanel } from "./ClipboardActionPanel";
import { ClipboardQueuePanel } from "./ClipboardQueuePanel";
import { useSyncedClipboardQueue } from "./clipboardQueueSync";
import { useClipboardStackBridge } from "./clipboardStackBridge";
import { FilterResultDialog } from "./FilterResultDialog";
import { formatRelativeTime, kindLabel } from "./format";
import { LocalLinkSendDialog } from "./LocalLinkSendDialog";
import {
  useCapturePreferences,
  useClipboardActions,
  useLocalDiagnosticsActions,
  useLocalTextActionMutations,
  useLocalTextActions,
  usePastePermissionActions,
  usePastePermissionStatus,
  useQuickPasteItems,
} from "./queries";
import type {
  ClipboardActivationMode,
  ClipboardItem,
  ClipboardItemKind,
  ClipboardUseResult,
  LocalTextActionPreview,
  PasteFailureReason,
  QuickPasteMatchKind,
  QuickPasteSearchResult,
  TextHighlightRange,
} from "./types";

const FIRST_VALUE_GUIDE_PENDING_KEY = "clipriva.first-value-guide-pending";
const FIRST_VALUE_COMPLETED_KEY = "clipriva.first-value-completed";

type QuickPasteFeedback =
  | {
      kind: "result";
      item: ClipboardItem;
      mode: ClipboardActivationMode;
      result: ClipboardUseResult;
    }
  | { kind: "restoreError" };

interface PendingPermission {
  item: ClipboardItem;
  mode: Exclude<ClipboardActivationMode, "copy">;
  queueActivation?: QueueActivation;
}

interface QueueActivation {
  token: string;
  advance: "paste" | "copy";
}

const kindIcon: Record<ClipboardItemKind, typeof FileText> = {
  text: FileText,
  code: Code2,
  command: Terminal,
  url: Link2,
  color: Palette,
  image: Image,
  richText: FileText,
  file: File,
};

export function QuickPasteOverlay() {
  // Tauri creates this webview hidden at startup. Keep its data queries fully
  // dormant until the global shortcut actually opens it; otherwise a hidden
  // window would rank clipboard history continuously for the whole session.
  const [overlayActive, setOverlayActive] = useState(() => !isDesktopRuntime());
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<QuickPasteFeedback | null>(null);
  const [showFailureReason, setShowFailureReason] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [pendingPermission, setPendingPermission] = useState<PendingPermission | null>(null);
  const [permissionMessage, setPermissionMessage] = useState<string | null>(null);
  const [localLinkSendOpen, setLocalLinkSendOpen] = useState(false);
  const [filterResultPreview, setFilterResultPreview] = useState<LocalTextActionPreview | null>(
    null,
  );
  const [filterResultError, setFilterResultError] = useState<"stale" | "write" | "audit" | null>(
    null,
  );
  const [queueState, dispatchQueue] = useSyncedClipboardQueue();
  const clipboardStack = useClipboardStackBridge(dispatchQueue);
  const [queueOpen, setQueueOpen] = useState(false);
  const [queueFailure, setQueueFailure] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const selectedIdRef = useRef<string | null>(null);
  const activationInProgressRef = useRef(false);
  const preserveStateOnNextOpenRef = useRef(false);
  // Fetch a small local candidate window, while rendering at most eight results.
  const itemsQuery = useQuickPasteItems({ query, pinnedOnly: false, limit: 20 }, overlayActive);
  const preferencesQuery = useCapturePreferences(overlayActive);
  const pastePermissionQuery = usePastePermissionStatus(overlayActive);
  const pastePermissionActions = usePastePermissionActions();
  const actions = useClipboardActions();
  const localDiagnosticsActions = useLocalDiagnosticsActions();
  const hasQuery = query.trim().length > 0;
  const searchResults = (
    hasQuery
      ? [...(itemsQuery.data ?? [])].sort(compareQuickPasteDisplayResults)
      : [...(itemsQuery.data ?? [])]
  ).slice(0, 8);
  const items = searchResults.map((result) => result.item);
  const selectedItem = items.find((item) => item.id === selectedId) ?? items[0] ?? null;
  const localTextActionsQuery = useLocalTextActions(
    overlayActive && (preferencesQuery.data?.labsEnabled ?? false),
  );
  const localTextActionMutations = useLocalTextActionMutations(selectedItem?.id ?? "");

  const firstFilterShortcut = localTextActionsQuery.data?.find(
    (action) => action.shortcutSlot !== null,
  );

  const recordDiagnostic = useCallback(
    (event: {
      eventType: "firstRecovery" | "directPaste" | "clipboardRestoreError";
      outcome: "started" | "completed" | "degraded" | "failed" | "restricted";
    }) => {
      void localDiagnosticsActions.record.mutateAsync(event).catch(() => undefined);
    },
    [localDiagnosticsActions.record],
  );

  const dismiss = useCallback(() => {
    if (isDesktopRuntime()) {
      setOverlayActive(false);
      void getCurrentWebviewWindow().hide();
    }
  }, []);

  const openFilterResultPreview = useCallback(
    async (actionId: string) => {
      try {
        setFilterResultPreview(await localTextActionMutations.preview.mutateAsync(actionId));
        setFilterResultError(null);
      } catch {
        setFilterResultError("write");
      }
    },
    [localTextActionMutations.preview],
  );

  const confirmFilterResult = useCallback(async () => {
    if (!filterResultPreview) return;
    setFilterResultError(null);
    try {
      await localTextActionMutations.confirm.mutateAsync(filterResultPreview);
      setFilterResultPreview(null);
      dismiss();
    } catch (error) {
      setFilterResultError(
        String(error).includes("Filter preview is out of date")
          ? "stale"
          : String(error).includes("Filter result copied, but local audit")
            ? "audit"
            : "write",
      );
    }
  }, [filterResultPreview, localTextActionMutations.confirm, dismiss]);

  const selectItem = useCallback((id: string | null) => {
    selectedIdRef.current = id;
    setSelectedId(id);
    setPreviewOpen(false);
  }, []);

  const activateSelected = useCallback(
    async (
      item: ClipboardItem,
      mode: ClipboardActivationMode,
      skipPermissionEducation = false,
      queueActivation?: QueueActivation,
    ) => {
      if (activationInProgressRef.current) return;
      if (mode !== "copy" && !skipPermissionEducation) {
        const permission = pastePermissionQuery.data ?? (await pastePermissionQuery.refetch()).data;
        if (permission !== "granted") {
          setPendingPermission({ item, mode, queueActivation });
          setPermissionMessage(null);
          return;
        }
      }

      if (activationInProgressRef.current) return;
      activationInProgressRef.current = true;

      setFeedback(null);
      setShowFailureReason(false);
      setPendingPermission(null);
      setPermissionMessage(null);
      const isFirstRecovery = !isFirstRecoveryCompleted();
      if (isFirstRecovery) {
        recordDiagnostic({ eventType: "firstRecovery", outcome: "started" });
      }
      if (mode !== "copy") {
        recordDiagnostic({ eventType: "directPaste", outcome: "started" });
      }
      try {
        const result = await activateClipboardItem(item.id, mode);
        setFeedback({ kind: "result", item, mode, result });
        if (result.outcome === "busy") return;
        if (queueActivation) {
          const succeeded =
            queueActivation.advance === "paste"
              ? result.outcome === "pasteSent"
              : result.outcome === "clipboardRestored" || result.outcome === "pasteSent";
          await clipboardStack.finish(queueActivation.token, succeeded);
          if (succeeded) {
            setQueueFailure(null);
          } else {
            setQueueFailure(recoveryFailureMessage(result));
          }
        }
        if (
          mode !== "copy" &&
          (result.outcome === "pasteSent" || result.outcome === "pasteNotSent")
        ) {
          if (result.outcome === "pasteSent") {
            recordDiagnostic({ eventType: "directPaste", outcome: "completed" });
          } else {
            // Direct Paste did not reach the prior app, but the native command
            // has already restored the item to the clipboard. Count that
            // recoverable fallback as a degradation and retain a broad error
            // category separately for the Settings troubleshooting summary.
            recordDiagnostic({ eventType: "directPaste", outcome: "degraded" });
            recordDiagnostic({
              eventType: "directPaste",
              outcome: result.failureReason === "permission" ? "restricted" : "failed",
            });
          }
        }
        if (
          result.outcome === "invalidItem" ||
          result.outcome === "writeFailed" ||
          result.outcome === "historyRecordFailed"
        ) {
          recordDiagnostic({ eventType: "clipboardRestoreError", outcome: "failed" });
        }
        if (
          isFirstRecovery &&
          (result.outcome === "clipboardRestored" || result.outcome === "pasteSent")
        ) {
          recordDiagnostic({ eventType: "firstRecovery", outcome: "completed" });
        }
        if (result.outcome !== "clipboardRestored" && result.outcome !== "pasteSent") {
          return;
        }
        markFirstValueCompleted();
        // Quick Paste's default recovery action is intentionally just a
        // clipboard restore. Close as soon as it succeeds so the person can
        // paste into their previous app themselves. Direct Paste is retained
        // only for explicit retry/permission-recovery paths.
        dismiss();
      } catch {
        if (queueActivation) {
          await clipboardStack.finish(queueActivation.token, false).catch(() => undefined);
          setQueueFailure("Could not activate this clip. Retry, copy it, or skip it.");
        }
        setFeedback({ kind: "restoreError" });
        recordDiagnostic({ eventType: "clipboardRestoreError", outcome: "failed" });
      } finally {
        activationInProgressRef.current = false;
      }
    },
    [
      dismiss,
      clipboardStack,
      pastePermissionQuery.data,
      pastePermissionQuery.refetch,
      recordDiagnostic,
    ],
  );

  const continueWithCopy = useCallback(() => {
    const pending = pendingPermission;
    if (!pending) return;
    void activateSelected(
      pending.item,
      pending.mode === "plainTextPaste" ? "plainTextPaste" : "copy",
      true,
      pending.queueActivation ? { ...pending.queueActivation, advance: "copy" } : undefined,
    );
  }, [activateSelected, pendingPermission]);

  const openPermissionSettings = useCallback(async () => {
    const pending = pendingPermission;
    preserveStateOnNextOpenRef.current = true;
    setPermissionMessage(null);
    try {
      const status = await pastePermissionActions.request.mutateAsync();
      if (status === "granted" && pending) {
        await activateSelected(pending.item, pending.mode, true, pending.queueActivation);
        return;
      }
      setPermissionMessage(
        "Accessibility permission is still off. Enable ClipRiva in System Settings, then retry.",
      );
      await pastePermissionActions.openSettings.mutateAsync();
    } catch (error) {
      setPermissionMessage(String(error).replace(/^Error:\s*/, ""));
    }
  }, [
    activateSelected,
    pastePermissionActions.openSettings,
    pastePermissionActions.request,
    pendingPermission,
  ]);

  const retryAfterPermissionChange = useCallback(async () => {
    const pending = pendingPermission;
    if (!pending) return;
    const status = (await pastePermissionQuery.refetch()).data;
    if (status !== "granted") {
      setPermissionMessage("Accessibility permission is still off.");
      return;
    }
    await activateSelected(pending.item, pending.mode, true, pending.queueActivation);
  }, [activateSelected, pastePermissionQuery.refetch, pendingPermission]);

  const runQueuedActivation = useCallback(
    async (itemId: string, advance: "paste" | "copy") => {
      setQueueFailure(null);
      try {
        const item = await getClipboardItem(itemId);
        if (!item) {
          await clipboardStack.markUnavailable(itemId, "deleted");
          return;
        }
        const token = await clipboardStack.begin(itemId);
        await activateSelected(
          item,
          advance === "paste" ? "directPaste" : "copy",
          advance === "copy",
          { token, advance },
        );
      } catch {
        setQueueFailure("Could not load or activate this Stack clip. Try again.");
      }
    },
    [activateSelected, clipboardStack],
  );

  const queueItemLabel = useCallback(
    (itemId: string) => {
      const item = itemsQuery.data?.find((result) => result.item.id === itemId)?.item;
      if (!item) return `Clip ${itemId}`;
      const content = item.content.replace(/\s+/gu, " ").trim();
      return `${kindLabel[item.kind]} · ${content.slice(0, 56)}`;
    },
    [itemsQuery.data],
  );

  useEffect(() => {
    if (selectedId && items.some((item) => item.id === selectedId)) return;
    selectItem(items[0]?.id ?? null);
  }, [items, selectItem, selectedId]);

  useEffect(() => {
    if (!isDesktopRuntime()) {
      searchRef.current?.focus();
      return;
    }

    const currentWindow = getCurrentWebviewWindow();
    const focusListener = currentWindow.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        setOverlayActive(false);
        void currentWindow.hide();
      }
    });
    const openedListener = currentWindow.listen("quick-paste-opened", () => {
      setOverlayActive(true);
      if (preserveStateOnNextOpenRef.current) {
        preserveStateOnNextOpenRef.current = false;
      } else {
        setQuery("");
        setFeedback(null);
        setShowFailureReason(false);
        setPreviewOpen(false);
        setPendingPermission(null);
        setPermissionMessage(null);
      }
      window.requestAnimationFrame(() => searchRef.current?.focus());
    });

    return () => {
      void focusListener.then((unlisten) => unlisten());
      void openedListener.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const focusSearch = window.requestAnimationFrame(() => searchRef.current?.focus());
    return () => window.cancelAnimationFrame(focusSearch);
  }, []);

  useEffect(() => {
    const handleKeydown = (event: KeyboardEvent) => {
      if (event.isComposing) return;

      // The send dialog owns Escape while it is open; Quick Paste must keep
      // its query and selection intact after that low-frequency detour.
      if (localLinkSendOpen || filterResultPreview) return;

      if (
        !isEditableTarget(event.target) &&
        (event.metaKey || event.ctrlKey) &&
        event.altKey &&
        !event.shiftKey &&
        /^[1-9]$/u.test(event.key)
      ) {
        const action = localTextActionsQuery.data?.find(
          (candidate) => candidate.shortcutSlot === Number(event.key),
        );
        if (!action || !selectedItem) return;
        event.preventDefault();
        void openFilterResultPreview(action.id);
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        if (previewOpen) {
          setPreviewOpen(false);
          return;
        }
        dismiss();
        return;
      }

      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "i") {
        if (items.length === 0) return;
        event.preventDefault();
        setPreviewOpen((open) => !open);
        return;
      }

      if (items.length === 0) return;
      const selectedIndex = Math.max(
        0,
        items.findIndex((item) => item.id === selectedIdRef.current),
      );
      const activeItem =
        items.find((item) => item.id === selectedIdRef.current) ?? items[0] ?? null;

      if (event.key === "ArrowDown") {
        event.preventDefault();
        selectItem(items[Math.min(selectedIndex + 1, items.length - 1)]?.id ?? null);
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        selectItem(items[Math.max(selectedIndex - 1, 0)]?.id ?? null);
      }

      if (event.key === "Enter") {
        if (
          event.target instanceof Element &&
          event.target.closest("button:not(.quick-paste-select)")
        ) {
          return;
        }
        event.preventDefault();
        if (activeItem) {
          void activateSelected(activeItem, "copy");
        }
        return;
      }

      if (event.metaKey && event.key.toLocaleLowerCase() === "c") {
        if (isEditableTarget(event.target) || hasSelectedPreviewText()) return;
        event.preventDefault();
        if (activeItem) {
          void activateSelected(activeItem, "copy");
        }
        return;
      }
    };

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  }, [
    activateSelected,
    dismiss,
    items,
    localLinkSendOpen,
    filterResultPreview,
    selectItem,
    previewOpen,
    localTextActionsQuery.data,
    openFilterResultPreview,
    selectedItem,
  ]);

  return (
    <main
      className="quick-paste-shell"
      data-local-link-open={localLinkSendOpen}
      aria-label="Quick paste clipboard search"
    >
      <header className="quick-paste-heading">
        <div className="quick-paste-title">
          <span className="quick-paste-mark">
            <Command size={15} strokeWidth={2.4} />
          </span>
          <div>
            <span className="eyebrow">ClipRiva</span>
            <h1>Quick paste</h1>
          </div>
        </div>
        <div className="quick-paste-heading-actions">
          <span
            className="quick-paste-header-status"
            data-tone={preferencesQuery.data?.capturePaused ? "warning" : "healthy"}
          >
            {preferencesQuery.data?.capturePaused ? "Paused" : "Capturing"}
          </span>
          <button
            type="button"
            className="icon-button quick-close"
            onClick={dismiss}
            aria-label="Close quick paste"
          >
            <X size={16} />
          </button>
        </div>
      </header>

      <label className="quick-search">
        <Search size={18} aria-hidden="true" />
        <input
          ref={searchRef}
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search clipboard history…"
          aria-label="Search clipboard history for quick paste"
        />
        <kbd>Esc</kbd>
      </label>

      <section className="quick-paste-results" aria-live="polite" aria-busy={itemsQuery.isLoading}>
        {itemsQuery.isLoading && !itemsQuery.data ? (
          <div className="quick-paste-state">Loading your local clipboard history…</div>
        ) : itemsQuery.isError ? (
          <div className="quick-paste-state">
            <strong>Could not load your clipboard history.</strong>
            <span>Your existing clipboard has not been changed.</span>
            <button type="button" className="text-button" onClick={() => void itemsQuery.refetch()}>
              Try again
            </button>
          </div>
        ) : items.length === 0 ? (
          <div className="quick-paste-state">
            {hasQuery ? (
              <>
                <strong>No results for “{query.trim()}”.</strong>
                <span>Clear the search to view your recent local clips.</span>
                <button type="button" className="text-button" onClick={() => setQuery("")}>
                  Clear search
                </button>
              </>
            ) : (
              <>
                <strong>No clipboard history yet.</strong>
                <span>Copy something to make it available here.</span>
              </>
            )}
          </div>
        ) : (
          searchResults.map((result) => (
            <QuickPasteItem
              key={result.item.id}
              result={result}
              query={query}
              showMatchKind={hasQuery}
              selected={result.item.id === selectedId}
              onSelect={() => selectItem(result.item.id)}
            />
          ))
        )}
      </section>

      {previewOpen ? <QuickPastePreview item={selectedItem} /> : null}

      {pendingPermission ? (
        <QuickPastePermissionPrompt
          mode={pendingPermission.mode}
          message={permissionMessage}
          isWorking={
            pastePermissionActions.request.isPending ||
            pastePermissionActions.openSettings.isPending
          }
          onOpenSettings={() => void openPermissionSettings()}
          onContinueWithCopy={continueWithCopy}
          onRetry={() => void retryAfterPermissionChange()}
        />
      ) : null}

      {feedback ? (
        <QuickPasteFeedbackPanel
          feedback={feedback}
          showFailureReason={showFailureReason}
          onToggleFailureReason={() => setShowFailureReason((visible) => !visible)}
          onRetry={() => {
            if (feedback.kind === "result") {
              void activateSelected(feedback.item, feedback.mode);
            }
          }}
          onContinueWithCopy={() => {
            if (feedback.kind === "result") {
              void activateSelected(
                feedback.item,
                feedback.mode === "plainTextPaste" ? "plainTextPaste" : "copy",
                true,
              );
            }
          }}
          onOpenPermissionSettings={() => {
            if (feedback.kind !== "result") return;
            setPendingPermission({
              item: feedback.item,
              mode: feedback.mode === "copy" ? "directPaste" : feedback.mode,
            });
            void openPermissionSettings();
          }}
        />
      ) : null}

      <LocalLinkSendDialog
        item={selectedItem}
        isOpen={localLinkSendOpen}
        onClose={() => setLocalLinkSendOpen(false)}
        presentation="sidecar"
      />

      {filterResultError && !filterResultPreview ? (
        <p role="alert">Could not preview this local Filter. Try again.</p>
      ) : null}
      <FilterResultDialog
        preview={filterResultPreview}
        busy={
          localTextActionMutations.preview.isPending || localTextActionMutations.confirm.isPending
        }
        error={filterResultError}
        onConfirm={() => void confirmFilterResult()}
        onPreviewAgain={() => {
          if (filterResultPreview) void openFilterResultPreview(filterResultPreview.action.id);
        }}
        onClose={() => {
          setFilterResultPreview(null);
          setFilterResultError(null);
        }}
      />

      {queueState.order.length > 0 ? (
        <section className="quick-paste-queue-drawer" aria-label="Temporary Stack">
          <button
            type="button"
            aria-expanded={queueOpen}
            onClick={() => setQueueOpen((open) => !open)}
          >
            <span>Stack</span>
            <span>
              {Math.min(queueState.cursor, queueState.order.length)} / {queueState.order.length}
            </span>
          </button>
          {queueOpen ? (
            <ClipboardQueuePanel
              state={queueState}
              activationMode="paste"
              failureMessage={queueFailure}
              getItemLabel={queueItemLabel}
              onActivate={(itemId) => void runQueuedActivation(itemId, "paste")}
              onRetry={(itemId) => void runQueuedActivation(itemId, "paste")}
              onCopyAdvance={(itemId) => void runQueuedActivation(itemId, "copy")}
              onSkip={() => {
                setQueueFailure(null);
                void clipboardStack
                  .advance()
                  .catch(() => setQueueFailure("Could not advance the Stack."));
              }}
              onReset={() => {
                setQueueFailure(null);
                setQueueOpen(false);
                void clipboardStack
                  .reset()
                  .catch(() => setQueueFailure("Could not reset the Stack."));
              }}
            />
          ) : null}
        </section>
      ) : null}

      <footer className="quick-paste-footer">
        <nav className="quick-paste-hints" aria-label="Quick Paste shortcuts">
          <span>
            <kbd>
              <ArrowUp size={11} />
            </kbd>
            <kbd>
              <ArrowDown size={11} />
            </kbd>
            Navigate
          </span>
          <span>
            <kbd>
              <CornerDownLeft size={11} />
            </kbd>
            Copy
          </span>
          <span>
            <kbd>⌘K</kbd>
            More
          </span>
        </nav>
        {firstFilterShortcut?.shortcutSlot ? (
          <span
            role="status"
            aria-label={`Filter shortcut ${firstFilterShortcut.shortcutSlot} ready`}
          >
            Filter ⌘⌥{firstFilterShortcut.shortcutSlot}
          </span>
        ) : null}
        {selectedItem ? (
          <ClipboardActionPanel
            isSaved={selectedItem.isPinned}
            shortcutEnabled
            triggerLabel="More Quick Paste actions"
            triggerClassName="quick-paste-more-button"
            onPaste={() => activateSelected(selectedItem, "directPaste")}
            onPlainTextPaste={() => activateSelected(selectedItem, "plainTextPaste")}
            onCopy={() => activateSelected(selectedItem, "copy")}
            onToggleSaved={() => actions.togglePin.mutate(selectedItem.id)}
            onEnqueue={() => {
              void clipboardStack
                .add([selectedItem.id])
                .catch(() => setQueueFailure("Could not add this clip to the Stack."));
              setQueueOpen(true);
            }}
            onSend={() => setLocalLinkSendOpen(true)}
            onRecycle={async () => {
              await actions.moveToRecycleBin.mutateAsync({
                scope: "selected",
                itemIds: [selectedItem.id],
                includePinned: true,
              });
              await clipboardStack.markUnavailable(selectedItem.id, "recycled");
            }}
          />
        ) : null}
        <button
          type="button"
          className="quick-paste-copy-button"
          disabled={!selectedItem}
          onClick={() => {
            if (selectedItem) void activateSelected(selectedItem, "copy");
          }}
        >
          Copy to clipboard
        </button>
      </footer>
    </main>
  );
}

function QuickPastePreview({ item }: { item: ClipboardItem | null }) {
  if (!item) return null;

  return (
    <section className="quick-paste-preview" aria-label="Quick paste preview">
      <div>
        <span className="eyebrow">Temporary preview</span>
        <span>{kindLabel[item.kind]}</span>
      </div>
      <pre>{item.content}</pre>
    </section>
  );
}

function QuickPastePermissionPrompt({
  mode,
  message,
  isWorking,
  onOpenSettings,
  onContinueWithCopy,
  onRetry,
}: {
  mode: Exclude<ClipboardActivationMode, "copy">;
  message: string | null;
  isWorking: boolean;
  onOpenSettings: () => void;
  onContinueWithCopy: () => void;
  onRetry: () => void;
}) {
  return (
    <section
      className="quick-paste-permission"
      role="dialog"
      aria-label="Enable Direct Paste"
      aria-describedby="quick-paste-permission-description"
    >
      <div>
        <strong>Enable Direct Paste?</strong>
        <p id="quick-paste-permission-description">
          Accessibility permission lets ClipRiva send Command-V to the app you were using. Without
          it, search and copy still work normally.
        </p>
        {mode === "plainTextPaste" ? (
          <small>Your plain-text paste choice will be kept while you update permission.</small>
        ) : null}
        {message ? <small className="quick-paste-permission-message">{message}</small> : null}
      </div>
      <div className="quick-paste-permission-actions">
        <button
          type="button"
          className="text-button"
          onClick={onContinueWithCopy}
          disabled={isWorking}
        >
          Continue with Copy
        </button>
        {message ? (
          <button type="button" className="text-button" onClick={onRetry} disabled={isWorking}>
            Retry Direct Paste
          </button>
        ) : null}
        <button
          type="button"
          className="settings-save"
          onClick={onOpenSettings}
          disabled={isWorking}
        >
          {isWorking ? "Checking…" : "Open Accessibility Settings"}
        </button>
      </div>
    </section>
  );
}

interface QuickPasteFeedbackPanelProps {
  feedback: QuickPasteFeedback;
  showFailureReason: boolean;
  onToggleFailureReason: () => void;
  onRetry: () => void;
  onContinueWithCopy: () => void;
  onOpenPermissionSettings: () => void;
}

function QuickPasteFeedbackPanel({
  feedback,
  showFailureReason,
  onToggleFailureReason,
  onRetry,
  onContinueWithCopy,
  onOpenPermissionSettings,
}: QuickPasteFeedbackPanelProps) {
  if (feedback.kind === "restoreError") {
    return (
      <div className="quick-paste-feedback" role="alert">
        Could not restore this item to the system clipboard.
      </div>
    );
  }

  const { mode, result } = feedback;
  return (
    <div className="quick-paste-feedback" role="alert">
      <strong>{pasteOutcomeMessage(result, mode)}</strong>
      {result.outcome === "pasteNotSent" ? (
        <div className="quick-paste-feedback-actions">
          <button type="button" className="text-button" onClick={onRetry}>
            Retry paste
          </button>
          <button type="button" className="text-button" onClick={onContinueWithCopy}>
            Continue with Copy
          </button>
          {result.failureReason === "permission" ? (
            <button type="button" className="text-button" onClick={onOpenPermissionSettings}>
              Open Accessibility Settings
            </button>
          ) : null}
          <button
            type="button"
            className="text-button"
            aria-expanded={showFailureReason}
            onClick={onToggleFailureReason}
          >
            {showFailureReason ? "Hide reason" : "View reason"}
          </button>
          {showFailureReason ? (
            <p className="quick-paste-failure-reason">
              {pasteFailureReasonMessage(result.failureReason)}
            </p>
          ) : null}
        </div>
      ) : result.outcome === "writeFailed" || result.outcome === "historyRecordFailed" ? (
        <div className="quick-paste-feedback-actions">
          <button type="button" className="text-button" onClick={onRetry}>
            Retry action
          </button>
        </div>
      ) : null}
    </div>
  );
}

function pasteOutcomeMessage(result: ClipboardUseResult, mode: ClipboardActivationMode) {
  if (mode === "copy" && result.outcome === "clipboardRestored") {
    return "Copied to the system clipboard. Return to your app and press Command-V.";
  }
  switch (result.outcome) {
    case "clipboardRestored":
      return "Restored to the clipboard. Return to your app and press Command-V.";
    case "pasteSent":
      return mode === "plainTextPaste"
        ? "Plain-text paste was sent to the previous app."
        : "Paste was sent to the previous app.";
    case "pasteNotSent":
      return "Paste was not sent. The item is still on your clipboard and ready for Command-V.";
    case "invalidItem":
      return "This item is no longer available. Choose another item.";
    case "writeFailed":
      return "Clipboard write could not be confirmed. Paste was not sent.";
    case "historyRecordFailed":
      return "The clipboard was written, but ClipRiva could not record this use. Paste was not sent.";
    case "busy":
      return "Another clipboard action is finishing. Try again in a moment.";
  }
}

function pasteFailureReasonMessage(reason: PasteFailureReason | null) {
  switch (reason) {
    case "permission":
      return "Accessibility permission is unavailable, so ClipRiva did not send Command-V.";
    case "focus":
      return "ClipRiva could not confirm the previous app is still available, so it did not send Command-V.";
    case "compatibility":
      return "macOS could not send the paste event for this app. The item remains on your clipboard.";
    default:
      return "ClipRiva did not send Command-V. The item remains on your clipboard.";
  }
}

function recoveryFailureMessage(result: ClipboardUseResult) {
  return result.outcome === "pasteNotSent"
    ? pasteFailureReasonMessage(result.failureReason)
    : pasteOutcomeMessage(result, "copy");
}

interface QuickPasteItemProps {
  result: QuickPasteSearchResult;
  query: string;
  showMatchKind: boolean;
  selected: boolean;
  onSelect: () => void;
}

function QuickPasteItem({ result, query, showMatchKind, selected, onSelect }: QuickPasteItemProps) {
  const { item, matchField, matchKind } = result;
  const Icon = kindIcon[item.kind];
  const displayText = quickPasteDisplayText(item, matchField, query);
  const contentHighlightRanges =
    matchField === "content" || matchField === "displayName" || matchField === "tag"
      ? (result.highlightRanges ?? findDisplayHighlightRanges(displayText, query))
      : [];
  const sourceHighlightRanges =
    matchField === "sourceApp"
      ? (result.highlightRanges ?? findDisplayHighlightRanges(item.sourceApp ?? "", query))
      : [];
  const occurrenceCount = displayOccurrenceCount(item);
  const latestOccurrenceAt = item.occurrences?.[0]?.occurredAt ?? item.updatedAt ?? item.createdAt;

  return (
    <article className="quick-paste-item" data-item-id={item.id} data-selected={selected}>
      <button
        type="button"
        className="quick-paste-select"
        onClick={onSelect}
        aria-pressed={selected}
      >
        <div className={`kind-icon kind-${item.kind}`}>
          {item.kind === "color" ? (
            <span className="color-chip" style={{ background: item.content }} />
          ) : (
            <Icon size={16} aria-hidden="true" />
          )}
        </div>
        <div className="quick-paste-item-body">
          <pre>
            <HighlightedText value={displayText} ranges={contentHighlightRanges} />
          </pre>
          <div className="quick-paste-item-meta">
            {showMatchKind ? (
              <span className="quick-paste-match">{matchKindLabel(matchKind)}</span>
            ) : null}
            {matchField === "note" || matchField === "imageText" ? (
              <span className="quick-paste-match">
                {matchField === "note" ? "Note match" : "Image text match"}
              </span>
            ) : null}
            <span>{kindLabel[item.kind]}</span>
            <span>
              <HighlightedText
                value={item.sourceApp ?? "Unknown app"}
                ranges={sourceHighlightRanges}
              />
            </span>
            <span>
              Copied {occurrenceCount} {occurrenceCount === 1 ? "time" : "times"}
            </span>
            <time dateTime={latestOccurrenceAt}>{formatRelativeTime(latestOccurrenceAt)}</time>
          </div>
        </div>
        {selected ? <Check className="quick-paste-selected" size={16} /> : null}
      </button>
    </article>
  );
}

function quickPasteDisplayText(
  item: ClipboardItem,
  matchField: QuickPasteSearchResult["matchField"],
  query: string,
) {
  const displayNames = (item.representations ?? []).flatMap((representation) =>
    representation.displayName ? [representation.displayName] : [],
  );
  if (matchField === "displayName") {
    return (
      displayNames.find(
        (displayName) => findDisplayHighlightRanges(displayName, query).length > 0,
      ) ??
      displayNames[0] ??
      item.content
    );
  }
  if (matchField === "tag") {
    return (
      (item.tags ?? []).find((tag) => findDisplayHighlightRanges(tag, query).length > 0) ??
      item.tags?.[0] ??
      item.content
    );
  }
  if (!query.trim() && (item.kind === "file" || item.kind === "image")) {
    return displayNames[0] ?? item.content;
  }
  return item.content;
}

function HighlightedText({ value, ranges }: { value: string; ranges: TextHighlightRange[] }) {
  const normalizedRanges = normalizeHighlightRanges(value, ranges);
  if (normalizedRanges.length === 0) return value;

  const parts: Array<{ text: string; highlighted: boolean; start: number }> = [];
  let cursor = 0;
  for (const range of normalizedRanges) {
    if (range.start > cursor) {
      parts.push({ text: value.slice(cursor, range.start), highlighted: false, start: cursor });
    }
    parts.push({
      text: value.slice(range.start, range.end),
      highlighted: true,
      start: range.start,
    });
    cursor = range.end;
  }
  if (cursor < value.length) {
    parts.push({ text: value.slice(cursor), highlighted: false, start: cursor });
  }

  return parts.map((part) =>
    part.highlighted ? <mark key={`${part.start}-${part.text}`}>{part.text}</mark> : part.text,
  );
}

function normalizeHighlightRanges(value: string, ranges: TextHighlightRange[]) {
  const sorted = ranges
    .map(({ start, end }) => ({
      start: Math.max(0, Math.min(value.length, start)),
      end: Math.max(0, Math.min(value.length, end)),
    }))
    .filter(({ start, end }) => end > start)
    .sort((left, right) => left.start - right.start || left.end - right.end);
  const normalized: TextHighlightRange[] = [];
  for (const range of sorted) {
    const previous = normalized.at(-1);
    if (previous && range.start <= previous.end) {
      previous.end = Math.max(previous.end, range.end);
    } else {
      normalized.push({ ...range });
    }
  }
  return normalized;
}

function findDisplayHighlightRanges(value: string, query: string) {
  const trimmedQuery = query.trim();
  if (!trimmedQuery) return [];
  const escaped = trimmedQuery.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&").replace(/\s+/gu, "\\s+");
  return Array.from(value.matchAll(new RegExp(escaped, "giu")), (match) => {
    const start = match.index ?? 0;
    return { start, end: start + match[0].length };
  });
}

function displayOccurrenceCount(item: ClipboardItem) {
  return Math.max(1, item.occurrenceCount ?? item.occurrences?.length ?? item.copyCount);
}

function compareQuickPasteDisplayResults(
  left: QuickPasteSearchResult,
  right: QuickPasteSearchResult,
) {
  const relevance = quickPasteMatchRank(left.matchKind) - quickPasteMatchRank(right.matchKind);
  const pinned = Number(right.item.isPinned) - Number(left.item.isPinned);
  const recency = quickPasteActivityAt(right.item).localeCompare(quickPasteActivityAt(left.item));
  const usage = right.item.copyCount - left.item.copyCount;
  const stableId = left.item.id.localeCompare(right.item.id);

  // Explicit search evidence remains the first discriminator. Zero-input
  // results intentionally keep the repository's
  // recent-activity order instead of promoting every pinned clip in React.
  return relevance || pinned || recency || usage || stableId;
}

function quickPasteActivityAt(item: ClipboardItem) {
  return item.lastUsedAt ?? item.occurrences?.[0]?.occurredAt ?? item.updatedAt ?? item.createdAt;
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

function matchKindLabel(matchKind: QuickPasteMatchKind) {
  return {
    exact: "Exact",
    prefix: "Starts with",
    word: "Contains",
    contains: "Contains",
    suggestion: "Suggestion",
  }[matchKind];
}

function isEditableTarget(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

function hasSelectedPreviewText() {
  const selection = window.getSelection();
  const preview = document.querySelector(".quick-paste-preview");
  return Boolean(
    preview &&
      selection &&
      !selection.isCollapsed &&
      selection.toString() &&
      selection.anchorNode &&
      selection.focusNode &&
      preview.contains(selection.anchorNode) &&
      preview.contains(selection.focusNode),
  );
}

function markFirstValueCompleted() {
  try {
    window.localStorage.setItem(FIRST_VALUE_COMPLETED_KEY, "true");
    window.localStorage.removeItem(FIRST_VALUE_GUIDE_PENDING_KEY);
  } catch {
    // The recovery itself must not be blocked if storage is unavailable.
  }
  window.dispatchEvent(new CustomEvent("clipriva:first-value-completed"));
}

function isFirstRecoveryCompleted() {
  try {
    return window.localStorage.getItem(FIRST_VALUE_COMPLETED_KEY) === "true";
  } catch {
    return false;
  }
}
