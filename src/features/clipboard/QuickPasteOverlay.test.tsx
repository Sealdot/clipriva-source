import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as clipboardApi from "./api";
import {
  copyBrowserItem,
  createBrowserLocalTextAction,
  deleteBrowserLocalTextAction,
  getBrowserCapturePreferences,
  getBrowserLocalLinkPreferences,
  resetBrowserLocalLinkForTests,
  setBrowserDirectPasteSimulationForTests,
  setBrowserPastePermissionStatusForTests,
  updateBrowserCapturePreferences,
  updateBrowserLocalLinkPreferences,
} from "./browserRepository";
import { QuickPasteOverlay } from "./QuickPasteOverlay";

function renderOverlay() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchInterval: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <QuickPasteOverlay />
    </QueryClientProvider>,
  );
}

describe("QuickPasteOverlay", () => {
  beforeEach(async () => {
    window.localStorage.clear();
    resetBrowserLocalLinkForTests();
    setBrowserPastePermissionStatusForTests("granted");
    setBrowserDirectPasteSimulationForTests("focus");
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({
      ...preferences,
      capturePaused: false,
      labsEnabled: false,
      onboardingCompleted: true,
    });
    vi.mocked(navigator.clipboard.writeText).mockClear();
  });

  it("supports search evidence, explicit copy, empty recovery, and collection actions", async () => {
    renderOverlay();
    await screen.findByText("Visual Studio Code");
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });

    fireEvent.change(search, { target: { value: "tauri" } });
    const highlightedMatch = await screen.findByText("tauri", { selector: "mark" });
    expect(highlightedMatch.closest("pre")).toHaveTextContent(
      "https://v2.tauri.app/plugin/clipboard/",
    );

    fireEvent.keyDown(window, { key: "c", metaKey: true });
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "https://v2.tauri.app/plugin/clipboard/",
      ),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("Copied to the system clipboard");

    fireEvent.keyDown(window, { key: "Escape" });
    expect(search).toHaveValue("tauri");
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1);

    fireEvent.change(search, { target: { value: "" } });
    await screen.findByText("Visual Studio Code");
    expect(document.querySelector(".quick-pin")).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    const collectionAction = screen.getByRole("menuitem", { name: /Save for reuse/i });
    fireEvent.click(collectionAction);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "More Quick Paste actions" })).toHaveAttribute(
        "aria-expanded",
        "false",
      ),
    );

    fireEvent.change(search, { target: { value: "does-not-exist" } });
    expect(await screen.findByText("No results for “does-not-exist”.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(search).toHaveValue("");
  });

  it("does not show or activate a late result from an older search term", async () => {
    const originalSearch = clipboardApi.searchQuickPasteItems;
    let releaseOlderSearch: (() => void) | undefined;
    const olderSearchGate = new Promise<void>((resolve) => {
      releaseOlderSearch = resolve;
    });
    const searchSpy = vi
      .spyOn(clipboardApi, "searchQuickPasteItems")
      .mockImplementation(async (options) => {
        if (options.query === "tauri") await olderSearchGate;
        return originalSearch(options);
      });
    const activateSpy = vi
      .spyOn(clipboardApi, "activateClipboardItem")
      .mockResolvedValue({ outcome: "clipboardRestored", failureReason: null });

    try {
      renderOverlay();
      await screen.findByText("Visual Studio Code");
      const search = screen.getByRole("searchbox", {
        name: "Search clipboard history for quick paste",
      });
      fireEvent.change(search, { target: { value: "tauri" } });
      await waitFor(() =>
        expect(searchSpy).toHaveBeenCalledWith(expect.objectContaining({ query: "tauri" })),
      );
      fireEvent.change(search, { target: { value: "ClipRiva" } });
      await waitFor(() =>
        expect(
          document.querySelector(".quick-paste-item[data-item-id='demo-text']"),
        ).toBeInTheDocument(),
      );
      expect(screen.queryByText("tauri", { selector: "mark" })).not.toBeInTheDocument();

      await act(async () => releaseOlderSearch?.());
      expect(search).toHaveValue("ClipRiva");
      expect(screen.queryByText("tauri", { selector: "mark" })).not.toBeInTheDocument();
      fireEvent.keyDown(window, { key: "Enter" });
      await waitFor(() => expect(activateSpy).toHaveBeenCalledWith("demo-text", "copy"));
      expect(activateSpy).toHaveBeenCalledTimes(1);
    } finally {
      releaseOlderSearch?.();
      searchSpy.mockRestore();
      activateSpy.mockRestore();
    }
  });

  it("copies with Enter without requesting Direct Paste permission", async () => {
    setBrowserPastePermissionStatusForTests("notGranted");
    renderOverlay();
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.change(search, { target: { value: "tauri" } });
    const highlightedMatch = await screen.findByText("tauri", { selector: "mark" });
    const selectedText = highlightedMatch.closest("pre")?.textContent;
    expect(selectedText).toBe("https://v2.tauri.app/plugin/clipboard/");

    fireEvent.keyDown(window, { key: "Enter" });
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith(selectedText));
    expect(search).toHaveValue("tauri");
    expect(screen.queryByRole("dialog", { name: "Enable Direct Paste" })).not.toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent("Copied to the system clipboard");
  });

  it("selects on click without writing and keeps search-field Copy native", async () => {
    renderOverlay();
    await screen.findByText("Visual Studio Code");
    const rows = Array.from(document.querySelectorAll<HTMLElement>(".quick-paste-item"));
    expect(rows.length).toBeGreaterThan(1);
    const firstId = rows[0]?.dataset.itemId;
    const secondId = rows[1]?.dataset.itemId;
    await waitFor(() =>
      expect(document.querySelector(".quick-paste-item[data-selected=true]")).toHaveAttribute(
        "data-item-id",
        firstId,
      ),
    );
    vi.mocked(navigator.clipboard.writeText).mockClear();

    fireEvent.mouseEnter(rows[1] as HTMLElement);
    expect(document.querySelector(".quick-paste-item[data-selected=true]")).toHaveAttribute(
      "data-item-id",
      firstId,
    );
    fireEvent.click(within(rows[1] as HTMLElement).getByRole("button"));
    expect(document.querySelector(".quick-paste-item[data-selected=true]")).toHaveAttribute(
      "data-item-id",
      secondId,
    );
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.keyDown(search, { key: "c", metaKey: true });
    fireEvent.keyDown(search, { key: " " });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    expect(screen.queryByLabelText("Quick paste preview")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Copy to clipboard" }));
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1));
  });

  it("ignores a repeated activation while the clipboard write is pending", async () => {
    let finishWrite: (() => void) | undefined;
    vi.mocked(navigator.clipboard.writeText).mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishWrite = resolve;
        }),
    );
    renderOverlay();
    await screen.findByText("Visual Studio Code");

    fireEvent.keyDown(window, { key: "Enter" });
    fireEvent.keyDown(window, { key: "Enter" });
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(1);
    finishWrite?.();
    expect(await screen.findByRole("alert")).toHaveTextContent("Copied to the system clipboard");
  });

  it("previews a contextual Filter shortcut and closes only after confirmed clipboard write", async () => {
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, labsEnabled: true });
    const filter = await createBrowserLocalTextAction({
      name: "Quick shortcut uppercase",
      transform: "uppercase",
      shortcutSlot: 3,
    });
    renderOverlay();
    await screen.findByText("Visual Studio Code");
    await screen.findByLabelText("Filter shortcut 3 ready");
    const selectedContent = document.querySelector<HTMLElement>(
      ".quick-paste-item[data-selected=true] pre",
    )?.textContent;
    expect(selectedContent).toBeTruthy();
    vi.mocked(navigator.clipboard.writeText).mockClear();

    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.keyDown(search, { key: "3", metaKey: true, altKey: true });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "3", metaKey: true, altKey: true });
    const dialog = await screen.findByRole("dialog", { name: "Quick shortcut uppercase" });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Copy Filter result" }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(selectedContent?.toUpperCase());
    });
    await deleteBrowserLocalTextAction(filter.id);
  });

  it("keeps a recently used unpinned clip ahead of pins and supports keyboard preview", async () => {
    await copyBrowserItem("demo-url");
    vi.mocked(navigator.clipboard.writeText).mockClear();
    renderOverlay();
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    await screen.findByText("Visual Studio Code");

    expect(
      Array.from(document.querySelectorAll<HTMLElement>(".quick-paste-item")).map((item) =>
        item.getAttribute("data-item-id"),
      ),
    ).toEqual(expect.arrayContaining(["demo-code", "demo-text", "demo-url"]));
    expect(
      Array.from(document.querySelectorAll<HTMLElement>(".quick-paste-item"))
        .slice(0, 3)
        .map((item) => item.getAttribute("data-item-id")),
    ).toEqual(["demo-url", "demo-code", "demo-text"]);
    expect(
      document.querySelector(".quick-paste-item[data-item-id='demo-url'] .quick-pin"),
    ).not.toBeInTheDocument();

    fireEvent.keyDown(search, { key: "ArrowDown" });
    await waitFor(() =>
      expect(document.querySelector(".quick-paste-item[data-selected=true]")).toHaveAttribute(
        "data-item-id",
        "demo-code",
      ),
    );
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        "export function cleanClip(input: string) {\n  return input.trim();\n}",
      ),
    );

    fireEvent.change(search, { target: { value: "" } });
    fireEvent.keyDown(search, { key: "i", metaKey: true });
    expect(await screen.findByLabelText("Quick paste preview")).toHaveTextContent(
      "export function cleanClip(input: string) { return input.trim(); }",
    );
    expect(search).toHaveValue("");
    fireEvent.keyDown(search, { key: "Escape" });
    expect(screen.queryByLabelText("Quick paste preview")).not.toBeInTheDocument();
  });

  it("opens a one-step device sidecar from Command-K and preserves search on close", async () => {
    const preferences = await getBrowserLocalLinkPreferences();
    await updateBrowserLocalLinkPreferences({
      ...preferences,
      enabled: true,
      discoveryEnabled: true,
    });
    renderOverlay();
    await screen.findByText("Visual Studio Code");
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.change(search, { target: { value: "tauri" } });
    expect(await screen.findByText("tauri", { selector: "mark" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "k", metaKey: true });
    fireEvent.click(screen.getByRole("menuitem", { name: "Send to device" }));
    const dialog = await screen.findByRole("dialog", { name: "Handoff Sheet" });
    expect(
      await within(dialog).findByRole("button", { name: "Select Studio Mac mini" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(dialog.closest("[data-presentation]")).toHaveAttribute("data-presentation", "sidecar");
    expect(within(dialog).queryByText("Checking policy")).not.toBeInTheDocument();
    expect(within(dialog).getByText(/Copy, Save, or Reject/i)).toBeInTheDocument();
    expect(within(dialog).queryByText("Checking policy")).not.toBeInTheDocument();

    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Handoff Sheet" })).not.toBeInTheDocument(),
    );
    expect(search).toHaveValue("tauri");
  });

  it("does not delete clips with Delete or Backspace", async () => {
    renderOverlay();
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.change(search, { target: { value: "tauri" } });
    await screen.findByText("tauri", { selector: "mark" });

    search.focus();
    fireEvent.keyDown(search, { key: "Delete" });
    fireEvent.keyDown(search, { key: "Backspace" });
    expect(screen.getByText("tauri", { selector: "mark" })).toBeInTheDocument();
    expect(screen.queryByText(/moved to Recycle Bin/i)).not.toBeInTheDocument();
  });

  it("searches and copies command clips while keeping the footer to three hints", async () => {
    renderOverlay();
    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    await screen.findByText("Visual Studio Code");
    expect(
      screen.queryByRole("group", { name: "Filter Quick Paste by type" }),
    ).not.toBeInTheDocument();
    const hints = screen.getByLabelText("Quick Paste shortcuts");
    expect(hints.children).toHaveLength(3);
    expect(hints).toHaveTextContent("Navigate");
    expect(hints).toHaveTextContent("Copy");
    expect(hints).toHaveTextContent("⌘KMore");

    fireEvent.change(search, { target: { value: "runInBand" } });
    expect(await screen.findByText("runInBand", { selector: "mark" })).toBeInTheDocument();
    expect(document.querySelector(".quick-paste-item .kind-command svg")).toBeInTheDocument();
    await waitFor(() =>
      expect(document.querySelector(".quick-paste-item[data-selected=true]")).toHaveAttribute(
        "data-item-id",
        "demo-command",
      ),
    );

    vi.mocked(navigator.clipboard.writeText).mockClear();
    fireEvent.keyDown(window, { key: "Enter" });
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("pnpm test -- --runInBand"),
    );
  });

  it("keeps failed Stack paste in place and supports copy-and-advance without storage", async () => {
    renderOverlay();
    await screen.findByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "More Quick Paste actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    fireEvent.keyDown(window, { key: "ArrowDown" });
    fireEvent.click(screen.getByRole("button", { name: "More Quick Paste actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    const queue = screen.getByRole("region", { name: "Stack" });
    expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("0 / 2");

    fireEvent.click(within(queue).getByRole("button", { name: "Direct Paste & advance" }));
    expect(await within(queue).findByRole("alert")).toHaveTextContent(
      "could not confirm the previous app",
    );
    expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("0 / 2");

    fireEvent.click(within(queue).getByRole("button", { name: "Copy & advance" }));
    await waitFor(() =>
      expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("1 / 2"),
    );
    fireEvent.click(within(queue).getByRole("button", { name: "Skip" }));
    expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("2 / 2");
    expect(within(queue).getByText("Stack complete.")).toBeInTheDocument();
    expect(screen.getByLabelText("Quick Paste shortcuts").children).toHaveLength(3);
    fireEvent.click(within(queue).getByRole("button", { name: "Reset Stack" }));
    expect(screen.queryByRole("region", { name: "Stack" })).not.toBeInTheDocument();
  });

  it("advances Stack exactly once after a successful paste", async () => {
    setBrowserDirectPasteSimulationForTests("sent");
    renderOverlay();
    await screen.findByText("Visual Studio Code");

    fireEvent.click(screen.getByRole("button", { name: "More Quick Paste actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    const queue = screen.getByRole("region", { name: "Stack" });
    fireEvent.click(within(queue).getByRole("button", { name: "Direct Paste & advance" }));

    await waitFor(() =>
      expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("1 / 1"),
    );
    expect(within(queue).getByText("Stack complete.")).toBeInTheDocument();
  });

  it("activates a queued Item after a different search hides it", async () => {
    setBrowserDirectPasteSimulationForTests("sent");
    renderOverlay();
    await screen.findByText("Visual Studio Code");
    fireEvent.click(screen.getByRole("button", { name: "More Quick Paste actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));

    const search = screen.getByRole("searchbox", {
      name: "Search clipboard history for quick paste",
    });
    fireEvent.change(search, { target: { value: "no matching item" } });
    await screen.findByText(/No results for “no matching item”/i);
    const queue = screen.getByRole("region", { name: "Stack" });
    fireEvent.click(within(queue).getByRole("button", { name: "Direct Paste & advance" }));

    await waitFor(() =>
      expect(within(queue).getByLabelText("Stack progress")).toHaveTextContent("1 / 1"),
    );
    expect(within(queue).queryByText(/deleted or recycled/i)).not.toBeInTheDocument();
  });
});
