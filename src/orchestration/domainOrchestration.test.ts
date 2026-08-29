import { describe, expect, it, vi } from "vitest";
import { createDomainStores } from "../stores/domainStores";
import { restoreStartupAuth, createSettingsMutationOrchestrator, subscribeDomainEvents, type DomainEventBridge } from "./domainOrchestration";

describe("domain orchestration", () => {
  it("routes a Twitch chat event to chat only and cleans up deferred listeners", async () => {
    const stores = createDomainStores();
    let chatListener: ((event: any) => void) | undefined;
    const unlisten = vi.fn();
    const bridge: DomainEventBridge = {
      subscribeAppLogEvents: async () => unlisten,
      subscribeTwitchStatusEvents: async () => unlisten,
      subscribeTwitchChatMessageEvents: async (listener) => { chatListener = listener; return unlisten; },
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
      receivedAt: "2026-08-01T00:00:00Z",
    });
    expect(stores.chat.getState().messages).toHaveLength(1);
    expect(stores.logs.getState().logs).toHaveLength(0);
    cleanup();
    expect(unlisten).toHaveBeenCalled();
  });

  it("serializes settings mutations and publishes the backend result", async () => {
    const resolvers: Array<(value: any) => void> = [];
    const updateSettings = vi.fn((_patch: any): Promise<any> => new Promise((resolve) => resolvers.push(resolve)));
    const loaded: any[] = [];
    const orchestrator = createSettingsMutationOrchestrator({
      updateSettings,
      onSettingsLoaded: (settings) => loaded.push(settings),
      onError: vi.fn(),
    });
    const first = orchestrator.mutate({ twitch: { autoConnect: true } });
    const second = orchestrator.mutate({ twitch: { autoConnect: false } });
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(1);
    const firstResult = { twitch: { autoConnect: true }, speech: {}, launcher: { items: [] } };
    resolvers[0]?.(firstResult);
    await first;
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(updateSettings).toHaveBeenCalledTimes(2);
    const secondResult = { twitch: { autoConnect: false }, speech: {}, launcher: { items: [] } };
    resolvers[1]?.(secondResult);
    await expect(second).resolves.toBe(true);
    expect(loaded).toEqual([firstResult, secondResult]);
  });

  it("keeps startup auth command orchestration dependency-injectable", async () => {
    const report = vi.fn();
    const result = await restoreStartupAuth({
      getStoredAuth: async () => ({ userId: "user-1", login: "viewer", scopes: ["user:read:chat"], expiresIn: 3600 }),
      validateAuth: async () => ({ profile: { userId: "user-1", login: "viewer", scopes: ["user:read:chat"], expiresIn: 3600 } }),
      reportSystemMessage: report,
    });
    expect(result.status).toBe("authenticated");
    expect(report).toHaveBeenCalled();
  });
});
