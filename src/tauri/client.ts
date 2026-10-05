import { z } from "zod";
import * as schemas from "./schemas";
import { invoke } from "@tauri-apps/api/core";
import { formatBouyomiAddress } from "../validation";
import { normalizeUtcTimestamp, utcNow, type UtcTimestamp } from "../time";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  parseAppLogEvent,
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
  LauncherAddResult,
  LauncherItem,
  LauncherCapabilities,
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
import { createDefaultAppSettings } from "../settings/model";

let previewSettings = createDefaultAppSettings();

const isTauriRuntime = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function authorizeSpeechEndpoint(): Promise<void> {
  if (!isTauriRuntime)
    throw new Error("外部接続の許可にはデスクトップ版のネイティブ確認が必要です。");
  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_authorize_endpoint"),
    "speech_authorize_endpoint",
  );
}

export type AppBuildInfo = z.infer<typeof schemas.appBuildInfoSchema>;

export async function getLauncherCapabilities(): Promise<LauncherCapabilities> {
  if (!isTauriRuntime) {
    return {
      canRegisterApplications: false,
      canLaunchApplications: false,
      reason: "アプリの登録・起動はWindowsのTauriデスクトップ版で利用できます。",
    };
  }
  const capabilities = (await getAppBuildInfo())?.launcher;
  if (
    !capabilities ||
    typeof capabilities.canRegisterApplications !== "boolean" ||
    typeof capabilities.canLaunchApplications !== "boolean" ||
    (capabilities.reason !== undefined && typeof capabilities.reason !== "string")
  ) {
    throw new Error("ランチャーのOS対応状況を確認できません。アプリを再起動してください。");
  }
  return capabilities;
}

export async function getAppBuildInfo(): Promise<AppBuildInfo | undefined> {
  if (!isTauriRuntime) {
    return undefined;
  }

  return schemas.parsePayload(
    schemas.appBuildInfoSchema,
    await invoke<unknown>("app_build_info"),
    "app_build_info",
  );
}

function normalizeSettings(
  settings:
    | (Omit<AppSettingsPatch, "launcher"> & {
        launcher?: Partial<AppSettings["launcher"]>;
        window?: AppSettings["window"];
      })
    | undefined,
): AppSettings {
  const defaults = createDefaultAppSettings();
  return {
    ...defaults,
    ...settings,
    twitch: {
      ...defaults.twitch,
      ...settings?.twitch,
    },
    speech: {
      ...defaults.speech,
      ...settings?.speech,
    },
    launcher: {
      ...defaults.launcher,
      ...settings?.launcher,
      items: settings?.launcher?.items ?? defaults.launcher.items,
    },
  };
}

export async function getSettings(): Promise<AppSettings> {
  if (!isTauriRuntime) {
    return structuredClone(previewSettings);
  }

  return normalizeSettings(
    schemas.parsePayload(
      schemas.appSettingsSchema,
      await invoke<unknown>("settings_get"),
      "settings_get",
    ),
  );
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
    : schemas.parsePayload(
        schemas.settingsRecoveryNoticeSchema,
        notice,
        "settings_take_recovery_notice",
      );
}

export async function updateSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  if (!isTauriRuntime) {
    const items = patch.launcher?.items?.map((edit) => {
      const item = previewSettings.launcher.items.find((item) => item.id === edit.id);
      if (!item) throw new Error("登録済みのアプリだけを編集できます。");
      return { ...item, ...edit };
    });
    previewSettings = structuredClone(
      normalizeSettings({
        ...previewSettings,
        ...patch,
        twitch: { ...previewSettings.twitch, ...patch.twitch },
        speech: { ...previewSettings.speech, ...patch.speech },
        launcher: { items: items ?? previewSettings.launcher.items },
      }),
    );
    return structuredClone(previewSettings);
  }

  return normalizeSettings(
    schemas.parsePayload(
      schemas.appSettingsSchema,
      await invoke<unknown>("settings_update", { patch }),
      "settings_update",
    ),
  );
}

export async function launcherAdd(paths: string[]): Promise<LauncherAddResult> {
  if (!isTauriRuntime) {
    return { items: [], addedCount: 0 };
  }

  return schemas.parsePayload(
    schemas.launcherAddResultSchema,
    await invoke<unknown>("launcher_add", { paths }),
    "launcher_add",
  );
}

export async function launcherRemove(itemId: string): Promise<LauncherItem[]> {
  if (!isTauriRuntime) {
    return [];
  }

  return schemas.parsePayload(
    z.array(schemas.launcherItemSchema),
    await invoke<unknown>("launcher_remove", { itemId }),
    "launcher_remove",
  );
}

export async function launcherLaunch(itemId: string): Promise<LauncherLaunchResult> {
  if (!isTauriRuntime) {
    return { launchedCount: 1, failures: [] };
  }

  return schemas.parsePayload(
    schemas.launcherLaunchResultSchema,
    await invoke<unknown>("launcher_launch", { itemId }),
    "launcher_launch",
  );
}

export async function launcherLaunchAll(): Promise<LauncherLaunchResult> {
  if (!isTauriRuntime) {
    return { launchedCount: 0, failures: [] };
  }

  return schemas.parsePayload(
    schemas.launcherLaunchResultSchema,
    await invoke<unknown>("launcher_launch_all"),
    "launcher_launch_all",
  );
}

export function isDesktopRuntime(): boolean {
  return isTauriRuntime;
}

export async function speechHealthCheck(): Promise<string> {
  if (!isTauriRuntime) {
    return "ブラウザプレビューでは棒読みちゃん接続確認をスキップします。";
  }

  return schemas.parsePayload(
    z.string(),
    await invoke<unknown>("speech_health_check"),
    "speech_health_check",
  );
}

export async function speechHealthProbe(): Promise<string> {
  if (!isTauriRuntime) {
    return "ブラウザプレビューでは棒読みちゃん接続確認をスキップします。";
  }

  return schemas.parsePayload(
    z.string(),
    await invoke<unknown>("speech_health_probe"),
    "speech_health_probe",
  );
}

export async function speechConnectionDiagnostics(): Promise<BouyomiConnectionDiagnostics> {
  if (!isTauriRuntime) {
    return {
      configuredAddr: formatBouyomiAddress(
        previewSettings.speech.bouyomiHost,
        previewSettings.speech.bouyomiPort,
      ),
      attempted: [
        {
          addr: formatBouyomiAddress(
            previewSettings.speech.bouyomiHost,
            previewSettings.speech.bouyomiPort,
          ),
          status: "failed",
          message: "ブラウザプレビューでは接続診断をスキップします。",
          elapsedMs: 0,
        },
      ],
      recommendation: "Tauri アプリとして起動して診断してください。",
    };
  }

  return schemas.parsePayload(
    schemas.bouyomiConnectionDiagnosticsSchema,
    await invoke<unknown>("speech_connection_diagnostics"),
    "speech_connection_diagnostics",
  );
}

export async function speechTest(text: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_test", { text }),
    "speech_test",
  );
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

  schemas.parsePayload(schemas.unitResultSchema, await invoke<unknown>(commandName), commandName);
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

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_queue_remove", { itemId }),
    "speech_queue_remove",
  );
}

export async function speechQueueDismiss(itemId: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_queue_dismiss", { itemId }),
    "speech_queue_dismiss",
  );
}

export async function speechQueueDismissHistory(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_queue_dismiss_history"),
    "speech_queue_dismiss_history",
  );
}

export async function speechQueueRetry(itemId: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("speech_queue_retry", { itemId }),
    "speech_queue_retry",
  );
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

  return schemas.parsePayload(
    schemas.twitchDeviceAuthStartSchema,
    await invoke<unknown>("twitch_start_auth"),
    "twitch_start_auth",
  );
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

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("twitch_connect", { channelLogin }),
    "twitch_connect",
  );
}

export async function twitchStopChat(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("twitch_stop_chat"),
    "twitch_stop_chat",
  );
}

export async function twitchDisconnect(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("twitch_disconnect"),
    "twitch_disconnect",
  );
}

export async function appExit(): Promise<void> {
  if (!isTauriRuntime) {
    window.close();
    return;
  }

  schemas.parsePayload(schemas.unitResultSchema, await invoke<unknown>("app_exit"), "app_exit");
}

export async function appOpenExternalUrl(url: string): Promise<void> {
  if (!isTauriRuntime) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }

  schemas.parsePayload(
    schemas.unitResultSchema,
    await invoke<unknown>("app_open_external_url", { url }),
    "app_open_external_url",
  );
}

export async function subscribeAppLogEvents(
  handler: (payload: AppLogEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("app://log", (event) => handler(parseAppLogEvent(event.payload)));
}

export async function subscribeTwitchStatusEvents(
  handler: (payload: TwitchStatusEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("twitch://status", (event) =>
    handler(parseTwitchStatusEvent(event.payload)),
  );
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

  return listen<unknown>("speech://status", (event) =>
    handler(parseSpeechStatusEvent(event.payload)),
  );
}

export async function subscribeSpeechQueueUpdatedEvents(
  handler: (payload: SpeechQueueUpdatedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime) {
    return () => {};
  }

  return listen<unknown>("speech://queue-updated", (event) =>
    handler(parseSpeechQueueUpdatedEvent(event.payload)),
  );
}
