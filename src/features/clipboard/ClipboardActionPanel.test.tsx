import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClipboardActionPanel } from "./ClipboardActionPanel";

describe("ClipboardActionPanel", () => {
  it("orders activation, collection, sharing and recycle actions", () => {
    render(
      <ClipboardActionPanel
        isSaved={false}
        onPaste={vi.fn()}
        onPlainTextPaste={vi.fn()}
        onCopy={vi.fn()}
        onToggleSaved={vi.fn()}
        onManageCollections={vi.fn()}
        onEnqueue={vi.fn()}
        onSend={vi.fn()}
        onRecycle={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "More item actions" }));
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Paste↵",
      "Paste as plain text⇧↵",
      "Copy⌘C",
      "Save for reuse",
      "Manage Collections",
      "Add to Stack",
      "Send to device",
      "Move to Recycle Bin",
    ]);
  });

  it("delegates the optional Queue action from the organize section", () => {
    const onEnqueue = vi.fn();
    render(<ClipboardActionPanel isSaved={false} onCopy={vi.fn()} onEnqueue={onEnqueue} />);

    fireEvent.click(screen.getByRole("button", { name: "More item actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to Stack" }));
    expect(onEnqueue).toHaveBeenCalledOnce();
  });

  it("supports roving focus, Escape focus restoration and Command-K opening", () => {
    render(
      <ClipboardActionPanel
        isSaved
        shortcutEnabled
        onCopy={vi.fn()}
        onToggleSaved={vi.fn()}
        onSend={vi.fn()}
      />,
    );
    const trigger = screen.getByRole("button", { name: "More item actions" });

    fireEvent.keyDown(window, { key: "k", metaKey: true });
    const menu = screen.getByRole("menu", { name: "Clipboard actions" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items[0]).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(items[1]).toHaveFocus();
    fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu", { name: "Clipboard actions" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("requires a second explicit choice before recycling", () => {
    const onRecycle = vi.fn();
    render(<ClipboardActionPanel isSaved={false} onCopy={vi.fn()} onRecycle={onRecycle} />);

    fireEvent.click(screen.getByRole("button", { name: "More item actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Move to Recycle Bin" }));
    expect(onRecycle).not.toHaveBeenCalled();
    const confirmation = screen.getByRole("alertdialog", { name: "Move clip to Recycle Bin?" });
    fireEvent.click(within(confirmation).getByRole("button", { name: "Move to Recycle Bin" }));
    expect(onRecycle).toHaveBeenCalledOnce();
  });
});
