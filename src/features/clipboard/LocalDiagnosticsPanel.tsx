import { Download, RotateCcw, ShieldCheck } from "lucide-react";
import { useState } from "react";
import type { LocalDiagnosticsExport, LocalDiagnosticsSummary } from "./types";

interface LocalDiagnosticsPanelProps {
  enabled: boolean;
  summary?: LocalDiagnosticsSummary;
  isClearing?: boolean;
  isExporting?: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onClear?: () => void;
  onExport?: () => Promise<LocalDiagnosticsExport>;
}

/**
 * A secondary Settings disclosure for anonymous, opt-in diagnostics. It
 * deliberately receives only aggregate counts and a typed export; neither
 * the panel nor its props accept clipboard content or identifying metadata.
 */
export function LocalDiagnosticsPanel({
  enabled,
  summary,
  isClearing = false,
  isExporting = false,
  onEnabledChange,
  onClear,
  onExport,
}: LocalDiagnosticsPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const firstRecoveryRate = formatDiagnosticRate(summary?.firstRecovery.percent);
  const directPasteRate = formatDiagnosticRate(summary?.directPaste.percent);

  const exportAnonymousDiagnostics = async () => {
    if (!onExport) return;
    setExportError(null);
    try {
      const data = await onExport();
      downloadAnonymousDiagnostics(data);
    } catch (error) {
      setExportError(String(error).replace(/^Error:\s*/, ""));
    }
  };

  return (
    <section className="local-diagnostics" aria-labelledby="local-diagnostics-heading">
      <div className="local-diagnostics-heading">
        <div>
          <span className="eyebrow">Optional</span>
          <h4 id="local-diagnostics-heading">Diagnostics</h4>
          <p>Keep anonymous reliability counts only on this Mac.</p>
        </div>
        <button
          type="button"
          className="text-button"
          aria-expanded={isOpen}
          aria-controls="local-diagnostics-details"
          onClick={() => setIsOpen((current) => !current)}
        >
          {isOpen ? "Hide" : "View diagnostics"}
        </button>
      </div>

      {isOpen ? (
        <div id="local-diagnostics-details" className="local-diagnostics-details">
          <label className="settings-toggle local-diagnostics-toggle">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => onEnabledChange(event.target.checked)}
            />
            <span>
              <strong>Collect anonymous local experience diagnostics</strong>
              <small>
                Records fixed event and result categories, a day bucket, and the app version. It
                never records clip contents, file paths, document or app names, account/network IDs,
                or reversible fingerprints.
              </small>
            </span>
            <ShieldCheck size={18} aria-hidden="true" />
          </label>

          <dl className="local-diagnostics-metrics" aria-label="Local diagnostics summary">
            <div>
              <dt>First recovery completion</dt>
              <dd>{firstRecoveryRate}</dd>
              <small>{summary?.firstRecovery.attempts ?? 0} starts</small>
            </div>
            <div>
              <dt>Direct Paste degradation</dt>
              <dd>{directPasteRate}</dd>
              <small>{summary?.directPaste.attempts ?? 0} attempts</small>
            </div>
            <div>
              <dt>Recycle Bin restores</dt>
              <dd>{summary?.recycleBinRestoreCount ?? 0}</dd>
              <small>completed restores</small>
            </div>
            <div>
              <dt>Duplicate group expands</dt>
              <dd>{summary?.duplicateGroupExpandCount ?? 0}</dd>
              <small>local views expanded</small>
            </div>
          </dl>

          <p className="local-diagnostics-event-count">
            {summary?.storedEventCount ?? 0} anonymous local event
            {(summary?.storedEventCount ?? 0) === 1 ? "" : "s"} retained for up to 90 days.
          </p>

          {summary && summary.criticalErrorCategories.length > 0 ? (
            <section className="local-diagnostics-errors" aria-label="Key error categories">
              <strong>Key error categories</strong>
              <ul>
                {summary.criticalErrorCategories.map((category) => (
                  <li key={`${category.eventType}-${category.outcome}`}>
                    {diagnosticCategoryLabel(category.eventType, category.outcome)} ·{" "}
                    {category.count}
                  </li>
                ))}
              </ul>
            </section>
          ) : (
            <p className="local-diagnostics-empty">No key error categories recorded.</p>
          )}

          <div className="local-diagnostics-actions">
            {onClear ? (
              <button
                type="button"
                className="text-button"
                onClick={onClear}
                disabled={isClearing || (summary?.storedEventCount ?? 0) === 0}
              >
                <RotateCcw size={14} aria-hidden="true" />
                {isClearing ? "Clearing…" : "Clear local diagnostics"}
              </button>
            ) : null}
            {onExport ? (
              <button
                type="button"
                className="text-button"
                onClick={() => void exportAnonymousDiagnostics()}
                disabled={isExporting}
              >
                <Download size={14} aria-hidden="true" />
                {isExporting ? "Preparing…" : "Export anonymous diagnostics"}
              </button>
            ) : null}
          </div>
          {exportError ? (
            <p className="settings-save-error" role="alert">
              {exportError}
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function formatDiagnosticRate(percent: number | null | undefined) {
  return percent === null || percent === undefined ? "—" : `${percent}%`;
}

function diagnosticCategoryLabel(eventType: string, outcome: string) {
  const labels: Record<string, string> = {
    "directPaste:failed": "Direct Paste failure",
    "directPaste:restricted": "Direct Paste restricted",
    "clipboardRestoreError:failed": "Clipboard restore error",
    "capturePermissionRestricted:restricted": "Capture permission restricted",
    "systemPolicyRestricted:restricted": "System policy restricted",
    "recycleBinRestore:failed": "Recycle Bin restore failure",
  };
  return labels[`${eventType}:${outcome}`] ?? `${eventType} ${outcome}`;
}

function downloadAnonymousDiagnostics(data: LocalDiagnosticsExport) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = objectUrl;
  link.download = "clipriva-anonymous-local-diagnostics.json";
  link.click();
  URL.revokeObjectURL(objectUrl);
}
