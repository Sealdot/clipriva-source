import { type FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  evaluateFilterPipeline,
  FILTER_COMPOSER_MAX_STEPS,
  FILTER_COMPOSER_STEP_KINDS,
  FILTER_COMPOSER_STEP_LABELS,
  type FilterComposerDraft,
  FilterComposerEvaluationError,
  type FilterComposerStep,
  type FilterComposerStepKind,
} from "./filterComposer";
import "./FilterComposerDialog.css";

export interface FilterComposerDialogProps {
  isOpen: boolean;
  input: string;
  initialDraft?: Partial<FilterComposerDraft>;
  unavailableShortcutSlots?: number[];
  isSaving?: boolean;
  onClose: () => void;
  onSave: (draft: FilterComposerDraft) => void | Promise<void>;
  onDelete?: () => void | Promise<void>;
}

interface FilterPreviewReady {
  status: "ready";
  output: string;
}

interface FilterPreviewFailed {
  status: "failed";
  message: string;
  stepIndex: number | null;
}

type FilterPreview = FilterPreviewReady | FilterPreviewFailed;

export function FilterComposerDialog({
  isOpen,
  input,
  initialDraft,
  unavailableShortcutSlots = [],
  isSaving = false,
  onClose,
  onSave,
  onDelete,
}: FilterComposerDialogProps) {
  const sequenceRef = useRef(0);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState("");
  const [steps, setSteps] = useState<FilterComposerStep[]>([]);
  const [shortcutSlot, setShortcutSlot] = useState<number | null>(null);
  const [nextKind, setNextKind] = useState<FilterComposerStepKind>("trim_whitespace");
  const [submitting, setSubmitting] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  useEffect(() => {
    if (!isOpen) return;
    sequenceRef.current = 0;
    const initialSteps = initialDraft?.steps?.length
      ? initialDraft.steps.slice(0, FILTER_COMPOSER_MAX_STEPS)
      : (["trim_whitespace"] as const);
    setName(initialDraft?.name ?? "");
    setShortcutSlot(initialDraft?.shortcutSlot ?? null);
    setSteps(
      initialSteps.map((kind) => ({
        id: `filter-step-${++sequenceRef.current}`,
        kind,
      })),
    );
    setNextKind("trim_whitespace");
    setSubmitting(false);
    setSaveError(null);
    setConfirmingDelete(false);
    window.requestAnimationFrame(() => nameInputRef.current?.focus());
  }, [initialDraft?.name, initialDraft?.shortcutSlot, initialDraft?.steps, isOpen]);

  const preview = useMemo<FilterPreview>(() => {
    try {
      return { status: "ready", output: evaluateFilterPipeline(input, steps) };
    } catch (error) {
      if (error instanceof FilterComposerEvaluationError) {
        return { status: "failed", message: error.message, stepIndex: error.stepIndex };
      }
      return {
        status: "failed",
        message: "This local text filter could not be previewed.",
        stepIndex: null,
      };
    }
  }, [input, steps]);

  if (!isOpen) return null;

  const moveStep = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= steps.length) return;
    setSteps((current) => {
      const next = [...current];
      const [moved] = next.splice(index, 1);
      if (!moved) return current;
      next.splice(target, 0, moved);
      return next;
    });
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const normalizedName = name.trim();
    if (!normalizedName || normalizedName.length > 64 || preview.status !== "ready") return;
    setSaveError(null);
    setSubmitting(true);
    try {
      await onSave({
        name: normalizedName,
        steps: steps.map((step) => step.kind),
        shortcutSlot,
      });
    } catch {
      setSaveError("Could not save this local filter.");
    } finally {
      setSubmitting(false);
    }
  };

  const busy = isSaving || submitting;
  const normalizedName = name.trim();
  const nameInvalid = normalizedName.length === 0 || normalizedName.length > 64;

  return (
    <div className="filter-composer-backdrop">
      <form
        className="filter-composer-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="filter-composer-title"
        onSubmit={(event) => void submit(event)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !busy) {
            event.preventDefault();
            onClose();
          }
        }}
      >
        <header>
          <div>
            <span>Local, allow-listed steps</span>
            <h2 id="filter-composer-title">{onDelete ? "Edit Filter" : "Filter Composer"}</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            aria-label="Close Filter Composer"
          >
            Close
          </button>
        </header>

        <label className="filter-composer-name">
          Filter name
          <input
            ref={nameInputRef}
            value={name}
            maxLength={64}
            onChange={(event) => setName(event.target.value)}
            placeholder="e.g. Clean meeting notes"
            required
          />
        </label>

        <label className="filter-composer-name">
          Contextual shortcut
          <select
            value={shortcutSlot ?? ""}
            onChange={(event) =>
              setShortcutSlot(event.target.value ? Number(event.target.value) : null)
            }
          >
            <option value="">None</option>
            {Array.from({ length: 9 }, (_, index) => index + 1).map((slot) => (
              <option key={slot} value={slot} disabled={unavailableShortcutSlots.includes(slot)}>
                ⌘⌥{slot}
              </option>
            ))}
          </select>
          <small>
            Works only for an explicitly selected text clip; never in an input or dialog.
          </small>
        </label>

        <section className="filter-composer-workspace" aria-label="Filter steps and preview">
          <div className="filter-composer-steps">
            <div className="filter-composer-section-heading">
              <div>
                <strong>Steps</strong>
                <small>
                  {steps.length} / {FILTER_COMPOSER_MAX_STEPS}
                </small>
              </div>
              <span>Runs from top to bottom</span>
            </div>

            <ol aria-label="Filter steps">
              {steps.map((step, index) => {
                const stepHasError = preview.status === "failed" && preview.stepIndex === index;
                return (
                  <li key={step.id} data-error={stepHasError ? "true" : undefined}>
                    <span className="filter-composer-step-number">{index + 1}</span>
                    <label>
                      <span className="filter-composer-visually-hidden">Step {index + 1}</span>
                      <select
                        aria-label={`Step ${index + 1} transform`}
                        value={step.kind}
                        onChange={(event) => {
                          const kind = event.target.value as FilterComposerStepKind;
                          setSteps((current) =>
                            current.map((candidate) =>
                              candidate.id === step.id ? { ...candidate, kind } : candidate,
                            ),
                          );
                        }}
                      >
                        {FILTER_COMPOSER_STEP_KINDS.map((kind) => (
                          <option key={kind} value={kind}>
                            {FILTER_COMPOSER_STEP_LABELS[kind]}
                          </option>
                        ))}
                      </select>
                    </label>
                    <div className="filter-composer-step-actions">
                      <button
                        type="button"
                        onClick={() => moveStep(index, -1)}
                        disabled={index === 0}
                        aria-label={`Move step ${index + 1} up`}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        onClick={() => moveStep(index, 1)}
                        disabled={index === steps.length - 1}
                        aria-label={`Move step ${index + 1} down`}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setSteps((current) =>
                            current.length === 1
                              ? current
                              : current.filter((candidate) => candidate.id !== step.id),
                          )
                        }
                        disabled={steps.length === 1}
                        aria-label={`Remove step ${index + 1}`}
                      >
                        Remove
                      </button>
                    </div>
                  </li>
                );
              })}
            </ol>

            <div className="filter-composer-add-step">
              <label>
                Add transform
                <select
                  value={nextKind}
                  onChange={(event) => setNextKind(event.target.value as FilterComposerStepKind)}
                  disabled={steps.length >= FILTER_COMPOSER_MAX_STEPS}
                >
                  {FILTER_COMPOSER_STEP_KINDS.map((kind) => (
                    <option key={kind} value={kind}>
                      {FILTER_COMPOSER_STEP_LABELS[kind]}
                    </option>
                  ))}
                </select>
              </label>
              <button
                type="button"
                disabled={steps.length >= FILTER_COMPOSER_MAX_STEPS}
                onClick={() =>
                  setSteps((current) => [
                    ...current,
                    { id: `filter-step-${++sequenceRef.current}`, kind: nextKind },
                  ])
                }
              >
                Add step
              </button>
            </div>
          </div>

          <div className="filter-composer-preview">
            <div className="filter-composer-preview-pane">
              <div>
                <strong>Before</strong>
                <small>Original clip</small>
              </div>
              <pre>{input}</pre>
            </div>
            <div className="filter-composer-preview-pane" data-status={preview.status}>
              <div>
                <strong>After</strong>
                {preview.status === "ready" ? (
                  <small role="status">Preview ready</small>
                ) : (
                  <small>Preview unavailable</small>
                )}
              </div>
              {preview.status === "ready" ? (
                <pre>{preview.output}</pre>
              ) : (
                <p role="alert">
                  {preview.stepIndex === null ? "" : `Step ${preview.stepIndex + 1}: `}
                  {preview.message}
                </p>
              )}
            </div>
          </div>
        </section>

        {nameInvalid ? (
          <p className="filter-composer-help">Enter a name up to 64 characters.</p>
        ) : null}
        {saveError ? <p role="alert">{saveError}</p> : null}
        {confirmingDelete ? (
          <p role="alert">Delete this Filter definition? Existing clipboard Items are unchanged.</p>
        ) : null}

        <footer>
          <small>Preview stays in this window. Saving does not run the filter.</small>
          <div>
            {onDelete ? (
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  if (!confirmingDelete) {
                    setConfirmingDelete(true);
                    return;
                  }
                  setSubmitting(true);
                  setSaveError(null);
                  void Promise.resolve(onDelete())
                    .then(onClose)
                    .catch(() => {
                      setSaveError("Could not delete this local filter.");
                      setSubmitting(false);
                      setConfirmingDelete(false);
                    });
                }}
              >
                {confirmingDelete ? "Confirm delete" : "Delete definition"}
              </button>
            ) : null}
            <button
              type="button"
              onClick={() => (confirmingDelete ? setConfirmingDelete(false) : onClose())}
              disabled={busy}
            >
              {confirmingDelete ? "Keep filter" : "Cancel"}
            </button>
            <button
              type="submit"
              disabled={busy || confirmingDelete || nameInvalid || preview.status !== "ready"}
            >
              {busy ? "Saving…" : onDelete ? "Save changes" : "Save filter"}
            </button>
          </div>
        </footer>
      </form>
    </div>
  );
}
