import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  acceptBrowserLocalLinkTransfer,
  createBrowserClipboardSmartCollection,
  createBrowserLocalTextAction,
  deleteBrowserClipboardItemNote,
  deleteBrowserLocalTextAction,
  getBrowserCapturePreferences,
  getBrowserClipboardItemNote,
  listBrowserLocalLinkTransfers,
  resetBrowserLocalLinkForTests,
  setBrowserDirectPasteSimulationForTests,
  setBrowserPastePermissionStatusForTests,
  updateBrowserCapturePreferences,
  updateBrowserLocalTextAction,
} from "../features/clipboard/browserRepository";
import {
  App,
  FIRST_VALUE_COMPLETED_KEY,
  FIRST_VALUE_GUIDE_PENDING_KEY,
  PRIVACY_COVER_KEY,
} from "./App";

function renderApp() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchInterval: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
  );
}

async function openCodeDetails() {
  fireEvent.click(await screen.findByRole("button", { name: "Inspect Code clipboard item" }));
  return screen.findByRole("complementary", { name: "Clip details" });
}

describe("ClipRiva app shell", () => {
  beforeEach(async () => {
    window.localStorage.clear();
    resetBrowserLocalLinkForTests();
    setBrowserPastePermissionStatusForTests("granted");
    setBrowserDirectPasteSimulationForTests("focus");
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({
      ...preferences,
      capturePaused: false,
      pauseReason: null,
      labsEnabled: false,
      diagnosticsEnabled: false,
      pasteBehavior: "restore",
      quickPasteShortcut: "CommandOrControl+Shift+Space",
      stackShortcut: "CommandOrControl+Alt+S",
      onboardingCompleted: true,
    });
  });

  it("renders clipboard history from the browser development adapter", async () => {
    renderApp();
    expect(screen.getByText("ClipRiva")).toBeInTheDocument();
    expect((await screen.findAllByText("Visual Studio Code"))[0]).toBeInTheDocument();
  });

  it("opens, renames, and definition-only deletes a dynamic Smart Collection", async () => {
    await createBrowserClipboardSmartCollection({
      name: "Synthetic code view",
      rule: { kinds: ["code"] },
    });
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: /Synthetic code view/i }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Edit Smart Collection Synthetic code view" }),
    );

    const name = screen.getByLabelText("Name");
    fireEvent.change(name, { target: { value: "Synthetic code renamed" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    const sidebar = screen.getByRole("complementary", { name: "ClipRiva sources" });
    expect(
      await within(sidebar).findByRole("button", { name: /Synthetic code renamed/i }),
    ).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Edit Smart Collection Synthetic code renamed" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Delete definition" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/No clips.*will be changed/i);
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));
    await waitFor(() =>
      expect(
        within(sidebar).queryByRole("button", { name: /Synthetic code renamed/i }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getAllByRole("option", { name: /Visual Studio Code/i }).length).toBeGreaterThan(
      0,
    );
  });

  it("renders Privacy Cover before any clipboard content and reveals only on request", async () => {
    window.localStorage.setItem(PRIVACY_COVER_KEY, "true");
    renderApp();

    expect(screen.getByRole("heading", { name: "Privacy Cover is on" })).toBeInTheDocument();
    expect(screen.queryByText("Visual Studio Code")).not.toBeInTheDocument();
    expect(screen.queryByRole("listbox", { name: "Clipboard history" })).not.toBeInTheDocument();
    expect(document.body.textContent).not.toContain("https://v2.tauri.app/plugin/clipboard/");
    expect(document.body.textContent).not.toContain("版本规划与体验升级");
    expect(
      screen.getByText("Capture continues unless paused. Existing History stays on this Mac."),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Cover is not disk encryption or a system-wide recording block/),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reveal ClipRiva" }));
    expect(await screen.findByRole("listbox", { name: "Clipboard history" })).toBeInTheDocument();
  });

  it("filters history using search", async () => {
    renderApp();
    const search = screen.getByRole("searchbox", { name: "Search clipboard history" });
    fireEvent.change(search, { target: { value: "tauri" } });

    expect(await screen.findByText("Arc")).toBeInTheDocument();
    expect(screen.queryByText("Visual Studio Code")).not.toBeInTheDocument();
  });

  it("keeps Labs controls contextual even when Labs is enabled", async () => {
    await enableLabs();
    renderApp();

    await screen.findAllByText("Visual Studio Code");
    expect(screen.queryByRole("button", { name: "Labs retrieval" })).not.toBeInTheDocument();
    await openCodeDetails();
    expect(
      await within(screen.getByRole("complementary", { name: "Clip details" })).findByRole(
        "button",
        { name: /uppercase/i },
      ),
    ).toBeInTheDocument();
  });

  it("navigates history with arrow keys and copies the selected clip with Enter", async () => {
    renderApp();
    const history = await screen.findByRole("listbox", { name: "Clipboard history" });
    const search = screen.getByRole("searchbox", { name: "Search clipboard history" });

    expect(history).toHaveAttribute("aria-activedescendant", "history-clip-demo-code");
    expect(screen.getByRole("option", { name: /Visual Studio Code/i })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    expect(fireEvent.keyDown(search, { key: "ArrowDown" })).toBe(true);
    expect(history).toHaveAttribute("aria-activedescendant", "history-clip-demo-code");

    expect(fireEvent.keyDown(history, { key: "ArrowDown" })).toBe(false);
    expect(history).toHaveAttribute("aria-activedescendant", "history-clip-demo-url");
    expect(document.getElementById("history-clip-demo-url")).toHaveAttribute(
      "aria-current",
      "true",
    );

    vi.mocked(navigator.clipboard.writeText).mockClear();
    expect(fireEvent.keyDown(history, { key: "Enter" })).toBe(false);
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "https://v2.tauri.app/plugin/clipboard/",
      );
    });
  });

  it("confirms a completed main-workspace copy for 1.5 seconds", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const inspector = await openCodeDetails();
    fireEvent.click(within(inspector).getByRole("button", { name: "Copy" }));

    expect(await screen.findByRole("status", { name: "Copy result" })).toHaveTextContent(
      "Code copied to clipboard",
    );
  });

  it("keeps the Item and offers an explicit retry after a clipboard write failure", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const inspector = await openCodeDetails();
    vi.mocked(navigator.clipboard.writeText).mockClear();
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(new Error("synthetic failure"));
    fireEvent.click(within(inspector).getByRole("button", { name: "Copy" }));
    const failure = await screen.findByRole("alert", { name: "Copy failed" });
    expect(failure).toHaveTextContent("It remains in History");
    expect(screen.queryByRole("status", { name: "Copy result" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Visual Studio Code/i })).toBeInTheDocument();
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1);
    fireEvent.click(within(failure).getByRole("button", { name: "Retry Copy" }));
    expect(await screen.findByRole("status", { name: "Copy result" })).toHaveTextContent(
      "Code copied to clipboard",
    );
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert", { name: "Copy failed" })).not.toBeInTheDocument();
  });

  it("keeps Labs hidden by default", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    expect(screen.queryByRole("button", { name: "Labs retrieval" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /uppercase/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/future cloud boundary/i)).not.toBeInTheDocument();
  });

  it("persists Labs plus distinct Quick Paste and Stack shortcuts", async () => {
    renderApp();
    fireEvent.click(await screen.findByRole("button", { name: "Open settings" }));

    fireEvent.click(screen.getByText("Advanced local tools"));
    fireEvent.click(screen.getByRole("checkbox", { name: /Enable ClipRiva Labs/i }));
    fireEvent.click(screen.getByRole("tab", { name: /Shortcuts/i }));
    fireEvent.change(await screen.findByLabelText("Open Quick Paste with"), {
      target: { value: "CommandOrControl+Shift+V" },
    });
    fireEvent.change(await screen.findByLabelText("Open Stack with"), {
      target: { value: "CommandOrControl+Shift+S" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Local clipboard settings" }),
      ).not.toBeInTheDocument(),
    );
    const preferences = await getBrowserCapturePreferences();
    expect(preferences.labsEnabled).toBe(true);
    expect(preferences.quickPasteShortcut).toBe("CommandOrControl+Shift+V");
    expect(preferences.stackShortcut).toBe("CommandOrControl+Shift+S");
    expect(screen.queryByRole("button", { name: "Labs retrieval" })).not.toBeInTheDocument();
    await openCodeDetails();
    expect(await screen.findByRole("button", { name: /uppercase/i })).toBeInTheDocument();
  });

  it("applies local text actions only after Labs is enabled", async () => {
    await enableLabs();
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    await openCodeDetails();
    expect(await screen.findByRole("button", { name: /uppercase/i })).toBeInTheDocument();

    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.click(screen.getByRole("button", { name: /uppercase/i }));
    const dialog = await screen.findByRole("dialog", { name: "Uppercase" });
    expect(dialog.parentElement?.parentElement).toBe(document.body);
    expect(within(dialog).getByText("Before")).toBeInTheDocument();
    expect(within(dialog).getByText("After")).toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy Filter result" }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "EXPORT FUNCTION CLEANCLIP(INPUT: STRING) {\n  RETURN INPUT.TRIM();\n}",
      );
    });
    expect(await screen.findByText(/Uppercase copied locally/i)).toBeInTheDocument();
    expect(screen.queryByText(/future cloud boundary/i)).not.toBeInTheDocument();
  });

  it("cancels a local Filter preview without changing the clipboard", async () => {
    await enableLabs();
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    await openCodeDetails();
    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.click(await screen.findByRole("button", { name: /uppercase/i }));
    const dialog = await screen.findByRole("dialog", { name: "Uppercase" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog", { name: "Uppercase" })).not.toBeInTheDocument();
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it("requires another Filter preview after the rule changes", async () => {
    const filter = await createBrowserLocalTextAction({
      name: "Versioned filter",
      transform: "uppercase",
      shortcutSlot: null,
    });
    await enableLabs();
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    await openCodeDetails();
    fireEvent.click(await screen.findByRole("button", { name: /Versioned filter.*step filter/i }));
    const dialog = await screen.findByRole("dialog", { name: "Versioned filter" });
    await updateBrowserLocalTextAction(filter.id, {
      name: "Versioned filter changed",
      transform: "trim_whitespace",
      shortcutSlot: null,
    });
    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy Filter result" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Preview again before copying",
    );
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Preview again" }));
    expect(
      await screen.findByRole("dialog", { name: "Versioned filter changed" }),
    ).toBeInTheDocument();
    await deleteBrowserLocalTextAction(filter.id);
  });

  it("runs a custom Filter shortcut only outside editable controls for the selected text clip", async () => {
    const filter = await createBrowserLocalTextAction({
      name: "Shortcut uppercase",
      transform: "uppercase",
      shortcutSlot: 2,
    });
    await enableLabs();
    renderApp();
    await openCodeDetails();
    expect(
      await screen.findByRole("button", { name: /^Shortcut uppercase.*step filter/i }),
    ).toBeInTheDocument();
    const selectedContent = screen
      .getByRole("complementary", { name: "Clip details" })
      .querySelector("pre")?.textContent;
    expect(selectedContent).toBeTruthy();

    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.keyDown(screen.getByRole("searchbox", { name: "Search clipboard history" }), {
      key: "2",
      metaKey: true,
      altKey: true,
    });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "2", metaKey: true, altKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Shortcut uppercase" });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy Filter result" }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(selectedContent?.toUpperCase());
    });
    await deleteBrowserLocalTextAction(filter.id);
  });

  it("shows the local privacy explanation on first launch", async () => {
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, onboardingCompleted: false });
    renderApp();

    expect(
      await screen.findByRole("heading", { name: "Your clipboard stays under your control" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/use Quick Paste to copy the earlier one back to your clipboard/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Copy needs no Accessibility permission/)).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: /sent a text request/i })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /show incoming text request/i }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Start using ClipRiva" }));
    await waitFor(() => {
      expect(
        screen.queryByRole("heading", { name: "Your clipboard stays under your control" }),
      ).not.toBeInTheDocument();
    });
    expect(await screen.findByLabelText("Your first Quick Paste")).toHaveTextContent(
      "Copy two different non-sensitive things",
    );
    expect(screen.getByLabelText("Your first Quick Paste")).toHaveTextContent(
      "select the first one, then choose Copy",
    );
    expect(screen.queryByRole("dialog", { name: /sent a text request/i })).not.toBeInTheDocument();
  });

  it("ends the single first-value task after a successful Quick Paste recovery", async () => {
    window.localStorage.setItem(FIRST_VALUE_GUIDE_PENDING_KEY, "true");
    renderApp();

    expect(await screen.findByLabelText("Your first Quick Paste")).toBeInTheDocument();
    window.localStorage.setItem(FIRST_VALUE_COMPLETED_KEY, "true");
    window.localStorage.removeItem(FIRST_VALUE_GUIDE_PENDING_KEY);
    window.dispatchEvent(new CustomEvent("clipriva:first-value-completed"));

    await waitFor(() =>
      expect(screen.queryByLabelText("Your first Quick Paste")).not.toBeInTheDocument(),
    );
  });

  it("renders the quick paste preview with a focused filter", async () => {
    window.history.replaceState({}, "", "/?quick-paste-preview");

    try {
      renderApp();
      const search = screen.getByRole("searchbox", {
        name: "Search clipboard history for quick paste",
      });

      expect(screen.getByRole("heading", { name: "Quick paste" })).toBeInTheDocument();
      expect(search).toHaveFocus();

      fireEvent.change(search, { target: { value: "tauri" } });

      const highlightedMatch = await screen.findByText("tauri", { selector: "mark" });
      expect(highlightedMatch.closest("pre")).toHaveTextContent(
        "https://v2.tauri.app/plugin/clipboard/",
      );
      expect(screen.queryByText("Visual Studio Code")).not.toBeInTheDocument();
    } finally {
      window.history.replaceState({}, "", "/");
    }
  });

  it("copies the keyboard-selected Quick Paste item with Enter", async () => {
    window.history.replaceState({}, "", "/?quick-paste-preview");

    try {
      renderApp();
      await screen.findAllByText("Visual Studio Code");
      vi.mocked(navigator.clipboard.writeText).mockClear();

      fireEvent.keyDown(window, { key: "ArrowDown" });
      const selectedContent = document.querySelector<HTMLElement>(
        ".quick-paste-item[data-selected=true] pre",
      )?.textContent;
      expect(selectedContent).toBeTruthy();
      fireEvent.keyDown(window, { key: "Enter" });

      await waitFor(() => {
        expect(navigator.clipboard.writeText).toHaveBeenCalledWith(selectedContent);
      });
    } finally {
      window.history.replaceState({}, "", "/");
    }
  });

  it("does not send a Direct Paste when Shift-Enter is used", async () => {
    window.history.replaceState({}, "", "/?quick-paste-preview");

    try {
      renderApp();
      await screen.findAllByText("Visual Studio Code");
      vi.mocked(navigator.clipboard.writeText).mockClear();
      const selectedContent = document.querySelector<HTMLElement>(
        ".quick-paste-item[data-selected=true] pre",
      )?.textContent;
      expect(selectedContent).toBeTruthy();

      fireEvent.keyDown(window, { key: "Enter", shiftKey: true });

      await waitFor(() => {
        expect(navigator.clipboard.writeText).toHaveBeenCalledWith(selectedContent);
      });
      expect(await screen.findByRole("alert")).toHaveTextContent(
        "Copied to the system clipboard. Return to your app and press Command-V.",
      );
    } finally {
      window.history.replaceState({}, "", "/");
    }
  });

  it("toggles a temporary quick paste preview with Command-I", async () => {
    window.history.replaceState({}, "", "/?quick-paste-preview");

    try {
      renderApp();
      await screen.findAllByText("Visual Studio Code");
      const selectedContent = document.querySelector<HTMLElement>(
        ".quick-paste-item[data-selected=true] pre",
      )?.textContent;
      expect(selectedContent).toBeTruthy();

      fireEvent.keyDown(window, { key: "i", metaKey: true });
      expect(await screen.findByLabelText("Quick paste preview")).toHaveTextContent(
        (selectedContent as string).replace(/\s+/g, " "),
      );

      fireEvent.keyDown(window, { key: "i", metaKey: true });
      expect(screen.queryByLabelText("Quick paste preview")).not.toBeInTheDocument();
    } finally {
      window.history.replaceState({}, "", "/");
    }
  });

  it("previews unpinned cleanup with local recovery details before any move", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    fireEvent.click(screen.getByRole("button", { name: "More history actions" }));
    const clear = screen.getByRole("menuitem", {
      name: /Move \d+ unpinned clipboard items to Recycle Bin/i,
    });
    expect(clear).toHaveTextContent("Clear 7 unpinned clips");

    fireEvent.click(clear);

    const preview = await screen.findByRole("alertdialog", { name: "Move clips to Recycle Bin" });
    expect(preview).toHaveTextContent("7 clips are affected");
    expect(preview).toHaveTextContent("Pinned clips are skipped by default");
    expect(preview).toHaveTextContent("Recycle Bin recovery lasts up to 7 days");
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
    expect(screen.getAllByText("Arc").length).toBeGreaterThan(0);
  });

  it("keeps history filters on demand and applies type, source, pin, and recent-use conditions", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    expect(screen.queryByLabelText("Clipboard history filters")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Filter clipboard history" }));
    expect(screen.getByLabelText("Clipboard history filters")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Link"));
    expect(await screen.findByText("https://v2.tauri.app/plugin/clipboard/")).toBeInTheDocument();
    expect(document.getElementById("history-clip-demo-code")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.change(screen.getByLabelText("Source app"), { target: { value: "Figma" } });
    fireEvent.click(screen.getByLabelText("Image"));
    fireEvent.change(screen.getByLabelText("Saved status"), { target: { value: "unpinned" } });
    expect(await screen.findByText(/Product capture/i)).toBeInTheDocument();
    expect(screen.queryByText("#6EE7B7")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.click(screen.getByLabelText("Recently used"));
    await waitFor(() =>
      expect(document.getElementById("history-clip-demo-code")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/Product capture/i)).not.toBeInTheDocument();
  });

  it("filters only explicit Local Link Save results and presents their source clearly", async () => {
    const incoming = (await listBrowserLocalLinkTransfers()).find(
      (transfer) => transfer.direction === "incoming",
    );
    if (!incoming) throw new Error("Expected browser incoming-transfer fixture.");
    await acceptBrowserLocalLinkTransfer(incoming.id, "save");

    renderApp();
    await screen.findAllByText("Visual Studio Code");
    fireEvent.click(screen.getByRole("button", { name: "Filter clipboard history" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Received with Local Link" }));

    expect(await screen.findByText("From Studio Mac mini · Local Link")).toBeInTheDocument();
    expect(document.getElementById("history-clip-demo-code")).not.toBeInTheDocument();
  });

  it("keeps History, Saved, and Collections in primary navigation and scopes search visibly", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    const navigation = screen.getByRole("navigation", { name: "ClipRiva workspace" });
    expect(navigation).toHaveTextContent("History");
    expect(navigation).toHaveTextContent("Saved");
    expect(navigation).not.toHaveTextContent("Devices");
    expect(navigation).not.toHaveTextContent("Inbox");
    expect(screen.getByRole("searchbox", { name: "Search clipboard history" })).toHaveAttribute(
      "placeholder",
      "Search history…",
    );
    fireEvent.click(screen.getByRole("button", { name: "Open product tools" }));
    const productTools = screen.getByRole("menu", { name: "Product tools" });
    expect(within(productTools).getByRole("menuitem", { name: /Local Link/i })).toHaveTextContent(
      "Preview",
    );
    expect(
      within(productTools).getByRole("menuitem", { name: "Open Inbox, 1 pending request" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open settings" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Collections" })).toHaveTextContent("Reference");

    fireEvent.click(within(navigation).getByRole("button", { name: /Saved/ }));
    expect(await screen.findByRole("heading", { name: "Saved" })).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "Search saved clipboard clips" })).toHaveAttribute(
      "placeholder",
      "Search saved clips…",
    );
    fireEvent.click(within(navigation).getByRole("button", { name: /History/ }));
    expect(await screen.findByRole("heading", { name: "History" })).toBeInTheDocument();
  });

  it("filters, searches, and copies command clips from History", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "Filter clipboard history" }));
    fireEvent.click(screen.getByLabelText("Command"));
    const history = await screen.findByRole("listbox", { name: "Clipboard history" });
    expect(within(history).getByRole("option", { name: /Command·Terminal/i })).toBeInTheDocument();
    expect(document.getElementById("history-clip-demo-code")).not.toBeInTheDocument();
    expect(document.querySelector(".clip-card .kind-command svg")).toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search clipboard history" }), {
      target: { value: "runInBand" },
    });
    await waitFor(() =>
      expect(history).toHaveAttribute("aria-activedescendant", "history-clip-demo-command"),
    );

    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.keyDown(history, { key: "Enter" });
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("pnpm test -- --runInBand"),
    );
    expect(await screen.findByRole("status", { name: "Copy result" })).toHaveTextContent(
      "Command copied to clipboard",
    );
  });

  it("opens Local Link as a secondary Preview and fails closed while it is off", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "Open product tools" }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Local Link/i }));
    const center = await screen.findByRole("region", { name: "Device center" });
    expect(center).toHaveTextContent("Local Link · Preview");
    expect(center).toHaveTextContent("Recent sends");
    expect(center).toHaveTextContent("Device readiness");
    expect(center).toHaveTextContent("Local network access");
    const deviceStatus = within(center).getByRole("region", {
      name: "Device status and details",
    });
    expect(within(deviceStatus).getByRole("button", { name: /Studio Mac mini/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(deviceStatus).toHaveTextContent("Local Link off");
    expect(deviceStatus).not.toHaveTextContent("Sendable");
    expect(deviceStatus).not.toHaveTextContent("Online · Trusted");
    fireEvent.click(within(deviceStatus).getByRole("button", { name: /Travel MacBook Pro/i }));
    expect(
      within(deviceStatus).getByRole("article", { name: "Travel MacBook Pro details" }),
    ).toHaveTextContent("Not checked while Local Link is off");
    expect(
      screen.queryByRole("dialog", { name: "Local clipboard settings" }),
    ).not.toBeInTheDocument();
    expect(center).not.toHaveTextContent("Review the agenda before the meeting.");

    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));
    expect(
      await screen.findByRole("dialog", { name: "Local clipboard settings" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Devices" })).not.toBeInTheDocument();
  });

  it("opens Send directly from a clip card", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const firstCard = document.querySelector<HTMLElement>(".clip-card");
    if (!firstCard) throw new Error("Expected a clipboard card.");
    fireEvent.click(within(firstCard).getByRole("button", { name: "More item actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Send to device" }));

    expect(await screen.findByRole("dialog", { name: "Handoff Sheet" })).toBeInTheDocument();
  });

  it("moves an unpinned clip immediately and restores it with Command-Z", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    const moreButtons = screen.getAllByRole("button", { name: "More item actions" });
    const moreButton = moreButtons[1];
    if (!moreButton) throw new Error("Expected a More menu on the Link clipboard card.");
    fireEvent.click(moreButton);
    fireEvent.click(screen.getByRole("menuitem", { name: "Move to Recycle Bin" }));
    fireEvent.click(screen.getByRole("button", { name: "Move to Recycle Bin" }));

    await waitFor(() =>
      expect(document.getElementById("history-clip-demo-url")).not.toBeInTheDocument(),
    );
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await waitFor(() =>
      expect(
        screen
          .getAllByRole("status")
          .some((status) => status.textContent?.includes("Link clip moved to Recycle Bin")),
      ).toBe(true),
    );

    fireEvent.keyDown(window, { key: "z", metaKey: true });
    await waitFor(() =>
      expect(document.getElementById("history-clip-demo-url")).toBeInTheDocument(),
    );
  });

  it("opens details on demand and returns to the list", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const incoming = screen.queryByRole("button", { name: /Collapse incoming text/i });
    if (incoming) fireEvent.click(incoming);
    expect(screen.queryByRole("complementary", { name: "Clip details" })).not.toBeInTheDocument();
    const inspector = await openCodeDetails();
    expect(inspector).toHaveTextContent("Code clip");
    fireEvent.click(within(inspector).getByRole("button", { name: "Back to results" }));
    expect(screen.queryByRole("complementary", { name: "Clip details" })).not.toBeInTheDocument();
    await waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-label")).toBe(
        "Inspect Code clipboard item",
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Inspect Link clipboard item" }));
    expect(await screen.findByRole("complementary", { name: "Clip details" })).toHaveTextContent(
      "Link clip",
    );
  });

  it("keeps an unsaved Note draft until the user explicitly discards it", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const inspector = await openCodeDetails();
    const note = within(inspector).getByRole("textbox", { name: "Searchable Note" });
    fireEvent.change(note, { target: { value: "Synthetic working draft" } });
    fireEvent.click(screen.getByRole("button", { name: /^Saved\d/u }));
    const warning = await screen.findByRole("alertdialog", { name: "Unsaved Note" });
    expect(note).toHaveValue("Synthetic working draft");
    const keepEditing = within(warning).getByRole("button", { name: "Keep editing" });
    await waitFor(() => expect(document.activeElement).toBe(keepEditing));
    fireEvent.keyDown(keepEditing, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(
      within(warning).getByRole("button", { name: "Discard draft" }),
    );
    fireEvent.click(keepEditing);
    expect(note).toHaveValue("Synthetic working draft");
    fireEvent.click(screen.getByRole("button", { name: /^Saved\d/u }));
    fireEvent.click(
      within(await screen.findByRole("alertdialog", { name: "Unsaved Note" })).getByRole("button", {
        name: "Discard draft",
      }),
    );
    expect(screen.queryByRole("complementary", { name: "Clip details" })).not.toBeInTheDocument();
    expect(await getBrowserClipboardItemNote("demo-code")).toBeNull();
  });

  it("keeps a saved Note when leaving details for Saved", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    const inspector = await openCodeDetails();
    fireEvent.change(within(inspector).getByRole("textbox", { name: "Searchable Note" }), {
      target: { value: "Synthetic saved context" },
    });
    fireEvent.click(within(inspector).getByRole("button", { name: "Save Note" }));
    expect(await within(inspector).findByRole("status")).toHaveTextContent("Note saved locally");
    fireEvent.click(screen.getByRole("button", { name: /^Saved\d/u }));
    expect(screen.queryByRole("alertdialog", { name: "Unsaved Note" })).not.toBeInTheDocument();
    expect(await getBrowserClipboardItemNote("demo-code")).toMatchObject({
      text: "Synthetic saved context",
    });
    await deleteBrowserClipboardItemNote("demo-code");
  });

  it("replaces details with Settings instead of stacking main surfaces", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    await openCodeDetails();
    fireEvent.click(screen.getByRole("button", { name: "Open settings" }));

    expect(
      await screen.findByRole("dialog", { name: "Local clipboard settings" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("complementary", { name: "Clip details" })).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Incoming Center" })).not.toBeInTheDocument();
  });

  it("uses one Local Link main surface and toggles Transfers with Command-Shift-L", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");
    await openCodeDetails();

    fireEvent.keyDown(window, { key: "l", metaKey: true, shiftKey: true });
    expect(await screen.findByRole("complementary", { name: "Transfers" })).toBeInTheDocument();
    expect(screen.queryByRole("complementary", { name: "Clip details" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Local Link transfers" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "l", metaKey: true, shiftKey: true });
    expect(screen.queryByRole("complementary", { name: "Transfers" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Local Link transfers" })).not.toBeInTheDocument();
  });

  it("opens Incoming Center from navigation and Command-Shift-I", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "Open product tools" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Open Inbox, 1 pending request" }));
    const center = await screen.findByRole("dialog", { name: "Incoming Center" });
    expect(center).toHaveTextContent("Studio Mac mini");
    expect(center).not.toHaveTextContent("Review the agenda before the meeting.");

    fireEvent.keyDown(window, { key: "i", metaKey: true, shiftKey: true });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Incoming Center" })).not.toBeInTheDocument(),
    );

    fireEvent.keyDown(window, { key: "i", metaKey: true, shiftKey: true });
    expect(await screen.findByRole("dialog", { name: "Incoming Center" })).toBeInTheDocument();
  });

  it("pauses and resumes local capture without removing history", async () => {
    renderApp();
    const pause = await screen.findByRole("button", { name: "Capture active · Pause" });
    await waitFor(() => expect(pause).toBeEnabled());

    fireEvent.click(pause);
    const resume = await screen.findByRole("button", {
      name: "Capture paused · Paused manually · Resume",
    });
    expect(screen.getAllByText("Visual Studio Code").length).toBeGreaterThan(0);

    fireEvent.click(resume);
    expect(
      await screen.findByRole("button", { name: "Capture active · Pause" }),
    ).toBeInTheDocument();
  });

  it("uses temporary selection mode for counted, previewed bulk cleanup", async () => {
    renderApp();
    await screen.findAllByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "More history actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Select clips" }));
    expect(screen.getByText("0 selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Exit selection mode" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Select Link clipboard item" }));
    fireEvent.click(screen.getByRole("button", { name: "Select Color clipboard item" }));
    expect(screen.getByText("2 selected")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByText("2 selected")).not.toBeInTheDocument();
    expect(document.getElementById("history-clip-demo-url")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "More history actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Select clips" }));
    fireEvent.click(screen.getByRole("button", { name: "Select Link clipboard item" }));
    fireEvent.click(screen.getByRole("button", { name: "Select Color clipboard item" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Move 2 selected clipboard clips to Recycle Bin" }),
    );

    const preview = await screen.findByRole("alertdialog", { name: "Move clips to Recycle Bin" });
    expect(preview).toHaveTextContent("2 clips are affected");
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Move to Recycle Bin" }));

    await waitFor(() => {
      expect(document.getElementById("history-clip-demo-url")).not.toBeInTheDocument();
      expect(document.getElementById("history-clip-demo-color")).not.toBeInTheDocument();
    });
    expect(screen.getAllByText("Visual Studio Code").length).toBeGreaterThan(0);
    expect(screen.getByText(/Product capture/i)).toBeInTheDocument();
  });
});

async function enableLabs() {
  const preferences = await getBrowserCapturePreferences();
  await updateBrowserCapturePreferences({ ...preferences, labsEnabled: true });
}
