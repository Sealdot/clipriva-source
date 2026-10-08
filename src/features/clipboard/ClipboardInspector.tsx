import { Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { ClipboardActionPanel } from "./ClipboardActionPanel";
import { ClipboardActionsPanel } from "./ClipboardActionsPanel";
import { ClipboardNoteEditor } from "./ClipboardNoteEditor";
import { formatByteSize, formatClipboardSource, formatRelativeTime, kindLabel } from "./format";
import { useClipboardEnrichment, useClipboardImageText } from "./queries";
import type { ClipboardItem } from "./types";

interface ClipboardInspectorProps {
  item: ClipboardItem | null;
  labsEnabled: boolean;
  onCopy: (id: string) => void;
  onTogglePin: (id: string) => void;
  onSend: (item: ClipboardItem) => void;
  onEnqueue?: (item: ClipboardItem) => void;
  onReplaceTags?: (id: string, tags: string[]) => Promise<unknown>;
  onNoteDirtyChange?: (dirty: boolean) => void;
}

export function ClipboardInspector({
  item,
  labsEnabled,
  onCopy,
  onTogglePin,
  onSend,
  onEnqueue,
  onReplaceTags,
  onNoteDirtyChange,
}: ClipboardInspectorProps) {
  const enrichmentQuery = useClipboardEnrichment(item?.id ?? null, labsEnabled);
  const imageText = useClipboardImageText(item?.id ?? null, item?.kind === "image");
  const [tagDraft, setTagDraft] = useState("");
  const [tagError, setTagError] = useState<string | null>(null);
  const [savingTags, setSavingTags] = useState(false);
  const tagsRef = useRef<HTMLElement>(null);
  const activeItemId = item?.id;
  const activeTagSignature = (item?.tags ?? []).join("\u001f");

  useEffect(() => {
    setTagDraft(activeItemId ? activeTagSignature.split("\u001f").join(", ") : "");
    setTagError(null);
  }, [activeItemId, activeTagSignature]);

  if (!item) {
    return (
      <aside className="inspector empty-inspector">
        <div className="empty-orbit">
          <Copy size={20} />
        </div>
        <h2>Select a clip</h2>
        <p>Preview its contents, source and local storage details.</p>
      </aside>
    );
  }

  const representations = item.representations ?? [];
  const supportsTextActions = representations.length === 0;
  const occurrences =
    item.occurrences && item.occurrences.length > 0
      ? item.occurrences
      : [
          {
            id: `legacy-${item.id}`,
            sourceApp: item.sourceApp,
            deviceId: item.deviceId ?? "legacy-local-device",
            occurredAt: item.updatedAt,
          },
        ];
  const occurrenceCount = Math.max(
    1,
    item.occurrenceCount ?? item.occurrences?.length ?? item.copyCount,
  );
  const latestOccurrence = occurrences[0];

  return (
    <aside className="inspector">
      <div className="inspector-heading">
        <div>
          <span className="eyebrow">Clip details</span>
          <h2>{kindLabel[item.kind]} clip</h2>
        </div>
        <div className="inspector-actions">
          <button
            type="button"
            className="inspector-copy-action"
            aria-label="Copy"
            aria-keyshortcuts="Meta+C Control+C"
            onClick={() => onCopy(item.id)}
          >
            <Copy size={15} aria-hidden="true" />
            Copy
            <kbd>⌘C</kbd>
          </button>
          <ClipboardActionPanel
            isSaved={item.isPinned}
            onToggleSaved={() => onTogglePin(item.id)}
            onManageCollections={() => tagsRef.current?.focus()}
            onEnqueue={onEnqueue ? () => onEnqueue(item) : undefined}
            onSend={() => onSend(item)}
          />
        </div>
      </div>

      <section className="content-preview">
        <pre>{item.content}</pre>
      </section>

      {item.kind === "image" ? (
        <section className="storage-explanation" aria-label="Text in image">
          <span className="eyebrow">Text in image</span>
          <strong>Manual · processed locally on this Mac</strong>
          {imageText.query.data ? (
            <pre>{imageText.query.data.text}</pre>
          ) : (
            <p>No extracted text is stored. Extraction runs only when you choose it.</p>
          )}
          {imageText.extract.data?.status === "sensitiveBlocked" ? (
            <p className="inline-error">
              Text was detected but not stored because it may contain a high-confidence secret.
            </p>
          ) : null}
          {imageText.extract.data?.status === "noText" ? <p>No readable text was found.</p> : null}
          {imageText.extract.data?.status === "cancelled" ? (
            <p>Extraction cancelled. No text was stored.</p>
          ) : null}
          {imageText.query.error || imageText.extract.error || imageText.remove.error ? (
            <p className="inline-error">
              {String(
                imageText.query.error ?? imageText.extract.error ?? imageText.remove.error,
              ).replace(/^Error:\s*/u, "")}
            </p>
          ) : null}
          <div>
            <button
              type="button"
              className="text-button"
              disabled={imageText.extract.isPending}
              onClick={() => imageText.extract.mutate()}
            >
              {imageText.extract.isPending ? "Extracting locally…" : "Extract text locally"}
            </button>
            {imageText.extract.isPending ? (
              <button
                type="button"
                className="text-button"
                disabled={imageText.cancel.isPending}
                onClick={() => imageText.cancel.mutate()}
              >
                {imageText.cancel.isPending ? "Cancelling…" : "Cancel extraction"}
              </button>
            ) : null}
            {imageText.query.data ? (
              <button
                type="button"
                className="text-button"
                disabled={imageText.remove.isPending}
                onClick={() => imageText.remove.mutate()}
              >
                {imageText.remove.isPending ? "Deleting…" : "Delete extracted text"}
              </button>
            ) : null}
          </div>
        </section>
      ) : null}

      <dl className="metadata-grid">
        <div>
          <dt>Source</dt>
          <dd>{formatClipboardSource(latestOccurrence?.sourceApp ?? item.sourceApp)}</dd>
        </div>
        <div>
          <dt>Last seen</dt>
          <dd>{formatRelativeTime(latestOccurrence?.occurredAt ?? item.updatedAt)}</dd>
        </div>
        <div>
          <dt>Occurrences</dt>
          <dd>{occurrenceCount}</dd>
        </div>
        <div>
          <dt>Status</dt>
          <dd>{item.isPinned ? "Pinned" : "In history"}</dd>
        </div>
      </dl>

      <section
        ref={tagsRef}
        className="clip-tags-editor"
        aria-label="Clip Collections"
        tabIndex={-1}
      >
        <div>
          <span className="eyebrow">Collections</span>
          <small>Stored locally · up to 8</small>
        </div>
        {(item.tags ?? []).length > 0 ? (
          <ul className="clip-tags" aria-label="Collection memberships">
            {(item.tags ?? []).map((tag) => (
              <li key={tag}>{tag}</li>
            ))}
          </ul>
        ) : (
          <p>Not in a Collection yet.</p>
        )}
        {onReplaceTags ? (
          <form
            onSubmit={(event) => {
              event.preventDefault();
              setTagError(null);
              setSavingTags(true);
              void onReplaceTags(
                item.id,
                tagDraft.split(",").map((tag) => tag.trim()),
              )
                .catch((reason: unknown) => {
                  setTagError(String(reason).replace(/^Error:\s*/u, ""));
                })
                .finally(() => setSavingTags(false));
            }}
          >
            <label htmlFor={`clip-tags-${item.id}`}>Comma-separated Collections</label>
            <div>
              <input
                id={`clip-tags-${item.id}`}
                value={tagDraft}
                onChange={(event) => setTagDraft(event.target.value)}
                placeholder="work, reference"
                aria-describedby={`clip-tags-help-${item.id}`}
              />
              <button type="submit" className="text-button" disabled={savingTags}>
                {savingTags ? "Saving…" : "Save Collections"}
              </button>
            </div>
            <small id={`clip-tags-help-${item.id}`}>
              24 characters per Collection name; stored locally and never shared.
            </small>
            {tagError ? <p className="inline-error">{tagError}</p> : null}
          </form>
        ) : null}
      </section>

      <ClipboardNoteEditor itemId={item.id} onDirtyChange={onNoteDirtyChange} />

      <section className="storage-explanation" aria-label="Why this clip is saved">
        <span className="eyebrow">Local retention</span>
        <strong>
          {item.isPinned
            ? "Pinned · retained until you remove it"
            : "Saved by local history policy"}
        </strong>
        <p>
          {item.isPinned
            ? "This clip remains in local History because you pinned it. Unpin it to let the normal retention policy apply."
            : item.retentionUntil
              ? `Captured from ${formatClipboardSource(latestOccurrence?.sourceApp ?? item.sourceApp)} and stored locally until ${formatRetentionDate(item.retentionUntil)}.`
              : "Captured from this Mac and stored only in local History. Its retention date is unavailable on this app version."}
        </p>
      </section>

      <section className="occurrence-timeline" aria-label="Copy occurrences">
        <div>
          <span className="eyebrow">Copy occurrences</span>
          <small>
            Copied {occurrenceCount} {occurrenceCount === 1 ? "time" : "times"}
          </small>
        </div>
        <ol>
          {occurrences.slice(0, 8).map((occurrence) => (
            <li key={occurrence.id}>
              <span>{formatClipboardSource(occurrence.sourceApp)}</span>
              <time dateTime={occurrence.occurredAt}>
                {formatRelativeTime(occurrence.occurredAt)}
              </time>
            </li>
          ))}
        </ol>
      </section>

      {representations.length > 0 ? (
        <section className="representation-list" aria-label="Format variants">
          <span className="eyebrow">Format variants</span>
          {representations.map((representation) => (
            <div key={representation.storageKey} className="representation-row">
              <div>
                <strong>{representation.displayName ?? kindLabel[representation.kind]}</strong>
                <span>
                  {representation.mimeType}
                  {representation.imageDimensions
                    ? ` · ${representation.imageDimensions.width} × ${representation.imageDimensions.height}`
                    : ""}
                </span>
              </div>
              <span>{formatByteSize(representation.byteSize)}</span>
            </div>
          ))}
        </section>
      ) : null}

      {labsEnabled && enrichmentQuery.data ? (
        <section className="context-enrichment" aria-label="Local context enrichment">
          <span className="eyebrow">ClipRiva Labs</span>
          <p>{enrichmentQuery.data.summary.text}</p>
          <div className="context-tags">
            {enrichmentQuery.data.tags.map((tag) => (
              <span key={tag.id}>{tag.label}</span>
            ))}
          </div>
          {enrichmentQuery.data.redaction.requiresExplicitConsent ? (
            <small>Potentially sensitive values are redacted from this local summary.</small>
          ) : null}
        </section>
      ) : null}

      {labsEnabled && supportsTextActions ? (
        <ClipboardActionsPanel clipboardItemId={item.id} input={item.content} />
      ) : null}
    </aside>
  );
}

function formatRetentionDate(value: string) {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "the configured retention date";
  return date.toISOString().slice(0, 10);
}
