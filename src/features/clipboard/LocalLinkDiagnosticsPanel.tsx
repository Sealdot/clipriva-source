import { Download, FileSearch, ShieldCheck, X } from "lucide-react";
import { useState } from "react";
import { buildLocalLinkDiagnostics } from "./api";
import type { LocalLinkDiagnostics } from "./types";

/** Explicit, on-demand preview/export for content-free Local Link support data. */
export function LocalLinkDiagnosticsPanel() {
  const [bundle, setBundle] = useState<LocalLinkDiagnostics | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const preview = async () => {
    setLoading(true);
    setError(null);
    try {
      setBundle(await buildLocalLinkDiagnostics());
    } catch (reason) {
      setError(String(reason).replace(/^Error:\s*/u, ""));
    } finally {
      setLoading(false);
    }
  };

  return (
    <section className="local-link-diagnostics" aria-label="Local Link diagnostics">
      <div>
        <span className="local-link-diagnostics-icon" aria-hidden="true">
          <ShieldCheck size={17} />
        </span>
        <span>
          <strong>Content-free diagnostics</strong>
          <small>
            Built only when requested. Includes readiness and up to 50 pseudonymous metadata
            records—never text, device names, stable IDs, fingerprints, addresses, or paths.
          </small>
        </span>
      </div>
      <button
        type="button"
        className="text-button"
        onClick={() => void preview()}
        disabled={loading}
      >
        <FileSearch size={14} aria-hidden="true" />
        {loading ? "Preparing…" : "Preview diagnostics"}
      </button>

      {bundle ? (
        <div
          className="local-link-diagnostics-preview"
          role="dialog"
          aria-label="Diagnostics preview"
        >
          <button
            type="button"
            className="icon-button"
            onClick={() => setBundle(null)}
            aria-label="Close diagnostics preview"
          >
            <X size={14} />
          </button>
          <span className="eyebrow">Review before download</span>
          <strong>{bundle.recordCount} metadata records</strong>
          <p>
            Schema {bundle.schemaVersion} · {bundle.osFamily} / {bundle.architecture} · readiness{" "}
            {bundle.readiness.state}
          </p>
          <button
            type="button"
            className="settings-save"
            onClick={() => downloadLocalLinkDiagnostics(bundle)}
          >
            <Download size={14} aria-hidden="true" /> Download JSON
          </button>
        </div>
      ) : null}
      {error ? (
        <p className="local-link-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}

function downloadLocalLinkDiagnostics(data: LocalLinkDiagnostics) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
  const objectUrl = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = objectUrl;
  link.download = "clipriva-local-link-diagnostics.json";
  link.click();
  URL.revokeObjectURL(objectUrl);
}
