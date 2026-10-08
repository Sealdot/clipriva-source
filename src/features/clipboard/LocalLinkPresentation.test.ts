import { describe, expect, it } from "vitest";
import {
  filterLocalLinkTransfers,
  normalizedLocalLinkStatus,
  presentLocalLinkDevice,
  presentLocalLinkFailure,
  presentLocalLinkRecovery,
  presentLocalLinkTransfer,
  presentOperationalLocalLinkDevice,
} from "./LocalLinkPresentation";
import type { LocalLinkDevice, LocalLinkTransfer } from "./types";

const base: LocalLinkTransfer = {
  id: "transfer",
  direction: "outgoing",
  deviceId: "peer",
  itemKind: "text",
  status: "connecting",
  createdAt: "2026-07-28T00:00:00.000Z",
  updatedAt: "2026-07-28T00:00:00.000Z",
};

describe("Local Link status presentation", () => {
  it.each([
    ["connecting", "Request created", false],
    ["encrypted", "Sending securely", false],
    ["awaitingReceiver", "Delivered to recipient", false],
    ["viewed", "Recipient viewed", false],
    ["copied", "Copied", true],
    ["saved", "Saved", true],
    ["rejected", "Rejected", true],
    ["cancelled", "Cancelled", true],
    ["expired", "Expired", true],
  ] as const)("presents %s as %s", (status, label, terminal) => {
    expect(presentLocalLinkTransfer({ ...base, status })).toMatchObject({ label, terminal });
  });

  it("fails closed when a receipt action contradicts the transfer status", () => {
    const inconsistent = { ...base, status: "copied" as const, receiverAction: "save" as const };
    expect(normalizedLocalLinkStatus(inconsistent)).toBe("failed");
    expect(presentLocalLinkTransfer(inconsistent)).toMatchObject({
      label: "Status could not be verified",
      terminal: true,
    });
  });

  it.each([
    "transportUnavailable",
    "deviceUnavailable",
    "authFailed",
    "versionMismatch",
    "receiverLocked",
    "receiverBusy",
    "rateLimited",
  ] as const)("states that blocked class %s did not deliver text", (failureReason) => {
    expect(presentLocalLinkFailure(failureReason).detail).toMatch(
      /No text (was delivered|was accepted)/i,
    );
  });

  it("filters by direction and failed state without inspecting content", () => {
    const transfers: LocalLinkTransfer[] = [
      { ...base, id: "incoming", direction: "incoming", status: "copied" },
      { ...base, id: "outgoing", direction: "outgoing", status: "awaitingReceiver" },
      { ...base, id: "failed", direction: "outgoing", status: "failed" },
    ];
    expect(filterLocalLinkTransfers(transfers, "incoming").map(({ id }) => id)).toEqual([
      "incoming",
    ]);
    expect(filterLocalLinkTransfers(transfers, "failed").map(({ id }) => id)).toEqual(["failed"]);
  });

  it.each([
    [{ trustStatus: "trusted", online: true }, "Online · Trusted"],
    [{ trustStatus: "trusted", online: false }, "Offline"],
    [{ trustStatus: "needsRePairing", online: true }, "Needs re-pairing"],
    [{ trustStatus: "revoked", online: true }, "Blocked"],
  ] as const)("maps the native device DTO to a user state", (fields, label) => {
    const device: LocalLinkDevice = {
      deviceId: "device",
      displayName: "Studio Mac",
      pairedAt: null,
      ...fields,
    };
    expect(presentLocalLinkDevice(device).label).toBe(label);
  });

  it("makes expiry and recovery paths explicit without exposing a payload", () => {
    const device: LocalLinkDevice = {
      deviceId: "device",
      displayName: "Studio Mac",
      pairedAt: null,
      trustStatus: "needsRePairing",
      rePairingReason: "trustExpired",
    };
    expect(presentLocalLinkDevice(device).detail).toMatch(/30-day trust period ended/i);
    expect(presentLocalLinkRecovery("rePairDevice")).toBe("Next step: re-pair this device.");
  });

  it("fails closed when an online device is stale while Local Link is off", () => {
    const device: LocalLinkDevice = {
      deviceId: "device",
      displayName: "Studio Mac",
      pairedAt: "2026-07-28T00:00:00.000Z",
      trustStatus: "trusted",
      online: true,
    };

    expect(
      presentOperationalLocalLinkDevice({ enabled: false, discoveryEnabled: false }, device),
    ).toMatchObject({
      state: "localLinkOff",
      label: "Local Link off",
      sendable: false,
      networkPresence: "Not checked while Local Link is off",
    });
  });

  it("reports a trusted online device as sendable only after enablement and discovery", () => {
    const device: LocalLinkDevice = {
      deviceId: "device",
      displayName: "Studio Mac",
      pairedAt: "2026-07-28T00:00:00.000Z",
      trustStatus: "trusted",
      online: true,
    };

    expect(
      presentOperationalLocalLinkDevice({ enabled: true, discoveryEnabled: true }, device),
    ).toMatchObject({
      state: "trustedOnline",
      label: "Online · Trusted",
      sendable: true,
      networkPresence: "Authenticated recently",
    });
  });

  it("does not flash a sendable state before Local Link preferences load", () => {
    const device: LocalLinkDevice = {
      deviceId: "device",
      displayName: "Studio Mac",
      pairedAt: "2026-07-28T00:00:00.000Z",
      trustStatus: "trusted",
      online: true,
    };

    expect(presentOperationalLocalLinkDevice(undefined, device)).toMatchObject({
      state: "checking",
      sendable: false,
    });
  });
});
