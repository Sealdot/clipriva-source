import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isDesktopRuntime } from "../lib/runtime";

export function isQuickPasteWindow() {
  if (isDesktopRuntime()) {
    return getCurrentWebviewWindow().label === "quick-paste";
  }

  return new URLSearchParams(window.location.search).has("quick-paste-preview");
}
