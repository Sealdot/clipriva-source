import {
  Check,
  Copy,
  KeyRound,
  Laptop,
  MonitorUp,
  RotateCcw,
  ShieldCheck,
  TimerReset,
  Wifi,
} from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { formatRelativeTime } from "./format";
import {
  formatLocalLinkFingerprint,
  formatLocalLinkProtocol,
  presentLocalLinkDevice,
} from "./LocalLinkPresentation";
import {
  LocalLinkStatusBadge,
  useLocalLinkCountdown,
  useLocalLinkDialogFocus,
} from "./LocalLinkUi";
import {
  useLocalLinkActions,
  useLocalLinkDevices,
  useLocalLinkPairing,
  useLocalLinkPreferences,
} from "./queries";
import type { LocalLinkDevice, LocalLinkPairing, LocalLinkTrustDuration } from "./types";
import "./LocalLink.css";

/** The trust-management portion of the first-class Devices center. */
export function LocalLinkDevicesSettings() {
  const preferencesQuery = useLocalLinkPreferences();
  const preferences = preferencesQuery.data;
  // Trust can always be revoked, including while Local Link is off.
  const devicesQuery = useLocalLinkDevices(true);
  const pairingQuery = useLocalLinkPairing(true);
  const actions = useLocalLinkActions();
  const [deviceName, setDeviceName] = useState("");
  const [pairing, setPairing] = useState<LocalLinkPairing | null>(null);
  const [trustDuration, setTrustDuration] = useState<LocalLinkTrustDuration>("thirtyDays");
  const [localConfirmationSubmitted, setLocalConfirmationSubmitted] = useState(false);
  const [revoking, setRevoking] = useState<LocalLinkDevice | null>(null);
  const [confirmingIdentityReset, setConfirmingIdentityReset] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pairingDialogRef = useRef<HTMLElement>(null);
  const pairingConfirmRef = useRef<HTMLButtonElement>(null);
  const pairingTriggerRef = useRef<HTMLButtonElement>(null);
  const identityResetDialogRef = useRef<HTMLElement>(null);
  const identityResetConfirmRef = useRef<HTMLButtonElement>(null);
  const identityResetTriggerRef = useRef<HTMLButtonElement>(null);
  const comparisonSeconds = useLocalLinkCountdown(pairing?.expiresAt);
  const pairingDeviceId = pairing?.device.deviceId;

  useEffect(() => {
    if (preferences) setDeviceName(preferences.deviceName);
  }, [preferences]);

  useEffect(() => {
    if (!pairingQuery.data) return;
    setPairing(pairingQuery.data);
  }, [pairingQuery.data]);

  useEffect(() => {
    // Never carry an "always" choice from a prior peer into a new pairing.
    if (pairingDeviceId) setTrustDuration("thirtyDays");
  }, [pairingDeviceId]);

  useLocalLinkDialogFocus(
    Boolean(pairing),
    pairingDialogRef,
    () => {
      const deviceId = pairing?.device.deviceId;
      setPairing(null);
      setLocalConfirmationSubmitted(false);
      if (deviceId) void actions.cancelPairing.mutateAsync(deviceId).catch(() => undefined);
    },
    pairingConfirmRef,
    pairingTriggerRef,
  );

  useLocalLinkDialogFocus(
    confirmingIdentityReset,
    identityResetDialogRef,
    () => setConfirmingIdentityReset(false),
    identityResetConfirmRef,
    identityResetTriggerRef,
  );

  useEffect(() => {
    if (!pairing || comparisonSeconds !== 0) return;
    const pairingDeviceId = pairing.device.deviceId;
    setPairing(null);
    setLocalConfirmationSubmitted(false);
    setStatus("The pairing request expired. No device trust was created.");
    void actions.cancelPairing.mutateAsync(pairingDeviceId).catch((reason: unknown) => {
      setError(errorMessage(reason));
    });
  }, [actions.cancelPairing, comparisonSeconds, pairing]);

  useEffect(() => {
    if (!pairing) return;
    const trustedDevice = (devicesQuery.data ?? []).find(
      (device) => device.deviceId === pairing.device.deviceId && device.trustStatus === "trusted",
    );
    if (!trustedDevice) return;
    setPairing(null);
    setLocalConfirmationSubmitted(false);
    setStatus(`${trustedDevice.displayName} is now trusted.`);
  }, [devicesQuery.data, pairing]);

  if (preferencesQuery.isError) {
    return <p className="local-link-error">Could not load Local Link settings.</p>;
  }

  if (preferencesQuery.isLoading || !preferences) {
    return <p className="local-link-loading">Loading Local Link settings…</p>;
  }

  const trusted = (devicesQuery.data ?? []).filter((device) => device.trustStatus === "trusted");
  const needsRePairing = (devicesQuery.data ?? []).filter(
    (device) => device.trustStatus === "needsRePairing",
  );
  const blocked = (devicesQuery.data ?? []).filter(
    (device) => device.trustStatus === "revoked" && Boolean(device.revokedAt),
  );
  const nearby = (devicesQuery.data ?? []).filter(
    (device) => device.trustStatus === "revoked" && !device.revokedAt,
  );
  const saving = actions.updatePreferences.isPending;

  const updateEnabled = async (enabled: boolean) => {
    setError(null);
    setStatus(null);
    if (!enabled) {
      const pairingDeviceId = pairing?.device.deviceId;
      setPairing(null);
      setLocalConfirmationSubmitted(false);
      if (pairingDeviceId) {
        await actions.cancelPairing.mutateAsync(pairingDeviceId).catch(() => undefined);
      }
    }
    try {
      await actions.updatePreferences.mutateAsync({
        ...preferences,
        enabled,
        discoveryEnabled: enabled,
      });
      setStatus(
        enabled
          ? "Local Link is on. ClipRiva now listens only on your local network and advertises with Bonjour."
          : "Local Link is off. Clipboard History and Quick Paste continue to work on this Mac.",
      );
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const saveDeviceName = async () => {
    setError(null);
    try {
      await actions.updatePreferences.mutateAsync({ ...preferences, deviceName });
      setStatus("This Mac’s Local Link name was saved.");
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const startPairing = async (device: LocalLinkDevice) => {
    setError(null);
    setStatus(null);
    try {
      if (!preferences.enabled || !preferences.discoveryEnabled) {
        await actions.updatePreferences.mutateAsync({
          ...preferences,
          enabled: true,
          discoveryEnabled: true,
        });
      }
      const started = await actions.beginPairing.mutateAsync({
        deviceId: device.deviceId,
        displayName: device.displayName,
      });
      setLocalConfirmationSubmitted(false);
      setPairing(started);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const rePair = async (device: LocalLinkDevice) => {
    const route = resolveLocalLinkRePairRoute(device, nearby);
    if (route.kind === "unavailable" || route.kind === "chooseNearby") {
      setError(null);
      setStatus(route.message);
      if (route.kind === "chooseNearby") {
        document.querySelector("[aria-label='Nearby devices']")?.scrollIntoView({
          behavior: "smooth",
          block: "start",
        });
      }
      return;
    }
    await startPairing(route.candidate);
  };

  const confirmPairing = async () => {
    if (!pairing) return;
    setError(null);
    try {
      await actions.confirmPairing.mutateAsync({
        deviceId: pairing.device.deviceId,
        trustDuration,
      });
      setLocalConfirmationSubmitted(true);
      await Promise.all([devicesQuery.refetch(), pairingQuery.refetch()]);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const cancelPairing = async () => {
    if (!pairing) return;
    setError(null);
    const deviceId = pairing.device.deviceId;
    setPairing(null);
    setLocalConfirmationSubmitted(false);
    try {
      await actions.cancelPairing.mutateAsync(deviceId);
      setStatus("Pairing cancelled. No device trust was created.");
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const revoke = async () => {
    if (!revoking) return;
    setError(null);
    try {
      await actions.revokeDevice.mutateAsync(revoking.deviceId);
      setStatus(`Trust was revoked for ${revoking.displayName}. Active requests must be cleared.`);
      setRevoking(null);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  const resetIdentity = async () => {
    setError(null);
    setStatus(null);
    try {
      await actions.resetIdentity.mutateAsync();
      setConfirmingIdentityReset(false);
      setStatus("This Mac now has a new identity. Previously trusted devices need re-pairing.");
    } catch (reason) {
      setError(errorMessage(reason));
    }
  };

  return (
    <section className="local-link-settings" aria-label="Local Link devices">
      <div className="capture-settings-panel-heading">
        <div>
          <h3>Manage devices</h3>
          <p>One selected text, one trusted Mac, one explicit receiving decision.</p>
        </div>
        <LocalLinkStatusBadge label={preferences.enabled ? "On" : "Off"} tone="neutral" />
      </div>

      <aside className="local-link-safety-note" role="note">
        <ShieldCheck size={16} aria-hidden="true" />
        <span>
          Pairing compares a short safety code and both identity fingerprints. No account or cloud
          relay is used.
        </span>
      </aside>

      <label className="settings-toggle local-link-master-toggle">
        <input
          type="checkbox"
          checked={preferences.enabled}
          onChange={(event) => void updateEnabled(event.target.checked)}
          disabled={saving}
          aria-label="Enable Local Link"
        />
        <span>
          <strong>{preferences.enabled ? "Local Link is on" : "Enable Local Link"}</strong>
          <small>
            When enabled, ClipRiva listens only on your local network and advertises with Bonjour.
            Nothing is copied, saved, or sent automatically.
          </small>
        </span>
        <LocalLinkStatusBadge
          label={preferences.enabled ? "Listening on LAN" : "Off"}
          tone={preferences.enabled ? "success" : "neutral"}
        />
      </label>

      <section className="local-link-this-mac" aria-label="This Mac">
        <div className="local-link-device-section-heading">
          <div>
            <strong>This Mac</strong>
            <small>Device name and short identity fingerprint</small>
          </div>
          <span className="local-link-protocol">
            {preferences.protocolVersion ?? "Protocol pending"}
          </span>
        </div>
        <div className="settings-grid local-link-device-name">
          <label>
            <span>This Mac’s name</span>
            <input
              value={deviceName}
              maxLength={80}
              onChange={(event) => setDeviceName(event.target.value)}
              aria-label="This Mac’s Local Link name"
            />
          </label>
          <button
            type="button"
            className="text-button local-link-name-save"
            onClick={() => void saveDeviceName()}
            disabled={saving || deviceName.trim() === preferences.deviceName}
          >
            Save name
          </button>
        </div>
        <div className="local-link-fingerprint-row">
          <span>Short identity fingerprint</span>
          <code>{formatLocalLinkFingerprint(preferences.identityFingerprint)}</code>
          <button
            type="button"
            className="icon-button"
            aria-label="Copy this Mac identity fingerprint"
            onClick={() =>
              void navigator.clipboard?.writeText(preferences.identityFingerprint ?? "")
            }
            disabled={!preferences.identityFingerprint}
          >
            <Copy size={14} />
          </button>
        </div>
      </section>

      <DeviceSection
        title="Trusted devices"
        description="Availability is advisory; identity fingerprint is the trust anchor."
        devices={trusted}
        empty="No trusted devices yet. Add a device when both Macs are available."
        action={(device) => (
          <button
            type="button"
            className="text-button local-link-revoke"
            onClick={() => setRevoking(device)}
            disabled={actions.revokeDevice.isPending}
            aria-label={`Revoke ${device.displayName}`}
          >
            Revoke
          </button>
        )}
      />

      {needsRePairing.length > 0 ? (
        <DeviceSection
          title="Needs re-pairing"
          description="Identity changed. Old trust cannot be reused."
          devices={needsRePairing}
          empty=""
          action={(device) => (
            <button
              type="button"
              className="settings-save"
              onClick={() => void rePair(device)}
              disabled={actions.beginPairing.isPending || actions.updatePreferences.isPending}
              aria-label={`Re-pair ${device.displayName}`}
            >
              <RotateCcw size={15} aria-hidden="true" /> Re-pair
            </button>
          )}
        />
      ) : null}

      {blocked.length > 0 ? (
        <DeviceSection
          title="Blocked devices"
          description="These identities cannot authenticate unless you explicitly pair again."
          devices={blocked}
          empty=""
          action={(device) => (
            <button
              type="button"
              className="text-button"
              onClick={() => void rePair(device)}
              disabled={actions.beginPairing.isPending || actions.updatePreferences.isPending}
              aria-label={`Pair ${device.displayName} again`}
            >
              Pair again
            </button>
          )}
        />
      ) : null}

      {preferences.enabled && !pairing ? (
        nearby.length > 0 ? (
          <DeviceSection
            title="Nearby devices"
            description="Availability does not establish trust. Start pairing to compare both identities."
            devices={nearby}
            empty=""
            action={(device) => (
              <button
                ref={pairingTriggerRef}
                type="button"
                className="settings-save"
                onClick={(event) => {
                  pairingTriggerRef.current = event.currentTarget;
                  void startPairing(device);
                }}
                disabled={actions.beginPairing.isPending}
                aria-label={`Pair with ${device.displayName}`}
              >
                Pair
              </button>
            )}
          />
        ) : (
          <section className="local-link-device-section" aria-label="No nearby devices">
            <strong>No nearby ClipRiva device was found</strong>
            <ul className="local-link-discovery-checks">
              <li>
                <Wifi size={15} aria-hidden="true" /> Both Macs use the same Wi-Fi
              </li>
              <li>
                <MonitorUp size={15} aria-hidden="true" /> ClipRiva is open on the other Mac
              </li>
              <li>
                <ShieldCheck size={15} aria-hidden="true" /> Local Link is enabled on both Macs
              </li>
            </ul>
            <button
              type="button"
              className="text-button"
              onClick={() => void devicesQuery.refetch()}
              disabled={devicesQuery.isFetching}
            >
              <RotateCcw size={15} aria-hidden="true" />
              {devicesQuery.isFetching ? "Scanning…" : "Scan again"}
            </button>
          </section>
        )
      ) : null}

      {pairing ? (
        <div className="local-link-pairing-backdrop">
          <section
            ref={pairingDialogRef}
            className="local-link-pairing"
            role="dialog"
            aria-modal="true"
            aria-labelledby="local-link-pairing-title"
            aria-describedby="local-link-pairing-description"
            tabIndex={-1}
          >
            <header>
              <div>
                <span className="eyebrow">Local Link · Pair device</span>
                <strong id="local-link-pairing-title">
                  Verify the same safety code on both Macs
                </strong>
                <p id="local-link-pairing-description">
                  Compare the safety code and both short identity fingerprints before confirming on
                  this Mac.
                </p>
              </div>
              <LocalLinkStatusBadge
                label={comparisonSeconds === null ? "Active" : `${comparisonSeconds}s`}
                tone={comparisonSeconds === 0 ? "warning" : "progress"}
              />
            </header>
            <ol className="local-link-pairing-steps" aria-label="Pairing progress">
              <li data-state="complete">
                <Check size={12} aria-hidden="true" /> Discover
              </li>
              <li data-state="current" aria-current="step">
                2 of 3 · Verify both Macs
              </li>
              <li data-state="upcoming">Complete</li>
            </ol>
            <fieldset className="local-link-trust-duration">
              <legend>Trust this device on this Mac</legend>
              <label>
                <input
                  type="radio"
                  name="local-link-trust-duration"
                  value="thisSession"
                  checked={trustDuration === "thisSession"}
                  onChange={() => setTrustDuration("thisSession")}
                  disabled={Boolean(pairing.localConfirmed) || localConfirmationSubmitted}
                />
                <span className="local-link-trust-duration-copy">
                  <strong>Only this session</strong>
                  <small>Forget this device when Local Link closes or is turned off.</small>
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="local-link-trust-duration"
                  value="thirtyDays"
                  checked={trustDuration === "thirtyDays"}
                  onChange={() => setTrustDuration("thirtyDays")}
                  disabled={Boolean(pairing.localConfirmed) || localConfirmationSubmitted}
                />
                <span className="local-link-trust-duration-copy">
                  <strong>30 days (default)</strong>
                  <small>Require this safety-code verification again after 30 days.</small>
                </span>
              </label>
              <label>
                <input
                  type="radio"
                  name="local-link-trust-duration"
                  value="always"
                  checked={trustDuration === "always"}
                  onChange={() => setTrustDuration("always")}
                  disabled={Boolean(pairing.localConfirmed) || localConfirmationSubmitted}
                />
                <span className="local-link-trust-duration-copy">
                  <strong>Always trust this device</strong>
                  <small>
                    Keep this verified device trusted until you revoke it or reset identity.
                  </small>
                </span>
              </label>
            </fieldset>
            <div className="local-link-pairing-comparison">
              <PairingIdentityCard
                label="This Mac"
                name={preferences.deviceName}
                code={pairing.verificationCode}
                fingerprint={pairing.localFingerprint ?? preferences.identityFingerprint}
                confirmed={Boolean(pairing.localConfirmed) || localConfirmationSubmitted}
                action={
                  pairing.localConfirmed || localConfirmationSubmitted ? (
                    <span className="local-link-peer-confirmation" role="status">
                      <TimerReset size={15} />
                      Confirmed here · waiting for nearby Mac
                    </span>
                  ) : (
                    <button
                      ref={pairingConfirmRef}
                      type="button"
                      className="settings-save"
                      onClick={() => void confirmPairing()}
                      disabled={actions.confirmPairing.isPending || comparisonSeconds === 0}
                      aria-label={`Confirm ${pairing.device.displayName} on this Mac`}
                    >
                      <Check size={15} />
                      {actions.confirmPairing.isPending ? "Confirming…" : "Confirm this device"}
                    </button>
                  )
                }
              />
              <span className="local-link-pairing-lock" aria-hidden="true">
                <KeyRound size={17} />
              </span>
              <PairingIdentityCard
                label="Nearby Mac"
                name={pairing.device.displayName}
                code={pairing.verificationCode}
                fingerprint={pairing.peerFingerprint ?? pairing.device.publicKeyFingerprint}
                confirmed={false}
                action={
                  <span className="local-link-peer-confirmation" role="status">
                    <TimerReset size={15} />
                    Waiting for nearby Mac to confirm
                  </span>
                }
              />
            </div>
            <footer>
              <button
                type="button"
                className="text-button local-link-mismatch-button"
                onClick={() => void cancelPairing()}
                disabled={actions.cancelPairing.isPending || actions.confirmPairing.isPending}
              >
                Codes or fingerprints do not match
              </button>
              <button
                type="button"
                className="text-button"
                aria-label="Cancel pairing"
                onClick={() => void cancelPairing()}
                disabled={actions.cancelPairing.isPending || actions.confirmPairing.isPending}
              >
                Cancel
              </button>
            </footer>
          </section>
        </div>
      ) : null}

      {revoking ? (
        <section
          className="local-link-revoke-confirmation"
          role="alertdialog"
          aria-labelledby="revoke-local-link-title"
          aria-describedby="revoke-local-link-description"
        >
          <strong id="revoke-local-link-title">Revoke {revoking.displayName}?</strong>
          <p id="revoke-local-link-description">
            Fingerprint {formatLocalLinkFingerprint(revoking.publicKeyFingerprint)}. Active requests
            will be cancelled; local History will not be deleted.
          </p>
          <div>
            <button type="button" className="text-button" onClick={() => setRevoking(null)}>
              Keep trusted
            </button>
            <button
              type="button"
              className="settings-save local-link-danger-button"
              onClick={() => void revoke()}
              disabled={actions.revokeDevice.isPending}
              aria-label={`Confirm revoke ${revoking.displayName}`}
            >
              {actions.revokeDevice.isPending ? "Revoking…" : "Revoke trust"}
            </button>
          </div>
        </section>
      ) : null}

      <details className="local-link-security-details">
        <summary>Security details</summary>
        <p>
          Resetting this Mac’s identity invalidates existing trust. Every device must verify a new
          safety code before it can send again.
        </p>
        <button
          ref={identityResetTriggerRef}
          type="button"
          className="text-button local-link-danger-button"
          onClick={() => setConfirmingIdentityReset(true)}
          disabled={actions.resetIdentity.isPending}
        >
          <RotateCcw size={15} aria-hidden="true" />
          Reset Local Link identity
        </button>
      </details>

      {confirmingIdentityReset ? (
        <div className="local-link-pairing-backdrop">
          <section
            ref={identityResetDialogRef}
            className="local-link-pairing local-link-reset-identity-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="reset-local-link-identity-title"
            aria-describedby="reset-local-link-identity-description"
            tabIndex={-1}
          >
            <header>
              <div>
                <span className="eyebrow">Local Link · Security</span>
                <strong id="reset-local-link-identity-title">Reset this Mac’s identity?</strong>
                <p id="reset-local-link-identity-description">
                  All trusted devices will need to verify a new safety code. This does not delete
                  Clipboard History.
                </p>
              </div>
            </header>
            <footer>
              <button
                type="button"
                className="text-button"
                onClick={() => setConfirmingIdentityReset(false)}
                disabled={actions.resetIdentity.isPending}
              >
                Keep current identity
              </button>
              <button
                ref={identityResetConfirmRef}
                type="button"
                className="settings-save local-link-danger-button"
                onClick={() => void resetIdentity()}
                disabled={actions.resetIdentity.isPending}
              >
                {actions.resetIdentity.isPending ? "Resetting…" : "Reset identity and re-pair"}
              </button>
            </footer>
          </section>
        </div>
      ) : null}

      {status ? (
        <p className="local-link-operation-status" role="status">
          {status}
        </p>
      ) : null}
      {devicesQuery.isError ? (
        <p className="local-link-error" role="alert">
          Could not load trusted devices.
        </p>
      ) : null}
      {error ? (
        <p className="local-link-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}

function PairingIdentityCard({
  label,
  name,
  code,
  fingerprint,
  confirmed,
  action,
}: {
  label: string;
  name: string;
  code: string;
  fingerprint?: string | null;
  confirmed: boolean;
  action: ReactNode;
}) {
  return (
    <section className="local-link-pairing-identity" data-confirmed={confirmed}>
      <span className="local-link-device-icon">
        <Laptop size={16} aria-hidden="true" />
      </span>
      <div>
        <strong>{name}</strong>
        <small>{label}</small>
      </div>
      <span>Safety code (SAS)</span>
      <output aria-label={`${label} safety code`}>{formatPairingCode(code)}</output>
      <span>Identity fingerprint</span>
      <code>{formatLocalLinkFingerprint(fingerprint)}</code>
      {action}
    </section>
  );
}

function DeviceSection({
  title,
  description,
  devices,
  empty,
  action,
}: {
  title: string;
  description: string;
  devices: LocalLinkDevice[];
  empty: string;
  action: (device: LocalLinkDevice) => ReactNode;
}) {
  return (
    <section className="local-link-device-section" aria-label={title}>
      <div className="local-link-device-section-heading">
        <div>
          <strong>{title}</strong>
          <small>{description}</small>
        </div>
      </div>
      {devices.length === 0 ? (
        <p className="local-link-device-empty">{empty}</p>
      ) : (
        <ul>
          {devices.map((device) => {
            const presentation = presentLocalLinkDevice(device);
            return (
              <li key={device.deviceId}>
                <span className="local-link-device-icon">
                  <Laptop size={15} aria-hidden="true" />
                </span>
                <span className="local-link-device-copy">
                  <span className="local-link-device-title-row">
                    <strong>{device.displayName}</strong>
                    <LocalLinkStatusBadge label={presentation.label} tone={presentation.tone} />
                  </span>
                  <small>
                    ClipRiva on Mac · {formatLocalLinkProtocol(device)} · fingerprint{" "}
                    {formatLocalLinkFingerprint(device.publicKeyFingerprint)}
                  </small>
                  <small>
                    {presentation.detail}
                    {device.trustStatus === "trusted"
                      ? ` · ${formatTrustDuration(device.trustDuration)}`
                      : ""}
                    {device.lastSeenAt
                      ? ` · last seen ${formatRelativeTime(device.lastSeenAt)}`
                      : ""}
                    {device.trustedAt ? ` · trusted ${formatRelativeTime(device.trustedAt)}` : ""}
                    {device.trustExpiresAt
                      ? ` · trust expires ${formatRelativeTime(device.trustExpiresAt)}`
                      : ""}
                  </small>
                </span>
                {action(device)}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function formatPairingCode(code: string) {
  return `${code.slice(0, 3)} ${code.slice(3)}`;
}

type LocalLinkRePairRoute =
  | { kind: "unavailable"; message: string }
  | { kind: "singleCandidate"; candidate: LocalLinkDevice }
  | { kind: "chooseNearby"; message: string };

/**
 * A stored device ID is a trust record, not a routable Bonjour candidate.
 * Re-pairing must therefore start from the current anonymous discovery set
 * and let the native authenticated handshake establish the peer identity.
 */
export function resolveLocalLinkRePairRoute(
  device: Pick<LocalLinkDevice, "displayName">,
  nearbyCandidates: LocalLinkDevice[],
): LocalLinkRePairRoute {
  if (nearbyCandidates.length === 0) {
    return {
      kind: "unavailable",
      message: `No nearby pairing candidate is available for ${device.displayName}. Open ClipRiva and Local Link on the other Mac, then scan again.`,
    };
  }
  const soleCandidate = nearbyCandidates[0];
  if (nearbyCandidates.length === 1 && soleCandidate) {
    return { kind: "singleCandidate", candidate: soleCandidate };
  }
  return {
    kind: "chooseNearby",
    message: `More than one nearby Mac is available. Choose the Mac you want to re-pair with ${device.displayName}, then verify the safety code and fingerprints on both Macs.`,
  };
}

function formatTrustDuration(duration: LocalLinkTrustDuration | null | undefined) {
  switch (duration ?? "thirtyDays") {
    case "thisSession":
      return "this session";
    case "always":
      return "always trusted";
    case "thirtyDays":
      return "30-day trust";
  }
}

function errorMessage(reason: unknown) {
  return String(reason).replace(/^Error:\s*/u, "");
}
