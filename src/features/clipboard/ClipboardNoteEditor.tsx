import { useEffect, useRef, useState } from "react";
import { useClipboardItemNote } from "./queries";

const MAX_NOTE_BYTES = 16 * 1024;

export function ClipboardNoteEditor({
  itemId,
  onDirtyChange,
}: {
  itemId: string;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const note = useClipboardItemNote(itemId);
  const [draft, setDraft] = useState("");
  const [touched, setTouched] = useState(false);
  const [message, setMessage] = useState<{ itemId: string; text: string } | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!touched) setDraft(note.query.data?.text ?? "");
  }, [note.query.data?.text, touched]);

  useEffect(() => {
    onDirtyChange?.(touched && draft !== (note.query.data?.text ?? ""));
  }, [draft, note.query.data?.text, onDirtyChange, touched]);

  const byteSize = new TextEncoder().encode(draft.trim()).byteLength;
  const tooLarge = byteSize > MAX_NOTE_BYTES;

  return (
    <section className="clip-note-editor" aria-label="Clip Note">
      <div>
        <span className="eyebrow">Note</span>
        <small>Local · follows this clip's retention</small>
      </div>
      <p>A Note does not Save this clip or extend its retention date.</p>
      <label htmlFor={`clip-note-${itemId}`}>Searchable Note</label>
      <textarea
        ref={textareaRef}
        id={`clip-note-${itemId}`}
        value={draft}
        rows={4}
        maxLength={MAX_NOTE_BYTES}
        onChange={(event) => {
          setDraft(event.target.value);
          setTouched(true);
          setMessage(null);
        }}
        placeholder="Add context you want to find later"
        aria-describedby={`clip-note-help-${itemId}`}
      />
      <small id={`clip-note-help-${itemId}`}>{byteSize.toLocaleString()} / 16,384 bytes</small>
      <div>
        <button
          type="button"
          className="text-button"
          disabled={tooLarge || note.save.isPending}
          onClick={() => {
            setMessage(null);
            void note.save
              .mutateAsync(draft)
              .then((result) => {
                if (result.status === "sensitiveBlocked") {
                  setMessage({
                    itemId,
                    text: "This Note was not saved because it may contain a secret.",
                  });
                } else if (result.status === "deleted") {
                  setDraft("");
                  setTouched(false);
                  setMessage({ itemId, text: "Note deleted." });
                } else {
                  setDraft(result.note?.text ?? draft.trim());
                  setTouched(false);
                  setMessage({ itemId, text: "Note saved locally." });
                }
              })
              .catch((error: unknown) =>
                setMessage({ itemId, text: String(error).replace(/^Error:\s*/u, "") }),
              );
          }}
        >
          {note.save.isPending ? "Saving…" : "Save Note"}
        </button>
        {note.query.data ? (
          <button
            type="button"
            className="text-button"
            disabled={note.remove.isPending}
            onClick={() => {
              setMessage(null);
              void note.remove
                .mutateAsync()
                .then(() => {
                  setDraft("");
                  setTouched(false);
                  setMessage({ itemId, text: "Note deleted." });
                  textareaRef.current?.focus();
                })
                .catch((error: unknown) =>
                  setMessage({ itemId, text: String(error).replace(/^Error:\s*/u, "") }),
                );
            }}
          >
            {note.remove.isPending ? "Deleting…" : "Delete Note"}
          </button>
        ) : null}
      </div>
      {tooLarge ? <p role="alert">Notes must be at most 16 KiB.</p> : null}
      {message?.itemId === itemId ? <p role="status">{message.text}</p> : null}
    </section>
  );
}
