import { describe, expect, it } from "vitest";
import { autoConnectTimelineEvent, speechRecoveryTimelineEvent, SystemTimelineRouter, timelineEventFromTwitchStatus } from "./systemTimeline";
import { getChatMessagePresentation } from "./chat";

describe("system timeline routing", () => {
  it("routes EventSub and auth transitions but excludes keepalives", () => {
    expect(timelineEventFromTwitchStatus({ domain: "chat", status: "reconnecting", message: "Twitch EventSub が切断されました。", occurredAtMs: 1 })).toMatchObject({ source: "twitch-connection", transition: "reconnecting" });
    expect(timelineEventFromTwitchStatus({ domain: "auth", status: "authRequired", message: "Twitch 認証が無効です。再ログインしてください。", occurredAtMs: 1 })).toMatchObject({ source: "twitch-auth", transition: "authRequired:Twitch 認証が無効です。再ログインしてください。" });
    expect(timelineEventFromTwitchStatus({ domain: "chat", status: "connected", message: "session_keepalive", occurredAtMs: 1 })).toBeUndefined();
  });

  it("uses the event domain rather than localized message text", () => {
    expect(timelineEventFromTwitchStatus({ domain: "auth", status: "connected", message: "ログイン完了", occurredAtMs: 1 })).toMatchObject({ source: "twitch-auth" });
    expect(timelineEventFromTwitchStatus({ domain: "chat", status: "connected", message: "受信を開始しました", occurredAtMs: 1 })).toMatchObject({ source: "twitch-connection", transition: "connected" });
  });

  it("deduplicates an unchanged transition and records recovery after a change", () => {
    const router = new SystemTimelineRouter();
    const reconnecting = { source: "twitch-connection" as const, transition: "reconnecting", message: "再接続中" };
    expect(router.shouldRecord(reconnecting)).toBe(true);
    expect(router.shouldRecord(reconnecting)).toBe(false);
    expect(router.shouldRecord({ ...reconnecting, transition: "connected" })).toBe(true);
    expect(router.shouldRecord(reconnecting)).toBe(true);
  });

  it("models auto-connect and speech recovery as system events", () => {
    expect(autoConnectTimelineEvent("started", "開始")).toMatchObject({ source: "twitch-connection", transition: "auto-started" });
    expect(speechRecoveryTimelineEvent("棒読みちゃんへ再到達しました。", "idle")).toMatchObject({ source: "speech", transition: "idle" });
  });

  it("gives system messages a distinct Chat presentation", () => {
    expect(getChatMessagePresentation({ kind: "system" })).toEqual({ userNameClassName: "text-amber-300", textClassName: "text-zinc-300" });
  });
});
