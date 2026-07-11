import { describe, expect, it } from "vitest";
import { initialAppState, type AppState } from "../stores/appStore";
import type { AppSettings } from "../types";
import { claimStartupGuideForSession, getStartupGuideMessages } from "./startupGuide";

const receivedAt = "2026-07-11T12:00:00.000Z";
const settings: AppSettings = {
  twitch: { channelLogin: "rice_channel", autoConnect: false, confirmBeforeStopChat: true },
  speech: {
    adapter: "bouyomi",
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    bouyomiSpeed: -1,
    bouyomiTone: -1,
    bouyomiVolume: -1,
    bouyomiVoice: 0,
    readUserName: true,
    autoSpeak: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
    blockedUsers: [],
    blockedWords: [],
    urlHandling: "replace",
    readEmotes: false,
    connectionSuccessSpeechEnabled: true,
    connectionSuccessSpeechText: "棒読みちゃんと接続しました",
  },
};

describe("startup guide messages", () => {
  it("is claimed only once during a session", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    };

    expect(claimStartupGuideForSession(storage)).toBe(true);
    expect(claimStartupGuideForSession(storage)).toBe(false);
  });

  it("guides the user to incomplete setup", () => {
    const messages = getStartupGuideMessages(initialAppState, receivedAt);

    expect(messages.map(({ text }) => text)).toEqual([
      "Twitchと棒読みちゃんの状態を確認しています…",
      "をクリックして、Twitchとの連携を完了しましょう。",
      "をクリックして、読み上げるTwitchチャンネルを設定しましょう。",
      "棒読みちゃんの起動を確認できません。棒読みちゃんを起動すると、チャットの読み上げを始められます。",
    ]);
    expect(messages[1].action).toBe("login");
    expect(messages[2].action).toBe("login");
  });

  it("reports every successful check and readiness", () => {
    const state: AppState = {
      ...initialAppState,
      twitchAuthStatus: "authenticated",
      speechStatus: "idle",
      settings,
    };

    expect(getStartupGuideMessages(state, receivedAt).map(({ text }) => text)).toEqual([
      "Twitchと棒読みちゃんの状態を確認しています…",
      "Twitchとの連携が完了しています。",
      "読み上げチャンネルは「rice_channel」に設定されています。",
      "棒読みちゃんとの接続を確認しました。チャットを読み上げる準備ができています。",
      "準備ができました。チャット受信を開始すると、Twitchのチャットがここに表示されます。",
    ]);
  });
});
