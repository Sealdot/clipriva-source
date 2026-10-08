import { Check, CircleAlert, CircleDot, Clock3, XCircle } from "lucide-react";
import { type RefObject, useEffect, useRef, useState } from "react";
import type { LocalLinkTone } from "./LocalLinkPresentation";

const focusableSelector = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

export function useLocalLinkDialogFocus(
  isOpen: boolean,
  containerRef: RefObject<HTMLElement | null>,
  onEscape: () => void,
  initialFocusRef?: RefObject<HTMLElement | null>,
  returnFocusRef?: RefObject<HTMLElement | null>,
) {
  const onEscapeRef = useRef(onEscape);
  useEffect(() => {
    onEscapeRef.current = onEscape;
  }, [onEscape]);

  useEffect(() => {
    if (!isOpen) return;
    const container = containerRef.current;
    if (!container) return;
    const activeElement = document.activeElement;
    const trigger =
      activeElement instanceof HTMLElement && activeElement !== document.body
        ? activeElement
        : null;
    const focusFrame = window.requestAnimationFrame(() =>
      (initialFocusRef?.current ?? container).focus(),
    );

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onEscapeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      event.stopPropagation();

      const elements = Array.from(
        container.querySelectorAll<HTMLElement>(focusableSelector),
      ).filter((element) => element.tabIndex >= 0);
      if (elements.length === 0) {
        event.preventDefault();
        container.focus();
        return;
      }
      const currentIndex = elements.indexOf(document.activeElement as HTMLElement);
      const first = elements[0];
      const last = elements.at(-1);
      if (currentIndex === -1) {
        event.preventDefault();
        (event.shiftKey ? last : first)?.focus();
      } else if (event.shiftKey && currentIndex === 0) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && currentIndex === elements.length - 1) {
        event.preventDefault();
        first?.focus();
      }
    };

    container.addEventListener("keydown", handleKeyDown);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      container.removeEventListener("keydown", handleKeyDown);
      const restoreTarget = returnFocusRef?.current?.isConnected
        ? returnFocusRef.current
        : trigger?.isConnected
          ? trigger
          : null;
      restoreTarget?.focus();
      // Conditional triggers can be removed in the same commit as the dialog.
      // Re-resolve once the replacement control has mounted.
      window.requestAnimationFrame(() => {
        if (document.activeElement !== document.body) return;
        const fallback = returnFocusRef?.current?.isConnected
          ? returnFocusRef.current
          : trigger?.isConnected
            ? trigger
            : null;
        fallback?.focus();
      });
    };
  }, [containerRef, initialFocusRef, isOpen, returnFocusRef]);
}

export function useLocalLinkCountdown(expiresAt?: string | null) {
  const [seconds, setSeconds] = useState(() => secondsUntil(expiresAt));
  useEffect(() => {
    setSeconds(secondsUntil(expiresAt));
    if (!expiresAt) return;
    const interval = window.setInterval(() => setSeconds(secondsUntil(expiresAt)), 1_000);
    return () => window.clearInterval(interval);
  }, [expiresAt]);
  return seconds;
}

export function LocalLinkStatusBadge({ label, tone }: { label: string; tone: LocalLinkTone }) {
  const Icon =
    tone === "success"
      ? Check
      : tone === "failure"
        ? XCircle
        : tone === "warning"
          ? CircleAlert
          : tone === "progress"
            ? Clock3
            : CircleDot;
  return (
    <span className="local-link-status-badge" data-tone={tone}>
      <Icon size={13} aria-hidden="true" />
      {label}
    </span>
  );
}

function secondsUntil(expiresAt?: string | null) {
  if (!expiresAt) return null;
  return Math.max(0, Math.ceil((new Date(expiresAt).getTime() - Date.now()) / 1_000));
}
