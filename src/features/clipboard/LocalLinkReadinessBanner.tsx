import { AlertTriangle, Check, Circle, RefreshCw, WifiOff } from "lucide-react";
import { deriveLocalLinkReadiness } from "./LocalLinkReadiness";
import { useLocalLinkActions } from "./queries";
import type { LocalLinkDevice, LocalLinkPreferences } from "./types";

export function LocalLinkReadinessBanner({
  preferences,
  devices,
}: {
  preferences: LocalLinkPreferences | null | undefined;
  devices: LocalLinkDevice[];
}) {
  const actions = useLocalLinkActions();
  const readiness = deriveLocalLinkReadiness(preferences, devices);

  const performPrimaryAction = () => {
    if (!preferences) return;
    if (readiness.primaryAction === "enableLocalLink") {
      actions.updatePreferences.mutate({ ...preferences, enabled: true });
      return;
    }
    if (readiness.primaryAction === "enableDiscovery") {
      actions.updatePreferences.mutate({ ...preferences, discoveryEnabled: true });
      return;
    }
    if (readiness.primaryAction === "refreshDevices") {
      void actions.refreshPreferences();
      return;
    }
    document.querySelector("[aria-label='Local Link devices']")?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  };

  const StateIcon =
    readiness.state === "ready" ? Check : readiness.state === "offline" ? WifiOff : AlertTriangle;

  return (
    <section
      className="local-link-readiness"
      data-state={readiness.state}
      aria-label="Local Link readiness"
    >
      <div className="local-link-readiness-summary">
        <span className="local-link-readiness-icon" aria-hidden="true">
          <StateIcon size={19} />
        </span>
        <div>
          <span className="eyebrow">Device readiness</span>
          <h2>{readiness.label}</h2>
          <p>{readiness.detail}</p>
        </div>
        {readiness.primaryActionLabel ? (
          <button
            type="button"
            className="settings-save"
            onClick={performPrimaryAction}
            disabled={actions.updatePreferences.isPending}
          >
            {readiness.primaryAction === "refreshDevices" ? (
              <RefreshCw size={14} aria-hidden="true" />
            ) : null}
            {actions.updatePreferences.isPending ? "Updating…" : readiness.primaryActionLabel}
          </button>
        ) : (
          <span className="local-link-ready-count">
            {readiness.readyDeviceCount} ready · {readiness.pendingRequestCapacity} pending slots
          </span>
        )}
      </div>
      <details className="local-link-readiness-details">
        <summary>View readiness checks</summary>
        <ol className="local-link-readiness-checks" aria-label="Readiness checks">
          {readiness.checks.map((check) => (
            <li key={check.id} data-status={check.status} title={check.detail}>
              {check.status === "passed" ? (
                <Check size={12} aria-hidden="true" />
              ) : (
                <Circle size={10} aria-hidden="true" />
              )}
              <span>{check.label}</span>
            </li>
          ))}
        </ol>
      </details>
    </section>
  );
}
