import { FileOutput, Pencil, Plus, WandSparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { FilterComposerDialog } from "./FilterComposerDialog";
import { FilterResultDialog } from "./FilterResultDialog";
import { formatRelativeTime } from "./format";
import { useActionAudit, useLocalTextActionMutations, useLocalTextActions } from "./queries";
import type {
  LocalTextAction,
  LocalTextActionPreview,
  LocalTextTransform,
  TextActionExecution,
} from "./types";

interface ClipboardActionsPanelProps {
  clipboardItemId: string;
  input: string;
}

export function ClipboardActionsPanel({ clipboardItemId, input }: ClipboardActionsPanelProps) {
  const actionsQuery = useLocalTextActions();
  const auditQuery = useActionAudit(clipboardItemId);
  const mutations = useLocalTextActionMutations(clipboardItemId);
  const [composerOpen, setComposerOpen] = useState(false);
  const [editingAction, setEditingAction] = useState<LocalTextAction | null>(null);
  const [latestExecution, setLatestExecution] = useState<TextActionExecution | null>(null);
  const [resultPreview, setResultPreview] = useState<LocalTextActionPreview | null>(null);
  const [resultError, setResultError] = useState<"stale" | "write" | "audit" | null>(null);
  const [previewError, setPreviewError] = useState(false);

  const openResultPreview = useCallback(
    async (actionId: string) => {
      setPreviewError(false);
      try {
        setResultPreview(await mutations.preview.mutateAsync(actionId));
        setResultError(null);
      } catch {
        setPreviewError(true);
      }
    },
    [mutations.preview],
  );

  const confirmResult = async () => {
    if (!resultPreview) return;
    setResultError(null);
    try {
      setLatestExecution(await mutations.confirm.mutateAsync(resultPreview));
      setResultPreview(null);
    } catch (error) {
      setResultError(
        String(error).includes("Filter preview is out of date")
          ? "stale"
          : String(error).includes("Filter result copied, but local audit")
            ? "audit"
            : "write",
      );
    }
  };

  useEffect(() => {
    if (composerOpen || resultPreview) return;
    const handleShortcut = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        event.isComposing ||
        (target instanceof HTMLElement &&
          target.matches("input, textarea, select, [contenteditable='true']")) ||
        !(event.metaKey || event.ctrlKey) ||
        !event.altKey ||
        event.shiftKey ||
        !/^[1-9]$/u.test(event.key)
      ) {
        return;
      }
      const slot = Number(event.key);
      const action = actionsQuery.data?.find((candidate) => candidate.shortcutSlot === slot);
      if (!action) return;
      event.preventDefault();
      void openResultPreview(action.id);
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [actionsQuery.data, composerOpen, openResultPreview, resultPreview]);

  return (
    <section className="clipboard-actions-panel" aria-label="Local actions">
      <div className="actions-panel-heading">
        <div>
          <span className="eyebrow">Local actions</span>
          <p>Allow-listed transforms run locally and copy their result to the clipboard.</p>
        </div>
        <WandSparkles size={16} aria-hidden="true" />
      </div>

      <div className="action-button-grid">
        {actionsQuery.data?.map((action) => (
          <div className="local-action-entry" key={action.id}>
            <button
              type="button"
              className="local-action-button"
              onClick={() => void openResultPreview(action.id)}
              disabled={mutations.preview.isPending || mutations.confirm.isPending}
              aria-keyshortcuts={
                action.shortcutSlot
                  ? `Meta+Alt+${action.shortcutSlot} Control+Alt+${action.shortcutSlot}`
                  : undefined
              }
            >
              <FileOutput size={13} aria-hidden="true" />
              <span>{action.name}</span>
              {action.isBuiltin ? (
                <small>Built-in</small>
              ) : (
                <small>
                  {action.steps.length} step filter
                  {action.shortcutSlot ? ` · ⌘⌥${action.shortcutSlot}` : ""}
                </small>
              )}
            </button>
            {!action.isBuiltin ? (
              <button
                type="button"
                className="local-action-edit-button"
                aria-label={`Edit filter ${action.name}`}
                onClick={() => {
                  setEditingAction(action);
                  setComposerOpen(true);
                }}
              >
                <Pencil size={12} aria-hidden="true" /> Edit
              </button>
            ) : null}
          </div>
        ))}
      </div>

      {actionsQuery.isError ? (
        <p className="action-feedback error">Could not load local actions.</p>
      ) : null}
      {previewError ? (
        <p className="action-feedback error" role="alert">
          Could not preview this local Filter.
        </p>
      ) : null}

      {latestExecution ? (
        <div className="action-result" aria-live="polite">
          <strong>{latestExecution.action.name} copied locally</strong>
          <pre>{latestExecution.output}</pre>
          <small>Audit stored without clipboard content or content hashes.</small>
        </div>
      ) : null}

      <button
        type="button"
        className="new-action-button"
        onClick={() => {
          setEditingAction(null);
          setComposerOpen(true);
        }}
      >
        <Plus size={13} aria-hidden="true" />
        New filter
      </button>

      <FilterComposerDialog
        isOpen={composerOpen}
        input={input}
        initialDraft={
          editingAction
            ? {
                name: editingAction.name,
                steps: editingAction.steps.map((step) => step.transform),
                shortcutSlot: editingAction.shortcutSlot,
              }
            : undefined
        }
        unavailableShortcutSlots={(actionsQuery.data ?? [])
          .filter((action) => action.id !== editingAction?.id && action.shortcutSlot !== null)
          .map((action) => action.shortcutSlot as number)}
        isSaving={
          mutations.create.isPending || mutations.update.isPending || mutations.remove.isPending
        }
        onClose={() => {
          setComposerOpen(false);
          setEditingAction(null);
        }}
        onSave={async (draft) => {
          const [first, ...rest] = draft.steps;
          if (!first) return;
          const payload = {
            name: draft.name,
            transform: first as LocalTextTransform,
            steps: [first, ...rest].map((transform) => ({
              transform: transform as LocalTextTransform,
            })),
            shortcutSlot: draft.shortcutSlot,
          };
          if (editingAction) {
            await mutations.update.mutateAsync({ id: editingAction.id, draft: payload });
          } else {
            await mutations.create.mutateAsync(payload);
          }
          setComposerOpen(false);
          setEditingAction(null);
        }}
        onDelete={
          editingAction
            ? async () => {
                await mutations.remove.mutateAsync(editingAction.id);
                setEditingAction(null);
              }
            : undefined
        }
      />

      <FilterResultDialog
        preview={resultPreview}
        busy={mutations.confirm.isPending || mutations.preview.isPending}
        error={resultError}
        onConfirm={() => void confirmResult()}
        onPreviewAgain={() => void openResultPreview(resultPreview?.action.id ?? "")}
        onClose={() => {
          setResultPreview(null);
          setResultError(null);
        }}
      />

      {auditQuery.data && auditQuery.data.length > 0 ? (
        <details className="action-audit-log">
          <summary>Audit history ({auditQuery.data.length})</summary>
          <ul>
            {auditQuery.data.map((entry) => (
              <li key={entry.id}>
                <div>
                  <strong>{entry.actionName}</strong>
                  <span>{formatRelativeTime(entry.createdAt)}</span>
                </div>
                {entry.inputPreview ? <p>{entry.inputPreview}</p> : <p>Content not stored</p>}
                {entry.outputPreview ? <p>→ {entry.outputPreview}</p> : null}
                <small>
                  {entry.executionMode === "cloud_preview" ? "Legacy preview" : "Completed locally"}
                </small>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}
