import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SmartCollectionDialog } from "./SmartCollectionDialog";

describe("SmartCollectionDialog", () => {
  it("saves a named finite rule and explains excluded temporary scopes", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(
      <SmartCollectionDialog
        rule={{ kinds: ["code"], recentlyUsedOnly: true }}
        searchExcluded
        manualCollectionExcluded
        onSave={onSave}
        onClose={onClose}
      />,
    );

    const name = screen.getByLabelText("Name");
    expect(name).toHaveFocus();
    expect(screen.getByText("Current search text is not included.")).toBeInTheDocument();
    expect(screen.getByText("Manual Collection scope is not included.")).toBeInTheDocument();
    fireEvent.change(name, { target: { value: "Recent code" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Smart Collection" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith({
        name: "Recent code",
        rule: { kinds: ["code"], recentlyUsedOnly: true },
      }),
    );
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("edits a definition and requires confirmation before deleting only that definition", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onDelete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(
      <SmartCollectionDialog
        rule={{ pinFilter: "pinned" }}
        initialName="Saved research"
        searchExcluded={false}
        manualCollectionExcluded={false}
        onSave={onSave}
        onDelete={onDelete}
        onClose={onClose}
      />,
    );

    expect(screen.getByRole("heading", { name: "Edit Smart Collection" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("Saved research");
    fireEvent.click(screen.getByRole("button", { name: "Delete definition" }));
    expect(onDelete).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(
      /No clips, Saved state, Notes, or manual Collections will be changed/i,
    );
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() => expect(onDelete).toHaveBeenCalledOnce());
    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes with Escape without mutating the definition", () => {
    const onSave = vi.fn();
    const onClose = vi.fn();
    render(
      <SmartCollectionDialog
        rule={{ sourceApp: "Synthetic Editor" }}
        searchExcluded={false}
        manualCollectionExcluded={false}
        onSave={onSave}
        onClose={onClose}
      />,
    );

    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
    expect(onSave).not.toHaveBeenCalled();
  });
});
