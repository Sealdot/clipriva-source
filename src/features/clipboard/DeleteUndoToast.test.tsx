import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DeleteUndoToast } from "./DeleteUndoToast";

describe("DeleteUndoToast", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("offers recovery and dismisses only the message after six seconds", () => {
    vi.useFakeTimers();
    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(<DeleteUndoToast itemLabel="Link clip" onUndo={onUndo} onDismiss={onDismiss} />);

    const toast = screen.getByRole("status");
    expect(toast).toHaveAttribute("aria-keyshortcuts", "Meta+Z");
    fireEvent.click(screen.getByRole("button", { name: /^Undo/i }));
    expect(onUndo).toHaveBeenCalledOnce();

    vi.advanceTimersByTime(5_999);
    expect(onDismiss).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
