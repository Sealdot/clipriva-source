export const CLIPBOARD_QUEUE_LIMIT = 20;

export type ClipboardQueueAvailability = "available" | "unavailable";

export interface ClipboardQueueEntry {
  itemId: string;
  availability: ClipboardQueueAvailability;
}

/**
 * Process-local queue state. It intentionally contains item identifiers and
 * transition metadata only; clipboard content remains owned by the repository.
 */
export interface ClipboardQueueState {
  order: ClipboardQueueEntry[];
  cursor: number;
  busyToken: string | null;
}

export interface ClipboardQueueSnapshot {
  version: 1;
  order: ClipboardQueueEntry[];
  cursor: number;
  busyToken: string | null;
}

export type ClipboardQueueAction =
  | { type: "enqueueVisible"; itemIds: readonly string[] }
  | { type: "replaceFromSync"; itemIds: readonly string[]; cursor: number }
  | { type: "beginActivation"; token: string }
  | { type: "activationSucceeded"; token: string }
  | { type: "activationFailed"; token: string }
  | { type: "retry"; token: string }
  | { type: "copyAdvance" }
  | { type: "skip" }
  | { type: "moveWaiting"; from: number; to: number }
  | { type: "removeWaiting"; index: number }
  | { type: "itemUnavailable"; itemId: string; reason: "deleted" | "recycled" }
  | { type: "reset" };

export const EMPTY_CLIPBOARD_QUEUE: ClipboardQueueState = {
  order: [],
  cursor: 0,
  busyToken: null,
};

export function createClipboardQueueState(): ClipboardQueueState {
  return { ...EMPTY_CLIPBOARD_QUEUE, order: [] };
}

export function clipboardQueueReducer(
  state: ClipboardQueueState,
  action: ClipboardQueueAction,
): ClipboardQueueState {
  switch (action.type) {
    case "enqueueVisible": {
      const existingIds = new Set(state.order.map((entry) => entry.itemId));
      const additions: ClipboardQueueEntry[] = [];
      for (const itemId of action.itemIds) {
        if (
          itemId.trim().length === 0 ||
          existingIds.has(itemId) ||
          state.order.length + additions.length >= CLIPBOARD_QUEUE_LIMIT
        ) {
          continue;
        }
        existingIds.add(itemId);
        additions.push({ itemId, availability: "available" });
      }
      if (additions.length === 0) return state;
      return { ...state, order: [...state.order, ...additions] };
    }

    case "replaceFromSync": {
      // Never invalidate an in-flight activation token. Its terminal event
      // remains the only transition that may advance or clear this state.
      if (state.busyToken !== null) return state;
      const itemIds = uniqueBoundedItemIds(action.itemIds);
      const cursor = Number.isFinite(action.cursor)
        ? Math.max(0, Math.min(itemIds.length, Math.trunc(action.cursor)))
        : 0;
      const order = itemIds.map((itemId) => ({
        itemId,
        availability: "available" as const,
      }));
      if (
        cursor === state.cursor &&
        order.length === state.order.length &&
        order.every((entry, index) => entry.itemId === state.order[index]?.itemId)
      ) {
        return state;
      }
      return { order, cursor, busyToken: null };
    }

    case "beginActivation":
    case "retry": {
      const current = getCurrentClipboardQueueEntry(state);
      if (
        state.busyToken !== null ||
        !current ||
        current.availability === "unavailable" ||
        action.token.trim().length === 0
      ) {
        return state;
      }
      return { ...state, busyToken: action.token };
    }

    case "activationSucceeded":
      if (state.busyToken !== action.token) return state;
      return advance(state);

    case "activationFailed":
      if (state.busyToken !== action.token) return state;
      return { ...state, busyToken: null };

    case "copyAdvance":
    case "skip":
      if (state.busyToken !== null || !getCurrentClipboardQueueEntry(state)) return state;
      return advance(state);

    case "moveWaiting": {
      if (
        state.busyToken !== null ||
        action.from < state.cursor ||
        action.to < state.cursor ||
        action.from >= state.order.length ||
        action.to >= state.order.length ||
        action.from === action.to
      ) {
        return state;
      }
      const order = [...state.order];
      const [moved] = order.splice(action.from, 1);
      if (!moved) return state;
      order.splice(action.to, 0, moved);
      return { ...state, order };
    }

    case "removeWaiting": {
      if (
        state.busyToken !== null ||
        action.index < state.cursor ||
        action.index >= state.order.length
      ) {
        return state;
      }
      return {
        ...state,
        order: state.order.filter((_entry, index) => index !== action.index),
      };
    }

    case "itemUnavailable": {
      const index = state.order.findIndex((entry) => entry.itemId === action.itemId);
      if (index < 0 || state.order[index]?.availability === "unavailable") return state;
      const order = state.order.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, availability: "unavailable" as const } : entry,
      );
      return {
        ...state,
        order,
        busyToken: index === state.cursor ? null : state.busyToken,
      };
    }

    case "reset":
      return createClipboardQueueState();
  }
}

export function getCurrentClipboardQueueEntry(
  state: ClipboardQueueState,
): ClipboardQueueEntry | null {
  return state.order[state.cursor] ?? null;
}

export function createClipboardQueueSnapshot(state: ClipboardQueueState): ClipboardQueueSnapshot {
  return {
    version: 1,
    order: state.order.map((entry) => ({ ...entry })),
    cursor: state.cursor,
    busyToken: state.busyToken,
  };
}

/** Validates an untrusted process snapshot without reading or writing storage. */
export function restoreClipboardQueueSnapshot(snapshot: unknown): ClipboardQueueState {
  if (!isRecord(snapshot) || snapshot.version !== 1 || !Array.isArray(snapshot.order)) {
    return createClipboardQueueState();
  }

  const seen = new Set<string>();
  const order: ClipboardQueueEntry[] = [];
  for (const candidate of snapshot.order) {
    if (order.length >= CLIPBOARD_QUEUE_LIMIT) break;
    if (!isRecord(candidate) || typeof candidate.itemId !== "string") continue;
    if (candidate.itemId.trim().length === 0 || seen.has(candidate.itemId)) continue;
    if (candidate.availability !== "available" && candidate.availability !== "unavailable") {
      continue;
    }
    seen.add(candidate.itemId);
    order.push({
      itemId: candidate.itemId,
      availability: candidate.availability,
    });
  }

  const rawCursor =
    typeof snapshot.cursor === "number" && Number.isFinite(snapshot.cursor)
      ? Math.trunc(snapshot.cursor)
      : 0;
  const cursor = Math.max(0, Math.min(order.length, rawCursor));
  const current = order[cursor];
  const busyToken =
    current?.availability === "available" &&
    typeof snapshot.busyToken === "string" &&
    snapshot.busyToken.trim().length > 0
      ? snapshot.busyToken
      : null;

  return { order, cursor, busyToken };
}

function advance(state: ClipboardQueueState): ClipboardQueueState {
  return {
    ...state,
    cursor: Math.min(state.cursor + 1, state.order.length),
    busyToken: null,
  };
}

function uniqueBoundedItemIds(itemIds: readonly string[]) {
  const seen = new Set<string>();
  const unique: string[] = [];
  for (const itemId of itemIds) {
    if (unique.length >= CLIPBOARD_QUEUE_LIMIT) break;
    if (itemId.trim().length === 0 || seen.has(itemId)) continue;
    seen.add(itemId);
    unique.push(itemId);
  }
  return unique;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
