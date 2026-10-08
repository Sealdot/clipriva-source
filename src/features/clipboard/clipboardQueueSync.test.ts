import { describe, expect, it, vi } from "vitest";
import { clipboardQueueReducer, createClipboardQueueState } from "./clipboardQueue";
import {
  connectClipboardQueueSync,
  createClipboardQueueSyncSnapshot,
  sanitizeClipboardQueueSyncSnapshot,
} from "./clipboardQueueSync";

function queuedState(...itemIds: string[]) {
  return clipboardQueueReducer(createClipboardQueueState(), {
    type: "enqueueVisible",
    itemIds,
  });
}

describe("clipboard Queue synchronization snapshots", () => {
  it("contains only bounded item IDs and cursor, never busy transition state", () => {
    const busy = clipboardQueueReducer(
      queuedState(...Array.from({ length: 24 }, (_, index) => `item-${index}`)),
      { type: "beginActivation", token: "private-busy-token" },
    );

    const snapshot = createClipboardQueueSyncSnapshot(busy);
    expect(snapshot.itemIds).toHaveLength(20);
    expect(snapshot).toEqual({
      version: 1,
      itemIds: Array.from({ length: 20 }, (_, index) => `item-${index}`),
      cursor: 0,
    });
    expect(JSON.stringify(snapshot)).not.toContain("private-busy-token");
    expect(Object.keys(snapshot).sort()).toEqual(["cursor", "itemIds", "version"]);
  });

  it("sanitizes untrusted IDs, order, cap, and cursor", () => {
    const snapshot = sanitizeClipboardQueueSyncSnapshot({
      version: 1,
      itemIds: ["b", "", "a", "b", 17, ...Array.from({ length: 30 }, (_, i) => `id-${i}`)],
      cursor: 999,
      content: "must not cross windows",
    });

    expect(snapshot.itemIds.slice(0, 4)).toEqual(["b", "a", "id-0", "id-1"]);
    expect(snapshot.itemIds).toHaveLength(20);
    expect(snapshot.cursor).toBe(20);
    expect(sanitizeClipboardQueueSyncSnapshot({ version: 2 })).toEqual({
      version: 1,
      itemIds: [],
      cursor: 0,
    });
  });
});

describe("browser Queue sync peer", () => {
  it("answers late-window requests and synchronizes later state changes", async () => {
    let stateA = queuedState("a", "b");
    let stateB = createClipboardQueueState();
    const onSnapshotA = vi.fn((snapshot) => {
      stateA = clipboardQueueReducer(stateA, {
        type: "replaceFromSync",
        itemIds: snapshot.itemIds,
        cursor: snapshot.cursor,
      });
    });
    const onSnapshotB = vi.fn((snapshot) => {
      stateB = clipboardQueueReducer(stateB, {
        type: "replaceFromSync",
        itemIds: snapshot.itemIds,
        cursor: snapshot.cursor,
      });
    });
    const peerA = await connectClipboardQueueSync({
      sourceId: "main-window",
      getState: () => stateA,
      onSnapshot: onSnapshotA,
    });
    const peerB = await connectClipboardQueueSync({
      sourceId: "quick-paste-window",
      getState: () => stateB,
      onSnapshot: onSnapshotB,
    });

    peerB.request();
    expect(stateB.order.map((entry) => entry.itemId)).toEqual(["a", "b"]);
    expect(onSnapshotB).toHaveBeenCalledOnce();

    stateB = clipboardQueueReducer(stateB, { type: "skip" });
    peerB.publish(stateB);
    expect(stateA.cursor).toBe(1);
    expect(onSnapshotA).toHaveBeenCalledOnce();

    peerA.close();
    peerB.close();
  });

  it("does not echo a published snapshot back to its source", async () => {
    const onSnapshot = vi.fn();
    const state = queuedState("a");
    const peer = await connectClipboardQueueSync({
      sourceId: "only-window",
      getState: () => state,
      onSnapshot,
    });

    peer.publish(state);
    expect(onSnapshot).not.toHaveBeenCalled();
    peer.close();
  });
});
