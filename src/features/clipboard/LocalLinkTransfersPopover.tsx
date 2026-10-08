import { ArrowDownLeft, ArrowUpRight, Copy, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import { formatRelativeTime } from "./format";
import {
  filterLocalLinkTransfers,
  presentLocalLinkRecovery,
  presentLocalLinkTransfer,
} from "./LocalLinkPresentation";
import { LocalLinkStatusBadge } from "./LocalLinkUi";
import { useLocalLinkActions, useLocalLinkDevices, useLocalLinkTransfers } from "./queries";
import type { LocalLinkTransfer, LocalLinkTransferFilter } from "./types";
import "./LocalLink.css";

interface LocalLinkTransfersPopoverProps {
  isOpen: boolean;
  onClose: () => void;
}

const filters: Array<{ value: LocalLinkTransferFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "incoming", label: "Incoming" },
  { value: "outgoing", label: "Outgoing" },
  { value: "failed", label: "Failed" },
];

/** Non-modal, metadata-only status surface; it never becomes a content history. */
export function LocalLinkTransfersPopover({ isOpen, onClose }: LocalLinkTransfersPopoverProps) {
  const transfersQuery = useLocalLinkTransfers(isOpen);
  const devicesQuery = useLocalLinkDevices(isOpen);
  const actions = useLocalLinkActions();
  const [filter, setFilter] = useState<LocalLinkTransferFilter>("all");
  const [copiedDiagnostic, setCopiedDiagnostic] = useState<string | null>(null);

  useEffect(() => {
    if (!copiedDiagnostic) return;
    const timer = window.setTimeout(() => setCopiedDiagnostic(null), 1_500);
    return () => window.clearTimeout(timer);
  }, [copiedDiagnostic]);

  if (!isOpen) return null;
  const allTransfers = transfersQuery.data ?? [];
  const transfers = filterLocalLinkTransfers(allTransfers, filter).slice(0, 20);
  const terminalCount = allTransfers.filter(
    (transfer) => presentLocalLinkTransfer(transfer).terminal,
  ).length;
  const deviceName = (transfer: LocalLinkTransfer) =>
    transfer.peerDisplayName ??
    devicesQuery.data?.find((device) => device.deviceId === transfer.deviceId)?.displayName ??
    "Unknown device";
  const copyDiagnostic = (transferId: string) => {
    const diagnostic = localLinkDiagnosticId(transferId);
    void navigator.clipboard.writeText(diagnostic).then(() => {
      setCopiedDiagnostic(diagnostic);
    });
  };

  return (
    <aside
      className="local-link-transfers-popover"
      aria-labelledby="local-link-transfers-title"
      aria-describedby="local-link-transfers-description"
      onKeyDown={(event) => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }}
    >
      <header>
        <div>
          <span className="eyebrow">Local Link</span>
          <h2 id="local-link-transfers-title">Transfers</h2>
        </div>
        <button
          type="button"
          className="icon-button"
          onClick={onClose}
          aria-label="Close transfers"
        >
          <X size={17} />
        </button>
      </header>

      <fieldset className="local-link-transfer-filters">
        <legend>Filter transfers</legend>
        {filters.map((option) => (
          <button
            key={option.value}
            type="button"
            className="text-button"
            aria-pressed={filter === option.value}
            onClick={() => setFilter(option.value)}
          >
            {option.label}
          </button>
        ))}
      </fieldset>

      <p className="local-link-transfer-boundary" id="local-link-transfers-description">
        Metadata only · no text body, path, endpoint, key, or old-payload Retry is available here.
      </p>
      {allTransfers.some((transfer) => transfer.isUiFixture) ? (
        <small className="local-link-fixture-note">
          Browser UI fixtures are simulated display data, not network delivery evidence.
        </small>
      ) : null}

      {transfersQuery.isLoading ? <p className="local-link-loading">Loading transfers…</p> : null}
      {transfersQuery.isError ? (
        <p className="local-link-error" role="alert">
          Could not load recent transfer status.
        </p>
      ) : null}
      {!transfersQuery.isLoading && !transfersQuery.isError && transfers.length === 0 ? (
        <p className="local-link-device-empty">
          No {filter === "all" ? "" : `${filter} `}transfers.
        </p>
      ) : null}

      <ol aria-live="polite">
        {transfers.map((transfer) => (
          <TransferRow
            key={transfer.id}
            transfer={transfer}
            deviceName={deviceName(transfer)}
            diagnosticCopied={copiedDiagnostic === localLinkDiagnosticId(transfer.id)}
            onCopyDiagnostic={() => copyDiagnostic(transfer.id)}
          />
        ))}
      </ol>
      <footer className="local-link-transfers-footer">
        <span>Retained locally: up to 20 records / 24 hours · Esc closes this panel</span>
        <button
          type="button"
          className="text-button local-link-clear-transfers"
          onClick={() => actions.clearTransfers.mutate()}
          disabled={terminalCount === 0 || actions.clearTransfers.isPending}
        >
          <Trash2 size={15} aria-hidden="true" />
          {actions.clearTransfers.isPending
            ? "Clearing…"
            : `Clear terminal records (${terminalCount})`}
        </button>
        {actions.clearTransfers.data !== undefined ? (
          <span role="status">{actions.clearTransfers.data} terminal records cleared.</span>
        ) : null}
        {actions.clearTransfers.isError ? (
          <span className="local-link-error" role="alert">
            Could not clear terminal transfer records.
          </span>
        ) : null}
      </footer>
    </aside>
  );
}

function TransferRow({
  transfer,
  deviceName,
  diagnosticCopied,
  onCopyDiagnostic,
}: {
  transfer: LocalLinkTransfer;
  deviceName: string;
  diagnosticCopied: boolean;
  onCopyDiagnostic: () => void;
}) {
  const presentation = presentLocalLinkTransfer(transfer);
  const recovery = presentLocalLinkRecovery(transfer.recoveryAction);
  const DirectionIcon = transfer.direction === "incoming" ? ArrowDownLeft : ArrowUpRight;
  const diagnostic = localLinkDiagnosticId(transfer.id);
  return (
    <li data-tone={presentation.tone}>
      <DirectionIcon size={16} aria-hidden="true" />
      <div>
        <div className="local-link-transfer-title-row">
          <LocalLinkStatusBadge label={presentation.label} tone={presentation.tone} />
          <time dateTime={transfer.updatedAt}>{formatRelativeTime(transfer.updatedAt)}</time>
        </div>
        <strong>
          {transfer.direction === "incoming" ? "Incoming" : "Outgoing"} · {deviceName}
        </strong>
        <small>{presentation.detail}</small>
        {recovery ? <small>{recovery}</small> : null}
        <div className="local-link-transfer-diagnostic">
          <code>{diagnostic}</code>
          <button
            type="button"
            className="text-button"
            onClick={onCopyDiagnostic}
            aria-label={`Copy diagnostic ID ${diagnostic}`}
          >
            <Copy size={14} aria-hidden="true" />
            {diagnosticCopied ? "Copied" : "Copy diagnostic"}
          </button>
        </div>
      </div>
    </li>
  );
}

/**
 * Stable, short and content-free support correlation for one local transfer.
 * It is derived only from that transfer ID and is not a device or cross-install identity.
 */
export function localLinkDiagnosticId(transferId: string) {
  let hash = 2_166_136_261;
  for (const character of transferId) {
    hash ^= character.charCodeAt(0);
    hash = Math.imul(hash, 16_777_619);
  }
  return `LL-${(hash >>> 0).toString(16).padStart(8, "0").toUpperCase()}`;
}
