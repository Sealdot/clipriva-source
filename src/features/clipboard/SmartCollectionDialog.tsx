import { useEffect, useMemo, useRef, useState } from "react";
import type { ClipboardSmartCollectionInput, ClipboardSmartCollectionRule } from "./types";

export function SmartCollectionDialog({
  rule,
  initialName = "",
  searchExcluded,
  manualCollectionExcluded,
  onSave,
  onDelete,
  onClose,
}: {
  rule: ClipboardSmartCollectionRule;
  initialName?: string;
  searchExcluded: boolean;
  manualCollectionExcluded: boolean;
  onSave: (input: ClipboardSmartCollectionInput) => Promise<unknown>;
  onDelete?: () => Promise<unknown>;
  onClose: () => void;
}) {
  const [name, setName] = useState(initialName);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const summary = useMemo(() => summarizeSmartRule(rule), [rule]);
  const editing = Boolean(onDelete);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="dialog-backdrop">
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="smart-collection-title"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
      >
        <h2 id="smart-collection-title">
          {editing ? "Edit Smart Collection" : "Save filters as Smart Collection"}
        </h2>
        <p>Matches update automatically as your local History changes.</p>
        <label htmlFor="smart-collection-name">Name</label>
        <input
          ref={inputRef}
          id="smart-collection-name"
          value={name}
          maxLength={24}
          onChange={(event) => {
            setName(event.target.value);
            setError(null);
          }}
        />
        <fieldset>
          <legend>Saved Smart Collection rules</legend>
          {summary.map((part) => (
            <span className="filter-token" key={part}>
              {part}
            </span>
          ))}
        </fieldset>
        {searchExcluded ? <p>Current search text is not included.</p> : null}
        {manualCollectionExcluded ? <p>Manual Collection scope is not included.</p> : null}
        {confirmingDelete ? (
          <p role="alert">
            Delete this Smart Collection definition? No clips, Saved state, Notes, or manual
            Collections will be changed.
          </p>
        ) : null}
        {error ? <p role="alert">{error}</p> : null}
        <div>
          {editing ? (
            <button
              type="button"
              disabled={saving}
              onClick={() => {
                if (!confirmingDelete) {
                  setConfirmingDelete(true);
                  return;
                }
                if (!onDelete) return;
                setSaving(true);
                setError(null);
                void onDelete()
                  .then(onClose)
                  .catch((reason: unknown) => {
                    setError(String(reason).replace(/^Error:\s*/u, ""));
                    setSaving(false);
                    setConfirmingDelete(false);
                  });
              }}
            >
              {confirmingDelete ? "Confirm delete" : "Delete definition"}
            </button>
          ) : null}
          <button
            type="button"
            onClick={() => {
              if (confirmingDelete) {
                setConfirmingDelete(false);
              } else {
                onClose();
              }
            }}
          >
            {confirmingDelete ? "Keep Smart Collection" : "Cancel"}
          </button>
          <button
            type="button"
            disabled={!name.trim() || saving || confirmingDelete}
            onClick={() => {
              setSaving(true);
              setError(null);
              void onSave({ name, rule })
                .then(onClose)
                .catch((reason: unknown) => {
                  setError(String(reason).replace(/^Error:\s*/u, ""));
                  setSaving(false);
                });
            }}
          >
            {saving ? "Saving…" : editing ? "Save changes" : "Save Smart Collection"}
          </button>
        </div>
      </section>
    </div>
  );
}

export function summarizeSmartRule(rule: ClipboardSmartCollectionRule) {
  const parts: string[] = [];
  if (rule.kinds?.length) parts.push(`Types: ${rule.kinds.join(", ")}`);
  if (rule.sourceApp) parts.push(`Source: ${rule.sourceApp}`);
  if (rule.pinFilter && rule.pinFilter !== "all") parts.push(`Saved: ${rule.pinFilter}`);
  if (rule.timeFilter && rule.timeFilter !== "all") parts.push(`Time: ${rule.timeFilter}`);
  if (rule.recentlyUsedOnly) parts.push("Recently used");
  if (rule.localLinkOnly) parts.push("Local Link");
  return parts;
}
