import { localLinkDeviceAvailability } from "./LocalLinkPresentation";
import type { LocalLinkDevice, LocalLinkPreferences } from "./types";

export type LocalLinkReadinessState = "ready" | "actionRequired" | "offline";
export type LocalLinkReadinessAction =
  | "enableLocalLink"
  | "enableDiscovery"
  | "pairDevice"
  | "rePairDevice"
  | "refreshDevices"
  | null;
export type LocalLinkReadinessCheckStatus = "passed" | "blocked" | "pending";

export interface LocalLinkReadinessCheck {
  id: "localNetwork" | "localLink" | "discovery" | "trust" | "handshake";
  label: string;
  detail: string;
  status: LocalLinkReadinessCheckStatus;
}

export interface LocalLinkReadiness {
  state: LocalLinkReadinessState;
  label: "Ready to send" | "Action needed" | "Offline";
  detail: string;
  primaryAction: LocalLinkReadinessAction;
  primaryActionLabel: string | null;
  actionDeviceId: string | null;
  readyDeviceCount: number;
  pendingRequestCapacity: number;
  checks: LocalLinkReadinessCheck[];
}

const checkOrder: LocalLinkReadinessCheck["id"][] = [
  "localNetwork",
  "localLink",
  "discovery",
  "trust",
  "handshake",
];

/**
 * Projects native-safe metadata into one user-facing conclusion. The order is
 * deliberate: every blocked result stops at the first action the user can
 * take, and reachability can never substitute for authenticated trust.
 */
export function deriveLocalLinkReadiness(
  preferences: Pick<LocalLinkPreferences, "enabled" | "discoveryEnabled"> | null | undefined,
  devices: LocalLinkDevice[],
): LocalLinkReadiness {
  const enabled = Boolean(preferences?.enabled);
  const discoveryEnabled = enabled && Boolean(preferences?.discoveryEnabled);
  const trusted = devices.filter((device) => device.trustStatus === "trusted");
  const ready = trusted.filter((device) => localLinkDeviceAvailability(device) === "online");
  const needsRePairing = devices.filter((device) => device.trustStatus === "needsRePairing");

  const checks: LocalLinkReadinessCheck[] = [
    enabled
      ? passed(
          "localNetwork",
          "Local network access",
          "The native local-network runtime started successfully.",
        )
      : pending(
          "localNetwork",
          "Local network access",
          "Checked by macOS when Local Link is explicitly turned on.",
        ),
    enabled
      ? passed("localLink", "Local Link", "Local Link is on.")
      : blocked("localLink", "Local Link", "Local Link is off."),
    discoveryEnabled
      ? passed("discovery", "Network discovery", "Nearby-device discovery is active.")
      : enabled
        ? blocked("discovery", "Network discovery", "Nearby-device discovery is off.")
        : pending("discovery", "Network discovery", "Waiting for Local Link."),
    trusted.length > 0
      ? passed(
          "trust",
          "Device trust",
          `${trusted.length} verified ${trusted.length === 1 ? "device" : "devices"}.`,
        )
      : needsRePairing.length > 0
        ? blocked(
            "trust",
            "Device trust",
            `${needsRePairing.length} ${needsRePairing.length === 1 ? "device needs" : "devices need"} identity verification.`,
          )
        : discoveryEnabled
          ? blocked("trust", "Device trust", "No verified device is available yet.")
          : pending("trust", "Device trust", "Waiting for discovery."),
    ready.length > 0
      ? passed(
          "handshake",
          "Secure session",
          `${ready.length} verified ${ready.length === 1 ? "device is" : "devices are"} online.`,
        )
      : trusted.length > 0
        ? blocked("handshake", "Secure session", "Verified devices are currently offline.")
        : pending("handshake", "Secure session", "Waiting for a verified device."),
  ];

  if (!enabled) {
    return result({
      state: "actionRequired",
      label: "Action needed",
      detail: "Turn on Local Link to check local-network access and nearby devices.",
      primaryAction: "enableLocalLink",
      primaryActionLabel: "Turn on Local Link",
      checks,
      readyDeviceCount: 0,
    });
  }
  if (!discoveryEnabled) {
    return result({
      state: "actionRequired",
      label: "Action needed",
      detail: "Nearby-device discovery is off. No new pairing or session can start.",
      primaryAction: "enableDiscovery",
      primaryActionLabel: "Enable discovery",
      checks,
      readyDeviceCount: 0,
    });
  }
  if (trusted.length === 0) {
    const rePairingDevice = needsRePairing[0];
    return result({
      state: "actionRequired",
      label: "Action needed",
      detail: rePairingDevice
        ? "A previous trust binding needs identity verification before sending."
        : "Pair and verify a nearby Mac before sending text.",
      primaryAction: rePairingDevice ? "rePairDevice" : "pairDevice",
      primaryActionLabel: rePairingDevice ? "Verify device again" : "Pair a device",
      actionDeviceId: rePairingDevice?.deviceId ?? null,
      checks,
      readyDeviceCount: 0,
    });
  }
  if (ready.length === 0) {
    return result({
      state: "offline",
      label: "Offline",
      detail: "Trusted devices are preserved, but none can complete a secure session now.",
      primaryAction: "refreshDevices",
      primaryActionLabel: "Refresh status",
      checks,
      readyDeviceCount: 0,
    });
  }
  return result({
    state: "ready",
    label: "Ready to send",
    detail: `${ready.length} verified ${ready.length === 1 ? "device is" : "devices are"} online and ready for an explicit text request.`,
    primaryAction: null,
    primaryActionLabel: null,
    checks,
    readyDeviceCount: ready.length,
  });
}

function result(
  value: Omit<LocalLinkReadiness, "actionDeviceId" | "pendingRequestCapacity"> & {
    actionDeviceId?: string | null;
  },
): LocalLinkReadiness {
  return {
    ...value,
    actionDeviceId: value.actionDeviceId ?? null,
    // Native Local Link enforces a global maximum of three undecided bodies.
    pendingRequestCapacity: 3,
    checks: [...value.checks].sort(
      (left, right) => checkOrder.indexOf(left.id) - checkOrder.indexOf(right.id),
    ),
  };
}

function passed(
  id: LocalLinkReadinessCheck["id"],
  label: string,
  detail: string,
): LocalLinkReadinessCheck {
  return { id, label, detail, status: "passed" };
}

function blocked(
  id: LocalLinkReadinessCheck["id"],
  label: string,
  detail: string,
): LocalLinkReadinessCheck {
  return { id, label, detail, status: "blocked" };
}

function pending(
  id: LocalLinkReadinessCheck["id"],
  label: string,
  detail: string,
): LocalLinkReadinessCheck {
  return { id, label, detail, status: "pending" };
}
