import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { type Dispatch, useCallback, useEffect, useState } from "react";
import { isDesktopRuntime } from "../../lib/runtime";
import type { ClipboardQueueAction } from "./clipboardQueue";

const STACK_CHANGED_EVENT = "clipriva://clipboard-stack-changed-v1";

export interface ClipboardStackSnapshot {
  version: 1;
  order: Array<{ itemId: string; availability: "available" | "unavailable" }>;
  cursor: number;
  collecting: boolean;
  busy: boolean;
}

interface ClipboardStackReservation {
  token: string;
  itemId: string;
}

interface ClipboardStackEnqueueResult {
  addedCount: number;
  duplicateCount: number;
  limitSkippedCount: number;
  snapshot: ClipboardStackSnapshot;
}

export interface ClipboardStackBridge {
  collecting: boolean;
  setCollecting: (collecting: boolean) => Promise<void>;
  add: (itemIds: readonly string[]) => Promise<ClipboardStackEnqueueResult | null>;
  begin: (expectedItemId: string) => Promise<string>;
  finish: (token: string, succeeded: boolean) => Promise<void>;
  advance: () => Promise<void>;
  move: (from: number, to: number) => Promise<void>;
  remove: (index: number) => Promise<void>;
  markUnavailable: (itemId: string, reason: "deleted" | "recycled") => Promise<void>;
  reset: () => Promise<void>;
}

/**
 * Keeps the existing browser reducer as a development fallback while making
 * the native process-memory Stack authoritative in the desktop application.
 */
export function useClipboardStackBridge(
  dispatch: Dispatch<ClipboardQueueAction>,
): ClipboardStackBridge {
  const desktop = isDesktopRuntime() && "__TAURI_INTERNALS__" in window;
  const [collecting, setCollectingState] = useState(false);

  const applySnapshot = useCallback(
    (snapshot: ClipboardStackSnapshot) => {
      setCollectingState(snapshot.collecting);
      dispatch({
        type: "replaceFromSync",
        itemIds: snapshot.order.map((entry) => entry.itemId),
        cursor: snapshot.cursor,
      });
      for (const entry of snapshot.order) {
        if (entry.availability === "unavailable") {
          dispatch({ type: "itemUnavailable", itemId: entry.itemId, reason: "deleted" });
        }
      }
    },
    [dispatch],
  );

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    void listen<ClipboardStackSnapshot>(STACK_CHANGED_EVENT, (event) => {
      if (!disposed) applySnapshot(event.payload);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    void invoke<ClipboardStackSnapshot>("get_clipboard_stack").then((snapshot) => {
      if (!disposed) applySnapshot(snapshot);
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applySnapshot, desktop]);

  const add = useCallback(
    async (itemIds: readonly string[]) => {
      if (!desktop) {
        dispatch({ type: "enqueueVisible", itemIds });
        return null;
      }
      const result = await invoke<ClipboardStackEnqueueResult>("add_clipboard_items_to_stack", {
        itemIds: [...itemIds],
      });
      applySnapshot(result.snapshot);
      return result;
    },
    [applySnapshot, desktop, dispatch],
  );

  const begin = useCallback(
    async (expectedItemId: string) => {
      if (!desktop) {
        const token = `browser-stack-${Date.now()}-${expectedItemId}`;
        dispatch({ type: "beginActivation", token });
        return token;
      }
      const reservation = await invoke<ClipboardStackReservation>(
        "begin_clipboard_stack_activation",
      );
      if (reservation.itemId !== expectedItemId) {
        await invoke("finish_clipboard_stack_activation", {
          token: reservation.token,
          succeeded: false,
        });
        throw new Error("The Stack changed before activation.");
      }
      dispatch({ type: "beginActivation", token: reservation.token });
      return reservation.token;
    },
    [desktop, dispatch],
  );

  const finish = useCallback(
    async (token: string, succeeded: boolean) => {
      if (desktop) {
        const snapshot = await invoke<ClipboardStackSnapshot>("finish_clipboard_stack_activation", {
          token,
          succeeded,
        });
        applySnapshot(snapshot);
      }
      dispatch({ type: succeeded ? "activationSucceeded" : "activationFailed", token });
    },
    [applySnapshot, desktop, dispatch],
  );

  const advance = useCallback(async () => {
    if (desktop) {
      const snapshot = await invoke<ClipboardStackSnapshot>("advance_clipboard_stack");
      applySnapshot(snapshot);
    }
    dispatch({ type: "skip" });
  }, [applySnapshot, desktop, dispatch]);

  const move = useCallback(
    async (from: number, to: number) => {
      if (!desktop) {
        dispatch({ type: "moveWaiting", from, to });
        return;
      }
      const snapshot = await invoke<ClipboardStackSnapshot>("move_clipboard_stack_item", {
        from,
        to,
      });
      applySnapshot(snapshot);
    },
    [applySnapshot, desktop, dispatch],
  );

  const remove = useCallback(
    async (index: number) => {
      if (!desktop) {
        dispatch({ type: "removeWaiting", index });
        return;
      }
      const snapshot = await invoke<ClipboardStackSnapshot>("remove_clipboard_stack_item", {
        index,
      });
      applySnapshot(snapshot);
    },
    [applySnapshot, desktop, dispatch],
  );

  const markUnavailable = useCallback(
    async (itemId: string, reason: "deleted" | "recycled") => {
      if (desktop) {
        const snapshot = await invoke<ClipboardStackSnapshot>(
          "mark_clipboard_stack_item_unavailable",
          { itemId },
        );
        applySnapshot(snapshot);
      }
      dispatch({ type: "itemUnavailable", itemId, reason });
    },
    [applySnapshot, desktop, dispatch],
  );

  const reset = useCallback(async () => {
    if (desktop) {
      const snapshot = await invoke<ClipboardStackSnapshot>("reset_clipboard_stack");
      applySnapshot(snapshot);
    }
    dispatch({ type: "reset" });
  }, [applySnapshot, desktop, dispatch]);

  const setCollecting = useCallback(
    async (next: boolean) => {
      if (desktop) {
        const snapshot = await invoke<ClipboardStackSnapshot>("set_clipboard_stack_collecting", {
          collecting: next,
        });
        applySnapshot(snapshot);
      } else {
        setCollectingState(next);
      }
    },
    [applySnapshot, desktop],
  );

  return {
    collecting,
    setCollecting,
    add,
    begin,
    finish,
    advance,
    move,
    remove,
    markUnavailable,
    reset,
  };
}
