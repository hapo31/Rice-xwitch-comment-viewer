import type {
  AppLogEvent,
  AppNotification,
  AppSettings,
  AuthStatus,
  ChatMessage,
  QueueItem,
  SpeechAdapterHealth,
  SpeechQueuePhase,
  SpeechStateSnapshot,
  SpeechStatus,
  TwitchActiveConnection,
  TwitchChatConnectionStatus,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";
import type { LogsState } from "./logsStore";

/** Read-only composite view shape; mutations are owned by the domain stores. */
export interface AppState {
  twitchAuthStatus: AuthStatus;
  twitchConnectionStatus: TwitchChatConnectionStatus;
  twitchAuthPrompt?: TwitchDeviceAuthStart;
  twitchProfile?: TwitchUserProfile;
  twitchActiveConnection?: TwitchActiveConnection;
  twitchConnectionGeneration: number;
  speechStatus: SpeechStatus;
  speechAdapterHealth: SpeechAdapterHealth;
  speechQueuePhase: SpeechQueuePhase;
  settings?: AppSettings;
  chatMessages: ChatMessage[];
  queueItems: QueueItem[];
  logs: LogsState["logs"];
  notifications: AppNotification[];
}

/** Transitional action contract translated once by domainOrchestration. */
export type AppAction =
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "twitch.authStatus"; status: AuthStatus; revision?: number }
  | {
      type: "twitch.connectionStatus";
      status: TwitchChatConnectionStatus;
      revision?: number;
      connectionGeneration?: number;
      activeConnection?: TwitchActiveConnection;
    }
  | { type: "twitch.authPrompt"; prompt?: TwitchDeviceAuthStart }
  | { type: "twitch.profile"; profile?: TwitchUserProfile }
  | {
      type: "speech.status";
      status: SpeechStatus;
      revision?: number;
      adapterHealth?: SpeechAdapterHealth;
    }
  | { type: "speech.snapshot"; snapshot: SpeechStateSnapshot }
  | { type: "chat.message"; message: ChatMessage }
  | { type: "queue.changed"; items: QueueItem[]; revision?: number; phase?: SpeechQueuePhase }
  | { type: "launcher.changed"; items: AppSettings["launcher"]["items"] }
  | { type: "log.added"; log: AppLogEvent }
  | { type: "notification.added"; notification: Omit<AppNotification, "id"> & { id?: string } }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialAppState: AppState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  twitchConnectionGeneration: 0,
  speechStatus: "disconnected",
  speechAdapterHealth: "unknown",
  speechQueuePhase: "idle",
  chatMessages: [],
  queueItems: [],
  logs: [],
  notifications: [],
};
