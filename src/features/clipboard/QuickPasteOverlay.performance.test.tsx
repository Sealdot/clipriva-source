import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  onFocusChanged: vi.fn(),
  opened: null as null | (() => void),
}));

vi.mock("../../lib/runtime", () => ({
  isDesktopRuntime: () => true,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: tauri.invoke,
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    hide: vi.fn().mockResolvedValue(undefined),
    onFocusChanged: tauri.onFocusChanged.mockImplementation(() => Promise.resolve(() => undefined)),
    listen: tauri.listen.mockImplementation((_event: string, handler: () => void) => {
      tauri.opened = handler;
      return Promise.resolve(() => undefined);
    }),
  }),
}));

import { QuickPasteOverlay } from "./QuickPasteOverlay";

describe("QuickPasteOverlay background resource use", () => {
  it("keeps native queries dormant until the hidden window is opened", async () => {
    tauri.invoke.mockImplementation((command: string) => {
      switch (command) {
        case "search_quick_paste_items":
        case "list_recent_capture_status_events":
          return Promise.resolve([]);
        case "get_paste_permission_status":
          return Promise.resolve("granted");
        case "get_capture_preferences":
          return Promise.resolve({
            capturePaused: false,
            onboardingCompleted: true,
          });
        default:
          return Promise.resolve(undefined);
      }
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, refetchInterval: false } },
    });

    render(
      <QueryClientProvider client={client}>
        <QuickPasteOverlay />
      </QueryClientProvider>,
    );

    await waitFor(() =>
      expect(tauri.listen).toHaveBeenCalledWith("quick-paste-opened", expect.any(Function)),
    );
    expect(tauri.invoke).not.toHaveBeenCalled();

    act(() => tauri.opened?.());
    await waitFor(() =>
      expect(tauri.invoke).toHaveBeenCalledWith(
        "search_quick_paste_items",
        expect.objectContaining({ limit: 20 }),
      ),
    );
  });
});
