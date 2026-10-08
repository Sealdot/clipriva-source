import { Check, Laptop, MonitorUp, Send, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { formatByteSize, formatRelativeTime } from "./format";
import {
  formatLocalLinkProtocol,
  localLinkDeviceAvailability,
  normalizedLocalLinkStatus,
  presentLocalLinkDevice,
  presentLocalLinkTransfer,
} from "./LocalLinkPresentation";
import { deriveLocalLinkReadiness, type LocalLinkReadiness } from "./LocalLinkReadiness";
import {
  LocalLinkStatusBadge,
  useLocalLinkCountdown,
  useLocalLinkDialogFocus,
} from "./LocalLinkUi";
import {
  useLocalLinkActions,
  useLocalLinkDevices,
  useLocalLinkPreferences,
  useLocalLinkTransfers,
} from "./queries";
import type { ClipboardItem, LocalLinkDevice, LocalLinkTransfer } from "./types";
import "./LocalLink.css";

const MAX_TEXT_BYTES = 256 * 1024;
interface LocalLinkSendDialogProps {
  item: ClipboardItem | null;
  isOpen: boolean;
  onClose: () => void;
  onBackToItem?: () => void;
  presentation?: "modal" | "sidecar";
}

/** A deliberate, text-only send affordance for the selected clip. */
export function LocalLinkSendDialog({
  item,
  isOpen,
  onClose,
  onBackToItem,
  presentation: layout = "modal",
}: LocalLinkSendDialogProps) {
  const preferencesQuery = useLocalLinkPreferences(isOpen);
  const devicesQuery = useLocalLinkDevices(isOpen);
  const transfersQuery = useLocalLinkTransfers(isOpen);
  const actions = useLocalLinkActions();
  const dialogRef = useRef<HTMLElement>(null);
  const progressRef = useRef<HTMLElement>(null);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [result, setResult] = useState<LocalLinkTransfer | null>(null);
  const [fixtureProgressId, setFixtureProgressId] = useState<string | null>(null);
  const [progressOpen, setProgressOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const secondsRemaining = useLocalLinkCountdown(result?.expiresAt);

  const readiness = useMemo(
    () => deriveLocalLinkReadiness(preferencesQuery.data, devicesQuery.data ?? []),
    [devicesQuery.data, preferencesQuery.data],
  );
  const visibleDevices = useMemo(() => {
    return [...(devicesQuery.data ?? [])].sort((left, right) =>
      compareSendDevices(left, right, readiness),
    );
  }, [devicesQuery.data, readiness]);
  const trustedDevices = useMemo(
    () => visibleDevices.filter((device) => device.trustStatus === "trusted"),
    [visibleDevices],
  );
  const selectableDevices = useMemo(
    () =>
      readiness.state === "ready"
        ? trustedDevices.filter((device) => localLinkDeviceAvailability(device) === "online")
        : [],
    [readiness.state, trustedDevices],
  );

  useEffect(() => {
    if (!isOpen) return;
    setResult(null);
    setFixtureProgressId(null);
    setProgressOpen(false);
    setError(null);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    setSelectedDeviceId((current) => {
      if (current && selectableDevices.some((device) => device.deviceId === current))
        return current;
      const lastSuccessfulDeviceId = [...(transfersQuery.data ?? [])]
        .filter(
          (transfer) =>
            transfer.direction === "outgoing" &&
            (transfer.status === "copied" || transfer.status === "saved"),
        )
        .sort((left, right) => timestamp(right.updatedAt) - timestamp(left.updatedAt))[0]?.deviceId;
      return (
        selectableDevices.find((device) => device.deviceId === lastSuccessfulDeviceId)?.deviceId ??
        selectableDevices[0]?.deviceId ??
        null
      );
    });
  }, [isOpen, selectableDevices, transfersQuery.data]);

  useEffect(() => {
    if (!result) return;
    const refreshed = transfersQuery.data?.find((transfer) => transfer.id === result.id);
    if (
      !refreshed ||
      transferProgressRank(refreshed) < transferProgressRank(result) ||
      (refreshed.updatedAt === result.updatedAt &&
        refreshed.status === result.status &&
        refreshed.receiverAction === result.receiverAction &&
        refreshed.failureReason === result.failureReason)
    ) {
      return;
    }
    setResult(refreshed);
    if (presentLocalLinkTransfer(refreshed).terminal) setFixtureProgressId(null);
  }, [result, transfersQuery.data]);

  useEffect(() => {
    if (!fixtureProgressId) return;
    // Browser-only progression lets the real UI states be reviewed without
    // presenting fixture activity as native network evidence.
    const encryptedTimer = window.setTimeout(
      () =>
        setResult((current) =>
          current?.id === fixtureProgressId ? { ...current, status: "encrypted" } : current,
        ),
      350,
    );
    const awaitingTimer = window.setTimeout(
      () =>
        setResult((current) =>
          current?.id === fixtureProgressId ? { ...current, status: "awaitingReceiver" } : current,
        ),
      800,
    );
    const finalTimer = window.setTimeout(
      () =>
        setResult((current) =>
          current?.id === fixtureProgressId
            ? {
                ...current,
                status: "failed",
                failureReason: "transportUnavailable",
                completedAt: new Date().toISOString(),
              }
            : current,
        ),
      1_600,
    );
    return () => {
      window.clearTimeout(encryptedTimer);
      window.clearTimeout(awaitingTimer);
      window.clearTimeout(finalTimer);
    };
  }, [fixtureProgressId]);

  const closeOrCancel = useCallback(() => {
    if (result && !presentLocalLinkTransfer(result).terminal) {
      setFixtureProgressId(null);
      void actions.cancelTransfer
        .mutateAsync(result.id)
        .then((cancelled) => setResult(cancelled))
        .catch((reason: unknown) => setError(errorMessage(reason)));
      return;
    }
    onClose();
  }, [actions.cancelTransfer, onClose, result]);

  useLocalLinkDialogFocus(isOpen, dialogRef, closeOrCancel);

  if (!isOpen || !item) return null;

  const preferences = preferencesQuery.data;
  const isPlainText = isTextualLocalLinkItem(item);
  const textByteSize = new TextEncoder().encode(item.content).byteLength;
  const contentPreflightMessage = getContentPreflightMessage({
    isPlainText,
    textByteSize,
  });
  const blockerMessage =
    contentPreflightMessage ?? (readiness.state === "ready" ? null : readiness.detail);
  const canRequestTransfer =
    blockerMessage === null &&
    selectedDeviceId !== null &&
    selectableDevices.some((device) => device.deviceId === selectedDeviceId);
  const selectedDevice = visibleDevices.find((device) => device.deviceId === selectedDeviceId);
  const presentation = result ? presentLocalLinkTransfer(result) : null;

  const selectDevice = (deviceId: string) => {
    setSelectedDeviceId(deviceId);
    setError(null);
  };

  const send = async () => {
    if (!canRequestTransfer || !selectedDeviceId || result || actions.send.isPending) return;
    setError(null);
    try {
      const transfer = await actions.send.mutateAsync({
        clipboardItemId: item.id,
        deviceId: selectedDeviceId,
      });
      setResult(transfer);
      setProgressOpen(false);
      setFixtureProgressId(
        transfer.isUiFixture && transfer.status === "connecting" ? transfer.id : null,
      );
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const performReadinessAction = () => {
    if (!preferences || !readiness.primaryAction) return;
    setError(null);
    if (readiness.primaryAction === "enableLocalLink") {
      void actions.updatePreferences
        .mutateAsync({ ...preferences, enabled: true })
        .catch((reason: unknown) => setError(errorMessage(reason)));
      return;
    }
    if (readiness.primaryAction === "enableDiscovery") {
      void actions.updatePreferences
        .mutateAsync({ ...preferences, discoveryEnabled: true })
        .catch((reason: unknown) => setError(errorMessage(reason)));
      return;
    }
    if (readiness.primaryAction === "refreshDevices") {
      void Promise.all([devicesQuery.refetch(), actions.refreshPreferences()]).catch(
        (reason: unknown) => setError(errorMessage(reason)),
      );
      return;
    }
    // Pairing and identity verification need the existing two-Mac safety-code
    // ceremony. Keep this sheet scoped to the current item and take the person
    // to that one dedicated repair surface instead of starting a hidden pairing.
    document.querySelector("[aria-label='Local Link devices']")?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  };

  const openProgress = () => {
    setProgressOpen(true);
    window.requestAnimationFrame(() => progressRef.current?.focus());
  };

  const handleDeviceKeys = (event: React.KeyboardEvent) => {
    if (event.target !== event.currentTarget) return;
    if (
      result &&
      (event.metaKey || event.ctrlKey) &&
      event.shiftKey &&
      event.key.toLocaleLowerCase() === "l"
    ) {
      event.preventDefault();
      event.stopPropagation();
      openProgress();
      return;
    }
    if (result || selectableDevices.length === 0) return;
    if (event.key === "Enter") {
      if (!selectedDeviceId) return;
      event.preventDefault();
      void send();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const currentIndex = selectableDevices.findIndex(
      (device) => device.deviceId === selectedDeviceId,
    );
    const offset = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex =
      currentIndex === -1
        ? offset === 1
          ? 0
          : selectableDevices.length - 1
        : (currentIndex + offset + selectableDevices.length) % selectableDevices.length;
    setSelectedDeviceId(selectableDevices[nextIndex]?.deviceId ?? null);
  };

  return (
    <div className="local-link-backdrop" data-presentation={layout}>
      <section
        ref={dialogRef}
        className="local-link-send-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="send-to-device-title"
        aria-describedby="send-to-device-description"
        tabIndex={-1}
        onKeyDown={handleDeviceKeys}
      >
        <header>
          <div>
            <span className="eyebrow">Local Link · Handoff</span>
            <h2 id="send-to-device-title">Handoff Sheet</h2>
          </div>
          <button
            type="button"
            className="icon-button"
            onClick={closeOrCancel}
            aria-label={
              result && !presentation?.terminal ? "Cancel text request" : "Close send text dialog"
            }
          >
            <X size={18} />
          </button>
        </header>

        <div className="local-link-send-summary" id="send-to-device-description">
          <MonitorUp size={19} aria-hidden="true" />
          <div>
            <span>Selected item</span>
            <strong>
              {isPlainText ? "Text" : "Unsupported selection"} · {formatByteSize(textByteSize)}
            </strong>
            {isPlainText ? <small>{handoffSummary(item.content)}</small> : null}
          </div>
          <LocalLinkStatusBadge
            label={contentPreflightMessage ? "Blocked" : readiness.label}
            tone={contentPreflightMessage || readiness.state !== "ready" ? "warning" : "success"}
          />
        </div>

        {onBackToItem && !result ? (
          <button
            type="button"
            className="text-button local-link-back-to-item"
            onClick={onBackToItem}
          >
            Return to selected item
          </button>
        ) : null}

        {contentPreflightMessage ? (
          <p className="local-link-disabled-message" role="status">
            {contentPreflightMessage}
          </p>
        ) : !result ? (
          <section className="local-link-send-devices" aria-label="Trusted text recipients">
            {readiness.state !== "ready" ? (
              <section className="local-link-readiness-blocker" aria-label="Handoff readiness">
                <div>
                  <LocalLinkStatusBadge label={readiness.label} tone="warning" />
                  <p>{readiness.detail}</p>
                </div>
                {readiness.primaryActionLabel ? (
                  <button
                    type="button"
                    className="text-button local-link-readiness-repair"
                    onClick={performReadinessAction}
                    disabled={actions.updatePreferences.isPending}
                  >
                    {readiness.primaryActionLabel}
                  </button>
                ) : null}
              </section>
            ) : null}
            {visibleDevices.map((device) => (
              <SendDeviceRow
                key={device.deviceId}
                device={device}
                selected={device.deviceId === selectedDeviceId}
                onSelect={() => selectDevice(device.deviceId)}
                state={sendDeviceState(device, readiness)}
              />
            ))}
            {selectedDevice ? (
              <div className="local-link-send-one-step">
                <span>
                  <strong>{selectedDevice.displayName}</strong>
                  <small>
                    Text · {formatByteSize(textByteSize)} · recipient chooses Copy, Save, or Reject
                  </small>
                </span>
                <button
                  type="button"
                  className="settings-save local-link-send-button"
                  onClick={() => void send()}
                  disabled={!canRequestTransfer || actions.send.isPending}
                  aria-label={`Create text handoff request to ${selectedDevice.displayName}`}
                >
                  <Send size={15} aria-hidden="true" />
                  {actions.send.isPending ? "Creating…" : "Create request"}
                </button>
              </div>
            ) : null}
            <small className="local-link-send-boundary">
              The request is explicit. Activity and diagnostics keep metadata only, never this text
              body.
            </small>
            <small className="local-link-keyboard-help">
              ↑ / ↓ selects a device · Enter creates one request · Esc closes
            </small>
          </section>
        ) : (
          <section className="local-link-send-result" aria-live="polite">
            <div className="local-link-send-result-heading">
              <div>
                <span>Handoff request</span>
                <strong>
                  {result.peerDisplayName ?? selectedDevice?.displayName ?? "trusted device"}
                </strong>
              </div>
              {presentation ? (
                <LocalLinkStatusBadge label={presentation.label} tone={presentation.tone} />
              ) : null}
            </div>
            <p>{presentation?.detail}</p>
            {secondsRemaining !== null ? (
              <strong className="local-link-request-countdown">
                Request expires in {secondsRemaining}s
              </strong>
            ) : null}
            {result.isUiFixture ? (
              <small className="local-link-fixture-note">
                Browser UI fixture · simulated states are not LAN delivery or encryption evidence;
                the Preview transport always ends unavailable.
              </small>
            ) : null}
            <div className="local-link-send-actions">
              {!progressOpen ? (
                <button type="button" className="settings-save" onClick={openProgress}>
                  View progress <kbd>⌘⇧L</kbd>
                </button>
              ) : null}
              {!presentation?.terminal ? (
                <button
                  type="button"
                  className="text-button"
                  onClick={closeOrCancel}
                  disabled={actions.cancelTransfer.isPending}
                >
                  {actions.cancelTransfer.isPending
                    ? "Cancelling…"
                    : "Cancel request and clear pending text"}
                </button>
              ) : (
                <button type="button" className="settings-save" onClick={onClose}>
                  Done
                </button>
              )}
            </div>
            {progressOpen ? (
              <section
                ref={progressRef}
                className="local-link-send-progress"
                aria-label="Transfer progress"
                tabIndex={-1}
              >
                <ol className="local-link-progress-steps" aria-label="Transfer progress steps">
                  {(["connecting", "encrypted", "awaitingReceiver", "final"] as const).map(
                    (phase) => {
                      const state = progressStepState(result, phase);
                      return (
                        <li key={phase} data-state={state}>
                          <span>
                            {state === "complete" ? <Check size={12} /> : null}
                            {state === "failed" ? <X size={12} /> : null}
                          </span>
                          {progressStepLabel(phase)}
                        </li>
                      );
                    },
                  )}
                </ol>
              </section>
            ) : null}
          </section>
        )}

        {error ? (
          <p className="local-link-error" role="alert">
            {error}
          </p>
        ) : null}
      </section>
    </div>
  );
}

function SendDeviceRow({
  device,
  selected,
  onSelect,
  state,
}: {
  device: LocalLinkDevice;
  selected: boolean;
  onSelect: () => void;
  state: SendDeviceState;
}) {
  const availability = localLinkDeviceAvailability(device);
  const canSelect = state === "ready";
  const presentation = presentLocalLinkDevice(device);
  return (
    <div className="local-link-send-device" data-selected={selected} data-state={state}>
      <button
        type="button"
        className="local-link-device-choice"
        onClick={onSelect}
        disabled={!canSelect}
        aria-pressed={selected}
        aria-label={`Select ${device.displayName}`}
      >
        <Laptop size={17} aria-hidden="true" />
        <span>
          <strong>{device.displayName}</strong>
          <small>
            {presentation.label} · {formatLocalLinkProtocol(device)}
            {availability !== "online" && device.lastSeenAt
              ? ` · last seen ${formatRelativeTime(device.lastSeenAt)}`
              : ""}
          </small>
        </span>
      </button>
      <span className="local-link-unavailable">
        {canSelect ? "Ready" : state === "offline" ? "Offline" : presentation.label}
      </span>
    </div>
  );
}

function getContentPreflightMessage({
  isPlainText,
  textByteSize,
}: {
  isPlainText: boolean;
  textByteSize: number;
}) {
  if (!isPlainText) return "Local Link accepts UTF-8 text only. No text will leave this Mac.";
  if (textByteSize < 1)
    return "Empty text cannot be sent. Select a text Item with at least 1 byte.";
  if (textByteSize > MAX_TEXT_BYTES) {
    return `This text is ${formatByteSize(textByteSize)}. The limit is 256 KiB, so no text will leave this Mac.`;
  }
  return null;
}

function progressStepState(
  transfer: LocalLinkTransfer,
  phase: "connecting" | "encrypted" | "awaitingReceiver" | "final",
) {
  const current = normalizedLocalLinkStatus(transfer);
  const order = ["connecting", "encrypted", "awaitingReceiver", "final"] as const;
  const terminal = presentLocalLinkTransfer(transfer).terminal;
  if (current === "failed") {
    const failedPhase = failedProgressPhase(transfer.failureReason);
    const phaseIndex = order.indexOf(phase);
    const failedIndex = order.indexOf(failedPhase);
    if (phaseIndex < failedIndex) return "complete";
    return phaseIndex === failedIndex ? "failed" : "upcoming";
  }
  if (phase === "final") return terminal ? "current" : "upcoming";
  const phaseIndex = order.indexOf(phase);
  const currentIndex =
    current === "connecting"
      ? 0
      : current === "encrypted"
        ? 1
        : current === "awaitingReceiver"
          ? 2
          : -1;
  if (terminal || currentIndex > phaseIndex) return "complete";
  return current === phase ? "current" : "upcoming";
}

function failedProgressPhase(
  failure: LocalLinkTransfer["failureReason"],
): "connecting" | "encrypted" | "awaitingReceiver" | "final" {
  switch (failure) {
    case "captureBlocked":
    case "itemUnavailable":
    case "unsupportedItem":
    case "deviceNotTrusted":
      return "connecting";
    case "receiverLocked":
    case "receiverBusy":
    case "rateLimited":
    case "duplicate":
    case "protocolViolation":
    case "payloadUnavailable":
    case "outcomeUnknown":
      return "awaitingReceiver";
    case "transportUnavailable":
    case "deviceUnavailable":
    case "authFailed":
    case "versionMismatch":
    case "deviceRevoked":
    case null:
    case undefined:
      return "encrypted";
  }
}

function progressStepLabel(phase: "connecting" | "encrypted" | "awaitingReceiver" | "final") {
  if (phase === "connecting") return "Policy check";
  if (phase === "encrypted") return "Secure delivery";
  if (phase === "awaitingReceiver") return "Recipient decision";
  return "Complete";
}

type SendDeviceState = "ready" | "actionRequired" | "offline";

function sendDeviceState(
  device: LocalLinkDevice,
  readiness: Pick<LocalLinkReadiness, "state">,
): SendDeviceState {
  const online = localLinkDeviceAvailability(device) === "online";
  if (device.trustStatus === "trusted" && online && readiness.state === "ready") return "ready";
  if (device.trustStatus === "trusted" && !online) return "offline";
  return "actionRequired";
}

function compareSendDevices(
  left: LocalLinkDevice,
  right: LocalLinkDevice,
  readiness: Pick<LocalLinkReadiness, "state">,
) {
  const stateOrder: Record<SendDeviceState, number> = {
    ready: 0,
    actionRequired: 1,
    offline: 2,
  };
  const stateDifference =
    stateOrder[sendDeviceState(left, readiness)] - stateOrder[sendDeviceState(right, readiness)];
  if (stateDifference !== 0) return stateDifference;
  const recentDifference =
    timestamp(right.lastTransferAt) - timestamp(left.lastTransferAt) ||
    timestamp(right.lastSeenAt) - timestamp(left.lastSeenAt);
  return recentDifference || left.displayName.localeCompare(right.displayName);
}

function timestamp(value?: string | null) {
  if (!value) return 0;
  const parsed = new Date(value).getTime();
  return Number.isFinite(parsed) ? parsed : 0;
}

function transferProgressRank(transfer: LocalLinkTransfer) {
  if (presentLocalLinkTransfer(transfer).terminal) return 3;
  const status = normalizedLocalLinkStatus(transfer);
  if (status === "awaitingReceiver" || status === "viewed" || status === "reconciling") return 2;
  if (status === "encrypted") return 1;
  return 0;
}

function handoffSummary(content: string) {
  const compact = content.replace(/\s+/gu, " ").trim();
  if (compact.length <= 180) return compact;
  return `${compact.slice(0, 179)}…`;
}

function isTextualLocalLinkItem(item: ClipboardItem) {
  return (
    item.kind === "text" ||
    item.kind === "code" ||
    item.kind === "command" ||
    item.kind === "url" ||
    item.kind === "color"
  );
}

function errorMessage(reason: unknown) {
  return String(reason).replace(/^Error:\s*/u, "");
}
