import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { CaptureSettingsDialog } from "./CaptureSettingsDialog";
import type { CapturePreferences } from "./types";

const preferences: CapturePreferences = {
  capturePaused: false,
  sensitivePauseEnabled: true,
  retentionDays: 30,
  maxHistoryItems: 1_000,
  deniedApps: [],
  pauseReason: null,
  labsEnabled: false,
  pasteBehavior: "restore",
  quickPasteShortcut: "CommandOrControl+Shift+Space",
  stackShortcut: "CommandOrControl+Alt+S",
  onboardingCompleted: true,
};

function SettingsHarness() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <>
      <button type="button" onClick={() => setIsOpen(true)}>
        Open settings
      </button>
      <button type="button">Background action</button>
      <CaptureSettingsDialog
        isOpen={isOpen}
        preferences={preferences}
        isSaving={false}
        onClose={() => setIsOpen(false)}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />
    </>
  );
}

describe("CaptureSettingsDialog", () => {
  it("blocks saving when Quick Paste and Stack shortcuts conflict", () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={{
          ...preferences,
          quickPasteShortcut: "CommandOrControl+Shift+C",
          stackShortcut: "CommandOrControl+Shift+C",
        }}
        isSaving={false}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Quick Paste and Stack shortcuts must be different.",
    );
    expect(onSave).not.toHaveBeenCalled();
  });

  it("keeps automation off by default and grants each capability explicitly", async () => {
    const onSaveAutomation = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        automationPreferences={{
          enabled: false,
          historyMetadata: false,
          historyContent: false,
          clipboardWrite: false,
          libraryWrite: false,
          filtersRun: false,
        }}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onSaveAutomation={onSaveAutomation}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Shortcuts/i }));
    fireEvent.click(screen.getByText("Shortcuts & CLI"));
    const enable = screen.getByRole("checkbox", { name: /Enable local automation/i });
    expect(enable).not.toBeChecked();
    expect(screen.getByRole("group", { name: "Granted capabilities" })).toBeDisabled();
    fireEvent.click(enable);
    fireEvent.click(screen.getByRole("checkbox", { name: "Search History metadata" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "List and run local filters" }));
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(onSaveAutomation).toHaveBeenCalledWith({
        enabled: true,
        historyMetadata: true,
        historyContent: false,
        clipboardWrite: false,
        libraryWrite: false,
        filtersRun: true,
      }),
    );
  });

  it("manages focus, traps tab navigation, and restores the trigger on close", () => {
    render(<SettingsHarness />);

    const trigger = screen.getByRole("button", { name: "Open settings" });
    trigger.focus();
    fireEvent.click(trigger);

    const dialog = screen.getByRole("dialog", { name: "Local clipboard settings" });
    expect(dialog).toHaveFocus();
    expect(dialog).toHaveAttribute("aria-describedby", "capture-settings-description");
    expect(screen.getByText(/ClipRiva needs no account/i)).toHaveAttribute(
      "id",
      "capture-settings-description",
    );

    fireEvent.keyDown(window, { key: "Tab" });
    const closeButton = screen.getByRole("button", { name: "Close settings" });
    expect(closeButton).toHaveFocus();

    fireEvent.keyDown(window, { key: "Tab", shiftKey: true });
    const saveButton = screen.getByRole("button", { name: "Save settings" });
    expect(saveButton).toHaveFocus();

    fireEvent.keyDown(window, { key: "Tab" });
    expect(closeButton).toHaveFocus();
    expect(screen.getByRole("button", { name: "Background action" })).not.toHaveFocus();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("keeps exactly three primary tabs and supports Arrow, Home, and End navigation", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const generalTab = screen.getByRole("tab", { name: /General/i });
    const privacyTab = screen.getByRole("tab", { name: /Privacy/i });
    const shortcutsTab = screen.getByRole("tab", { name: /Shortcuts/i });
    expect(within(screen.getByRole("tablist")).getAllByRole("tab")).toHaveLength(3);
    expect(screen.queryByRole("tab", { name: /About/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: /Advanced/i })).not.toBeInTheDocument();

    expect(generalTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: /General/i })).toBeInTheDocument();
    expect(screen.getByLabelText("Direct Paste permission")).toBeInTheDocument();
    expect(screen.queryByLabelText("Never capture from these apps")).not.toBeInTheDocument();

    generalTab.focus();
    fireEvent.keyDown(generalTab, { key: "ArrowRight" });
    expect(privacyTab).toHaveFocus();
    expect(privacyTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: /Privacy/i })).toBeInTheDocument();
    expect(screen.getByLabelText("Never capture from these apps")).toBeInTheDocument();
    expect(screen.getByText("Local-first by default")).toBeInTheDocument();
    expect(screen.getByText(/no account, cloud sync, telemetry/i)).toBeInTheDocument();
    expect(screen.queryByLabelText("Open Quick Paste with")).not.toBeInTheDocument();

    fireEvent.keyDown(privacyTab, { key: "End" });
    expect(shortcutsTab).toHaveFocus();
    expect(shortcutsTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: /Shortcuts/i })).toBeInTheDocument();
    expect(screen.getByLabelText("Open Quick Paste with")).toBeInTheDocument();

    fireEvent.keyDown(shortcutsTab, { key: "Home" });
    expect(generalTab).toHaveFocus();
    expect(generalTab).toHaveAttribute("aria-selected", "true");

    fireEvent.keyDown(generalTab, { key: "ArrowLeft" });
    expect(shortcutsTab).toHaveFocus();
    expect(shortcutsTab).toHaveAttribute("aria-selected", "true");
  });

  it("keeps Labs collapsed under General while preserving its draft on save", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        isSaving={false}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    );

    const advancedSummary = screen.getByText("Advanced local tools").closest("summary");
    const advancedDetails = advancedSummary?.closest("details");
    expect(advancedSummary).not.toBeNull();
    expect(advancedDetails).not.toHaveAttribute("open");
    expect(screen.getByRole("checkbox", { name: /Enable ClipRiva Labs/i })).not.toBeVisible();

    advancedSummary?.focus();
    expect(advancedSummary).toHaveFocus();
    fireEvent.click(advancedSummary as HTMLElement);
    expect(advancedDetails).toHaveAttribute("open");
    const labsToggle = screen.getByRole("checkbox", { name: /Enable ClipRiva Labs/i });
    expect(labsToggle).toBeVisible();
    fireEvent.click(labsToggle);
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ labsEnabled: true })),
    );
  });

  it("explains the Quick Paste keyboard contract and preserves the privacy draft on save", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        isSaving={false}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Shortcuts/i }));
    expect(
      screen.getByText(/Enter copies and closes · Command-Enter chooses a trusted device/i),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: /General/i }));
    expect(
      screen.getByText(/Accessibility permission only when you first try it/i),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(screen.getByRole("radio", { name: /Default/i })).toBeChecked();
    fireEvent.change(screen.getByLabelText("Never capture from these apps"), {
      target: { value: "1Password\n  Keychain Access  \n" },
    });
    fireEvent.click(screen.getByRole("radio", { name: /Strict/i }));
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(
        expect.objectContaining({
          pasteBehavior: "restore",
          deniedApps: ["1Password", "Keychain Access"],
          sensitiveContentPolicy: "strict",
          sensitivePauseEnabled: true,
        }),
      ),
    );
  });

  it("reflects the saved Strict sensitive-content policy", () => {
    const strictPreferences = {
      ...preferences,
      sensitiveContentPolicy: "strict" as const,
      sensitivePauseEnabled: true,
    };

    render(
      <CaptureSettingsDialog
        isOpen
        preferences={strictPreferences}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(screen.getByRole("radio", { name: /Strict/i })).toBeChecked();
    expect(screen.getByText(/pause capture until you resume it/i)).toBeInTheDocument();
  });

  it("leads each settings section with an understandable local status", () => {
    const pausedPreferences = {
      ...preferences,
      capturePaused: true,
      pauseReason: "Sensitive content was blocked",
      sensitiveContentPolicy: "strict" as const,
      sensitivePauseEnabled: true,
    };

    render(
      <CaptureSettingsDialog
        isOpen
        preferences={pausedPreferences}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const statusSummary = (label: string) =>
      screen
        .getAllByRole("status")
        .find(
          (element) =>
            element.classList.contains("settings-status-summary") &&
            element.textContent?.includes(label),
        );

    fireEvent.click(screen.getByRole("tab", { name: /Shortcuts/i }));
    expect(statusSummary("Quick Paste:")).toHaveTextContent("Quick Paste: ⌘⇧Space");
    expect(statusSummary("Quick Paste:")).toHaveTextContent("Enter copies and closes");

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(statusSummary("Capture:")).toHaveTextContent("Capture: Paused");
    expect(statusSummary("Capture:")).toHaveTextContent("Sensitive content was blocked");

    fireEvent.click(screen.getByRole("tab", { name: /General/i }));
    fireEvent.click(screen.getByText("Advanced local tools").closest("summary") as HTMLElement);
    expect(statusSummary("Labs:")).toHaveTextContent("Labs: Off by default");
    expect(statusSummary("Labs:")).toHaveTextContent("No Actions, summaries, or derived data");
  });

  it("shows real Direct Paste permission state and exposes explicit recheck and settings actions", () => {
    const onRecheckPastePermission = vi.fn();
    const onOpenPastePermissionSettings = vi.fn();
    const { rerender } = render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        isSaving={false}
        pastePermissionStatus="notGranted"
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onRecheckPastePermission={onRecheckPastePermission}
        onOpenPastePermissionSettings={onOpenPastePermissionSettings}
      />,
    );

    expect(screen.getByLabelText("Direct Paste permission")).toHaveTextContent(
      "Direct Paste permission is off",
    );
    fireEvent.click(screen.getByRole("button", { name: "Recheck" }));
    fireEvent.click(screen.getByRole("button", { name: "Open System Settings" }));
    expect(onRecheckPastePermission).toHaveBeenCalledOnce();
    expect(onOpenPastePermissionSettings).toHaveBeenCalledOnce();

    rerender(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        isSaving={false}
        pastePermissionStatus="granted"
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );
    expect(screen.getByLabelText("Direct Paste permission")).toHaveTextContent(
      "Direct Paste is ready",
    );
  });

  it("shows only a safe, local explanation for recent capture events", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        captureStatusEvents={[
          {
            id: "event-1",
            reasonType: "sensitiveContentDefault",
            sourceApp: "Keychain Access",
            occurredAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 86_400_000).toISOString(),
          },
        ]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    const explanations = screen.getByLabelText("Privacy Activity");
    expect(explanations).toHaveTextContent(
      "A suspected sensitive clip was blocked; capture continued",
    );
    expect(explanations).toHaveTextContent("Keychain Access");
    expect(explanations).not.toHaveTextContent("PRIVATE KEY");
  });

  it("keeps Privacy Activity available when there is nothing content-bearing to show", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        captureStatusEvents={[]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    const activity = screen.getByLabelText("Privacy Activity");
    expect(activity).toHaveTextContent("maximum 20 events");
    expect(activity).toHaveTextContent("No recent privacy actions");
  });

  it("turns a Strict protection pause into an explicit resume confirmation without relaxing Strict", async () => {
    const onResumeCapture = vi.fn().mockResolvedValue(undefined);
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={{
          ...preferences,
          capturePaused: true,
          pauseReason: "Sensitive content was blocked",
          sensitiveContentPolicy: "strict",
          sensitivePauseEnabled: true,
        }}
        captureStatusEvents={[
          {
            id: "strict-pause",
            reasonType: "sensitiveContentStrict",
            sourceApp: null,
            occurredAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 86_400_000).toISOString(),
          },
        ]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={onSave}
        onResumeCapture={onResumeCapture}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    const protectionActions = screen.getByLabelText("Capture protection actions");
    expect(protectionActions).toHaveTextContent("Sensitive content protection");
    expect(protectionActions).toHaveTextContent("Capture paused");
    fireEvent.click(within(protectionActions).getByRole("button", { name: "Resume Capture" }));

    const confirmation = screen.getByRole("alertdialog", {
      name: /Resume Capture in Strict mode/i,
    });
    expect(confirmation).toHaveTextContent("Strict Sensitive content protection stays enabled");
    fireEvent.click(within(confirmation).getByRole("button", { name: "Resume Capture" }));

    await waitFor(() => expect(onResumeCapture).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("radio", { name: /Strict/i })).toBeChecked();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("makes an excluded-app explanation lead directly to the Excluded apps control", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        captureStatusEvents={[
          {
            id: "excluded-app",
            reasonType: "excludedApplication",
            sourceApp: "1Password",
            occurredAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 86_400_000).toISOString(),
          },
        ]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    const protectionActions = screen.getByLabelText("Capture protection actions");
    expect(protectionActions).toHaveTextContent("Excluded apps");
    fireEvent.click(
      within(protectionActions).getByRole("button", { name: "Manage Excluded apps" }),
    );
    expect(screen.getByLabelText("Never capture from these apps")).toHaveFocus();
    expect(
      screen.getByText("Excluding an app leaves Items already in History unchanged."),
    ).toBeInTheDocument();
  });

  it("explains a size-boundary skip without showing clipboard content", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        captureStatusEvents={[
          {
            id: "content-too-large",
            reasonType: "contentTooLarge",
            sourceApp: "Notes",
            occurredAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 86_400_000).toISOString(),
          },
        ]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(screen.getAllByText(/exceeded its local size limit/i).length).toBeGreaterThan(0);
    expect(screen.getByText("Size limits")).toBeInTheDocument();
  });

  it("keeps Diagnostics collapsed with local data in Privacy and saves only its opt-in flag", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        localDiagnosticsSummary={{
          enabled: false,
          storedEventCount: 2,
          firstRecovery: { attempts: 1, completedOrDegraded: 1, percent: 100 },
          directPaste: { attempts: 1, completedOrDegraded: 1, percent: 100 },
          recycleBinRestoreCount: 1,
          duplicateGroupExpandCount: 1,
          criticalErrorCategories: [],
        }}
        isSaving={false}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(screen.queryByRole("checkbox", { name: /Collect anonymous/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "View diagnostics" })).not.toBeVisible();

    const diagnosticsSummary = screen.getByText("Local diagnostics & data").closest("summary");
    const diagnosticsDetails = diagnosticsSummary?.closest("details");
    expect(diagnosticsDetails).not.toHaveAttribute("open");
    fireEvent.click(diagnosticsSummary as HTMLElement);
    expect(diagnosticsDetails).toHaveAttribute("open");
    expect(screen.getByRole("button", { name: "View diagnostics" })).toBeVisible();
    expect(screen.getByText(/Application Support\/com.clipriva.desktop/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "View diagnostics" }));
    expect(screen.getByText(/never records clip contents, file paths/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: /Collect anonymous/i }));
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() =>
      expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ diagnosticsEnabled: true })),
    );
  });

  it("previews how many unpinned clips the edited retention rule will remove", () => {
    render(
      <CaptureSettingsDialog
        isOpen
        preferences={preferences}
        retentionPreviewItems={[
          {
            id: "old-unpinned",
            content: "Old local clip",
            kind: "text",
            sourceApp: null,
            createdAt: "2020-01-01T00:00:00.000Z",
            updatedAt: "2020-01-01T00:00:00.000Z",
            isPinned: false,
            copyCount: 0,
          },
          {
            id: "old-pinned",
            content: "Pinned local clip",
            kind: "text",
            sourceApp: null,
            createdAt: "2020-01-01T00:00:00.000Z",
            updatedAt: "2020-01-01T00:00:00.000Z",
            isPinned: true,
            copyCount: 0,
          },
        ]}
        isSaving={false}
        onClose={vi.fn()}
        onSave={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    fireEvent.click(screen.getByRole("tab", { name: /Privacy/i }));
    expect(screen.getByText(/remove an estimated 1 unpinned clip/i)).toHaveTextContent(
      "Pinned clips stay available",
    );
  });
});
