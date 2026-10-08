import type { ClipboardCleanupRequest } from "../features/clipboard/types";

export type WorkspaceView = "history" | "pinned" | "localLinkPreview";

export type WorkspaceSurface =
  | { kind: "none" }
  | { kind: "details" }
  | { kind: "settings" }
  | { kind: "send" }
  | { kind: "transfers" }
  | { kind: "incoming" }
  | { kind: "cleanup"; request: ClipboardCleanupRequest }
  | { kind: "recycleBin" };

export interface WorkspaceUiState {
  view: WorkspaceView;
  surface: WorkspaceSurface;
}

export type WorkspaceUiAction =
  | { type: "switchView"; view: WorkspaceView }
  | { type: "openSurface"; surface: Exclude<WorkspaceSurface, { kind: "none" }> }
  | { type: "toggleSurface"; kind: "incoming" | "transfers" }
  | { type: "updateCleanup"; request: ClipboardCleanupRequest }
  | { type: "closeSurface" };

export const initialWorkspaceUiState: WorkspaceUiState = {
  view: "history",
  surface: { kind: "none" },
};

/**
 * Centralizes the workspace invariant that only one primary drawer, dialog or
 * popover can compete for attention at a time.
 */
export function reduceWorkspaceUiState(
  state: WorkspaceUiState,
  action: WorkspaceUiAction,
): WorkspaceUiState {
  switch (action.type) {
    case "switchView":
      return { view: action.view, surface: { kind: "none" } };
    case "openSurface":
      return { ...state, surface: action.surface };
    case "toggleSurface":
      return {
        ...state,
        surface: state.surface.kind === action.kind ? { kind: "none" } : { kind: action.kind },
      };
    case "updateCleanup":
      return state.surface.kind === "cleanup"
        ? { ...state, surface: { kind: "cleanup", request: action.request } }
        : state;
    case "closeSurface":
      return { ...state, surface: { kind: "none" } };
  }
}
