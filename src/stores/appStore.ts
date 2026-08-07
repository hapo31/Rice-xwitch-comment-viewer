import type {
  AppSettings,
  AppLogEvent,
  AuthStatus,
  ChatMessage,
  QueueItem,
  SpeechStatus,
  TwitchChatConnectionStatus,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";

export type StoredAppLogEvent = AppLogEvent & { id: string };

export interface AppState {
  twitchAuthStatus: AuthStatus;
  twitchConnectionStatus: TwitchChatConnectionStatus;
  twitchAuthPrompt?: TwitchDeviceAuthStart;
  twitchProfile?: TwitchUserProfile;
  speechStatus: SpeechStatus;
  settings?: AppSettings;
  chatMessages: ChatMessage[];
  queueItems: QueueItem[];
  logs: StoredAppLogEvent[];
  warnings: string[];
}

export type AppAction =
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "twitch.authStatus"; status: AuthStatus }
  | { type: "twitch.connectionStatus"; status: TwitchChatConnectionStatus }
  | { type: "twitch.authPrompt"; prompt?: TwitchDeviceAuthStart }
  | { type: "twitch.profile"; profile?: TwitchUserProfile }
  | { type: "speech.status"; status: SpeechStatus }
  | { type: "chat.message"; message: ChatMessage }
  | { type: "queue.changed"; items: QueueItem[] }
  | { type: "launcher.changed"; items: AppSettings["launcher"]["items"] }
  | { type: "log.added"; log: AppLogEvent }
  | { type: "warning.added"; warning: string }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialAppState: AppState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "disconnected",
  chatMessages: [],
  queueItems: [],
  logs: [],
  warnings: [],
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "settings.loaded":
      return { ...state, settings: action.settings };
    case "twitch.authStatus":
      return { ...state, twitchAuthStatus: action.status };
    case "twitch.connectionStatus":
      return { ...state, twitchConnectionStatus: action.status };
    case "twitch.authPrompt":
      return { ...state, twitchAuthPrompt: action.prompt };
    case "twitch.profile":
      return { ...state, twitchProfile: action.profile };
    case "speech.status":
      return { ...state, speechStatus: action.status };
    case "chat.message":
      return {
        ...state,
        chatMessages: [action.message, ...state.chatMessages].slice(0, 200),
      };
    case "queue.changed":
      return { ...state, queueItems: action.items };
    case "launcher.changed":
      return state.settings
        ? {
            ...state,
            settings: {
              ...state.settings,
              launcher: { ...state.settings.launcher, items: action.items },
            },
          }
        : state;
    case "log.added":
      return {
        ...state,
        logs: [
          { ...action.log, id: uniqueLogId(action.log, state.logs) },
          ...state.logs,
        ].slice(0, 500),
      };
    case "warning.added":
      return { ...state, warnings: [action.warning, ...state.warnings].slice(0, 5) };
    case "logs.cleared":
      return { ...state, logs: [] };
    case "warnings.cleared":
      return { ...state, warnings: [] };
    default:
      return state;
  }
}

function uniqueLogId(log: AppLogEvent, existingLogs: StoredAppLogEvent[]): string {
  const baseId = log.id ?? `${log.occurredAtMs}-${log.level}-${log.message}`;
  let id = baseId;
  let suffix = 1;

  while (existingLogs.some((existingLog) => existingLog.id === id)) {
    id = `${baseId}-${suffix}`;
    suffix += 1;
  }

  return id;
}
