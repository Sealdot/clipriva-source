import { describe, expect, it } from "vitest";
import {
  CLIPBOARD_QUEUE_LIMIT,
  clipboardQueueReducer,
  createClipboardQueueSnapshot,
  createClipboardQueueState,
  getCurrentClipboardQueueEntry,
  restoreClipboardQueueSnapshot,
} from "./clipboardQueue";

function enqueue(...itemIds: string[]) {
  return clipboardQueueReducer(createClipboardQueueState(), { type: "enqueueVisible", itemIds });
}

describe("clipboardQueueReducer", () => {
  it("enqueues visible order with stable deduplication and a hard 20-ID cap", () => {
    const itemIds = ["a", "b", "a", "", ...Array.from({ length: 24 }, (_, index) => `id-${index}`)];
    const state = clipboardQueueReducer(createClipboardQueueState(), {
      type: "enqueueVisible",
      itemIds,
    });

    expect(state.order).toHaveLength(CLIPBOARD_QUEUE_LIMIT);
    expect(state.order.map((entry) => entry.itemId).slice(0, 4)).toEqual([
      "a",
      "b",
      "id-0",
      "id-1",
    ]);
    expect(new Set(state.order.map((entry) => entry.itemId))).toHaveLength(CLIPBOARD_QUEUE_LIMIT);
    expect(state.order.every((entry) => entry.availability === "available")).toBe(true);
  });

  it("appends later visible IDs without reordering existing entries", () => {
    const initial = enqueue("b", "a");
    const state = clipboardQueueReducer(initial, {
      type: "enqueueVisible",
      itemIds: ["a", "c", "b", "d"],
    });

    expect(state.order.map((entry) => entry.itemId)).toEqual(["b", "a", "c", "d"]);
  });

  it("advances exactly once for the matching successful activation token", () => {
    const started = clipboardQueueReducer(enqueue("a", "b"), {
      type: "beginActivation",
      token: "activation-1",
    });
    const stale = clipboardQueueReducer(started, {
      type: "activationSucceeded",
      token: "another-token",
    });
    const succeeded = clipboardQueueReducer(stale, {
      type: "activationSucceeded",
      token: "activation-1",
    });
    const duplicate = clipboardQueueReducer(succeeded, {
      type: "activationSucceeded",
      token: "activation-1",
    });

    expect(stale.cursor).toBe(0);
    expect(succeeded.cursor).toBe(1);
    expect(succeeded.busyToken).toBeNull();
    expect(duplicate).toBe(succeeded);
    expect(getCurrentClipboardQueueEntry(duplicate)?.itemId).toBe("b");
  });

  it("keeps the cursor on failure and supports retry with a new token", () => {
    const started = clipboardQueueReducer(enqueue("a", "b"), {
      type: "beginActivation",
      token: "activation-1",
    });
    const failed = clipboardQueueReducer(started, {
      type: "activationFailed",
      token: "activation-1",
    });
    const retrying = clipboardQueueReducer(failed, { type: "retry", token: "activation-2" });

    expect(failed.cursor).toBe(0);
    expect(failed.busyToken).toBeNull();
    expect(retrying.cursor).toBe(0);
    expect(retrying.busyToken).toBe("activation-2");
  });

  it.each(["copyAdvance", "skip"] as const)("supports the %s failure transition", (type) => {
    const state = clipboardQueueReducer(enqueue("a", "b"), { type });
    expect(state.cursor).toBe(1);
    expect(getCurrentClipboardQueueEntry(state)?.itemId).toBe("b");
  });

  it("does not start or manually advance while another activation is busy", () => {
    const started = clipboardQueueReducer(enqueue("a"), {
      type: "beginActivation",
      token: "activation-1",
    });

    expect(clipboardQueueReducer(started, { type: "retry", token: "activation-2" })).toBe(started);
    expect(clipboardQueueReducer(started, { type: "copyAdvance" })).toBe(started);
    expect(clipboardQueueReducer(started, { type: "skip" })).toBe(started);
  });

  it.each(["deleted", "recycled"] as const)("marks a %s item unavailable", (reason) => {
    const state = clipboardQueueReducer(enqueue("a", "b"), {
      type: "itemUnavailable",
      itemId: "b",
      reason,
    });

    expect(state.order[1]).toEqual({ itemId: "b", availability: "unavailable" });
  });

  it("cancels the current busy token when that item becomes unavailable", () => {
    const started = clipboardQueueReducer(enqueue("a", "b"), {
      type: "beginActivation",
      token: "activation-1",
    });
    const unavailable = clipboardQueueReducer(started, {
      type: "itemUnavailable",
      itemId: "a",
      reason: "recycled",
    });
    const staleSuccess = clipboardQueueReducer(unavailable, {
      type: "activationSucceeded",
      token: "activation-1",
    });

    expect(unavailable.busyToken).toBeNull();
    expect(staleSuccess).toBe(unavailable);
    expect(staleSuccess.cursor).toBe(0);
    expect(clipboardQueueReducer(unavailable, { type: "retry", token: "activation-2" })).toBe(
      unavailable,
    );
  });

  it("resets order, cursor, and busy token", () => {
    const started = clipboardQueueReducer(enqueue("a"), {
      type: "beginActivation",
      token: "activation-1",
    });
    expect(clipboardQueueReducer(started, { type: "reset" })).toEqual(createClipboardQueueState());
  });

  it("accepts a bounded ID-only synchronized snapshot while idle", () => {
    const state = clipboardQueueReducer(enqueue("old"), {
      type: "replaceFromSync",
      itemIds: ["b", "a", "b", ...Array.from({ length: 24 }, (_, index) => `id-${index}`)],
      cursor: 2,
    });

    expect(state.order).toHaveLength(CLIPBOARD_QUEUE_LIMIT);
    expect(state.order.slice(0, 3)).toEqual([
      { itemId: "b", availability: "available" },
      { itemId: "a", availability: "available" },
      { itemId: "id-0", availability: "available" },
    ]);
    expect(state.cursor).toBe(2);
    expect(state.busyToken).toBeNull();
  });

  it("does not replace an active token from a synchronized snapshot", () => {
    const started = clipboardQueueReducer(enqueue("a", "b"), {
      type: "beginActivation",
      token: "activation-1",
    });
    const synchronized = clipboardQueueReducer(started, {
      type: "replaceFromSync",
      itemIds: ["remote"],
      cursor: 0,
    });

    expect(synchronized).toBe(started);
    expect(
      clipboardQueueReducer(synchronized, {
        type: "activationSucceeded",
        token: "activation-1",
      }).cursor,
    ).toBe(1);
  });
});

describe("clipboard queue snapshots", () => {
  it("creates a detached JSON-serializable snapshot and restores it", () => {
    const state = clipboardQueueReducer(enqueue("a", "b"), {
      type: "beginActivation",
      token: "activation-1",
    });
    const snapshot = createClipboardQueueSnapshot(state);
    const firstSnapshotEntry = snapshot.order[0];
    if (firstSnapshotEntry) firstSnapshotEntry.availability = "unavailable";

    expect(state.order[0]?.availability).toBe("available");
    const roundTrip = restoreClipboardQueueSnapshot(
      JSON.parse(JSON.stringify(createClipboardQueueSnapshot(state))),
    );
    expect(roundTrip).toEqual(state);
  });

  it("sanitizes untrusted snapshots and clears impossible busy state", () => {
    const restored = restoreClipboardQueueSnapshot({
      version: 1,
      order: [
        { itemId: "a", availability: "available" },
        { itemId: "a", availability: "available" },
        { itemId: "b", availability: "unavailable" },
        { itemId: "", availability: "available" },
        { itemId: "c", availability: "unknown" },
      ],
      cursor: 99,
      busyToken: "stale-token",
    });

    expect(restored).toEqual({
      order: [
        { itemId: "a", availability: "available" },
        { itemId: "b", availability: "unavailable" },
      ],
      cursor: 2,
      busyToken: null,
    });
    expect(restoreClipboardQueueSnapshot(null)).toEqual(createClipboardQueueState());
  });
});
