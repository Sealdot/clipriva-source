import { AlertTriangle, ArchiveRestore, Pin, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { formatRelativeTime, kindLabel } from "./format";
import type { ClipboardCleanupPreview } from "./types";
import "./CleanupPreviewDialog.css";

interface CleanupPreviewDialogProps {
  isOpen: boolean;
  preview: ClipboardCleanupPreview | null;
  isSubmitting?: boolean;
  title?: string;
  confirmLabel?: string;
  irreversible?: boolean;
  errorMessage?: string | null;
  onCancel: () => void;
  onConfirm: () => void;
  onIncludePinned?: () => void;
}

/**
 * An App-independent confirmation surface for all normal cleanup paths. It
 * displays only metadata supplied by `ClipboardCleanupPreview`, never clip
 * content, previews, hashes, or representations.
 */
export function CleanupPreviewDialog({
  isOpen,
  preview,
  isSubmitting = false,
  title = "Move clips to Recycle Bin",
  confirmLabel = "Move to Recycle Bin",
  irreversible = false,
  errorMessage = null,
  onCancel,
  onConfirm,
  onIncludePinned,
}: CleanupPreviewDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isOpen) return;
    const focusCancel = window.requestAnimationFrame(() => cancelRef.current?.focus());
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !isSubmitting) {
        event.preventDefault();
        onCancel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.cancelAnimationFrame(focusCancel);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isOpen, isSubmitting, onCancel]);

  if (!isOpen || !preview) return null;

  const expiry = preview.recycleBinRetention.latestExpiresAt
    ? new Date(preview.recycleBinRetention.latestExpiresAt).toLocaleDateString()
    : null;
  const recoveryWindow = `${preview.recycleBinRetention.maximumDays} ${
    preview.recycleBinRetention.maximumDays === 1 ? "day" : "days"
  }`;

  return (
    <div className="cleanup-preview-backdrop">
      <section
        className="cleanup-preview-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="cleanup-preview-title"
        aria-describedby="cleanup-preview-description"
      >
        <div className="cleanup-preview-heading">
          {irreversible ? <AlertTriangle size={18} /> : <ArchiveRestore size={18} />}
          <div>
            <span className="eyebrow">{irreversible ? "Permanent action" : "Local recovery"}</span>
            <h2 id="cleanup-preview-title">{title}</h2>
          </div>
        </div>
        <p id="cleanup-preview-description">
          {preview.affectedCount} {preview.affectedCount === 1 ? "clip is" : "clips are"} affected.
          {preview.pinnedSkippedCount > 0
            ? ` ${preview.pinnedSkippedCount} pinned ${preview.pinnedSkippedCount === 1 ? "clip is" : "clips are"} protected.`
            : ""}
        </p>

        {preview.representativeItems.length > 0 ? (
          <ul className="cleanup-preview-representatives" aria-label="Affected clip metadata">
            {preview.representativeItems.map((item) => (
              <li key={item.id}>
                <span>{kindLabel[item.kind]}</span>
                <span>{item.sourceApp ?? "Unknown app"}</span>
                <time dateTime={item.capturedAt}>{formatRelativeTime(item.capturedAt)}</time>
                {item.isPinned ? <Pin size={12} aria-label="Pinned" /> : null}
              </li>
            ))}
          </ul>
        ) : null}

        <ul className="cleanup-preview-rules">
          {preview.ruleConditions.map((condition) => (
            <li key={condition}>{condition}</li>
          ))}
        </ul>
        {!irreversible ? (
          <p className="cleanup-preview-retention">
            Recycle Bin recovery lasts up to {recoveryWindow}
            {expiry ? ` (latest expiry ${expiry})` : ""}.
          </p>
        ) : null}
        {preview.pinnedSkippedCount > 0 && !irreversible && onIncludePinned ? (
          <button
            type="button"
            className="text-button cleanup-preview-include-pinned"
            onClick={onIncludePinned}
            disabled={isSubmitting}
          >
            Include {preview.pinnedSkippedCount} pinned{" "}
            {preview.pinnedSkippedCount === 1 ? "clip" : "clips"}
          </button>
        ) : null}
        {errorMessage ? (
          <p className="cleanup-preview-error" role="alert">
            {errorMessage}
          </p>
        ) : null}

        <div className="cleanup-preview-actions">
          <button
            ref={cancelRef}
            type="button"
            className="text-button"
            onClick={onCancel}
            disabled={isSubmitting}
          >
            Cancel
          </button>
          <button
            type="button"
            className="text-button danger-confirmation"
            onClick={onConfirm}
            disabled={isSubmitting || preview.affectedCount === 0}
          >
            <Trash2 size={15} />
            {isSubmitting ? "Working…" : confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}
