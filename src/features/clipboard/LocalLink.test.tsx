import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { type ReactNode, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getBrowserLocalLinkPreferences,
  resetBrowserLocalLinkForTests,
  updateBrowserLocalLinkPreferences,
} from "./browserRepository";
import { LocalLinkDevicesSettings, resolveLocalLinkRePairRoute } from "./LocalLinkDevicesSettings";
import { LocalLinkIncomingCenter } from "./LocalLinkIncomingCenter";
import { LocalLinkReceiveCard } from "./LocalLinkReceiveCard";
import { LocalLinkSendDialog } from "./LocalLinkSendDialog";
import { LocalLinkTransfersPopover, localLinkDiagnosticId } from "./LocalLinkTransfersPopover";
import { QuickPasteOverlay } from "./QuickPasteOverlay";
import type { ClipboardItem, LocalLinkDevice, LocalLinkTransfer } from "./types";

const item: ClipboardItem = {
  id: "demo-text",
  content: "ClipRiva keeps searchable clipboard history private and available on this Mac.",
  kind: "text",
  sourceApp: "Notes",
  createdAt: "2026-07-27T00:00:00.000Z",
  updatedAt: "2026-07-27T00:00:00.000Z",
  isPinned: false,
  copyCount: 0,
};

function renderLocalLink(node: ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchInterval: false } },
  });
  return {
    ...render(<QueryClientProvider client={client}>{node}</QueryClientProvider>),
    client,
  };
}

describe("Local Link v1.0 UI contract", () => {
  beforeEach(() => {
    resetBrowserLocalLinkForTests();
    vi.mocked(navigator.clipboard.writeText).mockClear();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("stays disabled by default while keeping trusted devices visible for revocation", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);

    const enabled = await screen.findByRole("checkbox", { name: "Enable Local Link" });
    expect(enabled).not.toBeChecked();
    expect(screen.queryByRole("checkbox", { name: /discovery/i })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Trusted devices" })).toHaveTextContent(
      "Studio Mac mini",
    );
    expect(screen.getByText(/No account or cloud relay is used/i)).toBeInTheDocument();
    expect(screen.queryByText(/Preview/)).not.toBeInTheDocument();
  });

  it("shows the backend-owned safety code and waits for trusted-device state", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Enable Local Link" }));
    const pair = await screen.findByRole("button", {
      name: "Pair with Meeting MacBook Air",
    });
    pair.focus();
    fireEvent.click(pair);

    const pairing = await screen.findByRole("dialog", {
      name: "Verify the same safety code on both Macs",
    });
    const confirm = within(pairing).getByRole("button", {
      name: "Confirm Meeting MacBook Air on this Mac",
    });
    await waitFor(() => expect(confirm).toHaveFocus());
    expect(within(pairing).getByLabelText("This Mac safety code")).toHaveTextContent(
      /^\d{3} \d{3}$/u,
    );
    expect(within(pairing).getByLabelText("Nearby Mac safety code")).toHaveTextContent(
      within(pairing).getByLabelText("This Mac safety code").textContent ?? "",
    );
    expect(within(pairing).getByText("A3F7 · 91C2")).toBeInTheDocument();
    expect(within(pairing).getByText("B72D · 40EF")).toBeInTheDocument();
    expect(within(pairing).getByText("Waiting for nearby Mac to confirm")).toBeInTheDocument();
    expect(within(pairing).getByRole("list", { name: "Pairing progress" })).toHaveTextContent(
      "2 of 3 · Verify both Macs",
    );
    expect(
      within(pairing).getByRole("button", { name: "Codes or fingerprints do not match" }),
    ).toBeInTheDocument();
    expect(within(pairing).queryByText("Confirmed on nearby Mac")).not.toBeInTheDocument();

    fireEvent.click(within(pairing).getByRole("radio", { name: /Always trust this device/i }));

    fireEvent.click(confirm);
    await waitFor(() => expect(pairing).not.toBeInTheDocument());
    await waitFor(() =>
      expect(screen.getByRole("region", { name: "Trusted devices" })).toHaveTextContent(
        "Meeting MacBook Air",
      ),
    );
    expect(screen.getByRole("region", { name: "Trusted devices" })).toHaveTextContent(
      "always trusted",
    );
  });

  it("requires a fingerprint-bearing confirmation before revoking trust", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Revoke Travel MacBook Pro" }));

    const confirmation = screen.getByRole("alertdialog", { name: "Revoke Travel MacBook Pro?" });
    expect(confirmation).toHaveTextContent("98B2 · 6F0A");
    fireEvent.click(
      within(confirmation).getByRole("button", { name: "Confirm revoke Travel MacBook Pro" }),
    );
    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: "Revoke Travel MacBook Pro" }),
      ).not.toBeInTheDocument();
    });
  });

  it("cancels pairing with Escape without creating trust", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Enable Local Link" }));
    const pair = await screen.findByRole("button", {
      name: "Pair with Meeting MacBook Air",
    });
    pair.focus();
    fireEvent.click(pair);

    const pairing = await screen.findByRole("dialog", {
      name: "Verify the same safety code on both Macs",
    });
    const confirm = within(pairing).getByRole("button", {
      name: "Confirm Meeting MacBook Air on this Mac",
    });
    const cancel = within(pairing).getByRole("button", { name: "Cancel pairing" });
    await waitFor(() => expect(confirm).toHaveFocus());
    cancel.focus();
    fireEvent.keyDown(cancel, { key: "Tab" });
    expect(within(pairing).getByRole("radio", { name: /Only this session/i })).toHaveFocus();
    fireEvent.keyDown(pairing, { key: "Escape" });

    await waitFor(() => expect(pairing).not.toBeInTheDocument());
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Pair with Meeting MacBook Air" })).toHaveFocus(),
    );
    expect(screen.getByRole("region", { name: "Trusted devices" })).not.toHaveTextContent(
      "Meeting MacBook Air",
    );
  });

  it("requires confirmation before resetting identity and exposes needs-re-pairing state", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);
    fireEvent.click(await screen.findByText("Security details"));
    fireEvent.click(screen.getByRole("button", { name: "Reset Local Link identity" }));

    const confirmation = screen.getByRole("alertdialog", { name: "Reset this Mac’s identity?" });
    expect(confirmation).toHaveTextContent("All trusted devices will need to verify");
    expect(confirmation).toHaveTextContent("does not delete Clipboard History");
    expect(screen.getByRole("region", { name: "Trusted devices" })).toHaveTextContent(
      "Studio Mac mini",
    );
    const confirm = within(confirmation).getByRole("button", {
      name: "Reset identity and re-pair",
    });
    await waitFor(() => expect(confirm).toHaveFocus());
    fireEvent.click(confirm);

    const needsRePairing = await screen.findByRole("region", { name: "Needs re-pairing" });
    expect(needsRePairing).toHaveTextContent("Studio Mac mini");
    expect(within(needsRePairing).getAllByText("Needs re-pairing").length).toBeGreaterThan(1);
  });

  it("starts re-pairing through the sole anonymous nearby candidate", async () => {
    renderLocalLink(<LocalLinkDevicesSettings />);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Enable Local Link" }));
    await screen.findByRole("button", { name: "Pair with Meeting MacBook Air" });

    fireEvent.click(screen.getByText("Security details"));
    fireEvent.click(screen.getByRole("button", { name: "Reset Local Link identity" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog", { name: "Reset this Mac’s identity?" })).getByRole(
        "button",
        { name: "Reset identity and re-pair" },
      ),
    );

    const needsRePairing = await screen.findByRole("region", { name: "Needs re-pairing" });
    fireEvent.click(
      within(needsRePairing).getByRole("button", { name: "Re-pair Studio Mac mini" }),
    );

    const pairing = await screen.findByRole("dialog", {
      name: "Verify the same safety code on both Macs",
    });
    expect(within(pairing).getByText("Meeting MacBook Air")).toBeInTheDocument();
    expect(
      within(pairing).getByRole("button", {
        name: "Confirm Meeting MacBook Air on this Mac",
      }),
    ).toBeInTheDocument();
  });

  it("resolves zero, one, and multiple nearby re-pair routes without using stored trust IDs", () => {
    const storedDevice = { displayName: "Studio Mac mini" };
    const candidate = (deviceId: string): LocalLinkDevice => ({
      deviceId,
      displayName: "Nearby ClipRiva device",
      trustStatus: "revoked",
      pairedAt: null,
      revokedAt: null,
    });

    expect(resolveLocalLinkRePairRoute(storedDevice, [])).toEqual({
      kind: "unavailable",
      message:
        "No nearby pairing candidate is available for Studio Mac mini. Open ClipRiva and Local Link on the other Mac, then scan again.",
    });
    expect(resolveLocalLinkRePairRoute(storedDevice, [candidate("candidate-one")])).toEqual({
      kind: "singleCandidate",
      candidate: candidate("candidate-one"),
    });
    expect(
      resolveLocalLinkRePairRoute(storedDevice, [
        candidate("candidate-one"),
        candidate("candidate-two"),
      ]),
    ).toEqual({
      kind: "chooseNearby",
      message:
        "More than one nearby Mac is available. Choose the Mac you want to re-pair with Studio Mac mini, then verify the safety code and fingerprints on both Macs.",
    });
  });

  it("does not expose the incoming body and copies only after an explicit device-labelled choice", async () => {
    renderLocalLink(<LocalLinkReceiveCard />);

    const incoming = await screen.findByRole("dialog", {
      name: "Studio Mac mini sent a text request",
    });
    expect(
      within(incoming).queryByText("Review the agenda before the meeting."),
    ).not.toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    const copy = within(incoming).getByRole("button", {
      name: "Copy incoming text from Studio Mac mini to the system clipboard",
    });
    await waitFor(() => expect(copy).toHaveFocus());
    fireEvent.click(
      within(incoming).getByRole("button", {
        name: "Show incoming text from Studio Mac mini in the native secure view",
      }),
    );
    expect(
      await within(incoming).findByText(/No text entered this browser view/i),
    ).toBeInTheDocument();
    expect(
      within(incoming).queryByText("Review the agenda before the meeting."),
    ).not.toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    fireEvent.click(copy);
    fireEvent.click(copy);

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "Review the agenda before the meeting.",
      );
      expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1);
    });
  });

  it("keeps Save and Reject explicit while Escape only collapses the pending request", async () => {
    const rendered = renderLocalLink(<LocalLinkReceiveCard />);
    let incoming = await screen.findByRole("dialog", {
      name: "Studio Mac mini sent a text request",
    });
    fireEvent.keyDown(incoming, { key: "s", metaKey: true });
    expect(await within(incoming).findByText(/request saved/i)).toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    rendered.unmount();
    resetBrowserLocalLinkForTests();
    const collapsed = renderLocalLink(<LocalLinkReceiveCard />);
    incoming = await screen.findByRole("dialog", {
      name: "Studio Mac mini sent a text request",
    });
    fireEvent.keyDown(incoming, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    collapsed.unmount();
    resetBrowserLocalLinkForTests();
    renderLocalLink(<LocalLinkReceiveCard />);
    incoming = await screen.findByRole("dialog", {
      name: "Studio Mac mini sent a text request",
    });
    fireEvent.click(within(incoming).getByRole("button", { name: /Reject incoming text/i }));
    expect(await within(incoming).findByText(/request rejected/i)).toBeInTheDocument();
  });

  it("collapses an incoming request to a non-competing indicator while a main surface is open", async () => {
    const onReveal = vi.fn();
    renderLocalLink(<LocalLinkReceiveCard isSuppressed onReveal={onReveal} />);

    const indicator = await screen.findByRole("button", {
      name: "Show incoming text request from Studio Mac mini",
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(indicator);
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it("lists pending requests in Incoming Center without exposing their text", async () => {
    const onClose = vi.fn();
    const { client } = renderLocalLink(<LocalLinkIncomingCenter isOpen onClose={onClose} />);
    const center = await screen.findByRole("dialog", { name: "Incoming Center" });

    await waitFor(() => {
      const transfer = client
        .getQueryData<LocalLinkTransfer[]>(["local-link-transfers"])
        ?.find((transfer) => transfer.id === "browser-incoming-review-text");
      expect(transfer?.status).toBe("viewed");
    });
    const requests = await within(center).findByRole("list", {
      name: "Pending incoming text requests",
    });
    expect(within(requests).getAllByRole("listitem")).toHaveLength(1);
    expect(within(center).getByText("Studio Mac mini")).toBeInTheDocument();
    expect(
      within(center).queryByText("Review the agenda before the meeting."),
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(center).getByRole("button", {
        name: "Copy incoming text from Studio Mac mini to the system clipboard",
      }),
    );
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "Review the agenda before the meeting.",
      ),
    );
    expect(await within(center).findByRole("status")).toHaveTextContent(
      "Copied text from Studio Mac mini",
    );

    fireEvent.keyDown(center, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("opens a Handoff Sheet, orders ready/action/offline devices, and creates a request with Enter", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    renderLocalLink(<LocalLinkSendDialog item={item} isOpen onClose={vi.fn()} />);

    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    expect(within(dialog).getByText(item.content)).toBeInTheDocument();
    await within(dialog).findByRole("button", { name: "Select Studio Mac mini" });
    expect(
      Array.from(dialog.querySelectorAll<HTMLElement>(".local-link-send-device strong")).map(
        (device) => device.textContent,
      ),
    ).toEqual(["Studio Mac mini", "Meeting MacBook Air", "Travel MacBook Pro"]);
    expect(
      await within(dialog).findByRole("button", { name: "Select Travel MacBook Pro" }),
    ).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Select Studio Mac mini" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.keyDown(dialog, { key: "Enter" });
    expect(await within(dialog).findByText("Request created")).toBeInTheDocument();
    expect(
      within(dialog).getByText(/does not mean the text has been delivered yet/i),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: /View progress/i })).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: "l", metaKey: true, shiftKey: true });
    expect(
      await within(dialog).findByText("Not delivered", {}, { timeout: 3_000 }),
    ).toBeInTheDocument();
    const progress = within(dialog).getByRole("list", { name: "Transfer progress steps" });
    expect(within(progress).getByText("Policy check").closest("li")).toHaveAttribute(
      "data-state",
      "complete",
    );
    expect(within(progress).getByText("Secure delivery").closest("li")).toHaveAttribute(
      "data-state",
      "failed",
    );
    expect(within(progress).getByText("Recipient decision").closest("li")).toHaveAttribute(
      "data-state",
      "upcoming",
    );
    expect(within(dialog).getByText(/No text was delivered/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/simulated states are not LAN delivery/i)).toBeInTheDocument();
    expect(
      within(dialog).getByText(/Preview transport always ends unavailable/i),
    ).toBeInTheDocument();
  });

  it("keeps classified command clips inside the explicit text handoff boundary", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    renderLocalLink(
      <LocalLinkSendDialog
        item={{ ...item, kind: "command", content: "cargo test --locked", sourceApp: "Terminal" }}
        isOpen
        onClose={vi.fn()}
      />,
    );

    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    expect(within(dialog).getByText("cargo test --locked")).toBeInTheDocument();
    expect(
      await within(dialog).findByRole("button", { name: "Select Studio Mac mini" }),
    ).toBeEnabled();
  });

  it("uses the shared readiness blocker and its one repair action", async () => {
    renderLocalLink(<LocalLinkSendDialog item={item} isOpen onClose={vi.fn()} />);

    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    expect(within(dialog).getByRole("region", { name: "Handoff readiness" })).toHaveTextContent(
      "Turn on Local Link to check local-network access and nearby devices.",
    );
    expect(
      await within(dialog).findByRole("button", { name: "Select Studio Mac mini" }),
    ).toBeDisabled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Turn on Local Link" }));
    expect(
      await within(dialog).findByRole("button", { name: "Enable discovery" }),
    ).toBeInTheDocument();
  });

  it("sends the selected default with Enter and synchronizes the visible result", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    const { client } = renderLocalLink(
      <LocalLinkSendDialog item={item} isOpen onClose={vi.fn()} />,
    );
    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    await within(dialog).findByRole("button", { name: "Select Studio Mac mini" });
    fireEvent.keyDown(dialog, { key: "Enter" });
    await within(dialog).findByText("Request created");

    let transferId = "";
    await waitFor(() => {
      const cached = client.getQueryData<LocalLinkTransfer[]>(["local-link-transfers"]) ?? [];
      const outgoing = cached.find(
        (transfer) => transfer.direction === "outgoing" && transfer.status === "connecting",
      );
      expect(outgoing).toBeDefined();
      transferId = outgoing?.id ?? "";
    });
    act(() => {
      client.setQueryData<LocalLinkTransfer[]>(["local-link-transfers"], (current = []) =>
        current.map((transfer) =>
          transfer.id === transferId
            ? {
                ...transfer,
                peerDisplayName: "Polled Studio Mac",
                status: "copied",
                receiverAction: "copy",
                completedAt: "2026-07-28T00:00:03.000Z",
                updatedAt: "2026-07-28T00:00:03.000Z",
              }
            : transfer,
        ),
      );
    });

    expect(await within(dialog).findByText("Copied")).toBeInTheDocument();
    expect(within(dialog).getByText("Polled Studio Mac")).toBeInTheDocument();
  });

  it("returns to the selected Item while keeping the send summary to one bounded line", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    const onBackToItem = vi.fn();
    renderLocalLink(
      <LocalLinkSendDialog item={item} isOpen onClose={vi.fn()} onBackToItem={onBackToItem} />,
    );

    const back = await screen.findByRole("button", { name: "Return to selected item" });
    fireEvent.keyDown(back, { key: "Enter" });
    expect(screen.queryByText("Request created")).not.toBeInTheDocument();
    fireEvent.click(back);
    expect(onBackToItem).toHaveBeenCalledOnce();
    expect(screen.getByText(item.content)).toBeInTheDocument();
  });

  it("blocks non-text and empty selections before a device can be selected", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    const rendered = renderLocalLink(
      <LocalLinkSendDialog
        item={{ ...item, id: "image", kind: "image" }}
        isOpen
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText(/accepts UTF-8 text only/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Send to/i })).not.toBeInTheDocument();
    rendered.unmount();

    renderLocalLink(
      <LocalLinkSendDialog item={{ ...item, id: "empty", content: "" }} isOpen onClose={vi.fn()} />,
    );
    expect(await screen.findByText(/Empty text cannot be sent/i)).toBeInTheDocument();
  });

  it("opens Local Link from the Quick Paste action panel without changing Enter semantics", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    renderLocalLink(<QuickPasteOverlay />);

    await screen.findByText("Visual Studio Code");
    fireEvent.click(screen.getByRole("button", { name: "More Quick Paste actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Send to device" }));
    expect(await screen.findByRole("dialog", { name: "Handoff Sheet" })).toBeInTheDocument();
  });

  it("derives a local diagnostic correlation from only the transfer ID", () => {
    const transferId = "transfer-local-correlation";
    const first = {
      id: transferId,
      body: "private transfer body",
      peer: "Alice’s Mac",
      deviceId: "private-device-id",
      path: "/Users/alice/private.txt",
    };
    const second = {
      id: transferId,
      body: "different body",
      peer: "Different Mac",
      deviceId: "different-device-id",
      path: "/another/private/path",
    };

    const diagnostic = localLinkDiagnosticId(first.id);
    expect(diagnostic).toBe(localLinkDiagnosticId(second.id));
    expect(diagnostic).toMatch(/^LL-[0-9A-F]{8}$/u);
    for (const privateValue of [first.body, first.peer, first.deviceId, first.path]) {
      expect(diagnostic).not.toContain(privateValue);
    }
  });

  it("filters metadata-only Transfers and never offers old-payload Retry", async () => {
    const onClose = vi.fn();
    renderLocalLink(<LocalLinkTransfersPopover isOpen onClose={onClose} />);

    const transfers = await screen.findByRole("complementary", { name: "Transfers" });
    expect(within(transfers).getByText(/Metadata only/i)).toBeInTheDocument();
    expect(
      within(transfers).queryByText("Review the agenda before the meeting."),
    ).not.toBeInTheDocument();
    expect(within(transfers).queryByRole("button", { name: /Retry/i })).not.toBeInTheDocument();
    fireEvent.click(within(transfers).getByRole("button", { name: "Failed" }));
    expect(within(transfers).getByText("Not delivered")).toBeInTheDocument();
    expect(within(transfers).queryByText("Copied")).not.toBeInTheDocument();
    const diagnosticButton = within(transfers).getByRole("button", {
      name: /Copy diagnostic ID/i,
    });
    fireEvent.click(diagnosticButton);
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        localLinkDiagnosticId("browser-failed-text-transfer"),
      ),
    );
    fireEvent.click(
      within(transfers).getByRole("button", { name: /Clear terminal records \(3\)/i }),
    );
    expect(await within(transfers).findByText("3 terminal records cleared.")).toBeInTheDocument();
    expect(within(transfers).getByText("No failed transfers.")).toBeInTheDocument();
    fireEvent.keyDown(transfers, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("traps modal focus and restores the trigger after the send surface closes", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({ ...preferences, enabled: true });
    renderLocalLink(<SendDialogHarness />);

    const trigger = screen.getByRole("button", { name: "Open send" });
    trigger.focus();
    fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    await waitFor(() => expect(dialog).toHaveFocus());
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Handoff Sheet" })).not.toBeInTheDocument(),
    );
    expect(trigger).toHaveFocus();
  });
});

function SendDialogHarness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open send
      </button>
      <LocalLinkSendDialog item={item} isOpen={open} onClose={() => setOpen(false)} />
    </>
  );
}
