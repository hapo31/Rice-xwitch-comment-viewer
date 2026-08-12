import { describe, expect, it } from "vitest";
import { initialAppState } from "../stores/appStore";
import { getLiveStatusAnnouncement, toLiveStatusSnapshot, type LiveStatusSnapshot } from "./LiveStatusAnnouncer";

const initial: LiveStatusSnapshot = {
  twitchAuthStatus: "authenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "idle",
};

describe("getLiveStatusAnnouncement", () => {
  it("uses the latest warning notification from the structured notification store", () => {
    const snapshot = toLiveStatusSnapshot({
      ...initialAppState,
      notifications: [
        { id: "error", severity: "error", source: "command", message: "読み上げに失敗しました。", occurredAtMs: 2 },
        { id: "warning", severity: "warning", source: "event", message: "再接続を試行します。", occurredAtMs: 1 },
      ],
    });

    expect(snapshot.latestWarning).toBe("再接続を試行します。");
  });

  it("announces a connection error once at alert priority", () => {
    const connectionError = { ...initial, twitchConnectionStatus: "error" as const };

    expect(getLiveStatusAnnouncement(initial, connectionError)).toEqual({
      message: "Twitch 接続: 接続エラー",
      priority: "alert",
    });
    expect(getLiveStatusAnnouncement(connectionError, connectionError)).toBeUndefined();
  });

  it("uses polite status announcements for warnings and ordinary state changes", () => {
    expect(getLiveStatusAnnouncement(initial, { ...initial, latestWarning: "接続を確認しました。" })).toEqual({
      message: "警告: 接続を確認しました。",
      priority: "status",
    });
    expect(getLiveStatusAnnouncement(initial, { ...initial, twitchConnectionStatus: "connected" })).toEqual({
      message: "Twitch 接続: 受信中",
      priority: "status",
    });
  });

  it("prioritizes an error over a warning from the same update", () => {
    expect(
      getLiveStatusAnnouncement(initial, {
        ...initial,
        twitchConnectionStatus: "error",
        latestWarning: "EventSub への接続に失敗しました。",
      }),
    ).toEqual({ message: "Twitch 接続: 接続エラー", priority: "alert" });
  });

  it("announces authentication expiry once when a chat subscription is revoked", () => {
    const chatRevoked = { ...initial, twitchConnectionStatus: "authRequired" as const };
    const authRevoked = { ...chatRevoked, twitchAuthStatus: "expired" as const };

    expect(getLiveStatusAnnouncement(initial, chatRevoked)).toBeUndefined();
    expect(getLiveStatusAnnouncement(chatRevoked, authRevoked)).toEqual({
      message: "Twitch 認証: 再ログイン必要",
      priority: "alert",
    });
  });
});
