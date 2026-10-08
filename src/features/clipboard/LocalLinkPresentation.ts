import type {
  LocalLinkDevice,
  LocalLinkPreferences,
  LocalLinkTransfer,
  LocalLinkTransferFilter,
  LocalLinkTransferStatus,
} from "./types";

export type LocalLinkTone = "success" | "progress" | "warning" | "failure" | "neutral";

export interface LocalLinkStatusPresentation {
  label: string;
  detail: string;
  tone: LocalLinkTone;
  terminal: boolean;
}

export type LocalLinkDeviceUiState =
  | "pairing"
  | "trustedOnline"
  | "trustedOffline"
  | "needsRePairing"
  | "blocked";

export interface LocalLinkDevicePresentation {
  state: LocalLinkDeviceUiState;
  label: string;
  detail: string;
  tone: LocalLinkTone;
}

export type OperationalLocalLinkDeviceState =
  | LocalLinkDeviceUiState
  | "checking"
  | "localLinkOff"
  | "discoveryOff";

export interface OperationalLocalLinkDevicePresentation
  extends Omit<LocalLinkDevicePresentation, "state"> {
  state: OperationalLocalLinkDeviceState;
  sendable: boolean;
  networkPresence: string;
}

export function normalizedLocalLinkStatus(
  transfer: Pick<LocalLinkTransfer, "status" | "receiverAction">,
): LocalLinkTransferStatus {
  const receiptStatus =
    transfer.receiverAction === "copy"
      ? "copied"
      : transfer.receiverAction === "save"
        ? "saved"
        : transfer.receiverAction === "reject"
          ? "rejected"
          : null;
  if (receiptStatus && transfer.status !== receiptStatus) return "failed";
  return transfer.status;
}

export function presentLocalLinkTransfer(
  transfer: Pick<LocalLinkTransfer, "status" | "receiverAction" | "failureReason">,
): LocalLinkStatusPresentation {
  const status = normalizedLocalLinkStatus(transfer);
  switch (status) {
    case "connecting":
      return {
        label: "Request created",
        detail:
          "The handoff request was created. This does not mean the text has been delivered yet.",
        tone: "progress",
        terminal: false,
      };
    case "encrypted":
      return {
        label: "Sending securely",
        detail:
          "The request is moving through an authenticated local connection. It has not been delivered yet.",
        tone: "progress",
        terminal: false,
      };
    case "awaitingReceiver":
      return {
        label: "Delivered to recipient",
        detail: "The recipient can now choose Copy, Save, or Reject.",
        tone: "progress",
        terminal: false,
      };
    case "viewed":
      return {
        label: "Recipient viewed",
        detail: "The recipient opened Incoming Center and has not made a decision yet.",
        tone: "progress",
        terminal: false,
      };
    case "reconciling":
      return {
        label: "Confirming recipient result",
        detail:
          "The secure connection closed after delivery may have occurred. ClipRiva is querying status only and will not resend the text.",
        tone: "warning",
        terminal: false,
      };
    case "copied":
      return {
        label: "Copied",
        detail: "The recipient copied the text to the system clipboard.",
        tone: "success",
        terminal: true,
      };
    case "saved":
      return {
        label: "Saved",
        detail: "The recipient saved the text to local History.",
        tone: "success",
        terminal: true,
      };
    case "rejected":
      return {
        label: "Rejected",
        detail: "The recipient rejected the request. Pending text was cleared.",
        tone: "neutral",
        terminal: true,
      };
    case "cancelled":
      return {
        label: "Cancelled",
        detail: "The request was cancelled. Pending text was cleared.",
        tone: "neutral",
        terminal: true,
      };
    case "expired":
      return {
        label: "Expired",
        detail: "The request expired before a decision. Pending text was cleared.",
        tone: "warning",
        terminal: true,
      };
    case "failed":
      return normalizedLocalLinkStatus(transfer) === "failed" && transfer.receiverAction
        ? {
            label: "Status could not be verified",
            detail:
              "The receipt did not match the transfer state. Delivery is not being reported; review Transfers before trying again from the original Item.",
            tone: "failure",
            terminal: true,
          }
        : presentLocalLinkFailure(transfer.failureReason);
    default:
      return {
        label: "Preparing",
        detail: "Preparing the selected text on this Mac.",
        tone: "neutral",
        terminal: false,
      };
  }
}

export function presentLocalLinkFailure(
  reason: LocalLinkTransfer["failureReason"],
): LocalLinkStatusPresentation {
  const common = { label: "Not delivered", tone: "failure" as const, terminal: true };
  switch (reason) {
    case "transportUnavailable":
      return {
        ...common,
        detail:
          "No text was delivered because secure local transport is unavailable. Keep the original Item and try again after the device reconnects.",
      };
    case "deviceUnavailable":
      return {
        ...common,
        detail:
          "No text was delivered because the trusted device is offline. Check that both Macs are awake and on the same Wi-Fi, then start a new request.",
      };
    case "authFailed":
      return {
        ...common,
        detail:
          "No text was delivered because the device identity could not be verified. Re-pair the device before trying again.",
      };
    case "versionMismatch":
      return {
        ...common,
        detail:
          "No text was delivered because the other Mac uses an incompatible version. Update ClipRiva on both Macs before trying again.",
      };
    case "receiverLocked":
      return {
        ...common,
        detail:
          "No text was delivered because the receiving Mac is locked. Unlock it and start a new request from the original Item.",
      };
    case "receiverBusy":
      return {
        ...common,
        detail:
          "No text was delivered because the receiving Mac is handling another request. Wait for it to finish, then start a new request.",
      };
    case "rateLimited":
      return {
        ...common,
        detail:
          "No text was delivered because too many requests were started. Wait briefly, then retry from the original Item.",
      };
    case "captureBlocked":
      return {
        ...common,
        detail:
          "No text was delivered because this Item is protected by local privacy rules. Choose an allowed Item or review capture protection.",
      };
    case "deviceNotTrusted":
    case "deviceRevoked":
      return {
        ...common,
        detail:
          "No text was delivered because trust is missing or was revoked. Pair the device again before starting a new request.",
      };
    case "duplicate":
      return {
        ...common,
        detail:
          "This request was not delivered again because it was already handled. Check Transfers before starting a new request.",
      };
    case "protocolViolation":
      return {
        ...common,
        detail:
          "No text was delivered because the request failed safety validation. Re-pair the device or update both Macs before trying again.",
      };
    case "itemUnavailable":
      return {
        ...common,
        detail:
          "No text was delivered because the selected Item is no longer available. Select a current local Item and start a new request.",
      };
    case "unsupportedItem":
      return {
        ...common,
        detail:
          "No text was delivered because Local Link accepts UTF-8 text only. Select a text Item and start a new request.",
      };
    case "payloadUnavailable":
      return {
        ...common,
        detail:
          "No text was delivered because the pending request was cleared. Return to the original Item and start a new request.",
      };
    case "outcomeUnknown":
      return {
        label: "Result unknown",
        detail:
          "ClipRiva could not prove the recipient result and did not repeat the text or clipboard action. Check the other Mac before starting a new request.",
        tone: "warning",
        terminal: true,
      };
    default:
      return {
        ...common,
        detail:
          "No text was delivered because the request could not be completed. Check the device connection, then start a new request from the original Item.",
      };
  }
}

export function localLinkDeviceAvailability(device: LocalLinkDevice) {
  if (typeof device.online === "boolean") return device.online ? "online" : "offline";
  if (device.availability) return device.availability;
  return device.lastSeenAt ? "offline" : "unknown";
}

export function presentLocalLinkDevice(device: LocalLinkDevice): LocalLinkDevicePresentation {
  if (device.trustStatus === "needsRePairing") {
    return {
      state: "needsRePairing",
      label: "Needs re-pairing",
      detail:
        device.rePairingReason === "trustExpired"
          ? "Its 30-day trust period ended. Verify it again before sending."
          : "This device identity changed. Verify it again before sending.",
      tone: "warning",
    };
  }
  if (device.trustStatus === "revoked") {
    return {
      state: "blocked",
      label: "Blocked",
      detail: "This identity is no longer trusted.",
      tone: "failure",
    };
  }
  if (device.trustStatus === "pairing") {
    return {
      state: "pairing",
      label: "Pairing",
      detail: "Waiting for both Macs to confirm the same pairing code.",
      tone: "progress",
    };
  }
  if (localLinkDeviceAvailability(device) === "online") {
    return {
      state: "trustedOnline",
      label: "Online · Trusted",
      detail: "Available for a new text request.",
      tone: "success",
    };
  }
  return {
    state: "trustedOffline",
    label: "Offline",
    detail: "Trust is preserved, but this Mac is not currently reachable.",
    tone: "neutral",
  };
}

/**
 * Projects a device together with the feature-level switch. Native presence
 * can be stale after Local Link is turned off, so no device may remain
 * visually online or sendable without current feature consent and discovery.
 */
export function presentOperationalLocalLinkDevice(
  preferences: Pick<LocalLinkPreferences, "enabled" | "discoveryEnabled"> | null | undefined,
  device: LocalLinkDevice,
): OperationalLocalLinkDevicePresentation {
  if (!preferences) {
    return {
      state: "checking",
      label: "Checking status",
      detail: "Waiting for the current Local Link setting before showing availability.",
      tone: "neutral",
      sendable: false,
      networkPresence: "Checking current status",
    };
  }
  if (!preferences.enabled) {
    return {
      state: "localLinkOff",
      label: "Local Link off",
      detail: "Saved trust is retained, but this device is not reachable or sendable while off.",
      tone: "neutral",
      sendable: false,
      networkPresence: "Not checked while Local Link is off",
    };
  }
  if (!preferences.discoveryEnabled) {
    return {
      state: "discoveryOff",
      label: "Discovery off",
      detail: "Saved trust is retained, but no new secure session can start until discovery is on.",
      tone: "warning",
      sendable: false,
      networkPresence: "Not checked while discovery is off",
    };
  }

  const presentation = presentLocalLinkDevice(device);
  const sendable = presentation.state === "trustedOnline";
  return {
    ...presentation,
    sendable,
    networkPresence: sendable ? "Authenticated recently" : "Offline",
  };
}

export function presentLocalLinkRecovery(
  action: LocalLinkTransfer["recoveryAction"],
): string | null {
  switch (action) {
    case "rePairDevice":
      return "Next step: re-pair this device.";
    case "updateClipRiva":
      return "Next step: update ClipRiva on both Macs.";
    case "reviewLocalCapturePolicy":
      return "Next step: review local capture protection.";
    case "selectSupportedItem":
      return "Next step: select a current text Item.";
    case "checkTransferHistory":
      return "Next step: check Transfers before starting another request.";
    case "startNewTransferFromOriginalItem":
      return "Next step: start a new request from the original Item.";
    default:
      return null;
  }
}

export function formatLocalLinkProtocol(device: LocalLinkDevice) {
  if (device.protocolVersion) return device.protocolVersion;
  if (device.protocolMin == null || device.protocolMax == null) return "Protocol pending";
  return device.protocolMin === device.protocolMax
    ? `v${device.protocolMin}`
    : `v${device.protocolMin}–v${device.protocolMax}`;
}

export function filterLocalLinkTransfers(
  transfers: LocalLinkTransfer[],
  filter: LocalLinkTransferFilter,
) {
  if (filter === "all") return transfers;
  if (filter === "failed") {
    return transfers.filter((transfer) => normalizedLocalLinkStatus(transfer) === "failed");
  }
  return transfers.filter((transfer) => transfer.direction === filter);
}

export function formatLocalLinkFingerprint(value?: string | null) {
  return value?.trim() || "Identity pending";
}
