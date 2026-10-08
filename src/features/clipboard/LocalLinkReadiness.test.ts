import { describe, expect, it } from "vitest";
import { deriveLocalLinkReadiness } from "./LocalLinkReadiness";
import type { LocalLinkDevice } from "./types";

const trustedOnline: LocalLinkDevice = {
  deviceId: "ready",
  displayName: "Ready Mac",
  trustStatus: "trusted",
  pairedAt: "2026-07-30T00:00:00.000Z",
  online: true,
};

describe("Local Link readiness", () => {
  it("stops at the off switch without pretending permission was checked", () => {
    const readiness = deriveLocalLinkReadiness({ enabled: false, discoveryEnabled: false }, [
      trustedOnline,
    ]);

    expect(readiness).toMatchObject({
      state: "actionRequired",
      label: "Action needed",
      primaryAction: "enableLocalLink",
      readyDeviceCount: 0,
    });
    expect(readiness.checks.map(({ id }) => id)).toEqual([
      "localNetwork",
      "localLink",
      "discovery",
      "trust",
      "handshake",
    ]);
    expect(readiness.checks[0]).toMatchObject({ id: "localNetwork", status: "pending" });
    expect(readiness.checks[1]).toMatchObject({ id: "localLink", status: "blocked" });
  });

  it("never treats an unpaired online discovery record as trusted", () => {
    const readiness = deriveLocalLinkReadiness({ enabled: true, discoveryEnabled: true }, [
      { ...trustedOnline, deviceId: "nearby", trustStatus: "revoked" },
    ]);

    expect(readiness).toMatchObject({
      state: "actionRequired",
      primaryAction: "pairDevice",
      readyDeviceCount: 0,
    });
    expect(readiness.checks.find(({ id }) => id === "trust")).toMatchObject({
      status: "blocked",
    });
  });

  it("keeps trusted devices visible while reporting an offline secure session", () => {
    const readiness = deriveLocalLinkReadiness({ enabled: true, discoveryEnabled: true }, [
      { ...trustedOnline, online: false, lastSeenAt: "2026-07-30T00:00:00.000Z" },
    ]);

    expect(readiness).toMatchObject({
      state: "offline",
      label: "Offline",
      primaryAction: "refreshDevices",
      readyDeviceCount: 0,
    });
    expect(readiness.checks.find(({ id }) => id === "trust")).toMatchObject({
      status: "passed",
    });
    expect(readiness.checks.find(({ id }) => id === "handshake")).toMatchObject({
      status: "blocked",
    });
  });

  it("requires both authenticated trust and online presence before reporting ready", () => {
    const readiness = deriveLocalLinkReadiness({ enabled: true, discoveryEnabled: true }, [
      trustedOnline,
    ]);

    expect(readiness).toMatchObject({
      state: "ready",
      label: "Ready to send",
      primaryAction: null,
      readyDeviceCount: 1,
      pendingRequestCapacity: 3,
    });
    expect(readiness.checks.every(({ status }) => status === "passed")).toBe(true);
  });

  it("prioritizes explicit re-pairing over creating a new trust binding", () => {
    const readiness = deriveLocalLinkReadiness({ enabled: true, discoveryEnabled: true }, [
      {
        ...trustedOnline,
        deviceId: "expired",
        trustStatus: "needsRePairing",
        rePairingReason: "trustExpired",
      },
    ]);

    expect(readiness).toMatchObject({
      state: "actionRequired",
      primaryAction: "rePairDevice",
      actionDeviceId: "expired",
    });
  });
});
