import type {
  AppSettings,
  AppLogEvent,
  AppNotification,
  AuthStatus,
  ChatMessage,
  QueueItem,
  QueueDisplayState,
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
  notifications: AppNotification[];
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
  | { type: "notification.added"; notification: Omit<AppNotification, "id"> & { id?: string } }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialAppState: AppState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "disconnected",
  chatMessages: [],
  queueItems: [],
  logs: [],
  notifications: [],
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
      return {
        ...state,
        queueItems: action.items,
        chatMessages: syncChatMessageStatuses(state.chatMessages, action.items),
      };
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
    case "notification.added": {
      const notification = {
        ...action.notification,
        id: action.notification.id ?? notificationId(action.notification),
      };
      const duplicateIndex = state.notifications.findIndex(
        (existing) => isDuplicateNotification(existing, notification),
      );
      if (duplicateIndex >= 0) {
        const existing = state.notifications[duplicateIndex];
        if (notificationSeverityRank(notification.severity) <= notificationSeverityRank(existing.severity)) {
          return state;
        }
        const notifications = [...state.notifications];
        notifications[duplicateIndex] = { ...existing, severity: notification.severity };
        return { ...state, notifications };
      }
      return { ...state, notifications: [notification, ...state.notifications].slice(0, 100) };
    }
    case "logs.cleared":
      return { ...state, logs: [] };
    case "warnings.cleared":
      return {
        ...state,
        notifications: state.notifications.filter(
          (notification) => notification.severity !== "warning" && notification.severity !== "error",
        ),
      };
    default:
      return state;
  }
}

export function chatStatusFromQueueStatus(status: QueueDisplayState): Extract<ChatMessage, { kind: "user" }>["status"] {
  return status === "speaking" ? "queued" : status;
}

function syncChatMessageStatuses(messages: ChatMessage[], queueItems: QueueItem[]): ChatMessage[] {
  const statusByMessageId = new Map(
    queueItems.flatMap((item) =>
      item.sourceMessageId ? [[item.sourceMessageId, chatStatusFromQueueStatus(item.status)] as const] : [],
    ),
  );
  let changed = false;
  const updatedMessages = messages.map((message) => {
    if (message.kind !== "user") {
      return message;
    }
    const status = statusByMessageId.get(message.id);
    if (!status || status === message.status) {
      return message;
    }
    changed = true;
    return { ...message, status };
  });

  return changed ? updatedMessages : messages;
}

export function warningNotifications(notifications: AppNotification[]): AppNotification[] {
  return notifications
    .filter((notification) => notification.severity === "warning" || notification.severity === "error")
    .slice(0, 5);
}

function notificationId(notification: Omit<AppNotification, "id">): string {
  return `${notification.occurredAtMs}-${notification.severity}-${notification.source}-${notification.correlationId ?? notification.message}`;
}

function isDuplicateNotification(existing: AppNotification, incoming: AppNotification): boolean {
  if (existing.correlationId && incoming.correlationId) {
    return existing.correlationId === incoming.correlationId;
  }

  return (
    existing.message === incoming.message &&
    Math.abs(existing.occurredAtMs - incoming.occurredAtMs) <= 5_000
  );
}

function notificationSeverityRank(severity: AppNotification["severity"]): number {
  return { info: 0, success: 1, warning: 2, error: 3 }[severity];
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
