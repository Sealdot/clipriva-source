import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FilterComposerDialog } from "./FilterComposerDialog";

describe("FilterComposerDialog", () => {
  it("shows a local before/after preview and saves a normalized draft", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <FilterComposerDialog
        isOpen
        input="  meeting note  "
        initialDraft={{ name: "  Clean note  ", steps: ["trim_whitespace"] }}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    );

    expect(screen.getByText("Original clip").closest("div")?.nextElementSibling).toHaveTextContent(
      "meeting note",
    );
    expect(screen.getByRole("status")).toHaveTextContent("Preview ready");
    fireEvent.click(screen.getByRole("button", { name: "Save filter" }));

    expect(onSave).toHaveBeenCalledWith({
      name: "Clean note",
      steps: ["trim_whitespace"],
      shortcutSlot: null,
    });
  });

  it("adds, removes, and reorders steps while updating the preview", () => {
    render(
      <FilterComposerDialog
        isOpen
        input="Clip"
        initialDraft={{ name: "Case", steps: ["uppercase", "lowercase"] }}
        onClose={vi.fn()}
        onSave={vi.fn()}
      />,
    );

    const afterPane = screen.getByText("After").closest("div")?.parentElement;
    expect(afterPane).toHaveTextContent("clip");
    fireEvent.click(screen.getByRole("button", { name: "Move step 2 up" }));
    expect(afterPane).toHaveTextContent("CLIP");

    fireEvent.change(screen.getByLabelText("Add transform"), {
      target: { value: "trim_whitespace" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add step" }));
    expect(
      within(screen.getByRole("list", { name: "Filter steps" })).getAllByRole("listitem"),
    ).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: "Remove step 3" }));
    expect(
      within(screen.getByRole("list", { name: "Filter steps" })).getAllByRole("listitem"),
    ).toHaveLength(2);
  });

  it("pins a finite JSON error to its step and blocks save", () => {
    const privateInput = "private-invalid-json";
    render(
      <FilterComposerDialog
        isOpen
        input={privateInput}
        initialDraft={{ name: "JSON", steps: ["format_json"] }}
        onClose={vi.fn()}
        onSave={vi.fn()}
      />,
    );

    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Step 1: The JSON formatter accepts valid JSON only.");
    expect(alert).not.toHaveTextContent(privateInput);
    expect(screen.getByRole("button", { name: "Save filter" })).toBeDisabled();
    expect(screen.getByLabelText("Step 1 transform").closest("li")).toHaveAttribute(
      "data-error",
      "true",
    );
  });

  it("enforces one to eight steps with keyboard-accessible controls", () => {
    render(
      <FilterComposerDialog
        isOpen
        input="note"
        initialDraft={{
          name: "Eight steps",
          steps: [
            "trim_whitespace",
            "uppercase",
            "lowercase",
            "normalize_whitespace",
            "remove_blank_lines",
            "deduplicate_lines",
            "sort_lines_asc",
            "sort_lines_desc",
          ],
        }}
        onClose={vi.fn()}
        onSave={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Add step" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Remove step 8" }));
    expect(screen.getByRole("button", { name: "Add step" })).toBeEnabled();

    for (let index = 7; index > 1; index -= 1) {
      fireEvent.click(screen.getByRole("button", { name: `Remove step ${index}` }));
    }
    expect(screen.getByRole("button", { name: "Remove step 1" })).toBeDisabled();
  });

  it("uses a finite save failure instead of exposing an adapter error", async () => {
    const privateAdapterError = "backend included private clipboard content";
    render(
      <FilterComposerDialog
        isOpen
        input="safe note"
        initialDraft={{ name: "Clean", steps: ["trim_whitespace"] }}
        onClose={vi.fn()}
        onSave={() => Promise.reject(new Error(privateAdapterError))}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Save filter" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Could not save this local filter.");
    expect(alert).not.toHaveTextContent(privateAdapterError);
  });

  it("edits a contextual shortcut and confirms definition-only deletion", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const onDelete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(
      <FilterComposerDialog
        isOpen
        input="note"
        initialDraft={{ name: "Clean", steps: ["trim_whitespace"], shortcutSlot: 2 }}
        unavailableShortcutSlots={[1]}
        onClose={onClose}
        onSave={onSave}
        onDelete={onDelete}
      />,
    );

    expect(screen.getByRole("heading", { name: "Edit Filter" })).toBeInTheDocument();
    expect(screen.getByLabelText(/Contextual shortcut/i)).toHaveValue("2");
    fireEvent.change(screen.getByLabelText(/Contextual shortcut/i), { target: { value: "3" } });
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    expect(onSave).toHaveBeenCalledWith({
      name: "Clean",
      steps: ["trim_whitespace"],
      shortcutSlot: 3,
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Delete definition" })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Delete definition" }));
    expect(onDelete).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(/Existing clipboard Items are unchanged/i);
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));
    await waitFor(() => expect(onDelete).toHaveBeenCalledOnce());
  });
});
