import { ArchiveRestore, RotateCcw, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { formatRelativeTime, kindLabel } from "./format";
import type { RecycleBinItem } from "./types";
import "./RecycleBinDialog.css";

interface RecycleBinDialogProps {
  isOpen: boolean;
  items: RecycleBinItem[];
  isLoading?: boolean;
  isRestoringId?: string | null;
  isDeletingPermanently?: boolean;
  isEmptying?: boolean;
  onClose: () => void;
  onRestore: (id: string) => Promise<unknown>;
  onPermanentlyDelete: (ids: string[]) => Promise<unknown>;
  onEmpty: () => Promise<unknown>;
}

/**
 * Recycle Bin keeps recovery separate from active-history cleanup. Normal
 * deletion always lands here first; permanent removal needs its own explicit
 * acknowledgement and never reuses a destructive active-history shortcut.
 */
export function RecycleBinDialog({
  isOpen,
  items,
  isLoading = false,
  isRestoringId = null,
  isDeletingPermanently = false,
  isEmptying = false,
  onClose,
  onRestore,
  onPermanentlyDelete,
  onEmpty,
}: RecycleBinDialogProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const [confirmation, setConfirmation] = useState<"empty" | string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      setConfirmation(null);
      setActionError(null);
      return;
    }
    const focusClose = window.requestAnimationFrame(() => closeRef.current?.focus());
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || isDeletingPermanently || isEmptying) return;
      event.preventDefault();
      if (confirmation) {
        setConfirmation(null);
      } else {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.cancelAnimationFrame(focusClose);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [confirmation, isDeletingPermanently, isEmptying, isOpen, onClose]);

  if (!isOpen) return null;

  const confirmedItems =
    confirmation === "empty" ? items : items.filter((entry) => entry.item.id === confirmation);
  const isWorking = isDeletingPermanently || isEmptying;

  const restore = async (id: string) => {
    setActionError(null);
    try {
      await onRestore(id);
    } catch (error) {
      setActionError(String(error).replace(/^Error:\s*/, ""));
    }
  };

  const permanentlyDelete = async () => {
    if (confirmedItems.length === 0) {
      setConfirmation(null);
      return;
    }
    setActionError(null);
    try {
      if (confirmation === "empty") {
        await onEmpty();
      } else {
        await onPermanentlyDelete(confirmedItems.map((entry) => entry.item.id));
      }
      setConfirmation(null);
    } catch (error) {
      setActionError(String(error).replace(/^Error:\s*/, ""));
    }
  };

  return (
    <div className="recycle-bin-backdrop">
      <section
        className="recycle-bin-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="recycle-bin-title"
      >
        <header className="recycle-bin-heading">
          <div>
            <span className="eyebrow">Local recovery</span>
            <h2 id="recycle-bin-title">Recycle Bin</h2>
            <p>
              {items.length} {items.length === 1 ? "clip" : "clips"} can be restored until their
              original retention time expires.
            </p>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="icon-button"
            onClick={onClose}
            disabled={isWorking}
            aria-label="Close Recycle Bin"
          >
            <X size={16} />
          </button>
        </header>

        {isLoading ? (
          <p className="recycle-bin-state">Loading local recovery items…</p>
        ) : items.length === 0 ? (
          <p className="recycle-bin-state">Recycle Bin is empty.</p>
        ) : (
          <ul className="recycle-bin-list" aria-label="Recycled clips">
            {items.map((entry) => (
              <li key={entry.item.id} className="recycle-bin-item">
                <div className="recycle-bin-item-copy">
                  <div className="recycle-bin-item-meta">
                    <span>{kindLabel[entry.item.kind]}</span>
                    <span>{entry.item.sourceApp ?? "Unknown app"}</span>
                    <time dateTime={entry.deletedAt}>
                      Removed {formatRelativeTime(entry.deletedAt)}
                    </time>
                  </div>
                  <pre>{entry.item.content}</pre>
                  <small>
                    Available until {new Date(entry.recycleExpiresAt).toLocaleDateString()}
                  </small>
                </div>
                <div className="recycle-bin-item-actions">
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => void restore(entry.item.id)}
                    disabled={isRestoringId === entry.item.id}
                  >
                    <ArchiveRestore size={14} />
                    {isRestoringId === entry.item.id ? "Restoring…" : "Restore"}
                  </button>
                  <button
                    type="button"
                    className="text-button danger-confirmation"
                    onClick={() => setConfirmation(entry.item.id)}
                    disabled={isWorking}
                  >
                    <Trash2 size={14} />
                    Delete permanently
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}

        {items.length > 0 ? (
          <footer className="recycle-bin-footer">
            <button
              type="button"
              className="text-button danger-confirmation"
              onClick={() => setConfirmation("empty")}
              disabled={isWorking}
            >
              <Trash2 size={14} />
              Empty Recycle Bin
            </button>
          </footer>
        ) : null}
        {actionError ? (
          <p className="recycle-bin-error" role="alert">
            {actionError}
          </p>
        ) : null}

        {confirmation ? (
          <section
            className="recycle-bin-permanent-confirmation"
            role="alertdialog"
            aria-labelledby="recycle-bin-permanent-title"
            aria-describedby="recycle-bin-permanent-description"
          >
            <RotateCcw size={17} aria-hidden="true" />
            <div>
              <strong id="recycle-bin-permanent-title">
                Permanently delete {confirmedItems.length}{" "}
                {confirmedItems.length === 1 ? "clip" : "clips"}?
              </strong>
              <p id="recycle-bin-permanent-description">
                This cannot be undone and removes the local item and any private media it owns.
              </p>
              <div>
                <button
                  type="button"
                  className="text-button"
                  onClick={() => setConfirmation(null)}
                  disabled={isWorking}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="text-button danger-confirmation"
                  onClick={() => void permanentlyDelete()}
                  disabled={isWorking || confirmedItems.length === 0}
                >
                  <Trash2 size={14} />
                  {isWorking ? "Deleting…" : "Delete permanently"}
                </button>
              </div>
            </div>
          </section>
        ) : null}
      </section>
    </div>
  );
}
