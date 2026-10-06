import { describe, expect, it, vi } from "vitest";
import { createDomainStores } from "../stores/domainStores";
import {
  dispatchDomainAction,
  subscribeDomainEvents,
  type DomainEventBridge,
} from "./domainOrchestration";
import type {
  AppEventsSnapshot,
  SpeechStateSnapshot,
  SpeechQueueUpdatedEvent,
  SpeechStatusEvent,
  TwitchChatMessageEvent,
  TwitchStatusEvent,
  AppLogEvent,
} from "../types";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const item = {
  id: "q1",
  sourceMessageId: "chat1",
  userDisplayName: "viewer",
  text: "hello",
  status: "queued" as const,
};
function speechSnapshot(revision = 5): SpeechStateSnapshot {
  return {
    revision,
    status: {
      revision: revision - 1,
      status: "paused",
      adapterHealth: "connected",
      occurredAtMs: 1,
    },
    queue: { revision, queuedCount: 1, items: [item], phase: "paused", occurredAtMs: 1 },
  };
}
function chatMessage(
  id: string,
  overrides: Partial<TwitchChatMessageEvent> = {},
): TwitchChatMessageEvent {
  return {
    id,
    platform: "twitch",
    channelId: "channel-7",
    channelLogin: "channel_7",
    userId: "user-1",
    userLogin: "viewer",
    userDisplayName: "Viewer",
    text: id,
    fragments: [],
    badges: [],
    receivedAt: "2026-08-01T00:00:00Z" as TwitchChatMessageEvent["receivedAt"],
    connectionGeneration: 7,
    ...overrides,
  };
}
function connectedChatStatus(generation = 7): TwitchStatusEvent {
  return {
    revision: 10,
    domain: "chat",
    status: "connected",
    occurredAtMs: 1,
    connectionGeneration: generation,
    activeConnection: {
      generation,
      broadcasterUserId: `channel-${generation}`,
      broadcasterLogin: `channel_${generation}`,
    },
  };
}
function setup() {
  const stores = createDomainStores();
  const events = deferred<AppEventsSnapshot>();
  const speech = deferred<SpeechStateSnapshot>();
  const listeners: {
    queue?: (event: SpeechQueueUpdatedEvent) => void;
    speech?: (event: SpeechStatusEvent) => void;
    twitch?: (event: TwitchStatusEvent) => void;
    chat?: (event: TwitchChatMessageEvent) => void;
    log?: (event: AppLogEvent) => void;
  } = {};
  const unlisten = vi.fn();
  const bridge: DomainEventBridge = {
    subscribeAppLogEvents: async (listener) => {
      listeners.log = listener;
      return unlisten;
    },
    subscribeTwitchStatusEvents: async (listener) => {
      listeners.twitch = listener;
      return unlisten;
    },
    subscribeTwitchChatMessageEvents: async (listener) => {
      listeners.chat = listener;
      return unlisten;
    },
    subscribeSpeechStatusEvents: async (listener) => {
      listeners.speech = listener;
      return unlisten;
    },
    subscribeSpeechQueueUpdatedEvents: async (listener) => {
      listeners.queue = listener;
      return unlisten;
    },
    getAppEventsSnapshot: vi.fn(() => events.promise),
    speechQueueReload: vi.fn(() => speech.promise),
  };
  const reportNotification = vi.fn();
  const onRestored = vi.fn();
  const replaySystemLog = vi.fn();
  const start = () =>
    subscribeDomainEvents({ stores, bridge, reportNotification, onRestored, replaySystemLog });
  const resolve = () => {
    events.resolve({ revision: 5, logs: [], twitchStatuses: [], emitErrors: [] });
    speech.resolve(speechSnapshot());
  };
  return {
    stores,
    bridge,
    listeners,
    events,
    speech,
    unlisten,
    reportNotification,
    onRestored,
    replaySystemLog,
    start,
    resolve,
  };
}

describe("backend state replay", () => {
  it("waits for every subscription before querying either snapshot", async () => {
    const h = setup();
    const registration = deferred<() => void>();
    h.bridge.subscribeSpeechQueueUpdatedEvents = () => registration.promise;
    const cleanup = h.start();
    await tick();
    expect(h.bridge.getAppEventsSnapshot).not.toHaveBeenCalled();
    expect(h.bridge.speechQueueReload).not.toHaveBeenCalled();
    registration.resolve(h.unlisten);
    await tick();
    expect(h.bridge.getAppEventsSnapshot).toHaveBeenCalledOnce();
    expect(h.bridge.speechQueueReload).toHaveBeenCalledOnce();
    h.resolve();
    await tick();
    cleanup();
  });
  it("restores a paused queue and adapter health without waiting for a new event", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    h.resolve();
    await tick();
    expect(h.stores.queue.getState()).toMatchObject({
      items: [item],
      phase: "paused",
      revision: 5,
    });
    expect(h.stores.connection.getState()).toMatchObject({
      speechStatus: "paused",
      speechAdapterHealth: "connected",
      speechRevision: 4,
    });
    expect(h.onRestored).toHaveBeenCalledOnce();
    cleanup();
  });
  it("does not overwrite newer queue/status events with an older in-flight snapshot", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    h.listeners.queue!({ revision: 9, queuedCount: 0, items: [], phase: "idle", occurredAtMs: 2 });
    h.listeners.speech!({
      revision: 10,
      status: "error",
      adapterHealth: "disconnected",
      occurredAtMs: 2,
    });
    h.resolve();
    await tick();
    expect(h.stores.queue.getState()).toMatchObject({ items: [], phase: "idle", revision: 9 });
    expect(h.stores.connection.getState()).toMatchObject({
      speechStatus: "error",
      speechAdapterHealth: "disconnected",
      speechRevision: 10,
    });
    cleanup();
  });
  it("reconciles status and queue independently and synchronizes chat on manual reload", () => {
    const h = setup();
    dispatchDomainAction(h.stores, { type: "speech.status", status: "error", revision: 20 });
    dispatchDomainAction(h.stores, { type: "speech.snapshot", snapshot: speechSnapshot() });
    expect(h.stores.connection.getState().speechStatus).toBe("error");
    expect(h.stores.queue.getState().items).toEqual([item]);
    dispatchDomainAction(h.stores, { type: "queue.changed", items: [], revision: 3 });
    expect(h.stores.queue.getState().items).toEqual([item]);
  });
  it("replays startup logs once and keeps newer Twitch domain revisions", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    const live = {
      id: "log-7",
      level: "info" as const,
      message: "already received",
      occurredAtMs: 2,
    };
    h.listeners.log!(live);
    h.listeners.twitch!({ revision: 10, domain: "auth", status: "connected", occurredAtMs: 2 });
    h.events.resolve({
      revision: 7,
      logs: [live, { id: "log-1", level: "info", message: "startup", occurredAtMs: 1 }],
      emitErrors: [],
      twitchStatuses: [
        { revision: 2, domain: "auth", status: "validating", occurredAtMs: 1 },
        { revision: 3, domain: "chat", status: "connected", occurredAtMs: 1 },
      ],
    });
    h.speech.resolve(speechSnapshot());
    await tick();
    expect(h.stores.logs.getState().logs).toHaveLength(2);
    expect(h.replaySystemLog.mock.calls).toEqual([["startup"]]);
    expect(h.stores.connection.getState()).toMatchObject({
      twitchAuthStatus: "authenticated",
      twitchConnectionStatus: "connected",
    });
    cleanup();
  });
  it("buffers chat received before the startup connection snapshot and flushes it once in order", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    h.listeners.chat!(chatMessage("before"));
    h.listeners.chat!(chatMessage("before"));
    expect(h.stores.chat.getState().messages).toEqual([]);

    h.events.resolve({
      revision: 5,
      logs: [],
      emitErrors: [],
      twitchStatuses: [connectedChatStatus()],
    });
    h.speech.resolve(speechSnapshot());
    await tick();
    h.listeners.chat!(chatMessage("after"));

    expect(h.stores.chat.getState().messages.map((message) => message.id)).toEqual([
      "after",
      "before",
    ]);
    cleanup();
  });
  it("rechecks buffered messages against the restored generation and channel identity", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    h.listeners.chat!(chatMessage("old-generation", { connectionGeneration: 6 }));
    h.listeners.chat!(chatMessage("wrong-channel", { channelId: "channel-other" }));
    h.listeners.chat!(chatMessage("valid"));
    h.events.resolve({
      revision: 5,
      logs: [],
      emitErrors: [],
      twitchStatuses: [connectedChatStatus()],
    });
    h.speech.resolve(speechSnapshot());
    await tick();

    expect(h.stores.chat.getState().messages.map((message) => message.id)).toEqual(["valid"]);
    cleanup();
  });
  it("drops startup chat after snapshot failure and cleanup", async () => {
    const failed = setup();
    failed.bridge.getAppEventsSnapshot = async () => {
      throw Error("snapshot unavailable");
    };
    const cleanupFailed = failed.start();
    await tick();
    failed.listeners.chat!(chatMessage("failed-snapshot"));
    failed.speech.resolve(speechSnapshot());
    await tick();
    expect(failed.stores.chat.getState().messages).toEqual([]);
    expect(failed.reportNotification).toHaveBeenCalledOnce();
    cleanupFailed();

    const unmounted = setup();
    const cleanupUnmounted = unmounted.start();
    await tick();
    unmounted.listeners.chat!(chatMessage("unmounted"));
    cleanupUnmounted();
    unmounted.resolve();
    await tick();
    expect(unmounted.stores.chat.getState().messages).toEqual([]);
    expect(unmounted.onRestored).not.toHaveBeenCalled();
  });
  it("bounds startup chat buffering to the newest 200 distinct messages", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    for (let index = 0; index <= 200; index += 1) h.listeners.chat!(chatMessage(`buffer-${index}`));
    h.listeners.chat!(chatMessage("buffer-200"));
    h.events.resolve({
      revision: 5,
      logs: [],
      emitErrors: [],
      twitchStatuses: [connectedChatStatus()],
    });
    h.speech.resolve(speechSnapshot());
    await tick();

    const messages = h.stores.chat.getState().messages;
    expect(messages).toHaveLength(200);
    expect(messages[0]?.id).toBe("buffer-200");
    expect(messages[messages.length - 1]?.id).toBe("buffer-1");
    expect(messages.some((message) => message.id === "buffer-0")).toBe(false);
    cleanup();
  });
  it("restored unvalidated auth is checking, not authenticated", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    h.events.resolve({
      revision: 5,
      logs: [],
      emitErrors: [],
      twitchStatuses: [{ revision: 1, domain: "auth", status: "validating", occurredAtMs: 1 }],
    });
    h.speech.resolve(speechSnapshot());
    await tick();
    expect(h.stores.connection.getState().twitchAuthStatus).toBe("checking");
    cleanup();
  });
  it("does not query snapshots or signal readiness when a listener fails", async () => {
    const h = setup();
    h.bridge.subscribeSpeechStatusEvents = async () => {
      throw Error("listen failed");
    };
    const cleanup = h.start();
    await tick();
    expect(h.bridge.getAppEventsSnapshot).not.toHaveBeenCalled();
    expect(h.onRestored).not.toHaveBeenCalled();
    expect(h.reportNotification).toHaveBeenCalledOnce();
    cleanup();
    expect(h.unlisten).toHaveBeenCalledTimes(4);
  });
  it("ignores snapshot replies after unmount and reports query failures", async () => {
    const h = setup();
    const cleanup = h.start();
    await tick();
    cleanup();
    h.resolve();
    await tick();
    expect(h.stores.queue.getState().items).toEqual([]);
    expect(h.onRestored).not.toHaveBeenCalled();
    const failure = setup();
    failure.bridge.getAppEventsSnapshot = async () => {
      throw Error("query failed");
    };
    const close = failure.start();
    failure.speech.resolve(speechSnapshot());
    await tick();
    expect(failure.reportNotification).toHaveBeenCalledOnce();
    expect(failure.onRestored).not.toHaveBeenCalled();
    close();
  });
});
