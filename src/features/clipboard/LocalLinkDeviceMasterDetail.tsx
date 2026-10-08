import { Activity, Check, Laptop, ShieldCheck, WifiOff } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { formatRelativeTime } from "./format";
import {
  formatLocalLinkFingerprint,
  localLinkDeviceAvailability,
  presentOperationalLocalLinkDevice,
} from "./LocalLinkPresentation";
import type { LocalLinkDevice, LocalLinkPreferences } from "./types";

export function LocalLinkDeviceMasterDetail({
  preferences,
  devices,
  onOpenTransfers,
}: {
  preferences: LocalLinkPreferences | null | undefined;
  devices: LocalLinkDevice[];
  onOpenTransfers: () => void;
}) {
  const visibleDevices = useMemo(
    () =>
      [...devices]
        .filter((device) => device.trustStatus !== "revoked" || !device.revokedAt)
        .sort(
          (left, right) =>
            devicePriority(left) - devicePriority(right) ||
            left.displayName.localeCompare(right.displayName),
        ),
    [devices],
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    setSelectedId((current) =>
      current && visibleDevices.some((device) => device.deviceId === current)
        ? current
        : (visibleDevices[0]?.deviceId ?? null),
    );
  }, [visibleDevices]);

  if (visibleDevices.length === 0) return null;
  const selected =
    visibleDevices.find((device) => device.deviceId === selectedId) ?? visibleDevices[0];
  if (!selected) return null;
  const presentation = presentOperationalLocalLinkDevice(preferences, selected);

  return (
    <section className="local-link-device-master-detail" aria-label="Device status and details">
      <div className="local-link-device-master">
        <header>
          <div>
            <span className="eyebrow">Verified and nearby</span>
            <h2>Devices</h2>
          </div>
          <span>{visibleDevices.length}</span>
        </header>
        <ul>
          {visibleDevices.map((device) => {
            const devicePresentation = presentOperationalLocalLinkDevice(preferences, device);
            const online = devicePresentation.sendable;
            return (
              <li key={device.deviceId}>
                <button
                  type="button"
                  data-selected={device.deviceId === selected.deviceId}
                  onClick={() => setSelectedId(device.deviceId)}
                  aria-pressed={device.deviceId === selected.deviceId}
                >
                  <span className="local-link-device-master-icon" data-online={online}>
                    {online ? <Laptop size={16} /> : <WifiOff size={15} />}
                  </span>
                  <span>
                    <strong>{device.displayName}</strong>
                    <small>
                      {devicePresentation.label}
                      {device.lastSeenAt ? ` · ${formatRelativeTime(device.lastSeenAt)}` : ""}
                    </small>
                  </span>
                  <em>{devicePresentation.sendable ? "Sendable" : devicePresentation.label}</em>
                </button>
              </li>
            );
          })}
        </ul>
      </div>

      <article className="local-link-device-detail" aria-label={`${selected.displayName} details`}>
        <header>
          <div>
            <span className="eyebrow">Selected device</span>
            <h2>{selected.displayName}</h2>
          </div>
          <strong data-tone={presentation.tone}>{presentation.label}</strong>
        </header>
        <details className="local-link-device-security-details">
          <summary>Trust and connection details</summary>
          <dl>
            <div>
              <dt>Network presence</dt>
              <dd>{presentation.networkPresence}</dd>
            </div>
            <div>
              <dt>Identity fingerprint</dt>
              <dd>{formatLocalLinkFingerprint(selected.publicKeyFingerprint)}</dd>
            </div>
            <div>
              <dt>Trust period</dt>
              <dd>{trustPeriod(selected)}</dd>
            </div>
            <div>
              <dt>Last activity</dt>
              <dd>
                {selected.lastTransferAt
                  ? formatRelativeTime(selected.lastTransferAt)
                  : "No transfer yet"}
              </dd>
            </div>
          </dl>
        </details>
        <div className="local-link-device-detail-actions">
          <button type="button" className="text-button" onClick={onOpenTransfers}>
            <Activity size={14} aria-hidden="true" /> View transfer activity
          </button>
          <button
            type="button"
            className="text-button"
            onClick={() =>
              document.querySelector("[aria-label='Local Link devices']")?.scrollIntoView({
                behavior: "smooth",
                block: "start",
              })
            }
          >
            {selected.trustStatus === "trusted" ? (
              <ShieldCheck size={14} aria-hidden="true" />
            ) : (
              <Check size={14} aria-hidden="true" />
            )}
            Manage trust
          </button>
        </div>
        <small>Clipboard text is never shown in device details or diagnostics.</small>
      </article>
    </section>
  );
}

function devicePriority(device: LocalLinkDevice) {
  if (device.trustStatus === "trusted" && localLinkDeviceAvailability(device) === "online")
    return 0;
  if (device.trustStatus === "trusted") return 1;
  if (device.trustStatus === "needsRePairing") return 2;
  return 3;
}

function trustPeriod(device: LocalLinkDevice) {
  if (device.trustStatus === "needsRePairing") return "Verification required";
  if (device.trustDuration === "thisSession") return "This session";
  if (device.trustDuration === "always") return "Until revoked";
  if (device.trustExpiresAt) return `Until ${new Date(device.trustExpiresAt).toLocaleDateString()}`;
  return "30 days";
}
