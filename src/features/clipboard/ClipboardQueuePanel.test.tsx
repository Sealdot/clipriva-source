import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClipboardQueuePanel } from "./ClipboardQueuePanel";
import type { ClipboardQueueState } from "./clipboardQueue";

const state: ClipboardQueueState = {
  order: [
    { itemId: "a", availability: "available" },
    { itemId: "b", availability: "available" },
  ],
  cursor: 0,
  busyToken: null,
};

function renderPanel(overrides: Partial<Parameters<typeof ClipboardQueuePanel>[0]> = {}) {
  const props: Parameters<typeof ClipboardQueuePanel>[0] = {
    state,
    activationMode: "paste",
    getItemLabel: (itemId) => `Clip ${itemId.toUpperCase()}`,
    onActivate: vi.fn(),
    onRetry: vi.fn(),
    onCopyAdvance: vi.fn(),
    onSkip: vi.fn(),
    onReset: vi.fn(),
    ...overrides,
  };
  render(<ClipboardQueuePanel {...props} />);
  return props;
}

describe("ClipboardQueuePanel", () => {
  it("shows ordered progress and delegates activation without clipboard content", () => {
    const props = renderPanel();

    expect(screen.getByLabelText("Stack progress")).toHaveTextContent("0 / 2");
    expect(
      screen.getByText("Temporary · up to 20 clips · clears when ClipRiva quits"),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "1Clip ACurrent",
      "2Clip BNext",
    ]);
    fireEvent.click(screen.getByRole("button", { name: "Direct Paste & advance" }));
    expect(props.onActivate).toHaveBeenCalledWith("a");
  });

  it("offers retry, copy-and-advance, and skip after a failure", () => {
    const props = renderPanel({ failureMessage: "The target app did not accept paste." });

    expect(screen.getByRole("alert")).toHaveTextContent("did not accept paste");
    fireEvent.click(screen.getByRole("button", { name: "Retry Direct Paste" }));
    fireEvent.click(screen.getByRole("button", { name: "Copy & advance" }));
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(props.onRetry).toHaveBeenCalledWith("a");
    expect(props.onCopyAdvance).toHaveBeenCalledWith("a");
    expect(props.onSkip).toHaveBeenCalledWith("a");
  });

  it("names the main workspace action as Copy and avoids a duplicate failure option", () => {
    const onActivate = vi.fn();
    const onRetry = vi.fn();
    const onCopyAdvance = vi.fn();
    const view = render(
      <ClipboardQueuePanel
        state={state}
        activationMode="copy"
        onActivate={onActivate}
        onRetry={onRetry}
        onCopyAdvance={onCopyAdvance}
        onSkip={vi.fn()}
        onReset={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Copy & advance" }));
    expect(onActivate).toHaveBeenCalledWith("a");
    view.rerender(
      <ClipboardQueuePanel
        state={state}
        activationMode="copy"
        failureMessage="Could not copy this clip."
        onActivate={onActivate}
        onRetry={onRetry}
        onCopyAdvance={onCopyAdvance}
        onSkip={vi.fn()}
        onReset={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry Copy" }));
    expect(onRetry).toHaveBeenCalledWith("a");
    expect(screen.queryByRole("button", { name: "Copy & advance" })).not.toBeInTheDocument();
    expect(onCopyAdvance).not.toHaveBeenCalled();
  });

  it("prevents activation of an unavailable current item and allows skip/reset", () => {
    const props = renderPanel({
      state: {
        order: [{ itemId: "a", availability: "unavailable" }],
        cursor: 0,
        busyToken: null,
      },
    });

    expect(screen.getByText(/deleted or recycled/i)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Direct Paste & advance" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Skip" }));
    fireEvent.click(screen.getByRole("button", { name: "Reset Stack" }));
    expect(props.onSkip).toHaveBeenCalledWith("a");
    expect(props.onReset).toHaveBeenCalledOnce();
  });
});
