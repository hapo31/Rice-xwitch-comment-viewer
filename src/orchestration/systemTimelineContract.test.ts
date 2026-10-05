import { describe, expect, it } from "vitest";
import type { SystemTimelineEvent } from "../models/systemTimeline";
import {
  autoConnectTimelineEvent,
  speechRecoveryTimelineEvent,
  SystemTimelineRouter,
  timelineEventFromTwitchStatus,
} from "../presentation/systemTimeline";
import { createDomainStores } from "../stores/domainStores";
import {
  subscribeDomainEvents,
  type DomainEventBridge,
  type DomainEventSubscriptionOptions,
} from "./domainOrchestration";

describe("system timeline orchestration contract", () => {
  it("retains first replay/auth/speech events and deduplicates each source independently", async () => {
    const stores = createDomainStores();
    const router = new SystemTimelineRouter();
    const recorded: SystemTimelineEvent[] = [];
    let twitchListener: Parameters<DomainEventBridge["subscribeTwitchStatusEvents"]>[0] | undefined;
    let speechListener: Parameters<DomainEventBridge["subscribeSpeechStatusEvents"]>[0] | undefined;
    const auth = {
      domain: "auth",
      status: "validating",
      message: "保存済み認証を確認しています。",
      occurredAtMs: 1,
    } as const;
    const speechFailure = {
      status: "disconnected",
      adapterHealth: "disconnected",
      message: "棒読みちゃんを起動してください。",
      occurredAtMs: 1,
    } as const;
    const routeSystemTimelineEvent = (event: SystemTimelineEvent) => {
      if (router.shouldRecord(event)) recorded.push(event);
    };
    let restored: (() => void) | undefined;
    const ready = new Promise<void>((resolve) => {
      restored = resolve;
    });
    const cleanup = subscribeDomainEvents({
      stores,
      reportNotification: () => undefined,
      routeSystemTimelineEvent,
      twitchTimelineEvent: timelineEventFromTwitchStatus,
      speechRecoveryMessage: speechRecoveryTimelineEvent,
      onRestored: restored,
      bridge: {
        subscribeAppLogEvents: async () => () => undefined,
        subscribeTwitchStatusEvents: async (listener) => {
          twitchListener = listener;
          return () => undefined;
        },
        subscribeTwitchChatMessageEvents: async () => () => undefined,
        subscribeSpeechStatusEvents: async (listener) => {
          speechListener = listener;
          return () => undefined;
        },
        subscribeSpeechQueueUpdatedEvents: async () => () => undefined,
        getAppEventsSnapshot: async () => ({
          revision: 1,
          logs: [],
          twitchStatuses: [auth],
          speechStatus: speechFailure,
          emitErrors: [],
        }),
      },
    });
    try {
      await ready;
      expect(recorded.map((event) => event.source)).toEqual(["twitch-auth", "speech"]);
      twitchListener?.(auth);
      speechListener?.(speechFailure);
      expect(recorded).toHaveLength(2);

      twitchListener?.({ ...auth, status: "connected", message: "認証を復元しました。" });
      twitchListener?.({ ...auth, status: "connected", message: "認証を復元しました。" });
      // The health monitor routes successful recovery through this same router.
      const recovery = speechRecoveryTimelineEvent("棒読みちゃんへ再接続しました。", "idle");
      routeSystemTimelineEvent(recovery);
      routeSystemTimelineEvent(recovery);
      speechListener?.(speechFailure);
      routeSystemTimelineEvent(autoConnectTimelineEvent("started", "自動接続を開始します。"));
      routeSystemTimelineEvent(autoConnectTimelineEvent("started", "自動接続を開始します。"));
      expect(recorded.map((event) => event.transition)).toEqual([
        "validating",
        "disconnected",
        "connected",
        "idle",
        "disconnected",
        "auto-started",
      ]);
    } finally {
      cleanup();
    }
    twitchListener?.({ ...auth, status: "error", message: "終了後" });
    expect(recorded).toHaveLength(6);
  });

  it("rejects missing routing keys and incompatible source/transition pairs at compile time", () => {
    type TwitchCallback = NonNullable<DomainEventSubscriptionOptions["twitchTimelineEvent"]>;
    type SpeechCallback = NonNullable<DomainEventSubscriptionOptions["speechRecoveryMessage"]>;
    // @ts-expect-error A message-only producer cannot satisfy the subscription contract.
    const invalidTwitch: TwitchCallback = () => ({ message: "missing keys" });
    // @ts-expect-error Speech callbacks also retain their routing keys.
    const invalidSpeech: SpeechCallback = () => ({ message: "missing keys" });
    // @ts-expect-error Speech events cannot carry auto-connect transitions.
    const wrongSource: SystemTimelineEvent = {
      source: "speech",
      transition: "auto-started",
      message: "",
    };
    // @ts-expect-error Authentication cannot enter the Chat reconnecting state.
    const wrongAuth: SystemTimelineEvent = {
      source: "twitch-auth",
      transition: "reconnecting",
      message: "",
    };
    // @ts-expect-error Chat cannot enter the Auth validating state.
    const wrongChat: SystemTimelineEvent = {
      source: "twitch-connection",
      transition: "validating",
      message: "",
    };
    const unknownAuth: SystemTimelineEvent = {
      source: "twitch-auth",
      // @ts-expect-error Authentication transitions are known statuses, not arbitrary keys.
      transition: "connected:anything",
      message: "",
    };
    expect([
      invalidTwitch,
      invalidSpeech,
      wrongSource,
      wrongAuth,
      wrongChat,
      unknownAuth,
    ]).toHaveLength(6);
  });
});
