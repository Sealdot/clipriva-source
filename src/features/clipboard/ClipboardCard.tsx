import { Check, Code2, File, FileText, Image, Link2, Palette, Terminal } from "lucide-react";
import { ClipboardActionPanel } from "./ClipboardActionPanel";
import { formatByteSize, formatClipboardSource, formatRelativeTime, kindLabel } from "./format";
import type { ClipboardItem, ClipboardItemKind } from "./types";

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

interface ClipboardCardProps {
  item: ClipboardItem;
  selected: boolean;
  selectionMode?: boolean;
  bulkSelected?: boolean;
  /** Metadata supplied by a parent grouping surface; never derived from content. */
  contextLabel?: string;
  onSelect: () => void;
  onCopy: () => Promise<unknown>;
  onSend: () => void;
  onTogglePin: () => void;
  onEnqueue?: () => void;
  onDelete: () => void;
}

export function ClipboardCard({
  item,
  selected,
  selectionMode = false,
  bulkSelected = false,
  contextLabel,
  onSelect,
  onCopy,
  onSend,
  onTogglePin,
  onEnqueue,
  onDelete,
}: ClipboardCardProps) {
  const Icon = kindIcon[item.kind];
  const representation = item.representations?.[0];
  const occurrenceCount = Math.max(
    1,
    item.occurrenceCount ?? item.occurrences?.length ?? item.copyCount,
  );
  const latestOccurrence = item.occurrences?.[0];

  return (
    <article
      className="clip-card"
      data-selected={selected}
      data-bulk-selected={selectionMode && bulkSelected ? "true" : undefined}
    >
      <button
        type="button"
        className="clip-select"
        onClick={onSelect}
        aria-label={
          selectionMode
            ? `${bulkSelected ? "Deselect" : "Select"} ${kindLabel[item.kind]} clipboard item`
            : `Inspect ${kindLabel[item.kind]} clipboard item`
        }
        aria-pressed={selectionMode ? bulkSelected : undefined}
      >
        <div className={`kind-icon kind-${item.kind}`}>
          {item.kind === "color" ? (
            <span className="color-chip" style={{ background: item.content }} />
          ) : (
            <Icon size={16} aria-hidden="true" />
          )}
        </div>
        <div className="clip-card-body">
          <pre className="clip-content">{item.content}</pre>
          <div className="clip-card-meta">
            {contextLabel ? (
              <>
                <span className="clip-context-label">{contextLabel}</span>
                <span className="meta-separator">·</span>
              </>
            ) : null}
            <span>{kindLabel[item.kind]}</span>
            <span className="meta-separator">·</span>
            {item.textInImageMatch ? (
              <>
                <span>Text in image</span>
                <span className="meta-separator">·</span>
              </>
            ) : null}
            <span>{formatClipboardSource(latestOccurrence?.sourceApp ?? item.sourceApp)}</span>
            <span className="meta-separator">·</span>
            <span>
              Copied {occurrenceCount} {occurrenceCount === 1 ? "time" : "times"}
            </span>
            <span className="meta-separator">·</span>
            <time dateTime={latestOccurrence?.occurredAt ?? item.updatedAt}>
              {formatRelativeTime(latestOccurrence?.occurredAt ?? item.updatedAt)}
            </time>
          </div>
          {(item.tags ?? []).length > 0 ? (
            <span className="clip-tags clip-card-tags">
              {(item.tags ?? []).slice(0, 3).map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </span>
          ) : null}
          {representation ? (
            <span className="clip-representation-meta">
              {representation.displayName ?? representation.mimeType} ·{" "}
              {formatByteSize(representation.byteSize)}
            </span>
          ) : null}
        </div>
      </button>
      {selectionMode ? (
        <span className="clip-bulk-selection" aria-hidden="true">
          <span>{bulkSelected ? "Selected" : "Select"}</span>
          <span className="clip-bulk-checkbox">{bulkSelected ? <Check size={14} /> : null}</span>
        </span>
      ) : (
        <div className="clip-actions">
          <ClipboardActionPanel
            isSaved={item.isPinned}
            onCopy={onCopy}
            onToggleSaved={onTogglePin}
            onManageCollections={onSelect}
            onEnqueue={onEnqueue}
            onSend={onSend}
            onRecycle={onDelete}
          />
        </div>
      )}
    </article>
  );
}
