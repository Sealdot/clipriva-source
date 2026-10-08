import type { ClipboardItemKind } from "./types";

export function formatRelativeTime(value: string, now = Date.now()) {
  const elapsedSeconds = Math.round((new Date(value).getTime() - now) / 1000);
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

  if (Math.abs(elapsedSeconds) < 60) {
    return formatter.format(elapsedSeconds, "second");
  }
  const minutes = Math.round(elapsedSeconds / 60);
  if (Math.abs(minutes) < 60) {
    return formatter.format(minutes, "minute");
  }
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) {
    return formatter.format(hours, "hour");
  }
  return formatter.format(Math.round(hours / 24), "day");
}

export const kindLabel: Record<ClipboardItemKind, string> = {
  text: "Text",
  code: "Code",
  command: "Command",
  url: "Link",
  color: "Color",
  image: "Image",
  richText: "Rich text",
  file: "File",
};

export function formatByteSize(byteSize: number) {
  if (byteSize < 1024) return `${byteSize} B`;
  if (byteSize < 1024 * 1024) return `${(byteSize / 1024).toFixed(1)} KB`;
  return `${(byteSize / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * The native Save path deliberately persists a compact source string instead
 * of a transferable device identifier. Keep its user-facing presentation
 * clear without widening the History DTO or exposing a Local Link peer ref.
 */
export function isLocalLinkHistorySource(source: string | null | undefined) {
  return source?.startsWith("Local Link from ") ?? false;
}

export function formatClipboardSource(source: string | null | undefined) {
  if (!source) return "Unknown app";
  if (!isLocalLinkHistorySource(source)) return source;
  return `From ${source.slice("Local Link from ".length)} · Local Link`;
}
