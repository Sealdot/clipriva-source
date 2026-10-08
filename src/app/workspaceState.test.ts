import { describe, expect, it } from "vitest";
import { initialWorkspaceUiState, reduceWorkspaceUiState } from "./workspaceState";

describe("workspace UI state", () => {
  it("replaces the current primary surface instead of stacking surfaces", () => {
    const details = reduceWorkspaceUiState(initialWorkspaceUiState, {
      type: "openSurface",
      surface: { kind: "details" },
    });
    const settings = reduceWorkspaceUiState(details, {
      type: "openSurface",
      surface: { kind: "settings" },
    });
    const incoming = reduceWorkspaceUiState(settings, {
      type: "openSurface",
      surface: { kind: "incoming" },
    });

    expect(incoming).toEqual({ view: "history", surface: { kind: "incoming" } });
  });

  it("closes the current surface when the same transient surface is toggled", () => {
    const open = reduceWorkspaceUiState(initialWorkspaceUiState, {
      type: "toggleSurface",
      kind: "transfers",
    });
    expect(
      reduceWorkspaceUiState(open, { type: "toggleSurface", kind: "transfers" }).surface,
    ).toEqual({ kind: "none" });
  });

  it("switches workspace views with no stale surface", () => {
    const open = reduceWorkspaceUiState(initialWorkspaceUiState, {
      type: "openSurface",
      surface: { kind: "details" },
    });
    expect(reduceWorkspaceUiState(open, { type: "switchView", view: "localLinkPreview" })).toEqual({
      view: "localLinkPreview",
      surface: { kind: "none" },
    });
  });

  it("updates only an active cleanup request", () => {
    const request = { scope: "allUnpinned" as const, includePinned: false };
    const cleanup = reduceWorkspaceUiState(initialWorkspaceUiState, {
      type: "openSurface",
      surface: { kind: "cleanup", request },
    });
    expect(
      reduceWorkspaceUiState(cleanup, {
        type: "updateCleanup",
        request: { ...request, includePinned: true },
      }).surface,
    ).toEqual({ kind: "cleanup", request: { ...request, includePinned: true } });
  });
});
