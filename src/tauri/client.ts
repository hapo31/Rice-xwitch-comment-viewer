import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  BouyomiConnectionDiagnostics,
  TwitchAuthPollResult,
  TwitchAuthValidationResult,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";

const fallbackSettings: AppSettings = {
  twitch: {
    clientId: "",
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

export async function speechConnectionDiagnostics(): Promise<BouyomiConnectionDiagnostics> {
  if (!isTauriRuntime) {
    return {
      configuredAddr: `${fallbackSettings.speech.bouyomiHost}:${fallbackSettings.speech.bouyomiPort}`,
      attempted: [
        {
          addr: `${fallbackSettings.speech.bouyomiHost}:${fallbackSettings.speech.bouyomiPort}`,
          status: "failed",
          message: "ブラウザプレビューでは接続診断をスキップします。",
          elapsedMs: 0,
        },
      ],
      recommendation: "Tauri アプリとして起動して診断してください。",
    };
  }

  return invoke<BouyomiConnectionDiagnostics>("speech_connection_diagnostics");
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

export async function twitchStartAuth(): Promise<TwitchDeviceAuthStart> {
  if (!isTauriRuntime) {
    return {
      userCode: "ABCDEFGH",
      verificationUri: "https://www.twitch.tv/activate",
      expiresIn: 1800,
      interval: 5,
    };
  }

  return invoke<TwitchDeviceAuthStart>("twitch_start_auth");
}

export async function twitchPollAuth(): Promise<TwitchAuthPollResult> {
  if (!isTauriRuntime) {
    return {
      status: "pending",
      message: "ブラウザプレビューでは Twitch 認証を完了できません。",
      interval: 5,
    };
  }

  return invoke<TwitchAuthPollResult>("twitch_poll_auth");
}

export async function twitchValidateAuth(): Promise<TwitchAuthValidationResult> {
  if (!isTauriRuntime) {
    return {
      profile: {
        userId: "preview",
        login: "preview",
        clientId: fallbackSettings.twitch.clientId,
        scopes: ["user:read:chat"],
        expiresIn: 3600,
      },
    };
  }

  return invoke<TwitchAuthValidationResult>("twitch_validate_auth");
}

export async function twitchGetStoredAuth(): Promise<TwitchUserProfile | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  const profile = await invoke<TwitchUserProfile | null>("twitch_get_stored_auth");
  return profile ?? undefined;
}

export async function twitchDisconnect(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("twitch_disconnect");
}

export async function appExit(): Promise<void> {
  if (!isTauriRuntime) {
    window.close();
    return;
  }

  return invoke<void>("app_exit");
}
