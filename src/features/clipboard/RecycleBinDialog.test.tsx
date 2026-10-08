import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RecycleBinDialog } from "./RecycleBinDialog";
import type { RecycleBinItem } from "./types";

const item: RecycleBinItem = {
  item: {
    id: "recycle-1",
    content: "A locally recoverable note",
    kind: "text",
    sourceApp: "Notes",
    createdAt: "2026-07-26T00:00:00.000Z",
    updatedAt: "2026-07-26T00:00:00.000Z",
    isPinned: false,
    copyCount: 0,
  },
  deletedAt: "2026-07-26T10:00:00.000Z",
  recycleExpiresAt: "2026-07-27T10:00:00.000Z",
};

describe("RecycleBinDialog", () => {
  it("restores locally and requires a separate acknowledgement for permanent deletion", async () => {
    const onRestore = vi.fn().mockResolvedValue(undefined);
    const onPermanentlyDelete = vi.fn().mockResolvedValue(undefined);
    render(
      <RecycleBinDialog
        isOpen
        items={[item]}
        onClose={vi.fn()}
        onRestore={onRestore}
        onPermanentlyDelete={onPermanentlyDelete}
        onEmpty={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByRole("dialog", { name: "Recycle Bin" })).toHaveTextContent(
      "A locally recoverable note",
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Close Recycle Bin" })).toHaveFocus(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(onRestore).toHaveBeenCalledWith("recycle-1"));

    fireEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("Permanently delete 1 clip?");
    const permanentButtons = screen.getAllByRole("button", { name: "Delete permanently" });
    const confirmPermanentDelete = permanentButtons.at(1);
    expect(confirmPermanentDelete).toBeDefined();
    if (!confirmPermanentDelete) {
      throw new Error("Expected the permanent-delete confirmation button");
    }
    fireEvent.click(confirmPermanentDelete);
    await waitFor(() => expect(onPermanentlyDelete).toHaveBeenCalledWith(["recycle-1"]));
  });
});
