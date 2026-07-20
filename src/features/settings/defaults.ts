import type { AppSettings } from "../../types";

export const defaultTwitchSettings: AppSettings["twitch"] = {
  channelLogin: "",
  autoConnect: false,
  confirmBeforeStopChat: true,
};

export const defaultSpeechSettings: AppSettings["speech"] = {
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
  connectionSuccessSpeechText: "",
};
