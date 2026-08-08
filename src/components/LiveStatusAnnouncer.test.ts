import { describe, expect, it } from "vitest";
import { getLiveStatusAnnouncement, type LiveStatusSnapshot } from "./LiveStatusAnnouncer";

const initial: LiveStatusSnapshot = {
  twitchAuthStatus: "authenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "idle",
};

describe("getLiveStatusAnnouncement", () => {
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
});
