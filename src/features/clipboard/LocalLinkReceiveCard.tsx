import { Archive, ClipboardCheck, Copy, Eye, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { isDesktopRuntime } from "../../lib/runtime";
import { formatByteSize } from "./format";
import { normalizedLocalLinkStatus, presentLocalLinkTransfer } from "./LocalLinkPresentation";
import { LocalLinkStatusBadge, useLocalLinkCountdown } from "./LocalLinkUi";
import { useLocalLinkActions, useLocalLinkDevices, useLocalLinkTransfers } from "./queries";
import type { LocalLinkAcceptAction, LocalLinkTransfer } from "./types";
import "./LocalLink.css";

interface LocalLinkReceiveCardProps {
  isSuppressed?: boolean;
  onReveal?: () => void;
  presentation?: "floating" | "inline";
}

/** Incoming plaintext stays outside clipboard/history until one explicit decision. */
export function LocalLinkReceiveCard({
  isSuppressed = false,
  onReveal,
  presentation: layout = "floating",
}: LocalLinkReceiveCardProps) {
  const transfersQuery = useLocalLinkTransfers();
  const devicesQuery = useLocalLinkDevices();
  const actions = useLocalLinkActions();
  const [dismissedTransferId, setDismissedTransferId] = useState<string | null>(null);
  const [revealStatus, setRevealStatus] = useState<string | null>(null);
  const copyButtonRef = useRef<HTMLButtonElement>(null);
  const actionStartedRef = useRef(false);
  const transfer = useMemo(
    () => findPresentableIncomingTransfer(transfersQuery.data ?? [], dismissedTransferId),
    [dismissedTransferId, transfersQuery.data],
  );
  const transferId = transfer?.id ?? null;
  const transferIsTerminal = transfer ? presentLocalLinkTransfer(transfer).terminal : true;
  const busy = actions.accept.isPending || actions.reject.isPending;
  const secondsRemaining = useLocalLinkCountdown(transfer?.expiresAt);

  useEffect(() => {
    actionStartedRef.current = false;
    setRevealStatus(null);
    if (!transferId || isSuppressed || transferIsTerminal) return;
    const focusFrame = window.requestAnimationFrame(() => copyButtonRef.current?.focus());
    return () => window.cancelAnimationFrame(focusFrame);
  }, [isSuppressed, transferId, transferIsTerminal]);

  if (!transfer) return null;

  const deviceName =
    transfer.peerDisplayName ??
    devicesQuery.data?.find((device) => device.deviceId === transfer.deviceId)?.displayName ??
    "another Mac";
  const presentation = presentLocalLinkTransfer(transfer);

  if (isSuppressed) {
    return (
      <button
        type="button"
        className="local-link-incoming-indicator"
        onClick={onReveal}
        aria-label={`Show incoming text request from ${deviceName}`}
      >
        <ClipboardCheck size={16} aria-hidden="true" />
        Incoming request from {deviceName}
      </button>
    );
  }

  const accept = (action: LocalLinkAcceptAction) => {
    if (busy || actionStartedRef.current) return;
    actionStartedRef.current = true;
    void actions.accept.mutateAsync({ transferId: transfer.id, action }).catch(() => {
      actionStartedRef.current = false;
    });
  };
  const reject = () => {
    if (busy || actionStartedRef.current) return;
    actionStartedRef.current = true;
    void actions.reject.mutateAsync(transfer.id).catch(() => {
      actionStartedRef.current = false;
    });
  };
  const reveal = () => {
    if (actions.revealTransfer.isPending || presentation.terminal) return;
    setRevealStatus(null);
    void actions.revealTransfer
      .mutateAsync(transfer.id)
      .then(() =>
        setRevealStatus(
          isDesktopRuntime()
            ? "Native secure view opened. The text was not returned to this WebView."
            : "Secure text viewing is available in the desktop Preview. No text entered this browser view.",
        ),
      )
      .catch(() => undefined);
  };
  const close = () => {
    setDismissedTransferId(transfer.id);
  };

  const handleCardKeys = (event: React.KeyboardEvent<HTMLElement>) => {
    if (presentation.terminal || busy || actionStartedRef.current) return;
    const key = event.key.toLocaleLowerCase();
    if ((event.metaKey || event.ctrlKey) && key === "c") {
      event.preventDefault();
      event.stopPropagation();
      accept("copy");
      return;
    }
    if ((event.metaKey || event.ctrlKey) && key === "s") {
      event.preventDefault();
      event.stopPropagation();
      accept("save");
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setDismissedTransferId(transfer.id);
    }
  };

  return (
    <section
      className="local-link-receive-card"
      aria-label={`Incoming Local Link transfer from ${deviceName}`}
      aria-labelledby="incoming-local-link-title"
      aria-describedby="incoming-local-link-description"
      role="dialog"
      data-terminal={presentation.terminal}
      data-presentation={layout}
      onKeyDown={handleCardKeys}
    >
      <div className="local-link-receive-heading">
        <span className="local-link-receive-icon">
          <ClipboardCheck size={18} aria-hidden="true" />
        </span>
        <div>
          <span className="eyebrow">Incoming Local Link</span>
          <strong id="incoming-local-link-title">
            {presentation.terminal
              ? `${deviceName} request ${presentation.label.toLowerCase()}`
              : `${deviceName} sent a text request`}
          </strong>
        </div>
        <LocalLinkStatusBadge
          label={
            !presentation.terminal && secondsRemaining !== null
              ? `${secondsRemaining}s`
              : presentation.label
          }
          tone={presentation.tone}
        />
      </div>

      <div className="local-link-receive-summary" id="incoming-local-link-description">
        <strong>
          TEXT{transfer.byteSize ? ` · ${formatByteSize(transfer.byteSize)}` : ""}
          {!presentation.terminal && secondsRemaining !== null
            ? ` · expires in ${secondsRemaining} seconds`
            : ""}
        </strong>
        <span>Content is not shown before you choose a destination.</span>
      </div>

      {presentation.terminal ? (
        <p className="local-link-receive-terminal">{presentation.detail}</p>
      ) : (
        <p className="local-link-receive-help">
          Copy writes to the system clipboard only. Save writes to local History only. Reject
          discards the pending text. Nothing is pasted automatically.
        </p>
      )}

      {!presentation.terminal ? (
        <>
          <button
            type="button"
            className="text-button local-link-reveal-button"
            onClick={reveal}
            disabled={actions.revealTransfer.isPending}
            aria-label={`Show incoming text from ${deviceName} in the native secure view`}
          >
            <Eye size={15} aria-hidden="true" />
            {actions.revealTransfer.isPending ? "Opening secure view…" : "Show content securely"}
          </button>
          {revealStatus ? (
            <p className="local-link-reveal-status" role="status">
              {revealStatus}
            </p>
          ) : null}
          <div className="local-link-receive-actions">
            <button
              ref={copyButtonRef}
              type="button"
              className="settings-save"
              onClick={() => accept("copy")}
              disabled={busy}
              aria-label={`Copy incoming text from ${deviceName} to the system clipboard`}
              aria-keyshortcuts="Meta+C Control+C"
            >
              <Copy size={15} aria-hidden="true" />
              {actions.accept.isPending && actions.accept.variables?.action === "copy"
                ? "Copying…"
                : "Copy"}
            </button>
            <button
              type="button"
              className="text-button"
              onClick={() => accept("save")}
              disabled={busy}
              aria-label={`Save incoming text from ${deviceName} to local History`}
              aria-keyshortcuts="Meta+S Control+S"
            >
              <Archive size={15} aria-hidden="true" />
              {actions.accept.isPending && actions.accept.variables?.action === "save"
                ? "Saving…"
                : "Save"}
            </button>
            <button
              type="button"
              className="text-button local-link-reject"
              onClick={reject}
              disabled={busy}
              aria-label={`Reject incoming text from ${deviceName}`}
              aria-keyshortcuts="Escape"
            >
              <X size={15} aria-hidden="true" /> Reject
            </button>
          </div>
        </>
      ) : null}

      <small className="local-link-receive-footer">
        {presentation.terminal
          ? "The transfer status contains metadata only; no text body is retained here."
          : "Esc collapses · Reject discards · sender can cancel · Mac lock makes the receiver unavailable"}
      </small>

      <button
        type="button"
        className="icon-button local-link-receive-close"
        onClick={close}
        disabled={busy}
        aria-label={
          presentation.terminal
            ? `Close ${presentation.label.toLowerCase()} request from ${deviceName}`
            : `Collapse incoming text from ${deviceName}; the request remains pending`
        }
      >
        <X size={16} />
      </button>

      {actions.accept.isError || actions.reject.isError || actions.revealTransfer.isError ? (
        <p className="local-link-error" role="alert">
          {String(
            actions.accept.error ?? actions.reject.error ?? actions.revealTransfer.error,
          ).replace(/^Error:\s*/u, "")}
        </p>
      ) : null}
    </section>
  );
}

function findPresentableIncomingTransfer(
  transfers: LocalLinkTransfer[],
  dismissedTransferId: string | null,
) {
  const incoming = transfers.filter(
    (candidate) => candidate.direction === "incoming" && candidate.id !== dismissedTransferId,
  );
  const pending = incoming.find((candidate) =>
    ["awaitingReceiver", "viewed"].includes(normalizedLocalLinkStatus(candidate)),
  );
  if (pending) return pending;
  return incoming.find((candidate) => {
    const status = normalizedLocalLinkStatus(candidate);
    if (!["copied", "saved", "rejected", "cancelled", "expired", "failed"].includes(status)) {
      return false;
    }
    return Date.now() - new Date(candidate.updatedAt).getTime() < 8_000;
  });
}

export function localLinkTransferLabel(transfer: LocalLinkTransfer) {
  return presentLocalLinkTransfer(transfer).label;
}
