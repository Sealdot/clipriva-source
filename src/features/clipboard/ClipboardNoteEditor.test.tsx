import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { deleteBrowserClipboardItemNote } from "./browserRepository";
import { ClipboardNoteEditor } from "./ClipboardNoteEditor";

function renderEditor() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ClipboardNoteEditor itemId="demo-code" />
    </QueryClientProvider>,
  );
}

describe("ClipboardNoteEditor", () => {
  beforeEach(async () => {
    await deleteBrowserClipboardItemNote("demo-code");
  });

  it("saves, protects, and explicitly deletes an Item-owned searchable Note", async () => {
    renderEditor();
    const editor = screen.getByLabelText("Searchable Note");
    fireEvent.change(editor, { target: { value: "Synthetic launch owner" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Note" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Note saved locally.");
    expect(await screen.findByRole("button", { name: "Delete Note" })).toBeInTheDocument();

    fireEvent.change(editor, { target: { value: "-----BEGIN OPENSSH PRIVATE KEY-----" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Note" }));
    expect(await screen.findByRole("status")).toHaveTextContent(/may contain a secret/i);

    fireEvent.click(screen.getByRole("button", { name: "Delete Note" }));
    await waitFor(() => expect(editor).toHaveValue(""));
    expect(editor).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("Note deleted.");
  });

  it("blocks a UTF-8 Note beyond 16 KiB before calling the repository", () => {
    renderEditor();
    fireEvent.change(screen.getByLabelText("Searchable Note"), {
      target: { value: "界".repeat(5_500) },
    });
    expect(screen.getByRole("alert")).toHaveTextContent("Notes must be at most 16 KiB.");
    expect(screen.getByRole("button", { name: "Save Note" })).toBeDisabled();
  });
});
