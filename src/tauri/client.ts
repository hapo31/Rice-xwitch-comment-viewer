import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "../types";

const fallbackSettings: AppSettings = {
  twitch: {
    channelLogin: "",
    autoConnect: false,
  },
  speech: {
    adapter: "bouyomi",
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    bouyomiSpeed: -1,
    bouyomiTone: -1,
    bouyomiVolume: -1,
    bouyomiVoice: 0,
    readUserName: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
  },
};

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

export async function getSettings(): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return fallbackSettings;
  }

  return invoke<AppSettings>("settings_get");
}

export async function updateSettings(patch: Partial<AppSettings>): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return {
      ...fallbackSettings,
      ...patch,
      twitch: { ...fallbackSettings.twitch, ...patch.twitch },
      speech: { ...fallbackSettings.speech, ...patch.speech },
    };
  }

  return invoke<AppSettings>("settings_update", { patch });
}

export async function speechHealthCheck(): Promise<string> {
  if (!isTauriRuntime) {
    return "ブラウザプレビューでは棒読みちゃん接続確認をスキップします。";
  }

  return invoke<string>("speech_health_check");
}

export async function speechTest(text: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("speech_test", { text });
}

export async function speechControl(command: "pause" | "resume" | "skip" | "clear"): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  const commandName = {
    pause: "speech_pause",
    resume: "speech_resume",
    skip: "speech_skip",
    clear: "speech_clear",
  }[command];

  return invoke<void>(commandName);
}
