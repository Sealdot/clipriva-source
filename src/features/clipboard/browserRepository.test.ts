import { afterEach, describe, expect, it, vi } from "vitest";
import {
  activateBrowserItem,
  cancelBrowserClipboardImageTextExtraction,
  clearBrowserLocalDiagnostics,
  confirmBrowserLocalTextAction,
  copyBrowserItem,
  createBrowserClipboardSmartCollection,
  createBrowserLocalTextAction,
  deleteBrowserClipboardCollection,
  deleteBrowserClipboardImageText,
  deleteBrowserClipboardItemNote,
  deleteBrowserClipboardSmartCollection,
  deleteBrowserLocalTextAction,
  exportBrowserLocalDiagnostics,
  extractBrowserClipboardImageText,
  getBrowserCapturePreferences,
  getBrowserClipboardFilterOptions,
  getBrowserClipboardImageText,
  getBrowserClipboardItemNote,
  getBrowserLocalDiagnosticsSummary,
  listBrowserActionAudit,
  listBrowserItems,
  listBrowserRecycleBinItems,
  moveBrowserItemsToRecycleBin,
  permanentlyDeleteBrowserRecycleBinItems,
  previewBrowserClipboardCleanup,
  previewBrowserLocalTextAction,
  recordBrowserLocalDiagnosticEvent,
  removeBrowserClipboardSnippet,
  renameBrowserClipboardCollection,
  replaceBrowserClipboardTags,
  restoreBrowserRecycleBinItem,
  saveBrowserClipboardItemNote,
  saveBrowserClipboardSnippet,
  searchBrowserQuickPasteItems,
  setBrowserDirectPasteSimulationForTests,
  setBrowserPastePermissionStatusForTests,
  updateBrowserCapturePreferences,
  updateBrowserLocalTextAction,
} from "./browserRepository";

describe("browser Quick Paste adapter", () => {
  afterEach(async () => {
    setBrowserDirectPasteSimulationForTests("focus");
    setBrowserPastePermissionStatusForTests("granted");
    await clearBrowserLocalDiagnostics();
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, diagnosticsEnabled: false });
  });

  it("previews local Filters without writing and rejects changed rules at confirmation", async () => {
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, labsEnabled: true });
    const action = await createBrowserLocalTextAction({
      name: "Preview contract",
      transform: "uppercase",
      shortcutSlot: null,
    });
    try {
      vi.mocked(navigator.clipboard.writeText).mockClear();
      const auditCount = (await listBrowserActionAudit("demo-code", 100)).length;
      const preview = await previewBrowserLocalTextAction(action.id, "demo-code");
      expect(preview.output).toBe(preview.input.toUpperCase());
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
      expect(await listBrowserActionAudit("demo-code", 100)).toHaveLength(auditCount);

      await updateBrowserLocalTextAction(action.id, {
        name: "Changed rule",
        transform: "trim_whitespace",
        shortcutSlot: null,
      });
      await expect(confirmBrowserLocalTextAction(preview)).rejects.toThrow(
        "Filter preview is out of date",
      );
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
      expect(await listBrowserActionAudit("demo-code", 100)).toHaveLength(auditCount);

      const refreshed = await previewBrowserLocalTextAction(action.id, "demo-code");
      await confirmBrowserLocalTextAction(refreshed);
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(refreshed.output);
      expect(await listBrowserActionAudit("demo-code", 100)).toHaveLength(auditCount + 1);
    } finally {
      await deleteBrowserLocalTextAction(action.id);
      await updateBrowserCapturePreferences({ ...preferences, labsEnabled: false });
    }
  });

  it("uses exact, prefix, and contains tiers with file-name evidence", async () => {
    const results = await searchBrowserQuickPasteItems({
      query: "  CLIPRIVA\n",
      pinnedOnly: false,
      limit: 8,
    });

    expect(results[0]?.item.id).toBe("demo-text");
    expect(results[0]?.matchKind).toBe("prefix");
    expect(results.find((result) => result.item.id === "demo-file")).toMatchObject({
      matchKind: "prefix",
      matchField: "displayName",
      highlightRanges: [expect.objectContaining({ start: expect.any(Number) })],
    });
  });

  it("returns bounded zero-input local suggestions", async () => {
    const results = await searchBrowserQuickPasteItems({
      query: "   ",
      pinnedOnly: false,
      limit: 3,
    });

    expect(results).toHaveLength(3);
    expect(results.every((result) => result.matchKind === "suggestion")).toBe(true);
  });

  it("treats URL and code punctuation as literal Quick Paste search text", async () => {
    expect(
      (await searchBrowserQuickPasteItems({ query: "{", pinnedOnly: false }))[0]?.item.id,
    ).toBe("demo-code");
    expect(
      (await searchBrowserQuickPasteItems({ query: "/plugin/", pinnedOnly: false }))[0]?.item.id,
    ).toBe("demo-url");
  });

  it("finds both long and short Chinese substrings", async () => {
    for (const query of ["版本规划", "规划"]) {
      expect((await searchBrowserQuickPasteItems({ query, pinnedOnly: false }))[0]?.item.id).toBe(
        "demo-chinese",
      );
    }
  });

  it("records last-used activity and ranks it ahead of older pinned clips", async () => {
    const used = await copyBrowserItem("demo-url");
    expect(used).toMatchObject({
      id: "demo-url",
      isPinned: false,
      lastUsedAt: expect.any(String),
    });
    expect(used.updatedAt).toBe(used.lastUsedAt);

    const results = await searchBrowserQuickPasteItems({
      query: "",
      pinnedOnly: false,
      limit: 3,
    });
    expect(results[0]?.item).toMatchObject({ id: "demo-url", isPinned: false });
  });

  it("normalizes bounded local tags and searches them without changing clip content", async () => {
    const original = (await listBrowserItems({ query: "", pinnedOnly: false })).find(
      (item) => item.id === "demo-text",
    );
    const tagged = await replaceBrowserClipboardTags("demo-text", [
      "  Release   Train ",
      "release train",
      "Triage",
    ]);
    expect(tagged.tags).toEqual(["Release Train", "Triage"]);
    expect(tagged.content).toBe(original?.content);

    const historyMatches = await listBrowserItems({ query: "triage", pinnedOnly: false });
    expect(historyMatches.map((item) => item.id)).toContain("demo-text");
    const quickMatches = await searchBrowserQuickPasteItems({
      query: "release train",
      pinnedOnly: false,
      limit: 8,
    });
    expect(quickMatches[0]).toMatchObject({
      item: { id: "demo-text" },
      matchField: "tag",
      matchKind: "exact",
    });
    await expect(
      replaceBrowserClipboardTags(
        "demo-text",
        Array.from({ length: 9 }, (_, index) => `tag-${index}`),
      ),
    ).rejects.toThrow(/at most 8/i);
    await replaceBrowserClipboardTags("demo-text", []);
  });

  it("combines structured History filters before paging and exposes complete facets", async () => {
    await replaceBrowserClipboardTags("demo-command", ["Release"]);
    const results = await listBrowserItems({
      query: "pnpm",
      pinnedOnly: false,
      limit: 1,
      filters: {
        kinds: ["command"],
        sourceApp: "terminal",
        collection: "release",
        pinFilter: "unpinned",
        timeFilter: "past24Hours",
        recentlyUsedOnly: true,
      },
    });
    expect(results).toHaveLength(1);
    expect(results[0]).toMatchObject({ id: "demo-command", tags: ["Release"] });

    const options = await getBrowserClipboardFilterOptions();
    expect(options.sourceApps).toContain("Terminal");
    expect(options.totalCount).toBeGreaterThan(7);
    expect(options.collections).toContainEqual({
      id: null,
      name: "Release",
      kind: "manual",
      rule: null,
      itemCount: 1,
      savedItemCount: 0,
    });
    await replaceBrowserClipboardTags("demo-command", []);
  });

  it("keeps synthetic image extraction manual, searchable, marked, and deletable", async () => {
    expect(await getBrowserClipboardImageText("demo-image")).toBeNull();
    const cancelledPromise = extractBrowserClipboardImageText(
      "demo-image",
      "browser-ocr-request-0001",
    );
    await expect(
      extractBrowserClipboardImageText("demo-image", "browser-ocr-request-0002"),
    ).rejects.toThrow(/already running/i);
    expect(await cancelBrowserClipboardImageTextExtraction("browser-ocr-request-0001")).toBe(true);
    await expect(cancelledPromise).resolves.toEqual({ status: "cancelled", extraction: null });
    expect(await getBrowserClipboardImageText("demo-image")).toBeNull();

    const result = await extractBrowserClipboardImageText("demo-image", "browser-ocr-request-0003");
    expect(result).toMatchObject({
      status: "extracted",
      extraction: {
        clipboardItemId: "demo-image",
        text: "Release dashboard synthetic browser preview",
      },
    });

    const matches = await listBrowserItems({ query: "synthetic browser", pinnedOnly: false });
    expect(matches).toEqual([
      expect.objectContaining({ id: "demo-image", textInImageMatch: true }),
    ]);
    const quick = await searchBrowserQuickPasteItems({
      query: "synthetic browser",
      pinnedOnly: false,
    });
    expect(quick).toEqual([
      expect.objectContaining({
        item: expect.objectContaining({ id: "demo-image" }),
        matchField: "imageText",
      }),
    ]);
    expect(JSON.stringify(quick)).not.toContain("Release dashboard synthetic browser preview");
    expect(await deleteBrowserClipboardImageText("demo-image")).toBe(true);
    expect(await getBrowserClipboardImageText("demo-image")).toBeNull();
    expect(await listBrowserItems({ query: "synthetic browser", pinnedOnly: false })).toEqual([]);
    expect(
      await searchBrowserQuickPasteItems({ query: "synthetic browser", pinnedOnly: false }),
    ).toEqual([]);
  });

  it("uses pins and tags as Saved snippets and mergeable local Collections", async () => {
    const codeWasPinned = (await listBrowserItems({ query: "", pinnedOnly: false })).find(
      (item) => item.id === "demo-code",
    )?.isPinned;
    await saveBrowserClipboardSnippet("demo-code", ["Work", "Inbox"]);
    await saveBrowserClipboardSnippet("demo-text", ["inbox"]);

    const renamed = await renameBrowserClipboardCollection("Inbox", "Work");
    expect(renamed.affectedItemCount).toBe(2);
    expect(
      (await listBrowserItems({ query: "", pinnedOnly: false })).find(
        (item) => item.id === "demo-code",
      )?.tags,
    ).toEqual(["Work"]);

    const deleted = await deleteBrowserClipboardCollection("work");
    expect(deleted.affectedItemCount).toBe(2);
    const stillSaved = (await listBrowserItems({ query: "", pinnedOnly: false })).find(
      (item) => item.id === "demo-text",
    );
    expect(stillSaved).toMatchObject({ isPinned: true, tags: [] });

    const removed = await removeBrowserClipboardSnippet("demo-text");
    expect(removed.isPinned).toBe(false);
    expect(removed.content).toContain("ClipRiva keeps searchable");
    await saveBrowserClipboardSnippet("demo-text", []);
    await replaceBrowserClipboardTags("demo-code", []);
    if (!codeWasPinned) await removeBrowserClipboardSnippet("demo-code");
  });

  it("keeps Notes searchable without adding plaintext to list items and evaluates Smart rules", async () => {
    const note = await saveBrowserClipboardItemNote(
      "demo-code",
      "Quarterly release owner checklist",
    );
    expect(note.status).toBe("saved");
    const history = await listBrowserItems({ query: "release owner", pinnedOnly: false });
    expect(history).toEqual([expect.objectContaining({ id: "demo-code", noteMatch: true })]);
    expect(JSON.stringify(history)).not.toContain("Quarterly release owner checklist");
    expect(
      await searchBrowserQuickPasteItems({ query: "owner checklist", pinnedOnly: false }),
    ).toEqual([
      expect.objectContaining({
        item: expect.objectContaining({ id: "demo-code" }),
        matchField: "note",
      }),
    ]);

    const smart = await createBrowserClipboardSmartCollection({
      name: "Code only",
      rule: { kinds: ["code"] },
    });
    expect(
      await listBrowserItems({
        query: "",
        pinnedOnly: false,
        filters: { smartCollectionId: smart.id },
      }),
    ).toEqual([expect.objectContaining({ id: "demo-code" })]);

    expect(await deleteBrowserClipboardSmartCollection(smart.id ?? "")).toBe(true);
    expect(await deleteBrowserClipboardItemNote("demo-code")).toBe(true);
    expect(await getBrowserClipboardItemNote("demo-code")).toBeNull();
  });

  it("restores before reporting a bounded, retryable Direct Paste focus failure", async () => {
    vi.mocked(navigator.clipboard.writeText).mockClear();
    const initialHistory = await listBrowserItems({ query: "", pinnedOnly: false });
    const initialCopyCount = initialHistory.find((item) => item.id === "demo-code")?.copyCount ?? 0;

    const result = await activateBrowserItem("demo-code", "directPaste");
    const retryResult = await activateBrowserItem("demo-code", "directPaste");

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      "export function cleanClip(input: string) {\n  return input.trim();\n}",
    );
    expect(navigator.clipboard.writeText).toHaveBeenCalledTimes(2);
    expect(result).toMatchObject({ outcome: "pasteNotSent", failureReason: "focus" });
    expect(retryResult).toMatchObject({ outcome: "pasteNotSent", failureReason: "focus" });
    const retriedHistory = await listBrowserItems({ query: "", pinnedOnly: false });
    expect(retriedHistory).toHaveLength(initialHistory.length);
    expect(retriedHistory.find((item) => item.id === "demo-code")?.copyCount).toBe(
      initialCopyCount + 2,
    );
  });

  it("classifies a missing item and a clipboard write failure before paste dispatch", async () => {
    expect(await activateBrowserItem("missing-item", "directPaste")).toEqual({
      outcome: "invalidItem",
      failureReason: null,
    });
    vi.mocked(navigator.clipboard.writeText).mockRejectedValueOnce(new Error("synthetic failure"));
    expect(await activateBrowserItem("demo-code", "directPaste")).toEqual({
      outcome: "writeFailed",
      failureReason: null,
    });
  });

  it("simulates only explicit direct-paste result categories", async () => {
    setBrowserDirectPasteSimulationForTests("permission");
    expect(await activateBrowserItem("demo-url", "directPaste")).toMatchObject({
      outcome: "pasteNotSent",
      failureReason: "permission",
    });

    setBrowserDirectPasteSimulationForTests("compatibility");
    expect(await activateBrowserItem("demo-url", "directPaste")).toMatchObject({
      outcome: "pasteNotSent",
      failureReason: "compatibility",
    });

    setBrowserDirectPasteSimulationForTests("sent");
    expect(await activateBrowserItem("demo-url", "plainTextPaste")).toMatchObject({
      outcome: "pasteSent",
      failureReason: null,
    });

    expect(await activateBrowserItem("demo-url", "copy")).toMatchObject({
      outcome: "clipboardRestored",
      failureReason: null,
    });
  });

  it("shares a content-free preview between cleanup and recycle-bin recovery", async () => {
    const preview = await previewBrowserClipboardCleanup({
      scope: "selected",
      itemIds: ["demo-code", "demo-url"],
    });

    expect(preview.affectedCount).toBe(1);
    expect(preview.pinnedSkippedCount).toBe(1);
    expect(preview.representativeItems).toEqual([
      expect.objectContaining({ id: "demo-url", kind: "url", sourceApp: "Arc" }),
    ]);
    expect(preview.representativeItems[0]).not.toHaveProperty("content");
    expect(preview.recycleBinRetention.maximumDays).toBe(7);
    expect(preview.ruleConditions.join(" ")).toContain("Pinned clips are skipped");

    const result = await moveBrowserItemsToRecycleBin({
      scope: "selected",
      itemIds: ["demo-url"],
    });
    expect(result.movedToRecycleBinCount).toBe(1);
    const recycled = await listBrowserRecycleBinItems();
    const urlEntry = recycled.find((entry) => entry.item.id === "demo-url");
    expect(urlEntry).toBeDefined();

    const restored = await restoreBrowserRecycleBinItem("demo-url");
    expect(restored.createdAt).toBe(urlEntry?.item.createdAt);
    expect(restored.sourceApp).toBe("Arc");
    expect(restored.isPinned).toBe(false);
    expect(restored.restoredAt).toEqual(expect.any(String));

    await moveBrowserItemsToRecycleBin({ scope: "selected", itemIds: ["demo-url"] });
    expect(await permanentlyDeleteBrowserRecycleBinItems(["demo-url"])).toBe(1);
    expect(
      (await listBrowserRecycleBinItems()).find((entry) => entry.item.id === "demo-url"),
    ).toBeUndefined();
  });

  it("keeps diagnostics opt-in, anonymous, bounded to fixed categories, and clearable", async () => {
    expect(
      await recordBrowserLocalDiagnosticEvent({ eventType: "firstRecovery", outcome: "started" }),
    ).toEqual({ recorded: false });

    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, diagnosticsEnabled: true });
    for (const event of [
      { eventType: "firstRecovery", outcome: "started" },
      { eventType: "firstRecovery", outcome: "completed" },
      { eventType: "directPaste", outcome: "started" },
      { eventType: "directPaste", outcome: "degraded" },
      { eventType: "recycleBinRestore", outcome: "restored" },
      { eventType: "duplicateGroupExpand", outcome: "expanded" },
      { eventType: "clipboardRestoreError", outcome: "failed" },
      { eventType: "systemPolicyRestricted", outcome: "restricted" },
    ] as const) {
      await expect(recordBrowserLocalDiagnosticEvent(event)).resolves.toEqual({ recorded: true });
    }

    const summary = await getBrowserLocalDiagnosticsSummary();
    expect(summary).toMatchObject({
      enabled: true,
      storedEventCount: 8,
      firstRecovery: { attempts: 1, percent: 100 },
      directPaste: { attempts: 1, percent: 100 },
      recycleBinRestoreCount: 1,
      duplicateGroupExpandCount: 1,
    });
    expect(summary.criticalErrorCategories).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ eventType: "clipboardRestoreError", outcome: "failed" }),
        expect.objectContaining({ eventType: "systemPolicyRestricted", outcome: "restricted" }),
      ]),
    );

    const exportData = await exportBrowserLocalDiagnostics();
    expect(exportData.events).toHaveLength(8);
    expect(exportData.events.every((event) => /^\d{4}-\d{2}-\d{2}$/.test(event.occurredDay))).toBe(
      true,
    );
    const serialized = JSON.stringify(exportData);
    for (const prohibited of [
      "content",
      "sourceApp",
      "fileName",
      "documentName",
      "networkId",
      "occurredAt",
    ]) {
      expect(serialized).not.toContain(prohibited);
    }

    await clearBrowserLocalDiagnostics();
    await expect(getBrowserLocalDiagnosticsSummary()).resolves.toMatchObject({
      storedEventCount: 0,
    });
  });

  it("rejects event and result combinations that do not describe a supported flow", async () => {
    const preferences = await getBrowserCapturePreferences();
    await updateBrowserCapturePreferences({ ...preferences, diagnosticsEnabled: true });
    await expect(
      recordBrowserLocalDiagnosticEvent({
        eventType: "firstRecovery",
        outcome: "degraded",
      }),
    ).rejects.toThrow(/not supported/i);
  });
});
