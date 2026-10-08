import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CleanupPreviewDialog } from "./CleanupPreviewDialog";
import type { ClipboardCleanupPreview } from "./types";

const preview: ClipboardCleanupPreview = {
  scope: "selected",
  affectedCount: 2,
  pinnedSkippedCount: 1,
  ruleConditions: ["Only the selected clips are included.", "Pinned clips are skipped by default."],
  representativeItems: [
    {
      id: "clip-1",
      kind: "text",
      sourceApp: "Notes",
      capturedAt: "2026-07-26T00:00:00.000Z",
      isPinned: false,
    },
  ],
  recycleBinRetention: {
    maximumDays: 7,
    earliestExpiresAt: "2026-08-02T00:00:00.000Z",
    latestExpiresAt: "2026-08-02T00:00:00.000Z",
  },
};

describe("CleanupPreviewDialog", () => {
  it("starts on Cancel and describes only cleanup metadata", async () => {
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(
      <CleanupPreviewDialog isOpen preview={preview} onCancel={onCancel} onConfirm={onConfirm} />,
    );

    expect(screen.getByRole("alertdialog")).toHaveTextContent("2 clips are affected");
    expect(screen.getByLabelText("Affected clip metadata")).toHaveTextContent("TextNotes");
    expect(screen.getByText(/Pinned clips are skipped/i)).toBeInTheDocument();
    expect(screen.queryByText("clipboard secret content")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus());

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onCancel).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Move to Recycle Bin" }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });
});
