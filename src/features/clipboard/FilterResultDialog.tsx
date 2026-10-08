import { type KeyboardEvent, useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { LocalTextActionPreview } from "./types";
import "./FilterResultDialog.css";

interface FilterResultDialogProps {
  preview: LocalTextActionPreview | null;
  busy: boolean;
  error: "stale" | "write" | "audit" | null;
  onConfirm: () => void;
  onPreviewAgain: () => void;
  onClose: () => void;
}

export function FilterResultDialog({
  preview,
  busy,
  error,
  onConfirm,
  onPreviewAgain,
  onClose,
}: FilterResultDialogProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!preview) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frame = window.requestAnimationFrame(() => confirmRef.current?.focus());
    return () => {
      window.cancelAnimationFrame(frame);
      if (previous?.isConnected) previous.focus();
    };
  }, [preview]);

  if (!preview) return null;

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    event.stopPropagation();
    if (event.key === "Escape" && !busy) {
      event.preventDefault();
      onClose();
    }
    if (event.key !== "Tab") return;
    const controls = Array.from(
      dialogRef.current?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? [],
    );
    if (controls.length === 0) return;
    const first = controls[0];
    const last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  return createPortal(
    <div className="filter-result-backdrop">
      <section
        ref={dialogRef}
        className="filter-result-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="filter-result-title"
        onKeyDown={handleKeyDown}
      >
        <header>
          <div>
            <span className="eyebrow">Local Filter preview</span>
            <h2 id="filter-result-title">{preview.action.name}</h2>
          </div>
          <button type="button" onClick={onClose} disabled={busy}>
            Close
          </button>
        </header>
        <p>Review the result before copying. The original History Item stays unchanged.</p>
        <div className="filter-result-columns">
          <section aria-label="Original clip">
            <strong>Before</strong>
            <pre>{preview.input}</pre>
          </section>
          <section aria-label="Filter result">
            <strong>After</strong>
            <pre>{preview.output}</pre>
          </section>
        </div>
        {error ? (
          <p role="alert">
            {error === "stale"
              ? "The clip or Filter changed. Preview again before copying."
              : error === "audit"
                ? "Filter result copied, but the local audit could not be saved."
                : "Could not copy the Filter result. Your original clip is unchanged."}
          </p>
        ) : null}
        <footer>
          <button type="button" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          {error === "audit" ? (
            <button ref={confirmRef} type="button" onClick={onClose}>
              Done
            </button>
          ) : error === "stale" ? (
            <button ref={confirmRef} type="button" onClick={onPreviewAgain} disabled={busy}>
              Preview again
            </button>
          ) : (
            <button ref={confirmRef} type="button" onClick={onConfirm} disabled={busy}>
              {busy ? "Copying…" : "Copy Filter result"}
            </button>
          )}
        </footer>
      </section>
    </div>,
    document.body,
  );
}
