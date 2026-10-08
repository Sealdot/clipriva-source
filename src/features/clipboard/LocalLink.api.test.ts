import { beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
}));

vi.mock("@tauri-apps/api/core", () => tauri);

import {
  beginLocalLinkPairing,
  cancelLocalLinkTransfer,
  clearLocalLinkTransfers,
  confirmLocalLinkPairing,
  getLocalLinkPairing,
  getLocalLinkPreferences,
  listLocalLinkDevices,
  resetLocalLinkIdentity,
  revealLocalLinkTransfer,
  updateLocalLinkPreferences,
} from "./api";
import type { LocalLinkPairing, LocalLinkPreferences, LocalLinkTransfer } from "./types";

const enabledPreferences: LocalLinkPreferences = {
  enabled: true,
  discoveryEnabled: false,
  deviceName: "Work Mac",
  identityFingerprint: null,
  protocolVersion: null,
};

const cancelledTransfer: LocalLinkTransfer = {
  id: "transfer-42",
  direction: "outgoing",
  deviceId: "device-42",
  itemKind: "text",
  status: "cancelled",
  createdAt: "2026-07-28T00:00:00.000Z",
  updatedAt: "2026-07-28T00:00:01.000Z",
};

const nativePairing: LocalLinkPairing = {
  device: {
    deviceId: "device-42",
    displayName: "Studio Mac",
    trustStatus: "pairing",
    pairedAt: "2026-07-28T00:00:00.000Z",
  },
  verificationCode: "482915",
  expiresAt: "2026-07-28T00:01:00.000Z",
  localFingerprint: "A1B2 · C3D4",
  peerFingerprint: "E5F6 · A7B8",
  localConfirmed: false,
  peerConfirmed: false,
};

describe("Local Link desktop API contract", () => {
  beforeEach(() => {
    tauri.invoke.mockReset();
    tauri.isTauri.mockReturnValue(true);
  });

  it("reads identity and protocol only when Local Link is enabled", async () => {
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === "get_local_link_preferences") return enabledPreferences;
      if (command === "get_local_link_identity_fingerprint") return "A1B2 · C3D4";
      throw new Error(`Unexpected command ${command}`);
    });

    await expect(getLocalLinkPreferences()).resolves.toEqual({
      ...enabledPreferences,
      identityFingerprint: "A1B2 · C3D4",
      protocolVersion: "v2",
    });

    tauri.invoke.mockReset();
    tauri.invoke.mockResolvedValue({ ...enabledPreferences, enabled: false });
    await expect(getLocalLinkPreferences()).resolves.toMatchObject({
      enabled: false,
      identityFingerprint: null,
      protocolVersion: null,
    });
    expect(tauri.invoke).toHaveBeenCalledOnce();
  });

  it("sends only native preference fields before refreshing augmented preferences", async () => {
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === "update_local_link_preferences") return enabledPreferences;
      if (command === "get_local_link_preferences") return enabledPreferences;
      if (command === "get_local_link_identity_fingerprint") return "A1B2 · C3D4";
      throw new Error(`Unexpected command ${command}`);
    });

    await updateLocalLinkPreferences({
      ...enabledPreferences,
      identityFingerprint: "webview-must-not-write-this",
      protocolVersion: "v999",
    });

    expect(tauri.invoke).toHaveBeenNthCalledWith(1, "update_local_link_preferences", {
      preferences: {
        enabled: true,
        discoveryEnabled: false,
        deviceName: "Work Mac",
      },
    });
  });

  it("passes through native reachability, re-pairing, and protocol range DTO fields", async () => {
    tauri.invoke.mockResolvedValue([
      {
        deviceId: "device-42",
        displayName: "Studio Mac",
        trustStatus: "needsRePairing",
        pairedAt: null,
        online: false,
        protocolMin: 1,
        protocolMax: 2,
      },
    ]);

    await expect(listLocalLinkDevices()).resolves.toEqual([
      expect.objectContaining({
        trustStatus: "needsRePairing",
        online: false,
        protocolMin: 1,
        protocolMax: 2,
      }),
    ]);
  });

  it("uses the native pairing state as the single source for the SAS", async () => {
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === "begin_local_link_pairing") return nativePairing.device;
      if (command === "get_local_link_pairing") return nativePairing;
      if (command === "confirm_local_link_pairing") {
        return { ...nativePairing.device, trustStatus: "pairing" };
      }
      throw new Error(`Unexpected command ${command}`);
    });

    await expect(
      beginLocalLinkPairing({ deviceId: "device-42", displayName: "Studio Mac" }),
    ).resolves.toEqual(nativePairing);
    await expect(getLocalLinkPairing()).resolves.toEqual(nativePairing);
    await expect(confirmLocalLinkPairing("device-42")).resolves.toMatchObject({
      deviceId: "device-42",
      trustStatus: "pairing",
    });

    expect(tauri.invoke.mock.calls).toEqual([
      [
        "begin_local_link_pairing",
        { request: { deviceId: "device-42", displayName: "Studio Mac" } },
      ],
      ["get_local_link_pairing"],
      ["get_local_link_pairing"],
      [
        "confirm_local_link_pairing",
        { request: { deviceId: "device-42", trustDuration: "thirtyDays" } },
      ],
    ]);
  });

  it("uses the frozen native cancel, reveal, identity-reset, and clear commands", async () => {
    tauri.invoke.mockImplementation(async (command: string) => {
      if (command === "cancel_local_link_transfer") return cancelledTransfer;
      if (command === "clear_local_link_transfers") return 3;
      return undefined;
    });

    await expect(cancelLocalLinkTransfer("transfer-42")).resolves.toEqual(cancelledTransfer);
    await expect(revealLocalLinkTransfer("transfer-42")).resolves.toBeUndefined();
    await expect(resetLocalLinkIdentity()).resolves.toBeUndefined();
    await expect(clearLocalLinkTransfers()).resolves.toBe(3);

    expect(tauri.invoke.mock.calls).toEqual([
      ["cancel_local_link_transfer", { transferId: "transfer-42" }],
      ["reveal_local_link_transfer", { transferId: "transfer-42" }],
      ["reset_local_link_identity"],
      ["clear_local_link_transfers"],
    ]);
  });
});
