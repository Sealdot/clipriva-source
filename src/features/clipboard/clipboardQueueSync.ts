import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { type Dispatch, useCallback, useEffect, useReducer, useRef } from "react";
import { isDesktopRuntime } from "../../lib/runtime";
import {
  CLIPBOARD_QUEUE_LIMIT,
  type ClipboardQueueAction,
  type ClipboardQueueState,
  clipboardQueueReducer,
  createClipboardQueueState,
} from "./clipboardQueue";

const TAURI_QUEUE_SYNC_EVENT = "clipriva://clipboard-queue-sync-v1";
const BROWSER_QUEUE_SYNC_EVENT = "clipriva:clipboard-queue-sync-v1";
const BROWSER_QUEUE_SYNC_CHANNEL = "clipriva-clipboard-queue-sync-v1";

export interface ClipboardQueueSyncSnapshot {
  version: 1;
  itemIds: string[];
  cursor: number;
}

interface ClipboardQueueSyncRequest {
  kind: "request";
  messageId: string;
  sourceId: string;
}

interface ClipboardQueueSyncUpdate {
  kind: "snapshot";
  messageId: string;
  sourceId: string;
  clock: number;
  snapshot: ClipboardQueueSyncSnapshot;
}

type ClipboardQueueSyncMessage = ClipboardQueueSyncRequest | ClipboardQueueSyncUpdate;

interface ClipboardQueueSyncPeerOptions {
  sourceId?: string;
  getState: () => ClipboardQueueState;
  onSnapshot: (snapshot: ClipboardQueueSyncSnapshot) => void;
}

export interface ClipboardQueueSyncPeer {
  publish: (state: ClipboardQueueState) => void;
  request: () => void;
  close: () => void;
}

interface SyncStamp {
  clock: number;
  sourceId: string;
}

interface ClipboardQueueSyncTransport {
  publish: (message: ClipboardQueueSyncMessage) => void;
  close: () => void;
}

/**
 * Produces the complete cross-window payload. It intentionally excludes clip
 * content, availability, activation tokens, failure messages, and timestamps.
 */
export function createClipboardQueueSyncSnapshot(
  state: ClipboardQueueState,
): ClipboardQueueSyncSnapshot {
  return sanitizeClipboardQueueSyncSnapshot({
    version: 1,
    itemIds: state.order.map((entry) => entry.itemId),
    cursor: state.cursor,
  });
}

export function sanitizeClipboardQueueSyncSnapshot(snapshot: unknown): ClipboardQueueSyncSnapshot {
  if (!isRecord(snapshot) || snapshot.version !== 1 || !Array.isArray(snapshot.itemIds)) {
    return { version: 1, itemIds: [], cursor: 0 };
  }

  const itemIds: string[] = [];
  const seen = new Set<string>();
  for (const candidate of snapshot.itemIds) {
    if (itemIds.length >= CLIPBOARD_QUEUE_LIMIT) break;
    if (typeof candidate !== "string" || candidate.trim().length === 0 || seen.has(candidate)) {
      continue;
    }
    seen.add(candidate);
    itemIds.push(candidate);
  }
  const rawCursor =
    typeof snapshot.cursor === "number" && Number.isFinite(snapshot.cursor)
      ? Math.trunc(snapshot.cursor)
      : 0;
  return {
    version: 1,
    itemIds,
    cursor: Math.max(0, Math.min(itemIds.length, rawCursor)),
  };
}

export async function connectClipboardQueueSync(
  options: ClipboardQueueSyncPeerOptions,
): Promise<ClipboardQueueSyncPeer> {
  const sourceId = options.sourceId ?? createSourceId();
  let sequence = 0;
  let clock = 0;
  let acceptedStamp: SyncStamp = { clock: 0, sourceId: "" };
  const seenMessages = new Set<string>();
  let peer: ClipboardQueueSyncPeer | null = null;

  const handleMessage = (candidate: unknown) => {
    const message = parseClipboardQueueSyncMessage(candidate);
    if (!message || message.sourceId === sourceId || seenMessages.has(message.messageId)) return;
    rememberMessage(seenMessages, message.messageId);

    if (message.kind === "request") {
      peer?.publish(options.getState());
      return;
    }

    clock = Math.max(clock, message.clock);
    const incomingStamp = { clock: message.clock, sourceId: message.sourceId };
    if (compareSyncStamp(incomingStamp, acceptedStamp) <= 0) return;
    acceptedStamp = incomingStamp;
    options.onSnapshot(message.snapshot);
  };

  const transport = await createClipboardQueueSyncTransport(handleMessage);
  peer = {
    publish(state) {
      clock += 1;
      acceptedStamp = { clock, sourceId };
      const message: ClipboardQueueSyncUpdate = {
        kind: "snapshot",
        messageId: `${sourceId}:snapshot:${++sequence}`,
        sourceId,
        clock,
        snapshot: createClipboardQueueSyncSnapshot(state),
      };
      transport.publish(message);
    },
    request() {
      transport.publish({
        kind: "request",
        messageId: `${sourceId}:request:${++sequence}`,
        sourceId,
      });
    },
    close() {
      transport.close();
    },
  };
  return peer;
}

/** Shared Queue reducer with an ephemeral cross-window transport. */
export function useSyncedClipboardQueue(): [ClipboardQueueState, Dispatch<ClipboardQueueAction>] {
  const [state, dispatchReducer] = useReducer(
    clipboardQueueReducer,
    undefined,
    createClipboardQueueState,
  );
  const stateRef = useRef(state);
  const peerRef = useRef<ClipboardQueueSyncPeer | null>(null);
  const suppressPublishRef = useRef(false);
  const hasLocalTransitionRef = useRef(false);

  const dispatch = useCallback<Dispatch<ClipboardQueueAction>>((action) => {
    suppressPublishRef.current = false;
    hasLocalTransitionRef.current = true;
    dispatchReducer(action);
  }, []);

  useEffect(() => {
    stateRef.current = state;
    if (suppressPublishRef.current) {
      suppressPublishRef.current = false;
      return;
    }
    peerRef.current?.publish(state);
  }, [state]);

  useEffect(() => {
    let disposed = false;
    let connectedPeer: ClipboardQueueSyncPeer | null = null;
    void connectClipboardQueueSync({
      getState: () => stateRef.current,
      onSnapshot: (snapshot) => {
        if (disposed) return;
        suppressPublishRef.current = true;
        dispatchReducer({
          type: "replaceFromSync",
          itemIds: snapshot.itemIds,
          cursor: snapshot.cursor,
        });
      },
    })
      .then((peer) => {
        if (disposed) {
          peer.close();
          return;
        }
        connectedPeer = peer;
        peerRef.current = peer;
        peer.request();
        if (hasLocalTransitionRef.current) peer.publish(stateRef.current);
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      if (peerRef.current === connectedPeer) peerRef.current = null;
      connectedPeer?.close();
    };
  }, []);

  return [state, dispatch];
}

async function createClipboardQueueSyncTransport(
  onMessage: (message: unknown) => void,
): Promise<ClipboardQueueSyncTransport> {
  if (isDesktopRuntime() && "__TAURI_INTERNALS__" in window) {
    const unlisten: UnlistenFn = await listen<ClipboardQueueSyncMessage>(
      TAURI_QUEUE_SYNC_EVENT,
      (event) => onMessage(event.payload),
    );
    return {
      publish(message) {
        void emit<ClipboardQueueSyncMessage>(TAURI_QUEUE_SYNC_EVENT, message).catch(
          () => undefined,
        );
      },
      close: unlisten,
    };
  }

  const handleWindowMessage = (event: Event) => {
    if (event instanceof CustomEvent) onMessage(event.detail);
  };
  window.addEventListener(BROWSER_QUEUE_SYNC_EVENT, handleWindowMessage);
  const channel =
    typeof BroadcastChannel === "undefined"
      ? null
      : new BroadcastChannel(BROWSER_QUEUE_SYNC_CHANNEL);
  if (channel) channel.onmessage = (event: MessageEvent<unknown>) => onMessage(event.data);

  return {
    publish(message) {
      window.dispatchEvent(new CustomEvent(BROWSER_QUEUE_SYNC_EVENT, { detail: message }));
      channel?.postMessage(message);
    },
    close() {
      window.removeEventListener(BROWSER_QUEUE_SYNC_EVENT, handleWindowMessage);
      channel?.close();
    },
  };
}

function parseClipboardQueueSyncMessage(candidate: unknown): ClipboardQueueSyncMessage | null {
  if (
    !isRecord(candidate) ||
    typeof candidate.messageId !== "string" ||
    candidate.messageId.length === 0 ||
    typeof candidate.sourceId !== "string" ||
    candidate.sourceId.length === 0
  ) {
    return null;
  }
  if (candidate.kind === "request") {
    return {
      kind: "request",
      messageId: candidate.messageId,
      sourceId: candidate.sourceId,
    };
  }
  if (
    candidate.kind !== "snapshot" ||
    typeof candidate.clock !== "number" ||
    !Number.isSafeInteger(candidate.clock) ||
    candidate.clock < 1
  ) {
    return null;
  }
  return {
    kind: "snapshot",
    messageId: candidate.messageId,
    sourceId: candidate.sourceId,
    clock: candidate.clock,
    snapshot: sanitizeClipboardQueueSyncSnapshot(candidate.snapshot),
  };
}

function compareSyncStamp(left: SyncStamp, right: SyncStamp) {
  return left.clock - right.clock || left.sourceId.localeCompare(right.sourceId);
}

function rememberMessage(seenMessages: Set<string>, messageId: string) {
  seenMessages.add(messageId);
  if (seenMessages.size <= 100) return;
  const oldest = seenMessages.values().next().value;
  if (oldest) seenMessages.delete(oldest);
}

function createSourceId() {
  const randomId = globalThis.crypto?.randomUUID?.();
  if (randomId) return `queue-${randomId}`;
  return `queue-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
