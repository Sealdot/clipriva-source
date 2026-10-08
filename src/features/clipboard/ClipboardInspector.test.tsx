import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as clipboardApi from "./api";
import { ClipboardInspector } from "./ClipboardInspector";
import type { ClipboardItem } from "./types";

const item: ClipboardItem = {
  id: "clip-1",
  content: "Example clipboard content",
  kind: "text",
  sourceApp: "Notes",
  createdAt: "2026-07-26T00:00:00.000Z",
  updatedAt: "2026-07-26T00:00:00.000Z",
  isPinned: true,
  copyCount: 0,
};

describe("ClipboardInspector", () => {
  afterEach(() => vi.restoreAllMocks());

  it("shows Notes and Labs together without duplicate React keys", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      render(
        <QueryClientProvider client={client}>
          <ClipboardInspector
            item={item}
            labsEnabled={true}
            onCopy={vi.fn()}
            onTogglePin={vi.fn()}
            onSend={vi.fn()}
          />
        </QueryClientProvider>,
      );

      expect(screen.getByRole("textbox", { name: "Searchable Note" })).toBeInTheDocument();
      expect(await screen.findByRole("region", { name: "Local actions" })).toBeInTheDocument();
      expect(consoleError.mock.calls.some((args) => String(args[0]).includes("same key"))).toBe(
        false,
      );
    } finally {
      consoleError.mockRestore();
    }
  });

  it("runs image text extraction only after an explicit local action", async () => {
    let resolveExtraction: (
      result: Awaited<ReturnType<typeof clipboardApi.extractClipboardImageText>>,
    ) => void = () => undefined;
    const extractSpy = vi.spyOn(clipboardApi, "extractClipboardImageText").mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveExtraction = resolve;
        }),
    );
    const cancelSpy = vi
      .spyOn(clipboardApi, "cancelClipboardImageTextExtraction")
      .mockImplementationOnce(async () => {
        resolveExtraction({ status: "cancelled", extraction: null });
        return true;
      });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <ClipboardInspector
          item={{ ...item, id: "demo-image", kind: "image" }}
          labsEnabled={false}
          onCopy={vi.fn()}
          onTogglePin={vi.fn()}
          onSend={vi.fn()}
        />
      </QueryClientProvider>,
    );

    expect(screen.getByRole("region", { name: "Text in image" })).toHaveTextContent(
      "runs only when you choose it",
    );
    fireEvent.click(screen.getByRole("button", { name: "Extract text locally" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel extraction" }));
    await waitFor(() => {
      expect(screen.getByText("Extraction cancelled. No text was stored.")).toBeInTheDocument();
    });
    extractSpy.mockRestore();
    cancelSpy.mockRestore();

    fireEvent.click(screen.getByRole("button", { name: "Extract text locally" }));
    await waitFor(() => {
      expect(screen.getByText("Release dashboard synthetic browser preview")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Delete extracted text" }));
    await waitFor(() => {
      expect(screen.queryByText("Release dashboard synthetic browser preview")).toBeNull();
    });
  });

  it("consolidates actions in the shared panel and exposes collection state", () => {
    const onCopy = vi.fn();
    const onTogglePin = vi.fn();
    const onSend = vi.fn();
    const onEnqueue = vi.fn();
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    render(
      <QueryClientProvider client={client}>
        <ClipboardInspector
          item={item}
          labsEnabled={false}
          onCopy={onCopy}
          onTogglePin={onTogglePin}
          onSend={onSend}
          onEnqueue={onEnqueue}
        />
      </QueryClientProvider>,
    );

    const more = screen.getByRole("button", { name: "More item actions" });
    expect(screen.queryByRole("menuitem", { name: "Copy" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    fireEvent.click(more);
    fireEvent.click(screen.getByRole("menuitem", { name: "Remove from Saved" }));
    fireEvent.click(more);
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    fireEvent.click(more);
    fireEvent.click(screen.getByRole("menuitem", { name: "Send to device" }));

    expect(onTogglePin).toHaveBeenCalledWith(item.id);
    expect(onCopy).toHaveBeenCalledWith(item.id);
    expect(onEnqueue).toHaveBeenCalledWith(item);
    expect(onSend).toHaveBeenCalledWith(item);
  });

  it("explains why a clip remains local without exposing a policy match", () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <ClipboardInspector
          item={{
            ...item,
            isPinned: false,
            retentionUntil: "2026-08-28T12:00:00.000Z",
          }}
          labsEnabled={false}
          onCopy={vi.fn()}
          onTogglePin={vi.fn()}
          onSend={vi.fn()}
        />
      </QueryClientProvider>,
    );

    const explanation = screen.getByRole("region", { name: "Why this clip is saved" });
    expect(explanation).toHaveTextContent("Saved by local history policy");
    expect(explanation).toHaveTextContent("stored locally until 2026-08-28");
    expect(explanation).not.toHaveTextContent("sensitive-content match");
  });

  it("edits bounded local tags without changing the clipboard body", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const onReplaceTags = vi.fn().mockResolvedValue(undefined);
    render(
      <QueryClientProvider client={client}>
        <ClipboardInspector
          item={{ ...item, tags: ["Reference"] }}
          labsEnabled={false}
          onCopy={vi.fn()}
          onTogglePin={vi.fn()}
          onSend={vi.fn()}
          onReplaceTags={onReplaceTags}
        />
      </QueryClientProvider>,
    );

    expect(screen.getByText("Reference")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Comma-separated Collections"), {
      target: { value: "Release, Triage" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save Collections" }));

    await waitFor(() => {
      expect(onReplaceTags).toHaveBeenCalledWith(item.id, ["Release", "Triage"]);
    });
    expect(screen.getByText("Example clipboard content")).toBeInTheDocument();
    expect(screen.getByText(/never shared/i)).toBeInTheDocument();
  });
});
