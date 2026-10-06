import { describe, expect, it, vi } from "vitest";
import { defaultSpeechSettings, defaultTwitchSettings } from "../features/settings/defaults";
import { createDomainStores } from "../stores/domainStores";
import { utcTimestamp } from "../time";
import type { AppSettings } from "../types";
import {
  type DomainEventBridge,
  dispatchDomainAction,
  restoreStartupAuth,
  subscribeDomainEvents,
} from "./domainOrchestration";

import { createSettingsController, type SettingsMutationDependencies } from "./settingsController";

const initializationDependencies = {
  initialGeneration: 0,
  getSettingsRevision: () => 0,
  loadSettings: async () => ({
    twitch: defaultTwitchSettings(),
    speech: defaultSpeechSettings(),
    launcher: { items: [] },
  }),
  takeRecoveryNotice: async () => undefined,
  onInitializationChanged: () => undefined,
  onRecoveryNotice: () => undefined,
  loadErrorMessage: () => "設定を読み込めませんでした。",
};

describe("domain orchestration", () => {
  it("routes a Twitch chat event to chat only and cleans up deferred listeners", async () => {
    const stores = createDomainStores();
    let chatListener:
      | Parameters<DomainEventBridge["subscribeTwitchChatMessageEvents"]>[0]
      | undefined;
    const unlisten = vi.fn();
    const renders = { chat: 0, queue: 0, connection: 0, settings: 0, logs: 0 };
    stores.chat.subscribe(() => renders.chat++);
    stores.queue.subscribe(() => renders.queue++);
    stores.connection.subscribe(() => renders.connection++);
    stores.settings.subscribe(() => renders.settings++);
    stores.logs.subscribe(() => renders.logs++);
    const bridge: DomainEventBridge = {
      subscribeAppLogEvents: async () => unlisten,
      subscribeTwitchStatusEvents: async () => unlisten,
      subscribeTwitchChatMessageEvents: async (listener) => {
        chatListener = listener;
        return unlisten;
      },
      subscribeSpeechStatusEvents: async () => unlisten,
      subscribeSpeechQueueUpdatedEvents: async () => unlisten,
    };
    const cleanup = subscribeDomainEvents({
      stores,
      bridge,
      reportNotification: vi.fn(),
    });
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    chatListener?.({
      id: "message-1",
      platform: "twitch",
      channelId: "channel-1",
      channelLogin: "rice",
      userId: "user-1",
      userLogin: "viewer",
      userDisplayName: "Viewer",
      text: "hello",
      fragments: [],
      badges: [],
      receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
    });
    expect(stores.chat.getState().messages).toHaveLength(1);
    expect(stores.logs.getState().logs).toHaveLength(0);
    expect(renders).toEqual({ chat: 1, queue: 0, connection: 0, settings: 0, logs: 0 });
    cleanup();
    expect(unlisten).toHaveBeenCalled();
  });

  it("drops chat events from an old connection generation", async () => {
    const stores = createDomainStores();
    let chatListener:
      | Parameters<DomainEventBridge["subscribeTwitchChatMessageEvents"]>[0]
      | undefined;
    let statusListener: Parameters<DomainEventBridge["subscribeTwitchStatusEvents"]>[0] | undefined;
    const unlisten = vi.fn();
    const bridge: DomainEventBridge = {
      subscribeAppLogEvents: async () => unlisten,
      subscribeTwitchStatusEvents: async (listener) => {
        statusListener = listener;
        return unlisten;
      },
      subscribeTwitchChatMessageEvents: async (listener) => {
        chatListener = listener;
        return unlisten;
      },
      subscribeSpeechStatusEvents: async () => unlisten,
      subscribeSpeechQueueUpdatedEvents: async () => unlisten,
    };
    const cleanup = subscribeDomainEvents({ stores, bridge, reportNotification: vi.fn() });
    await new Promise((resolve) => setTimeout(resolve, 0));
    statusListener?.({
      revision: 10,
      domain: "chat",
      status: "connected",
      occurredAtMs: 1,
      connectionGeneration: 2,
      activeConnection: {
        generation: 2,
        broadcasterUserId: "channel-b",
        broadcasterLogin: "channel_b",
      },
    });
    const base = {
      platform: "twitch" as const,
      userId: "user-1",
      userLogin: "viewer",
      userDisplayName: "Viewer",
      text: "hello",
      fragments: [],
      badges: [],
      receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
    };
    chatListener?.({
      ...base,
      id: "old",
      channelId: "channel-a",
      channelLogin: "channel_a",
      connectionGeneration: 1,
    });
    chatListener?.({
      ...base,
      id: "current",
      channelId: "channel-b",
      channelLogin: "channel_b",
      connectionGeneration: 2,
    });

    expect(stores.chat.getState().messages.map((message) => message.id)).toEqual(["current"]);
    cleanup();
  });

  it("deduplicates explicit replay IDs and retains independent ID-less logs through the runtime bridge", async () => {
    const stores = createDomainStores();
    let logListener: Parameters<DomainEventBridge["subscribeAppLogEvents"]>[0] | undefined;
    const bridge: DomainEventBridge = {
      subscribeAppLogEvents: async (listener) => {
        logListener = listener;
        return () => {};
      },
      subscribeTwitchStatusEvents: async () => () => {},
      subscribeTwitchChatMessageEvents: async () => () => {},
      subscribeSpeechStatusEvents: async () => () => {},
      subscribeSpeechQueueUpdatedEvents: async () => () => {},
    };
    const cleanup = subscribeDomainEvents({ stores, bridge, reportNotification: vi.fn() });
    await new Promise((resolve) => setTimeout(resolve, 0));
    const replay = {
      id: "backend-event-1",
      level: "warning" as const,
      message: "接続が切れました",
      occurredAtMs: 1,
    };
    logListener?.(replay);
    logListener?.(replay);
    const independent = {
      level: "warning" as const,
      message: "同じ時刻の独立したログ",
      occurredAtMs: 2,
    };
    logListener?.(independent);
    logListener?.(independent);

    expect(stores.logs.getState().logs.map((entry) => entry.id)).toEqual([
      "2-warning-同じ時刻の独立したログ-1",
      "2-warning-同じ時刻の独立したログ",
      "backend-event-1",
    ]);
    cleanup();
  });

  it("preserves queue status mapping and system chat rows through domain stores", () => {
    const stores = createDomainStores();
    const statuses = ["speaking", "spoken", "skipped", "blocked", "error"] as const;
    for (const status of statuses) {
      dispatchDomainAction(stores, {
        type: "chat.message",
        message: {
          kind: "user",
          id: `message-${status}`,
          receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
          userDisplayName: "viewer",
          text: status,
          status: "queued",
        },
      });
    }
    const systemMessage = {
      kind: "system" as const,
      id: "system",
      receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
      userDisplayName: "system" as const,
      text: "Twitch に接続しました",
    };
    dispatchDomainAction(stores, { type: "chat.message", message: systemMessage });
    dispatchDomainAction(stores, {
      type: "queue.changed",
      items: statuses.map((status) => ({
        id: `queue-${status}`,
        sourceMessageId: `message-${status}`,
        userDisplayName: "viewer",
        text: status,
        status,
      })),
    });

    const messages = stores.chat.getState().messages;
    expect(
      messages.filter((message) => message.kind === "user").map((message) => message.status),
    ).toEqual(["error", "blocked", "skipped", "spoken", "queued"]);
    expect(messages[0]).toEqual(systemMessage);
  });

  it("keeps a 200-message chat history and applies a clear snapshot as skipped", () => {
    const stores = createDomainStores();
    for (let index = 0; index < 205; index += 1) {
      dispatchDomainAction(stores, {
        type: "chat.message",
        message: {
          kind: "user",
          id: `message-${index}`,
          receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
          userDisplayName: "viewer",
          text: `message ${index}`,
          status: "queued",
        },
      });
    }
    dispatchDomainAction(stores, {
      type: "queue.changed",
      items: Array.from({ length: 205 }, (_, index) => ({
        id: `queue-${index}`,
        sourceMessageId: `message-${index}`,
        userDisplayName: "viewer",
        text: `message ${index}`,
        status: "skipped" as const,
      })),
    });

    const messages = stores.chat.getState().messages;
    expect(messages).toHaveLength(200);
    expect(
      messages.every((message) => message.kind === "user" && message.status === "skipped"),
    ).toBe(true);
  });

  it("replaces Launcher items through settings domain actions and preserves other settings", () => {
    const stores = createDomainStores();
    const settings = {
      twitch: defaultTwitchSettings(),
      speech: defaultSpeechSettings(),
      launcher: { items: [] },
    };
    dispatchDomainAction(stores, { type: "settings.loaded", settings });
    const items = [
      {
        id: "launcher-1",
        kind: "application" as const,
        target: "C:\\Apps\\Example.exe",
        displayName: "Example",
        order: 0,
      },
    ];
    dispatchDomainAction(stores, { type: "launcher.changed", items });

    expect(stores.settings.getState().settings).toEqual({ ...settings, launcher: { items } });
    expect(stores.settings.getState().settings?.twitch).toBe(settings.twitch);
    expect(stores.settings.getState().settings?.speech).toBe(settings.speech);
  });

  it("serializes settings mutations and publishes the backend result", async () => {
    const resolvers: Array<(value: AppSettings) => void> = [];
    const updateSettings = vi.fn<SettingsMutationDependencies["updateSettings"]>(
      () => new Promise((resolve) => resolvers.push(resolve)),
    );
    const loaded: AppSettings[] = [];
    const orchestrator = createSettingsController({
      ...initializationDependencies,
      updateSettings,
      onSettingsLoaded: (settings) => loaded.push(settings),
      onError: vi.fn(),
    });
    const first = orchestrator.mutate({ twitch: { autoConnect: true } });
    const second = orchestrator.mutate({ twitch: { autoConnect: false } });
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(1);
    const firstResult: AppSettings = {
      twitch: { ...defaultTwitchSettings(), autoConnect: true },
      speech: defaultSpeechSettings(),
      launcher: { items: [] },
    };
    resolvers[0]?.(firstResult);
    await first;
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(2);
    const secondResult: AppSettings = {
      ...firstResult,
      twitch: { ...firstResult.twitch, autoConnect: false },
    };
    resolvers[1]?.(secondResult);
    await expect(second).resolves.toBe(true);
    expect(loaded).toEqual([firstResult, secondResult]);
  });

  it("continues an already queued save after failure and waits for all queued work", async () => {
    let rejectFirst!: (reason: unknown) => void;
    let resolveSecond!: (settings: AppSettings) => void;
    const firstSave = new Promise<AppSettings>((_resolve, reject) => {
      rejectFirst = reject;
    });
    const secondSave = new Promise<AppSettings>((resolve) => {
      resolveSecond = resolve;
    });
    const updateSettings = vi
      .fn<SettingsMutationDependencies["updateSettings"]>()
      .mockReturnValueOnce(firstSave)
      .mockReturnValueOnce(secondSave);
    const errors: unknown[] = [];
    const loaded: AppSettings[] = [];
    const orchestrator = createSettingsController({
      ...initializationDependencies,
      updateSettings,
      onSettingsLoaded: (settings) => loaded.push(settings),
      onError: (error) => errors.push(error),
    });
    const first = orchestrator.mutate({ twitch: { autoConnect: true } });
    const second = orchestrator.mutate({ twitch: { autoConnect: false } });
    let idle = false;
    const idleWait = orchestrator.waitForIdle().then(() => {
      idle = true;
    });

    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(1);
    expect(idle).toBe(false);
    rejectFirst(new Error("save failed"));
    await expect(first).resolves.toBe(false);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(2);
    expect(idle).toBe(false);
    resolveSecond({
      twitch: { ...defaultTwitchSettings(), autoConnect: false },
      speech: defaultSpeechSettings(),
      launcher: { items: [] },
    });
    await expect(second).resolves.toBe(true);
    await idleWait;

    expect(idle).toBe(true);
    expect(errors).toHaveLength(1);
    expect(loaded).toHaveLength(1);
  });

  it("keeps startup auth command orchestration dependency-injectable", async () => {
    const report = vi.fn();
    const result = await restoreStartupAuth({
      getStoredAuth: async () => ({
        userId: "user-1",
        login: "viewer",
        scopes: ["user:read:chat"],
        expiresIn: 3600,
      }),
      validateAuth: async () => ({
        profile: { userId: "user-1", login: "viewer", scopes: ["user:read:chat"], expiresIn: 3600 },
      }),
      reportSystemMessage: report,
    });
    expect(result.status).toBe("authenticated");
    expect(report).toHaveBeenCalled();
  });
});
