import type { AppSettings } from "../types";

export function createDefaultTwitchSettings(): AppSettings["twitch"] {
  return {
    channelLogin: "",
    autoConnect: false,
    confirmBeforeStopChat: true,
    liveChatAnnouncements: true,
  };
}

export function createDefaultSpeechSettings(): AppSettings["speech"] {
  return {
    adapter: "bouyomi",
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    bouyomiRemoteMode: false,
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
    connectionSuccessSpeechText: "",
  };
}

export function createDefaultAppSettings(): AppSettings {
  return {
    twitch: createDefaultTwitchSettings(),
    speech: createDefaultSpeechSettings(),
    launcher: { items: [] },
    window: {},
  };
}
