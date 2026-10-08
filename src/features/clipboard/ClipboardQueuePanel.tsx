import type { ClipboardQueueState } from "./clipboardQueue";
import { getCurrentClipboardQueueEntry } from "./clipboardQueue";
import "./ClipboardQueuePanel.css";

interface ClipboardQueuePanelProps {
  state: ClipboardQueueState;
  activationMode: "copy" | "paste";
  failureMessage?: string | null;
  getItemLabel?: (itemId: string) => string;
  onActivate: (itemId: string) => void;
  onRetry: (itemId: string) => void;
  onCopyAdvance: (itemId: string) => void;
  onSkip: (itemId: string) => void;
  onMove?: (from: number, to: number) => void;
  onRemove?: (index: number) => void;
  onReset: () => void;
}

/**
 * Presentation-only queue controls. The parent owns reducer dispatch and any
 * activation side effects, which keeps this component independent of queries.
 */
export function ClipboardQueuePanel({
  state,
  activationMode,
  failureMessage = null,
  getItemLabel = (itemId) => itemId,
  onActivate,
  onRetry,
  onCopyAdvance,
  onSkip,
  onMove,
  onRemove,
  onReset,
}: ClipboardQueuePanelProps) {
  const current = getCurrentClipboardQueueEntry(state);
  const completedCount = Math.min(state.cursor, state.order.length);

  return (
    <section className="clipboard-queue-panel" aria-labelledby="clipboard-queue-heading">
      <header>
        <div>
          <span>Temporary · up to 20 clips · clears when ClipRiva quits</span>
          <h2 id="clipboard-queue-heading">Stack</h2>
        </div>
        <output aria-label="Stack progress">
          {completedCount} / {state.order.length}
        </output>
      </header>

      {state.order.length === 0 ? (
        <p className="clipboard-queue-empty">Add clips or start collecting to build a Stack.</p>
      ) : (
        <ol className="clipboard-queue-order" aria-label="Stack clips">
          {state.order.map((entry, index) => {
            const canChange = index >= state.cursor && state.busyToken === null;
            return (
              <li
                key={entry.itemId}
                data-current={index === state.cursor ? "true" : undefined}
                data-completed={index < state.cursor ? "true" : undefined}
                data-availability={entry.availability}
                draggable={Boolean(onMove && canChange)}
                onDragStart={(event) => {
                  event.dataTransfer.setData("application/x-clipriva-stack-index", String(index));
                  event.dataTransfer.effectAllowed = "move";
                }}
                onDragOver={(event) => {
                  if (onMove && canChange) event.preventDefault();
                }}
                onDrop={(event) => {
                  const from = Number(
                    event.dataTransfer.getData("application/x-clipriva-stack-index"),
                  );
                  if (onMove && canChange && Number.isInteger(from)) onMove(from, index);
                }}
              >
                <span>{index + 1}</span>
                <strong>{getItemLabel(entry.itemId)}</strong>
                <small>
                  {entry.availability === "unavailable"
                    ? "Unavailable"
                    : index < state.cursor
                      ? "Completed"
                      : index === state.cursor
                        ? "Current"
                        : index === state.cursor + 1
                          ? "Next"
                          : "Waiting"}
                </small>
                {canChange && (onMove || onRemove) ? (
                  <span className="clipboard-stack-item-actions">
                    {onMove ? (
                      <>
                        <button
                          type="button"
                          aria-label={`Move Stack item ${index + 1} up`}
                          disabled={index === state.cursor}
                          onClick={() => onMove(index, index - 1)}
                        >
                          ↑
                        </button>
                        <button
                          type="button"
                          aria-label={`Move Stack item ${index + 1} down`}
                          disabled={index === state.order.length - 1}
                          onClick={() => onMove(index, index + 1)}
                        >
                          ↓
                        </button>
                      </>
                    ) : null}
                    {onRemove ? (
                      <button
                        type="button"
                        aria-label={`Remove Stack item ${index + 1}`}
                        onClick={() => onRemove(index)}
                      >
                        ×
                      </button>
                    ) : null}
                  </span>
                ) : null}
              </li>
            );
          })}
        </ol>
      )}

      {current ? (
        <div className="clipboard-queue-controls">
          {current.availability === "unavailable" ? (
            <p role="status">This clip was deleted or recycled. Skip it to continue.</p>
          ) : failureMessage ? (
            <p role="alert">{failureMessage}</p>
          ) : state.busyToken ? (
            <p role="status">Activating current clip…</p>
          ) : null}

          <div>
            {current.availability === "available" && failureMessage ? (
              <>
                <button type="button" onClick={() => onRetry(current.itemId)}>
                  {activationMode === "copy" ? "Retry Copy" : "Retry Direct Paste"}
                </button>
                {activationMode === "paste" ? (
                  <button type="button" onClick={() => onCopyAdvance(current.itemId)}>
                    Copy &amp; advance
                  </button>
                ) : null}
              </>
            ) : current.availability === "available" ? (
              <button
                type="button"
                onClick={() => onActivate(current.itemId)}
                disabled={state.busyToken !== null}
              >
                {state.busyToken
                  ? "Working…"
                  : activationMode === "copy"
                    ? "Copy & advance"
                    : "Direct Paste & advance"}
              </button>
            ) : null}
            <button
              type="button"
              onClick={() => onSkip(current.itemId)}
              disabled={state.busyToken !== null}
            >
              Skip
            </button>
          </div>
        </div>
      ) : state.order.length > 0 ? (
        <p className="clipboard-queue-complete" role="status">
          Stack complete.
        </p>
      ) : null}

      {state.order.length > 0 ? (
        <button
          type="button"
          className="clipboard-queue-reset"
          onClick={onReset}
          disabled={state.busyToken !== null}
        >
          Reset Stack
        </button>
      ) : null}
    </section>
  );
}
