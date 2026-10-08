import { ArchiveRestore, RotateCcw, X } from "lucide-react";
import { useEffect } from "react";

interface DeleteUndoToastProps {
  itemLabel: string;
  durationMs?: number;
  isUndoing?: boolean;
  onUndo: () => Promise<unknown> | unknown;
  onDismiss: () => void;
}

export function DeleteUndoToast({
  itemLabel,
  durationMs = 6_000,
  isUndoing = false,
  onUndo,
  onDismiss,
}: DeleteUndoToastProps) {
  useEffect(() => {
    const timer = window.setTimeout(onDismiss, durationMs);
    return () => window.clearTimeout(timer);
  }, [durationMs, onDismiss]);

  return (
    <aside
      className="delete-undo-toast"
      role="status"
      aria-live="polite"
      aria-keyshortcuts="Meta+Z"
    >
      <ArchiveRestore size={15} aria-hidden="true" />
      <span>{itemLabel} moved to Recycle Bin.</span>
      <button
        type="button"
        className="text-button"
        onClick={() => void onUndo()}
        disabled={isUndoing}
      >
        <RotateCcw size={14} aria-hidden="true" />
        {isUndoing ? "Restoring…" : "Undo"}
        {!isUndoing ? <kbd>⌘Z</kbd> : null}
      </button>
      <button
        type="button"
        className="icon-button"
        onClick={onDismiss}
        aria-label="Dismiss undo message"
      >
        <X size={13} />
      </button>
    </aside>
  );
}
