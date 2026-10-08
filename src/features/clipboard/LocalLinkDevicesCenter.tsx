import { Activity } from "lucide-react";
import { formatRelativeTime } from "./format";
import { LocalLinkDeviceMasterDetail } from "./LocalLinkDeviceMasterDetail";
import { LocalLinkDevicesSettings } from "./LocalLinkDevicesSettings";
import { LocalLinkDiagnosticsPanel } from "./LocalLinkDiagnosticsPanel";
import { presentLocalLinkTransfer } from "./LocalLinkPresentation";
import { LocalLinkReadinessBanner } from "./LocalLinkReadinessBanner";
import { LocalLinkStatusBadge } from "./LocalLinkUi";
import { useLocalLinkDevices, useLocalLinkPreferences, useLocalLinkTransfers } from "./queries";

interface LocalLinkDevicesCenterProps {
  onOpenTransfers: () => void;
}

/**
 * A first-class home for Local Link. It intentionally renders only metadata:
 * the trusted-device controls own pairing, while recent activity never gets a
 * preview, payload or old-payload retry affordance.
 */
export function LocalLinkDevicesCenter({ onOpenTransfers }: LocalLinkDevicesCenterProps) {
  const preferencesQuery = useLocalLinkPreferences();
  const devicesQuery = useLocalLinkDevices(true);
  const transfersQuery = useLocalLinkTransfers(true);
  const devices = devicesQuery.data ?? [];
  const recentOutgoing = (transfersQuery.data ?? [])
    .filter((transfer) => transfer.direction === "outgoing")
    .slice(0, 4);

  return (
    <section className="devices-workspace" aria-label="Device center">
      <header className="devices-workspace-heading">
        <div>
          <span className="eyebrow">Local Link · Preview</span>
          <h1>Devices</h1>
          <p>
            Pair a trusted Mac and explicitly send one selected text item. Nothing synchronizes or
            saves automatically.
          </p>
        </div>
        <button
          type="button"
          className="text-button"
          onClick={onOpenTransfers}
          aria-label="Open Local Link transfers"
        >
          <Activity size={15} aria-hidden="true" /> Recent activity
        </button>
      </header>

      <LocalLinkReadinessBanner preferences={preferencesQuery.data} devices={devices} />

      <LocalLinkDeviceMasterDetail
        preferences={preferencesQuery.data}
        devices={devices}
        onOpenTransfers={onOpenTransfers}
      />

      <section className="devices-recent-sends" aria-label="Recent sends">
        <div className="devices-recent-sends-heading">
          <div>
            <span className="eyebrow">Metadata-only history</span>
            <h2>Recent sends</h2>
          </div>
          <button type="button" className="text-button" onClick={onOpenTransfers}>
            View all
          </button>
        </div>
        {recentOutgoing.length === 0 ? (
          <p>No recent sends. A send result will appear here without retaining its text.</p>
        ) : (
          <ol>
            {recentOutgoing.map((transfer) => {
              const presentation = presentLocalLinkTransfer(transfer);
              return (
                <li key={transfer.id}>
                  <div>
                    <strong>{transfer.peerDisplayName ?? "Trusted Mac"}</strong>
                    <small>
                      Text · {formatRelativeTime(transfer.updatedAt)} · no text retained here
                    </small>
                  </div>
                  <LocalLinkStatusBadge label={presentation.label} tone={presentation.tone} />
                </li>
              );
            })}
          </ol>
        )}
      </section>

      <details className="devices-secondary-tools">
        <summary>Device management and diagnostics</summary>
        <div>
          <LocalLinkDevicesSettings />
          <LocalLinkDiagnosticsPanel />
        </div>
      </details>
    </section>
  );
}
