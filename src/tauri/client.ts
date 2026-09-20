import { invoke } from "@tauri-apps/api/core";
import { formatBouyomiAddress } from "../validation";
import { normalizeUtcTimestamp, utcNow, type UtcTimestamp } from "../time";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  rejectUnexpectedNulls,
  parseAppEventsSnapshot,
  parseSpeechQueueUpdatedEvent,
  parseSpeechStateSnapshot,
  parseSpeechStatusEvent,
  parseTwitchAuthPollResult,
  parseTwitchAuthValidationResult,
  parseTwitchChatMessageWireEvent,
  parseTwitchStatusEvent,
  parseTwitchUserProfile,
  type TwitchChatMessageWireEvent,
} from "./bridge";
import type {
  AppLogEvent,
  AppSettings,
  AppSettingsPatch,
  BouyomiConnectionDiagnostics,
  LauncherItem,
  LauncherLaunchResult,
  SpeechQueueUpdatedEvent,
  SpeechStateSnapshot,
  SpeechStatusEvent,
  AppEventsSnapshot,
  SettingsRecoveryNotice,
  TwitchAuthPollResult,
  TwitchStatusEvent,
  TwitchChatMessageEvent,
  TwitchAuthValidationResult,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";

const fallbackSettings: AppSettings = {
  twitch: {
    channelLogin: "",
    autoConnect: false,
    confirmBeforeStopChat: true,
    liveChatAnnouncements: true,
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
    autoSpeak: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
    blockedUsers: [],
    blockedWords: [],
    urlHandling: "replace",
    readEmotes: false,
    connectionSuccessSpeechEnabled: true,
    connectionSuccessSpeechText: "",
  },
  launcher: {
    items: [],
  },
  window: {},
};

const isTauriRuntime = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function nullFreePayload<T>(payload: unknown, contract: string): T {
  rejectUnexpectedNulls(payload, contract);
  return payload as T;
}

export interface AppBuildInfo {
  version: string;
  isDev: boolean;
  commitHash?: string;
}

export async function getAppBuildInfo(): Promise<AppBuildInfo | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  return nullFreePayload<AppBuildInfo>(await invoke<unknown>("app_build_info"), "app_build_info");
}

function normalizeSettings(settings: Partial<AppSettings> | AppSettingsPatch | undefined): AppSettings {
  return {
    ...fallbackSettings,
    ...settings,
    twitch: {
      ...fallbackSettings.twitch,
      ...settings?.twitch,
    },
    speech: {
      ...fallbackSettings.speech,
      ...settings?.speech,
    },
    launcher: {
      ...fallbackSettings.launcher,
      ...settings?.launcher,
      items: settings?.launcher?.items ?? fallbackSettings.launcher.items,
    },
  };
}

export async function getSettings(): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return fallbackSettings;
  }

  return normalizeSettings(nullFreePayload<Partial<AppSettings>>(await invoke<unknown>("settings_get"), "settings_get"));
}

export async function getAppEventsSnapshot(): Promise<AppEventsSnapshot | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  return parseAppEventsSnapshot(await invoke<unknown>("app_events_snapshot"));
}

export async function takeSettingsRecoveryNotice(): Promise<SettingsRecoveryNotice | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  const notice = await invoke<unknown>("settings_take_recovery_notice");
  return notice === null
    ? undefined
    : nullFreePayload<SettingsRecoveryNotice>(notice, "settings_take_recovery_notice");
}

export async function updateSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return normalizeSettings(patch);
  }

  return normalizeSettings(nullFreePayload<Partial<AppSettings>>(await invoke<unknown>("settings_update", { patch }), "settings_update"));
}

export async function launcherAdd(paths: string[]): Promise<LauncherItem[]> {
  if (!isTauriRuntime) {
    return [];
  }

  return nullFreePayload<LauncherItem[]>(await invoke<unknown>("launcher_add", { paths }), "launcher_add");
}

export async function launcherRemove(itemId: string): Promise<LauncherItem[]> {
  if (!isTauriRuntime) {
    return [];
  }

  return nullFreePayload<LauncherItem[]>(await invoke<unknown>("launcher_remove", { itemId }), "launcher_remove");
}

export async function launcherLaunch(itemId: string): Promise<LauncherLaunchResult> {
  if (!isTauriRuntime) {
    return { launchedCount: 1, failures: [] };
  }

  return nullFreePayload<LauncherLaunchResult>(await invoke<unknown>("launcher_launch", { itemId }), "launcher_launch");
}

export async function launcherLaunchAll(): Promise<LauncherLaunchResult> {
  if (!isTauriRuntime) {
    return { launchedCount: 0, failures: [] };
  }

  return nullFreePayload<LauncherLaunchResult>(await invoke<unknown>("launcher_launch_all"), "launcher_launch_all");
}

export function isDesktopRuntime(): boolean {
  return isTauriRuntime;
}

export async function speechHealthCheck(): Promise<string> {
  if (!isTauriRuntime) {
    return "ブラウザプレビューでは棒読みちゃん接続確認をスキップします。";
  }

  return invoke<string>("speech_health_check");
}

export async function speechHealthProbe(): Promise<string> {
  if (!isTauriRuntime) {
    return "ブラウザプレビューでは棒読みちゃん接続確認をスキップします。";
  }

  return invoke<string>("speech_health_probe");
}

export async function speechConnectionDiagnostics(): Promise<BouyomiConnectionDiagnostics> {
  if (!isTauriRuntime) {
    return {
      configuredAddr: formatBouyomiAddress(fallbackSettings.speech.bouyomiHost, fallbackSettings.speech.bouyomiPort),
      attempted: [
        {
          addr: formatBouyomiAddress(fallbackSettings.speech.bouyomiHost, fallbackSettings.speech.bouyomiPort),
          status: "failed",
          message: "ブラウザプレビューでは接続診断をスキップします。",
          elapsedMs: 0,
        },
      ],
      recommendation: "Tauri アプリとして起動して診断してください。",
    };
  }

  return nullFreePayload<BouyomiConnectionDiagnostics>(await invoke<unknown>("speech_connection_diagnostics"), "speech_connection_diagnostics");
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

export async function speechQueueReload(): Promise<SpeechStateSnapshot | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  return parseSpeechStateSnapshot(await invoke<unknown>("speech_queue_reload"));
}

export async function speechQueueRemove(itemId: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("speech_queue_remove", { itemId });
}

export async function speechQueueDismiss(itemId: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("speech_queue_dismiss", { itemId });
}

export async function speechQueueDismissHistory(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("speech_queue_dismiss_history");
}

export async function speechQueueRetry(itemId: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("speech_queue_retry", { itemId });
}

export async function twitchStartAuth(): Promise<TwitchDeviceAuthStart> {
  if (!isTauriRuntime) {
    return {
      userCode: "ABCDEFGH",
      verificationUri: "https://www.twitch.tv/activate",
      expiresIn: 1800,
      expiresAtMs: Date.now() + 1800 * 1000,
      interval: 5,
    };
  }

  return nullFreePayload<TwitchDeviceAuthStart>(await invoke<unknown>("twitch_start_auth"), "twitch_start_auth");
}

export async function twitchPollAuth(): Promise<TwitchAuthPollResult> {
  if (!isTauriRuntime) {
    return {
      status: "pending",
      message: "ブラウザプレビューでは Twitch 認証を完了できません。",
      interval: 5,
    };
  }

  return parseTwitchAuthPollResult(await invoke<unknown>("twitch_poll_auth"));
}

export async function twitchValidateAuth(): Promise<TwitchAuthValidationResult> {
  if (!isTauriRuntime) {
    return {
      profile: {
        userId: "preview",
        login: "preview",
        scopes: ["user:read:chat"],
        expiresIn: 3600,
      },
    };
  }

  return parseTwitchAuthValidationResult(await invoke<unknown>("twitch_validate_auth"));
}

export async function twitchGetStoredAuth(): Promise<TwitchUserProfile | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  const profile = await invoke<unknown>("twitch_get_stored_auth");
  return profile === null ? undefined : parseTwitchUserProfile(profile);
}

export async function twitchConnect(channelLogin?: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("twitch_connect", { channelLogin });
}

export async function twitchStopChat(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("twitch_stop_chat");
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

export async function appOpenExternalUrl(url: string): Promise<void> {
  if (!isTauriRuntime) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }

  return invoke<void>("app_open_external_url", { url });
}

export async function subscribeAppLogEvents(
  handler: (payload: AppLogEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("app://log", (event) =>
    handler(nullFreePayload<AppLogEvent>(event.payload, "app://log")),
  );
}

export async function subscribeTwitchStatusEvents(
  handler: (payload: TwitchStatusEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("twitch://status", (event) => handler(parseTwitchStatusEvent(event.payload)));
}

export async function subscribeTwitchChatMessageEvents(
  handler: (payload: TwitchChatMessageEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("twitch://chat-message", (event) =>
    handler(normalizeTwitchChatMessageEvent(parseTwitchChatMessageWireEvent(event.payload))),
  );
}

export function normalizeTwitchChatMessageEvent(
  payload: TwitchChatMessageWireEvent,
  fallbackReceivedAt: UtcTimestamp = utcNow(),
): TwitchChatMessageEvent {
  return {
    ...payload,
    receivedAt: normalizeUtcTimestamp(payload.receivedAt, fallbackReceivedAt),
  };
}

export async function subscribeSpeechStatusEvents(
  handler: (payload: SpeechStatusEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("speech://status", (event) => handler(parseSpeechStatusEvent(event.payload)));
}

export async function subscribeSpeechQueueUpdatedEvents(
  handler: (payload: SpeechQueueUpdatedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("speech://queue-updated", (event) => handler(parseSpeechQueueUpdatedEvent(event.payload)));
}
