import { beforeEach, describe, expect, it, vi } from "vitest";
import { buildLocalLinkDiagnostics, getLocalLinkReadinessSnapshot } from "./api";
import {
  acceptBrowserLocalLinkTransfer,
  beginBrowserLocalLinkPairing,
  cancelBrowserLocalLinkTransfer,
  clearBrowserLocalLinkTransfers,
  confirmBrowserLocalLinkPairing,
  getBrowserLocalLinkPreferences,
  listBrowserItems,
  listBrowserLocalLinkDevices,
  listBrowserLocalLinkTransfers,
  markBrowserLocalLinkTransferViewed,
  rejectBrowserLocalLinkTransfer,
  resetBrowserLocalLinkForTests,
  resetBrowserLocalLinkIdentity,
  revealBrowserLocalLinkTransfer,
  revokeBrowserLocalLinkDevice,
  sendBrowserClipboardItemToLocalLinkDevice,
  updateBrowserLocalLinkPreferences,
} from "./browserRepository";

describe("browser Local Link adapter", () => {
  beforeEach(() => {
    resetBrowserLocalLinkForTests();
    vi.mocked(navigator.clipboard.writeText).mockClear();
  });

  it("keeps discovery off by default and requires the matching six-digit pairing code", async () => {
    expect(await getBrowserLocalLinkPreferences()).toMatchObject({
      enabled: false,
      discoveryEnabled: false,
    });
    expect((await listBrowserLocalLinkDevices()).map((device) => device.displayName)).not.toContain(
      "Meeting MacBook Air",
    );

    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    const nearby = (await listBrowserLocalLinkDevices()).find(
      (device) => device.displayName === "Meeting MacBook Air",
    );
    expect(nearby).toBeDefined();
    if (!nearby) throw new Error("Expected browser nearby-device fixture.");

    await beginBrowserLocalLinkPairing(nearby.deviceId, nearby.displayName, "482915");
    await expect(confirmBrowserLocalLinkPairing(nearby.deviceId, "000000")).rejects.toThrow(
      /does not match/i,
    );
    await expect(confirmBrowserLocalLinkPairing(nearby.deviceId, "482915")).resolves.toMatchObject({
      trustStatus: "trusted",
    });

    await expect(revokeBrowserLocalLinkDevice(nearby.deviceId)).resolves.toMatchObject({
      trustStatus: "revoked",
    });
  });

  it("records the selected trust duration and clears session trust when disabled", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    const nearby = (await listBrowserLocalLinkDevices()).find(
      (device) => device.displayName === "Meeting MacBook Air",
    );
    if (!nearby) throw new Error("Expected browser nearby-device fixture.");

    await beginBrowserLocalLinkPairing(nearby.deviceId, nearby.displayName, "482915");
    await expect(
      confirmBrowserLocalLinkPairing(nearby.deviceId, "482915", "thisSession"),
    ).resolves.toMatchObject({ trustStatus: "trusted", trustDuration: "thisSession" });

    await updateBrowserLocalLinkPreferences({
      ...(await getBrowserLocalLinkPreferences()),
      enabled: false,
    });
    await expect(listBrowserLocalLinkDevices()).resolves.toContainEqual(
      expect.objectContaining({
        deviceId: nearby.deviceId,
        trustStatus: "needsRePairing",
        trustDuration: "thirtyDays",
      }),
    );
  });

  it("does not write unconfirmed incoming content and removes it on reject", async () => {
    const before = await listBrowserItems({ query: "", pinnedOnly: false });
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    expect(incoming).toBeDefined();
    if (!incoming) throw new Error("Expected browser transfer fixture.");
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    await rejectBrowserLocalLinkTransfer(incoming.id);
    expect(await listBrowserItems({ query: "", pinnedOnly: false })).toHaveLength(before.length);
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).toMatchObject({ status: "rejected" });
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).not.toHaveProperty("preview");
  });

  it("records Incoming Center open as an idempotent metadata-only viewed state", async () => {
    const before = await listBrowserItems({ query: "", pinnedOnly: false });
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    if (!incoming) throw new Error("Expected browser incoming-transfer fixture.");

    await expect(markBrowserLocalLinkTransferViewed(incoming.id)).resolves.toMatchObject({
      status: "viewed",
    });
    await expect(markBrowserLocalLinkTransferViewed(incoming.id)).resolves.toMatchObject({
      status: "viewed",
    });
    const viewed = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.id === incoming.id,
    );
    expect(JSON.stringify(viewed)).not.toContain("Review the agenda before the meeting.");
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    expect(await listBrowserItems({ query: "", pinnedOnly: false })).toHaveLength(before.length);
  });

  it("writes an incoming Copy decision to the clipboard without adding History", async () => {
    const before = await listBrowserItems({ query: "", pinnedOnly: false });
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    if (!incoming) throw new Error("Expected browser incoming-transfer fixture.");

    await acceptBrowserLocalLinkTransfer(incoming.id, "copy");

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      "Review the agenda before the meeting.",
    );
    expect(await listBrowserItems({ query: "", pinnedOnly: false })).toHaveLength(before.length);
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).toMatchObject({ status: "copied", receiverAction: "copy" });
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).not.toHaveProperty("preview");
  });

  it("writes an incoming Save decision to History without changing the clipboard", async () => {
    const before = await listBrowserItems({ query: "", pinnedOnly: false });
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    if (!incoming) throw new Error("Expected browser incoming-transfer fixture.");

    await acceptBrowserLocalLinkTransfer(incoming.id, "save");

    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    expect(await listBrowserItems({ query: "", pinnedOnly: false })).toHaveLength(
      before.length + 1,
    );
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).toMatchObject({ status: "saved", receiverAction: "save" });
    expect(
      (await listBrowserLocalLinkTransfers()).find((transfer) => transfer.id === incoming.id),
    ).not.toHaveProperty("preview");
  });

  it("acknowledges secure reveal without returning plaintext in the transfer DTO", async () => {
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    if (!incoming) throw new Error("Expected browser incoming-transfer fixture.");

    await expect(revealBrowserLocalLinkTransfer(incoming.id)).resolves.toBeUndefined();
    expect(incoming).not.toHaveProperty("preview");
    expect(JSON.stringify(incoming)).not.toContain("Review the agenda before the meeting.");
  });

  it("marks every trusted device for re-pairing after identity reset", async () => {
    await resetBrowserLocalLinkIdentity();
    const devices = await listBrowserLocalLinkDevices();
    expect(devices.filter((device) => device.trustStatus === "trusted")).toHaveLength(0);
    expect(devices.filter((device) => device.trustStatus === "needsRePairing")).not.toHaveLength(0);
  });

  it("clears terminal metadata while preserving a pending incoming decision", async () => {
    const before = await listBrowserLocalLinkTransfers();
    const pending = before.find((transfer) => transfer.status === "awaitingReceiver");
    const expectedCleared = before.filter((transfer) =>
      ["copied", "saved", "rejected", "cancelled", "expired", "failed"].includes(transfer.status),
    ).length;

    await expect(clearBrowserLocalLinkTransfers()).resolves.toBe(expectedCleared);
    const after = await listBrowserLocalLinkTransfers();
    expect(after).toEqual(pending ? [pending] : []);
  });

  it("starts a fresh outgoing fixture and can cancel it before a final decision", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    const device = (await listBrowserLocalLinkDevices()).find(
      (candidate) => candidate.availability === "online" && candidate.trustStatus === "trusted",
    );
    if (!device) throw new Error("Expected an online trusted-device fixture.");

    const transfer = await sendBrowserClipboardItemToLocalLinkDevice("demo-text", device.deviceId);
    expect(transfer).toMatchObject({ status: "connecting", isUiFixture: true });
    await expect(cancelBrowserLocalLinkTransfer(transfer.id)).resolves.toMatchObject({
      status: "cancelled",
    });
  });

  it("rejects rich text rather than downgrading it into the text-only protocol", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    const richText = (await listBrowserItems({ query: "", pinnedOnly: false })).find(
      (item) => item.kind === "richText",
    );
    const device = (await listBrowserLocalLinkDevices()).find(
      (candidate) => candidate.trustStatus === "trusted",
    );
    if (!richText || !device) throw new Error("Expected rich-text and trusted-device fixtures.");

    await expect(
      sendBrowserClipboardItemToLocalLinkDevice(richText.id, device.deviceId),
    ).rejects.toThrow(/text clips only/i);
  });

  it("builds read-only, content-free readiness diagnostics with a bounded schema", async () => {
    const before = await getBrowserLocalLinkPreferences();
    const snapshot = await getLocalLinkReadinessSnapshot();
    expect(snapshot).toMatchObject({
      state: "actionRequired",
      firstBlocker: "localLinkDisabled",
      primaryAction: "enableLocalLink",
    });
    expect(await getBrowserLocalLinkPreferences()).toEqual(before);

    const bundle = await buildLocalLinkDiagnostics();
    const serialized = JSON.stringify(bundle);
    expect(bundle.recordCount).toBeLessThanOrEqual(50);
    expect(serialized).not.toContain("Review the agenda before the meeting.");
    expect(serialized).not.toContain("Studio Mac mini");
    expect(serialized).not.toMatch(
      /deviceId|displayName|fingerprint|clipboardItemId|byteSize|fileName/u,
    );
  });
});
