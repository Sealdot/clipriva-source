import { Archive, ClipboardCheck, Copy, Eye, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { isDesktopRuntime } from "../../lib/runtime";
import { formatByteSize } from "./format";
import { normalizedLocalLinkStatus } from "./LocalLinkPresentation";
import {
  LocalLinkStatusBadge,
  useLocalLinkCountdown,
  useLocalLinkDialogFocus,
} from "./LocalLinkUi";
import { useLocalLinkActions, useLocalLinkDevices, useLocalLinkTransfers } from "./queries";
import type { LocalLinkAcceptAction, LocalLinkTransfer } from "./types";
import "./LocalLink.css";

interface LocalLinkIncomingCenterProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * A stable, metadata-only inbox for pending Local Link text. Plaintext remains
 * native-memory owned and can only be revealed in the native secure view.
 */
export function LocalLinkIncomingCenter({ isOpen, onClose }: LocalLinkIncomingCenterProps) {
  const transfersQuery = useLocalLinkTransfers(isOpen);
  const devicesQuery = useLocalLinkDevices(isOpen);
  const actions = useLocalLinkActions();
  const centerRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const [actionInFlight, setActionInFlight] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

  const pendingTransfers = useMemo(
    () =>
      (transfersQuery.data ?? []).filter(
        (transfer) =>
          transfer.direction === "incoming" &&
          ["awaitingReceiver", "viewed"].includes(normalizedLocalLinkStatus(transfer)),
      ),
    [transfersQuery.data],
  );

  useEffect(() => {
    if (!isOpen) return;
    const unseenIds = pendingTransfers
      .filter((transfer) => normalizedLocalLinkStatus(transfer) === "awaitingReceiver")
      .map((transfer) => transfer.id);
    if (unseenIds.length === 0) return;
    void Promise.all(
      unseenIds.map((transferId) => actions.markViewed.mutateAsync(transferId)),
    ).catch(() => undefined);
  }, [actions.markViewed, isOpen, pendingTransfers]);

  useLocalLinkDialogFocus(isOpen, centerRef, onClose, closeButtonRef);

  if (!isOpen) return null;

  const deviceName = (transfer: LocalLinkTransfer) =>
    transfer.peerDisplayName ??
    devicesQuery.data?.find((device) => device.deviceId === transfer.deviceId)?.displayName ??
    "another Mac";
  const accept = (transfer: LocalLinkTransfer, action: LocalLinkAcceptAction) => {
    if (actionInFlight) return;
    setActionInFlight(transfer.id);
    setStatusMessage(null);
    void actions.accept
      .mutateAsync({ transferId: transfer.id, action })
      .then(() =>
        setStatusMessage(
          action === "copy"
            ? `Copied text from ${deviceName(transfer)} to the system clipboard.`
            : `Saved text from ${deviceName(transfer)} to local History.`,
        ),
      )
      .catch(() => undefined)
      .finally(() => setActionInFlight(null));
  };
  const reject = (transfer: LocalLinkTransfer) => {
    if (actionInFlight) return;
    setActionInFlight(transfer.id);
    setStatusMessage(null);
    void actions.reject
      .mutateAsync(transfer.id)
      .then(() => setStatusMessage(`Rejected text from ${deviceName(transfer)}.`))
      .catch(() => undefined)
      .finally(() => setActionInFlight(null));
  };
  const reveal = (transfer: LocalLinkTransfer) => {
    if (actionInFlight) return;
    setActionInFlight(transfer.id);
    setStatusMessage(null);
    void actions.revealTransfer
      .mutateAsync(transfer.id)
      .then(() =>
        setStatusMessage(
          isDesktopRuntime()
            ? "Native secure view opened. The text was not returned to this WebView."
            : "Secure text viewing is available in the desktop Preview. No text entered this browser view.",
        ),
      )
      .catch(() => undefined)
      .finally(() => setActionInFlight(null));
  };

  return (
    <div className="local-link-incoming-backdrop">
      <aside
        ref={centerRef}
        className="local-link-incoming-center"
        role="dialog"
        aria-modal="true"
        aria-labelledby="local-link-incoming-center-title"
        aria-describedby="local-link-incoming-center-description"
        tabIndex={-1}
      >
        <header>
          <div>
            <span className="eyebrow">Local Link</span>
            <h2 id="local-link-incoming-center-title">Incoming Center</h2>
          </div>
          <button
            ref={closeButtonRef}
            type="button"
            className="icon-button"
            onClick={onClose}
            aria-label="Close Incoming Center"
          >
            <X size={17} />
          </button>
        </header>

        <p className="local-link-incoming-boundary" id="local-link-incoming-center-description">
          You choose where each text request goes. Content stays in memory and is never shown in
          this window.
        </p>

        {transfersQuery.isLoading ? (
          <p className="local-link-loading">Loading incoming text…</p>
        ) : null}
        {transfersQuery.isError ? (
          <p className="local-link-error" role="alert">
            Could not load pending text requests.
          </p>
        ) : null}
        {!transfersQuery.isLoading && !transfersQuery.isError && pendingTransfers.length === 0 ? (
          <div className="local-link-incoming-empty">
            <ClipboardCheck size={22} aria-hidden="true" />
            <strong>No pending text</strong>
            <p>New requests will appear here until you Copy, Save, Reject, or they expire.</p>
          </div>
        ) : null}

        {pendingTransfers.length > 0 ? (
          <ol className="local-link-incoming-list" aria-label="Pending incoming text requests">
            {pendingTransfers.map((transfer) => (
              <IncomingTransferRow
                key={transfer.id}
                transfer={transfer}
                deviceName={deviceName(transfer)}
                busy={actionInFlight === transfer.id}
                onCopy={() => accept(transfer, "copy")}
                onSave={() => accept(transfer, "save")}
                onReject={() => reject(transfer)}
                onReveal={() => reveal(transfer)}
              />
            ))}
          </ol>
        ) : null}

        {statusMessage ? (
          <p className="local-link-incoming-status" role="status">
            {statusMessage}
          </p>
        ) : null}
        {actions.accept.isError || actions.reject.isError || actions.revealTransfer.isError ? (
          <p className="local-link-error" role="alert">
            {String(
              actions.accept.error ?? actions.reject.error ?? actions.revealTransfer.error,
            ).replace(/^Error:\s*/u, "")}
          </p>
        ) : null}
      </aside>
    </div>
  );
}

function IncomingTransferRow({
  transfer,
  deviceName,
  busy,
  onCopy,
  onSave,
  onReject,
  onReveal,
}: {
  transfer: LocalLinkTransfer;
  deviceName: string;
  busy: boolean;
  onCopy: () => void;
  onSave: () => void;
  onReject: () => void;
  onReveal: () => void;
}) {
  const secondsRemaining = useLocalLinkCountdown(transfer.expiresAt);
  const countdownLabel =
    secondsRemaining === null ? "Waiting for your decision" : `${secondsRemaining}s remaining`;

  return (
    <li>
      <div className="local-link-incoming-row-heading">
        <div>
          <span className="eyebrow">Incoming text</span>
          <strong>{deviceName}</strong>
        </div>
        <LocalLinkStatusBadge label={countdownLabel} tone="warning" />
      </div>
      <p>
        TEXT{transfer.byteSize ? ` · ${formatByteSize(transfer.byteSize)}` : ""} · expires in{" "}
        {secondsRemaining ?? "less than a minute"}
        {secondsRemaining === null ? "" : " seconds"}
      </p>
      <small>Copy writes only to the system clipboard. Save writes only to local History.</small>
      <button
        type="button"
        className="text-button local-link-reveal-button"
        onClick={onReveal}
        disabled={busy}
        aria-label={`Show incoming text from ${deviceName} in the native secure view`}
      >
        <Eye size={15} aria-hidden="true" /> Show content securely
      </button>
      <div className="local-link-incoming-actions">
        <button
          type="button"
          className="settings-save"
          onClick={onCopy}
          disabled={busy}
          aria-label={`Copy incoming text from ${deviceName} to the system clipboard`}
        >
          <Copy size={15} aria-hidden="true" /> {busy ? "Working…" : "Copy"}
        </button>
        <button
          type="button"
          className="text-button"
          onClick={onSave}
          disabled={busy}
          aria-label={`Save incoming text from ${deviceName} to local History`}
        >
          <Archive size={15} aria-hidden="true" /> Save
        </button>
        <button
          type="button"
          className="text-button local-link-reject"
          onClick={onReject}
          disabled={busy}
          aria-label={`Reject incoming text from ${deviceName}`}
        >
          <X size={15} aria-hidden="true" /> Reject
        </button>
      </div>
    </li>
  );
}
