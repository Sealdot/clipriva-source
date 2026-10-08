import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClipboardCard } from "./ClipboardCard";
import type { ClipboardItem } from "./types";

const item: ClipboardItem = {
  id: "clip-1",
  content: "Example clipboard content",
  kind: "text",
  sourceApp: "Notes",
  createdAt: "2026-07-26T00:00:00.000Z",
  updatedAt: "2026-07-26T00:00:00.000Z",
  isPinned: false,
  copyCount: 0,
};

describe("ClipboardCard", () => {
  it("presents command clips with the shared type label and terminal treatment", () => {
    render(
      <ClipboardCard
        item={{ ...item, kind: "command", content: "cargo test --locked", sourceApp: "Terminal" }}
        selected={false}
        onSelect={vi.fn()}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onSend={vi.fn()}
        onTogglePin={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Inspect Command clipboard item" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Command")).toBeInTheDocument();
    expect(document.querySelector(".kind-icon.kind-command svg")).toBeInTheDocument();
  });

  it("keeps deletion in a deliberate More menu", () => {
    const onDelete = vi.fn();

    render(
      <ClipboardCard
        item={item}
        selected={false}
        onSelect={vi.fn()}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onSend={vi.fn()}
        onTogglePin={vi.fn()}
        onDelete={onDelete}
      />,
    );

    expect(screen.queryByRole("menuitem", { name: "Move to Recycle Bin" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "More item actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Move to Recycle Bin" }));

    expect(onDelete).not.toHaveBeenCalled();
    const confirmation = screen.getByRole("alertdialog", {
      name: "Move clip to Recycle Bin?",
    });
    fireEvent.click(within(confirmation).getByRole("button", { name: "Move to Recycle Bin" }));

    expect(onDelete).toHaveBeenCalledOnce();
  });

  it("passes an optional Queue action through the shared panel", () => {
    const onEnqueue = vi.fn();
    render(
      <ClipboardCard
        item={item}
        selected={false}
        onSelect={vi.fn()}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onSend={vi.fn()}
        onTogglePin={vi.fn()}
        onEnqueue={onEnqueue}
        onDelete={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "More item actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    expect(onEnqueue).toHaveBeenCalledOnce();
  });
});
