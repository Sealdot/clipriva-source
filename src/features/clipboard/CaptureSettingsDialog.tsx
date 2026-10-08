import {
  Command,
  FlaskConical,
  HardDrive,
  Keyboard,
  MousePointerClick,
  PauseCircle,
  ShieldCheck,
  X,
} from "lucide-react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { formatRelativeTime } from "./format";
import { LocalDiagnosticsPanel } from "./LocalDiagnosticsPanel";
import type {
  AutomationPreferences,
  CapturePreferences,
  CaptureStatusEvent,
  ClipboardItem,
  LocalDiagnosticsExport,
  LocalDiagnosticsSummary,
  PastePermissionStatus,
} from "./types";
import "./CaptureSettingsDialog.css";

const shortcutOptions = [
  { value: "CommandOrControl+Shift+Space", label: "⌘⇧Space" },
  { value: "CommandOrControl+Shift+V", label: "⌘⇧V" },
  { value: "CommandOrControl+Alt+V", label: "⌘⌥V" },
  { value: "CommandOrControl+Shift+C", label: "⌘⇧C" },
] as const;

const stackShortcutOptions = [
  { value: "CommandOrControl+Alt+S", label: "⌘⌥S" },
  { value: "CommandOrControl+Shift+S", label: "⌘⇧S" },
  { value: "CommandOrControl+Alt+Space", label: "⌘⌥Space" },
  { value: "CommandOrControl+Shift+C", label: "⌘⇧C" },
] as const;

const defaultAutomationPreferences: AutomationPreferences = {
  enabled: false,
  historyMetadata: false,
  historyContent: false,
  clipboardWrite: false,
  libraryWrite: false,
  filtersRun: false,
};

const settingsSections = [
  {
    id: "general",
    label: "General",
    description: "App behavior",
  },
  {
    id: "privacy",
    label: "Privacy",
    description: "Capture, data, and trust",
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    description: "Quick Paste keys",
  },
] as const;

type SettingsSection = (typeof settingsSections)[number]["id"];

const focusableSelector = [
  "a[href]",
  "button:not([disabled])",
  "summary",
  "input:not([disabled]):not([type=hidden])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(focusableSelector)).filter(
    (element) => element.tabIndex >= 0,
  );
}

function SettingsStatusSummary({
  label,
  state,
  detail,
  tone = "healthy",
}: {
  label: string;
  state: string;
  detail: string;
  tone?: "healthy" | "warning" | "neutral";
}) {
  return (
    <aside className="settings-status-summary" data-tone={tone} role="status">
      <span className="settings-status-summary-label">Current status</span>
      <strong>
        {label}: {state}
      </strong>
      <span>{detail}</span>
    </aside>
  );
}

interface CaptureSettingsDialogProps {
  isOpen: boolean;
  preferences: CapturePreferences | undefined;
  automationPreferences?: AutomationPreferences;
  captureStatusEvents?: CaptureStatusEvent[];
  retentionPreviewItems?: ClipboardItem[];
  localDiagnosticsSummary?: LocalDiagnosticsSummary;
  isSaving: boolean;
  isClearingStatusEvents?: boolean;
  isClearingLocalDiagnostics?: boolean;
  isExportingLocalDiagnostics?: boolean;
  isResumingCapture?: boolean;
  pastePermissionStatus?: PastePermissionStatus;
  isCheckingPastePermission?: boolean;
  isOpeningPastePermissionSettings?: boolean;
  onClose: () => void;
  onSave: (preferences: CapturePreferences) => Promise<unknown>;
  onSaveAutomation?: (preferences: AutomationPreferences) => Promise<unknown>;
  onClearStatusEvents?: () => void;
  onResumeCapture?: () => Promise<unknown>;
  onClearLocalDiagnostics?: () => void;
  onExportLocalDiagnostics?: () => Promise<LocalDiagnosticsExport>;
  onRecheckPastePermission?: () => void;
  onOpenPastePermissionSettings?: () => void;
}

export function CaptureSettingsDialog({
  isOpen,
  preferences,
  automationPreferences,
  captureStatusEvents = [],
  retentionPreviewItems = [],
  localDiagnosticsSummary,
  isSaving,
  isClearingStatusEvents = false,
  isClearingLocalDiagnostics = false,
  isExportingLocalDiagnostics = false,
  isResumingCapture = false,
  pastePermissionStatus,
  isCheckingPastePermission = false,
  isOpeningPastePermissionSettings = false,
  onClose,
  onSave,
  onSaveAutomation,
  onClearStatusEvents,
  onResumeCapture,
  onClearLocalDiagnostics,
  onExportLocalDiagnostics,
  onRecheckPastePermission,
  onOpenPastePermissionSettings,
}: CaptureSettingsDialogProps) {
  const [draft, setDraft] = useState<CapturePreferences | null>(preferences ?? null);
  const [automationDraft, setAutomationDraft] = useState<AutomationPreferences>(
    automationPreferences ?? defaultAutomationPreferences,
  );
  const [deniedApps, setDeniedApps] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [resumeConfirmationOpen, setResumeConfirmationOpen] = useState(false);
  const [isResumingLocally, setIsResumingLocally] = useState(false);
  const dialogRef = useRef<HTMLElement>(null);
  const sensitivePolicyRef = useRef<HTMLFieldSetElement>(null);
  const deniedAppsRef = useRef<HTMLTextAreaElement>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const wasOpenRef = useRef(false);
  const onCloseRef = useRef(onClose);
  const isDialogReady = isOpen && draft !== null;

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    if (!isOpen || !preferences) return;
    setDraft(preferences);
    setAutomationDraft(automationPreferences ?? defaultAutomationPreferences);
    setDeniedApps(preferences.deniedApps.join("\n"));
    setSaveError(null);
    setResumeConfirmationOpen(false);
  }, [automationPreferences, isOpen, preferences]);

  useEffect(() => {
    if (!isDialogReady) {
      if (wasOpenRef.current) {
        wasOpenRef.current = false;
        if (triggerRef.current?.isConnected) {
          triggerRef.current.focus();
        }
      }
      return;
    }

    if (!wasOpenRef.current) {
      const activeElement = document.activeElement;
      triggerRef.current = activeElement instanceof HTMLElement ? activeElement : null;
      wasOpenRef.current = true;
    }

    dialogRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }

      if (event.key !== "Tab") return;

      const dialog = dialogRef.current;
      if (!dialog) return;

      const focusableElements = getFocusableElements(dialog);
      if (focusableElements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }

      const currentIndex = focusableElements.indexOf(document.activeElement as HTMLElement);
      if (currentIndex === -1) {
        event.preventDefault();
        (event.shiftKey ? focusableElements.at(-1) : focusableElements[0])?.focus();
        return;
      }

      const isMovingBackwardsFromFirst = event.shiftKey && currentIndex === 0;
      const isMovingForwardsFromLast =
        !event.shiftKey && currentIndex === focusableElements.length - 1;
      if (isMovingBackwardsFromFirst || isMovingForwardsFromLast) {
        event.preventDefault();
        (isMovingBackwardsFromFirst ? focusableElements.at(-1) : focusableElements[0])?.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isDialogReady]);

  if (!isOpen || !draft) return null;

  const sensitiveContentPolicy = draft.sensitiveContentPolicy ?? "default";
  const selectedShortcut = shortcutOptions.find(
    (option) => option.value === draft.quickPasteShortcut,
  );
  const selectedStackShortcut = stackShortcutOptions.find(
    (option) => option.value === draft.stackShortcut,
  );
  const captureState = draft.capturePaused ? "Paused" : "Recording locally";
  const captureDetail = draft.capturePaused
    ? `${draft.pauseReason ?? "Capture was paused"}. Resume capture to save new clips.`
    : `${sensitiveContentPolicy === "strict" ? "Strict" : "Default"} protection · ${draft.deniedApps.length} excluded ${draft.deniedApps.length === 1 ? "app" : "apps"}`;
  const retentionImpact = getRetentionImpact(retentionPreviewItems, draft);

  const handleSectionKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
    currentSection: SettingsSection,
  ) => {
    const currentIndex = settingsSections.findIndex(({ id }) => id === currentSection);
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? settingsSections.length - 1
          : event.key === "ArrowRight" || event.key === "ArrowDown"
            ? (currentIndex + 1) % settingsSections.length
            : event.key === "ArrowLeft" || event.key === "ArrowUp"
              ? (currentIndex - 1 + settingsSections.length) % settingsSections.length
              : null;

    if (nextIndex === null) return;

    const nextSection = settingsSections[nextIndex];
    if (!nextSection) return;

    event.preventDefault();
    setActiveSection(nextSection.id);
    const tabList = event.currentTarget.parentElement;
    tabList?.querySelector<HTMLButtonElement>(`#capture-settings-tab-${nextSection.id}`)?.focus();
  };

  const save = async () => {
    setSaveError(null);
    if (draft.quickPasteShortcut === draft.stackShortcut) {
      setSaveError("Quick Paste and Stack shortcuts must be different.");
      return;
    }
    try {
      await onSave({
        ...draft,
        deniedApps: deniedApps
          .split("\n")
          .map((value) => value.trim())
          .filter(Boolean),
      });
      if (onSaveAutomation) await onSaveAutomation(automationDraft);
      onClose();
    } catch (error) {
      setSaveError(String(error).replace(/^Error:\s*/, ""));
    }
  };

  const focusProtectionRules = () => {
    sensitivePolicyRef.current?.focus({ preventScroll: false });
  };

  const focusExcludedApps = () => {
    deniedAppsRef.current?.focus({ preventScroll: false });
  };

  const resumeCapture = async () => {
    if (!onResumeCapture) return;
    setSaveError(null);
    setIsResumingLocally(true);
    try {
      await onResumeCapture();
      setDraft((current) =>
        current ? { ...current, capturePaused: false, pauseReason: null } : current,
      );
      setResumeConfirmationOpen(false);
    } catch (error) {
      setSaveError(String(error).replace(/^Error:\s*/, ""));
    } finally {
      setIsResumingLocally(false);
    }
  };

  const requestResume = () => {
    if (sensitiveContentPolicy === "strict") {
      setResumeConfirmationOpen(true);
      return;
    }
    void resumeCapture();
  };

  const captureReasonCards = captureStatusEvents.slice(0, 3);
  const isResumePending = isResumingCapture || isResumingLocally;

  return (
    <div className="settings-backdrop">
      <button
        type="button"
        className="settings-dismiss"
        onClick={onClose}
        aria-label="Close settings"
        aria-hidden="true"
        tabIndex={-1}
      />
      <section
        ref={dialogRef}
        className="settings-dialog capture-settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="capture-settings-title"
        aria-describedby="capture-settings-description"
        tabIndex={-1}
      >
        <header className="settings-dialog-heading">
          <div>
            <span className="eyebrow">ClipRiva Core</span>
            <h2 id="capture-settings-title">Local clipboard settings</h2>
          </div>
          <button
            type="button"
            className="icon-button"
            onClick={onClose}
            aria-label="Close settings"
          >
            <X size={16} />
          </button>
        </header>

        <p id="capture-settings-description" className="settings-intro">
          ClipRiva needs no account or cloud service. Capture rules run before a new item is stored;
          Local Link connects only to a trusted Mac when you explicitly enable it.
        </p>

        <div className="capture-settings-tabs" role="tablist" aria-label="Settings sections">
          {settingsSections.map((section) => (
            <button
              key={section.id}
              id={`capture-settings-tab-${section.id}`}
              type="button"
              role="tab"
              aria-selected={activeSection === section.id}
              aria-controls={`capture-settings-panel-${section.id}`}
              tabIndex={activeSection === section.id ? 0 : -1}
              className="capture-settings-tab"
              onClick={() => setActiveSection(section.id)}
              onKeyDown={(event) => handleSectionKeyDown(event, section.id)}
            >
              <span>{section.label}</span>
              <small>{section.description}</small>
            </button>
          ))}
        </div>

        {activeSection === "general" ? (
          <section
            id="capture-settings-panel-general"
            className="capture-settings-panel"
            role="tabpanel"
            aria-labelledby="capture-settings-tab-general"
          >
            <div className="capture-settings-panel-heading">
              <div>
                <h3>Everyday behavior</h3>
                <p>Core capture stays separate from optional device-to-device text sharing.</p>
              </div>
              <Keyboard size={18} aria-hidden="true" />
            </div>

            <div className="settings-permission-note" role="status">
              <MousePointerClick
                className="settings-permission-icon"
                size={16}
                aria-hidden="true"
              />
              <span>
                Direct Paste returns to your previous app and sends Command-V. ClipRiva explains
                Accessibility permission only when you first try it; without permission, the
                selected clip can still be copied for a manual paste.
              </span>
            </div>

            <section className="settings-permission-status" aria-label="Direct Paste permission">
              <div>
                <span className="eyebrow">macOS Accessibility</span>
                <strong>
                  {isCheckingPastePermission && pastePermissionStatus === undefined
                    ? "Checking permission…"
                    : pastePermissionStatus === "granted"
                      ? "Direct Paste is ready"
                      : "Direct Paste permission is off"}
                </strong>
                <small>
                  Search, selection, and Copy remain available in every permission state.
                </small>
              </div>
              <div>
                <button
                  type="button"
                  className="text-button"
                  onClick={onRecheckPastePermission}
                  disabled={isCheckingPastePermission || !onRecheckPastePermission}
                >
                  {isCheckingPastePermission ? "Checking…" : "Recheck"}
                </button>
                <button
                  type="button"
                  className="text-button"
                  onClick={onOpenPastePermissionSettings}
                  disabled={isOpeningPastePermissionSettings || !onOpenPastePermissionSettings}
                >
                  {isOpeningPastePermissionSettings ? "Opening…" : "Open System Settings"}
                </button>
              </div>
            </section>

            <details className="capture-settings-disclosure">
              <summary>
                <span>
                  <strong className="capture-settings-disclosure-title">
                    Advanced local tools
                  </strong>
                  <small className="capture-settings-disclosure-description">
                    Keep Labs out of the everyday workflow until you need it.
                  </small>
                </span>
                <FlaskConical size={18} aria-hidden="true" />
              </summary>
              <div className="capture-settings-disclosure-content">
                <SettingsStatusSummary
                  label="Labs"
                  state={draft.labsEnabled ? "Enabled locally" : "Off by default"}
                  detail={
                    draft.labsEnabled
                      ? "Text Actions appear only in contextual clip menus."
                      : "No Actions, summaries, or derived data are available while Labs is off."
                  }
                  tone={draft.labsEnabled ? "healthy" : "neutral"}
                />

                <label className="settings-toggle settings-labs-toggle">
                  <input
                    type="checkbox"
                    checked={draft.labsEnabled}
                    onChange={(event) =>
                      setDraft((current) =>
                        current ? { ...current, labsEnabled: event.target.checked } : current,
                      )
                    }
                  />
                  <span>
                    <strong>Enable ClipRiva Labs</strong>
                    <small>
                      Shows optional local text Actions only for selected text clips. Labs never
                      adds a global workspace, stores derived content, or sends data off this Mac.
                    </small>
                  </span>
                  <FlaskConical size={18} aria-hidden="true" />
                </label>
              </div>
            </details>
          </section>
        ) : null}

        {activeSection === "shortcuts" ? (
          <section
            id="capture-settings-panel-shortcuts"
            className="capture-settings-panel"
            role="tabpanel"
            aria-labelledby="capture-settings-tab-shortcuts"
          >
            <div className="capture-settings-panel-heading">
              <div>
                <h3>Quick Paste shortcuts</h3>
                <p>Open, find, and use a clip without opening the main workspace.</p>
              </div>
              <Keyboard size={18} aria-hidden="true" />
            </div>

            <SettingsStatusSummary
              label="Quick Paste"
              state={selectedShortcut?.label ?? "Custom shortcut"}
              detail={`Enter copies and closes · Command-Enter chooses a trusted device · Stack ${selectedStackShortcut?.label ?? "custom shortcut"} opens its shared ordered workflow · the two shortcuts cannot conflict`}
            />

            <div className="capture-settings-first-value">
              <strong>Your fastest first use</strong>
              <span>
                Copy two things, then press {selectedShortcut?.label ?? "your Quick Paste shortcut"}{" "}
                to select the earlier one and choose Copy. Return to the other app and press ⌘V.
              </span>
            </div>

            <div className="settings-grid settings-quick-paste-grid">
              <label>
                <span>Open Quick Paste with</span>
                <select
                  value={draft.quickPasteShortcut}
                  onChange={(event) =>
                    setDraft((current) =>
                      current ? { ...current, quickPasteShortcut: event.target.value } : current,
                    )
                  }
                >
                  {shortcutOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>Open Stack with</span>
                <select
                  value={draft.stackShortcut}
                  onChange={(event) =>
                    setDraft((current) =>
                      current ? { ...current, stackShortcut: event.target.value } : current,
                    )
                  }
                >
                  {stackShortcutOptions.map((option) => (
                    <option
                      key={option.value}
                      value={option.value}
                      disabled={option.value === draft.quickPasteShortcut}
                    >
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <details className="capture-settings-disclosure">
              <summary>
                <span>
                  <strong className="capture-settings-disclosure-title">Shortcuts &amp; CLI</strong>
                  <small className="capture-settings-disclosure-description">
                    Default-off local automation for the running app.
                  </small>
                </span>
                <Command size={18} aria-hidden="true" />
              </summary>
              <div className="capture-settings-disclosure-content">
                <SettingsStatusSummary
                  label="Local automation"
                  state={automationDraft.enabled ? "Socket enabled" : "Off"}
                  detail={
                    automationDraft.enabled
                      ? "Only the capabilities selected below are accepted."
                      : "No automation socket exists while this switch is off."
                  }
                  tone={automationDraft.enabled ? "warning" : "neutral"}
                />
                <label className="settings-toggle">
                  <input
                    type="checkbox"
                    checked={automationDraft.enabled}
                    onChange={(event) =>
                      setAutomationDraft((current) => ({
                        ...current,
                        enabled: event.target.checked,
                      }))
                    }
                  />
                  <span>
                    <strong>Enable local automation</strong>
                    <small>
                      Creates an owner-only Unix socket while ClipRiva is running. It is not a
                      network listener or Agent/MCP connection.
                    </small>
                  </span>
                </label>
                <fieldset disabled={!automationDraft.enabled}>
                  <legend>Granted capabilities</legend>
                  {(
                    [
                      ["historyMetadata", "Search History metadata"],
                      ["historyContent", "Read selected text content"],
                      ["clipboardWrite", "Write the system clipboard"],
                      ["libraryWrite", "Save or unsave clips"],
                      ["filtersRun", "List and run local filters"],
                    ] as const
                  ).map(([capability, label]) => (
                    <label className="settings-toggle" key={capability}>
                      <input
                        type="checkbox"
                        checked={automationDraft[capability]}
                        onChange={(event) =>
                          setAutomationDraft((current) => ({
                            ...current,
                            [capability]: event.target.checked,
                          }))
                        }
                      />
                      <span>
                        <strong>{label}</strong>
                      </span>
                    </label>
                  ))}
                </fieldset>
                <small>
                  Apple Shortcuts can call the bundled CLI with “Run Shell Script”; ClipRiva does
                  not claim native App Intents. Search text and filter input are read from stdin.
                </small>
              </div>
            </details>
          </section>
        ) : null}

        {activeSection === "privacy" ? (
          <section
            id="capture-settings-panel-privacy"
            className="capture-settings-panel"
            role="tabpanel"
            aria-labelledby="capture-settings-tab-privacy"
          >
            <div className="capture-settings-panel-heading">
              <div>
                <h3>Local-first by default</h3>
                <p>Your history remains on this Mac unless you explicitly send one text clip.</p>
              </div>
              <ShieldCheck size={18} aria-hidden="true" />
            </div>

            <SettingsStatusSummary
              label="Account and cloud"
              state="Not used"
              detail="ClipRiva has no account, cloud sync, telemetry, or remote model service."
              tone="healthy"
            />

            <div className="settings-permission-note settings-local-first-note" role="status">
              <ShieldCheck className="settings-permission-icon" size={16} aria-hidden="true" />
              <span>
                Local Link is separate and default off. It sends only a selected UTF-8 text clip to
                a trusted device on the same local network after both devices confirm trust.
              </span>
            </div>

            <div className="capture-settings-subsection-heading">
              <h3>Capture and local history</h3>
              <p>Decide what stays on this Mac and how long it remains available.</p>
            </div>

            <SettingsStatusSummary
              label="Capture"
              state={captureState}
              detail={captureDetail}
              tone={draft.capturePaused ? "warning" : "healthy"}
            />

            {captureReasonCards.length > 0 ? (
              <section className="capture-reason-cards" aria-label="Capture protection actions">
                <div className="capture-reason-cards-heading">
                  <div>
                    <strong>Capture protection actions</strong>
                    <small>Each explanation leads to the relevant local control.</small>
                  </div>
                </div>
                <div className="capture-reason-card-list">
                  {captureReasonCards.map((event) => (
                    <CaptureReasonCard
                      key={event.id}
                      event={event}
                      capturePaused={draft.capturePaused}
                      onViewProtectionRules={focusProtectionRules}
                      onManageExcludedApps={focusExcludedApps}
                      onResumeCapture={requestResume}
                      isResuming={isResumePending}
                      canResume={Boolean(onResumeCapture)}
                    />
                  ))}
                </div>
              </section>
            ) : null}

            {resumeConfirmationOpen ? (
              <section
                className="capture-resume-confirmation"
                role="alertdialog"
                aria-labelledby="capture-resume-confirmation-title"
                aria-describedby="capture-resume-confirmation-description"
              >
                <strong id="capture-resume-confirmation-title">
                  Resume Capture in Strict mode?
                </strong>
                <p id="capture-resume-confirmation-description">
                  Capture will resume, but Strict Sensitive content protection stays enabled. Future
                  suspected sensitive clips will still be blocked and pause Capture again.
                </p>
                <div>
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => setResumeConfirmationOpen(false)}
                    disabled={isResumePending}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="settings-save"
                    onClick={() => void resumeCapture()}
                    disabled={isResumePending || !onResumeCapture}
                  >
                    {isResumePending ? "Resuming…" : "Resume Capture"}
                  </button>
                </div>
              </section>
            ) : null}

            <section className="capture-status-events" aria-label="Privacy Activity">
              <div className="capture-status-events-heading">
                <div>
                  <strong>Privacy Activity</strong>
                  <small>
                    Why a recent clipboard change was not retained. Only reason, time and source app
                    are kept locally for up to 7 days (maximum 20 events).
                  </small>
                </div>
                {onClearStatusEvents && captureStatusEvents.length > 0 ? (
                  <button
                    type="button"
                    className="text-button"
                    onClick={onClearStatusEvents}
                    disabled={isClearingStatusEvents}
                  >
                    {isClearingStatusEvents ? "Clearing…" : "Clear"}
                  </button>
                ) : null}
              </div>
              {captureStatusEvents.length > 0 ? (
                <ul>
                  {captureStatusEvents.map((event) => (
                    <li key={event.id}>
                      <span>{captureStatusReasonLabel(event.reasonType)}</span>
                      <small>
                        {event.sourceApp ?? "ClipRiva"} · {formatRelativeTime(event.occurredAt)}
                      </small>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="capture-status-events-empty">
                  No recent privacy actions. ClipRiva has no content to explain here.
                </p>
              )}
            </section>

            <label className="settings-toggle settings-pause-toggle">
              <input
                type="checkbox"
                checked={draft.capturePaused}
                onChange={(event) => {
                  if (!event.target.checked) {
                    requestResume();
                    return;
                  }
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          capturePaused: true,
                          pauseReason: "Paused manually",
                        }
                      : current,
                  );
                }}
              />
              <span>
                <strong>Pause all clipboard capture</strong>
                <small>
                  Existing local history stays available while new clipboard events are ignored.
                </small>
              </span>
              <PauseCircle size={18} aria-hidden="true" />
            </label>

            <fieldset
              ref={sensitivePolicyRef}
              className="capture-settings-sensitive-policy"
              tabIndex={-1}
            >
              <legend>Sensitive content protection</legend>
              <p>
                Suspected private keys and high-confidence access tokens never enter local history
                or Labs-derived data.
              </p>
              <div className="capture-settings-policy-options">
                <label className="capture-settings-policy-option">
                  <input
                    type="radio"
                    name="sensitive-content-policy"
                    value="default"
                    checked={sensitiveContentPolicy === "default"}
                    onChange={() =>
                      setDraft((current) =>
                        current
                          ? {
                              ...current,
                              sensitiveContentPolicy: "default",
                              sensitivePauseEnabled: false,
                            }
                          : current,
                      )
                    }
                  />
                  <span>
                    <strong>Default</strong>
                    <small>Skip the suspected item and keep capture running.</small>
                  </span>
                </label>
                <label className="capture-settings-policy-option">
                  <input
                    type="radio"
                    name="sensitive-content-policy"
                    value="strict"
                    checked={sensitiveContentPolicy === "strict"}
                    onChange={() =>
                      setDraft((current) =>
                        current
                          ? {
                              ...current,
                              sensitiveContentPolicy: "strict",
                              sensitivePauseEnabled: true,
                            }
                          : current,
                      )
                    }
                  />
                  <span>
                    <strong>Strict</strong>
                    <small>Skip the suspected item and pause capture until you resume it.</small>
                  </span>
                </label>
              </div>
            </fieldset>

            <div className="settings-grid">
              <label>
                <span>Retention period (days)</span>
                <input
                  type="number"
                  min="1"
                  max="3650"
                  value={draft.retentionDays}
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? {
                            ...current,
                            retentionDays: Number.parseInt(event.target.value, 10) || 1,
                          }
                        : current,
                    )
                  }
                />
              </label>
              <label>
                <span>Maximum unpinned clips</span>
                <input
                  type="number"
                  min="100"
                  max="100000"
                  value={draft.maxHistoryItems}
                  onChange={(event) =>
                    setDraft((current) =>
                      current
                        ? {
                            ...current,
                            maxHistoryItems: Number.parseInt(event.target.value, 10) || 100,
                          }
                        : current,
                    )
                  }
                />
              </label>
            </div>

            <p className="retention-preview" role="status">
              Saving these rules will remove an estimated {retentionImpact} unpinned{" "}
              {retentionImpact === 1 ? "clip" : "clips"}. Pinned clips stay available.
            </p>

            <label className="settings-deny-list">
              <span>Never capture from these apps</span>
              <textarea
                ref={deniedAppsRef}
                value={deniedApps}
                onChange={(event) => setDeniedApps(event.target.value)}
                placeholder={"One application name per line\nExample: 1Password"}
                rows={4}
              />
            </label>
            <p className="settings-intro">
              Excluding an app leaves Items already in History unchanged.
            </p>

            <details className="capture-settings-disclosure">
              <summary>
                <span>
                  <strong className="capture-settings-disclosure-title">
                    Local diagnostics &amp; data
                  </strong>
                  <small className="capture-settings-disclosure-description">
                    Review on-device storage and optional reliability diagnostics.
                  </small>
                </span>
                <HardDrive size={18} aria-hidden="true" />
              </summary>
              <div className="capture-settings-disclosure-content">
                <div className="settings-data-note">
                  <HardDrive size={17} aria-hidden="true" />
                  <div>
                    <code>~/Library/Application Support/com.clipriva.desktop/</code>
                    <small>
                      Quit ClipRiva before backing up or removing this directory. Removing it
                      permanently resets history and settings.
                    </small>
                  </div>
                </div>

                <LocalDiagnosticsPanel
                  enabled={draft.diagnosticsEnabled ?? false}
                  summary={localDiagnosticsSummary}
                  isClearing={isClearingLocalDiagnostics}
                  isExporting={isExportingLocalDiagnostics}
                  onEnabledChange={(diagnosticsEnabled) =>
                    setDraft((current) => (current ? { ...current, diagnosticsEnabled } : current))
                  }
                  onClear={onClearLocalDiagnostics}
                  onExport={onExportLocalDiagnostics}
                />
              </div>
            </details>
          </section>
        ) : null}

        {saveError ? (
          <p className="settings-save-error" role="alert">
            {saveError}
          </p>
        ) : null}

        <footer className="settings-dialog-footer">
          <button type="button" className="text-button" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="settings-save"
            onClick={() => void save()}
            disabled={isSaving}
          >
            {isSaving ? "Saving…" : "Save settings"}
          </button>
        </footer>
      </section>
    </div>
  );
}

function getRetentionImpact(items: ClipboardItem[], preferences: CapturePreferences) {
  const cutoff = Date.now() - preferences.retentionDays * 24 * 60 * 60 * 1_000;
  const unpinned = items.filter((item) => !item.isPinned);
  const olderThanRetention = new Set(
    unpinned.filter((item) => Date.parse(item.createdAt) < cutoff).map((item) => item.id),
  );
  const recentUnpinned = unpinned
    .filter((item) => !olderThanRetention.has(item.id))
    .sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt));

  for (const item of recentUnpinned.slice(preferences.maxHistoryItems)) {
    olderThanRetention.add(item.id);
  }

  return olderThanRetention.size;
}

function captureStatusReasonLabel(reason: CaptureStatusEvent["reasonType"]) {
  return {
    manualPause: "Capture was paused manually",
    sensitiveContentDefault: "A suspected sensitive clip was blocked; capture continued",
    sensitiveContentStrict: "A suspected sensitive clip was blocked; capture paused",
    excludedApplication: "A clip from an excluded app was not saved",
    unsupportedFormat: "This clipboard format was not saved",
    contentTooLarge: "A clip exceeded its local size limit and was not saved",
  }[reason];
}

function CaptureReasonCard({
  event,
  capturePaused,
  onViewProtectionRules,
  onManageExcludedApps,
  onResumeCapture,
  isResuming,
  canResume,
}: {
  event: CaptureStatusEvent;
  capturePaused: boolean;
  onViewProtectionRules: () => void;
  onManageExcludedApps: () => void;
  onResumeCapture: () => void;
  isResuming: boolean;
  canResume: boolean;
}) {
  const card = captureReasonCardDetails(event.reasonType);
  const action =
    card.action === "resume"
      ? {
          label: isResuming ? "Resuming…" : "Resume Capture",
          handler: onResumeCapture,
          disabled: !capturePaused || !canResume || isResuming,
        }
      : card.action === "excludedApps"
        ? { label: "Manage Excluded apps", handler: onManageExcludedApps, disabled: false }
        : { label: card.actionLabel, handler: onViewProtectionRules, disabled: false };

  return (
    <article className="capture-reason-card">
      <div className="capture-reason-card-meta">
        <span>{card.category}</span>
        <small>Capture {capturePaused ? "paused" : "active"}</small>
      </div>
      <strong>{captureStatusReasonLabel(event.reasonType)}</strong>
      <small>{formatRelativeTime(event.occurredAt)}</small>
      <button
        type="button"
        className="text-button"
        onClick={action.handler}
        disabled={action.disabled}
      >
        {action.label}
      </button>
    </article>
  );
}

function captureReasonCardDetails(reason: CaptureStatusEvent["reasonType"]) {
  return {
    manualPause: {
      category: "Capture control",
      action: "resume",
      actionLabel: "Resume Capture",
    },
    sensitiveContentDefault: {
      category: "Sensitive content protection",
      action: "rules",
      actionLabel: "View protection rules",
    },
    sensitiveContentStrict: {
      category: "Sensitive content protection",
      action: "resume",
      actionLabel: "Resume Capture",
    },
    excludedApplication: {
      category: "Excluded apps",
      action: "excludedApps",
      actionLabel: "Manage Excluded apps",
    },
    unsupportedFormat: {
      category: "Capture rules",
      action: "rules",
      actionLabel: "View capture rules",
    },
    contentTooLarge: {
      category: "Size limits",
      action: "rules",
      actionLabel: "View capture rules",
    },
  }[reason];
}
