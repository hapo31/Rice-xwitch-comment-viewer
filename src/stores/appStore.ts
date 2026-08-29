import type {
  AppSettings,
  AppLogEvent,
  AppNotification,
  AuthStatus,
  ChatMessage,
  QueueItem,
  QueueDisplayState,
  SpeechStatus,
  SpeechAdapterHealth,
  SpeechQueuePhase,
  SpeechStateSnapshot,
  AppEventsSnapshot,
  TwitchChatConnectionStatus,
  TwitchStatusEvent,
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
  speechAdapterHealth: SpeechAdapterHealth;
  speechQueuePhase: SpeechQueuePhase;
  speechStatusRevision: number;
  speechQueueRevision: number;
  twitchAuthRevision: number;
  twitchConnectionRevision: number;
  eventRevision: number;
  settings?: AppSettings;
  chatMessages: ChatMessage[];
  queueItems: QueueItem[];
  logs: StoredAppLogEvent[];
  notifications: AppNotification[];
}

export type AppAction =
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "twitch.authStatus"; status: AuthStatus; revision?: number }
  | {
      type: "twitch.connectionStatus";
      status: TwitchChatConnectionStatus;
      revision?: number;
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
  | { type: "events.snapshot"; snapshot: AppEventsSnapshot }
  | { type: "chat.message"; message: ChatMessage }
  | {
      type: "queue.changed";
      items: QueueItem[];
      revision?: number;
      phase?: SpeechQueuePhase;
    }
  | { type: "launcher.changed"; items: AppSettings["launcher"]["items"] }
  | { type: "log.added"; log: AppLogEvent }
  | {
      type: "notification.added";
      notification: Omit<AppNotification, "id"> & { id?: string };
    }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialAppState: AppState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "disconnected",
  speechAdapterHealth: "disconnected",
  speechQueuePhase: "idle",
  speechStatusRevision: 0,
  speechQueueRevision: 0,
  twitchAuthRevision: 0,
  twitchConnectionRevision: 0,
  eventRevision: 0,
  chatMessages: [],
  queueItems: [],
  logs: [],
  notifications: [],
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "settings.loaded":
      return { ...state, settings: action.settings };
    case "twitch.authStatus": {
      const revision = action.revision ?? 0;
      if (revision > 0 && revision < state.twitchAuthRevision) return state;
      return {
        ...state,
        twitchAuthStatus: action.status,
        twitchAuthRevision: Math.max(state.twitchAuthRevision, revision),
        eventRevision: Math.max(state.eventRevision, revision),
      };
    }
    case "twitch.connectionStatus": {
      const revision = action.revision ?? 0;
      if (revision > 0 && revision < state.twitchConnectionRevision)
        return state;
      return {
        ...state,
        twitchConnectionStatus: action.status,
        twitchConnectionRevision: Math.max(
          state.twitchConnectionRevision,
          revision,
        ),
        eventRevision: Math.max(state.eventRevision, revision),
      };
    }
    case "twitch.authPrompt":
      return { ...state, twitchAuthPrompt: action.prompt };
    case "twitch.profile":
      return { ...state, twitchProfile: action.profile };
    case "speech.status": {
      const revision = action.revision ?? 0;
      if (revision > 0 && revision < state.speechStatusRevision) return state;
      return {
        ...state,
        speechStatus: action.status,
        speechAdapterHealth: action.adapterHealth ?? state.speechAdapterHealth,
        speechStatusRevision: Math.max(state.speechStatusRevision, revision),
        speechQueuePhase:
          action.status === "paused" ? "paused" : state.speechQueuePhase,
      };
    }
    case "speech.snapshot": {
      const snapshot = action.snapshot;
      const statusRevision = snapshot.status.revision ?? snapshot.revision;
      const queueRevision = snapshot.queue.revision ?? snapshot.revision;
      const nextStatus = statusRevision >= state.speechStatusRevision;
      const nextQueue = queueRevision >= state.speechQueueRevision;
      if (!nextStatus && !nextQueue) return state;
      return {
        ...state,
        speechStatus: nextStatus ? snapshot.status.status : state.speechStatus,
        speechAdapterHealth: nextStatus
          ? (snapshot.status.adapterHealth ?? state.speechAdapterHealth)
          : state.speechAdapterHealth,
        speechStatusRevision: nextStatus
          ? Math.max(state.speechStatusRevision, statusRevision)
          : state.speechStatusRevision,
        queueItems: nextQueue ? snapshot.queue.items : state.queueItems,
        speechQueuePhase: nextQueue
          ? (snapshot.queue.phase ?? inferQueuePhase(snapshot.queue.items))
          : state.speechQueuePhase,
        speechQueueRevision: nextQueue
          ? Math.max(state.speechQueueRevision, queueRevision)
          : state.speechQueueRevision,
        eventRevision: Math.max(state.eventRevision, snapshot.revision),
        chatMessages: nextQueue
          ? syncChatMessageStatuses(state.chatMessages, snapshot.queue.items)
          : state.chatMessages,
      };
    }
    case "events.snapshot": {
      const snapshot = action.snapshot;
      let next = state;
      for (const log of [...snapshot.logs].reverse())
        next = appReducer(next, { type: "log.added", log });
      for (const emitError of [...snapshot.emitErrors].reverse()) {
        next = appReducer(next, {
          type: "log.added",
          log: {
            level: "error",
            message: `イベント送信に失敗しました（${emitError.event}）: ${emitError.error}`,
            occurredAtMs: emitError.occurredAtMs,
          },
        });
      }
      for (const status of snapshot.twitchStatuses)
        next = applyTwitchStatus(next, status, snapshot.revision);
      if (snapshot.speechStatus)
        next = appReducer(next, {
          type: "speech.status",
          status: snapshot.speechStatus.status,
          revision: snapshot.speechStatus.revision ?? snapshot.revision,
          adapterHealth: snapshot.speechStatus.adapterHealth,
        });
      return {
        ...next,
        eventRevision: Math.max(next.eventRevision, snapshot.revision),
      };
    }
    case "chat.message":
      return {
        ...state,
        chatMessages: [
          syncChatMessageStatus(action.message, state.queueItems),
          ...state.chatMessages,
        ].slice(0, 200),
      };
    case "queue.changed": {
      const revision = action.revision ?? 0;
      if (revision > 0 && revision < state.speechQueueRevision) return state;
      return {
        ...state,
        queueItems: action.items,
        speechQueuePhase: action.phase ?? inferQueuePhase(action.items),
        speechQueueRevision: Math.max(state.speechQueueRevision, revision),
        chatMessages: syncChatMessageStatuses(state.chatMessages, action.items),
      };
    }
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
      if (action.log.id && state.logs.some((log) => log.id === action.log.id))
        return state;
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
      const duplicateIndex = state.notifications.findIndex((existing) =>
        isDuplicateNotification(existing, notification),
      );
      if (duplicateIndex >= 0) {
        const existing = state.notifications[duplicateIndex];
        if (
          notificationSeverityRank(notification.severity) <=
          notificationSeverityRank(existing.severity)
        ) {
          return state;
        }
        const notifications = [...state.notifications];
        notifications[duplicateIndex] = {
          ...existing,
          severity: notification.severity,
        };
        return { ...state, notifications };
      }
      return {
        ...state,
        notifications: [notification, ...state.notifications].slice(0, 100),
      };
    }
    case "logs.cleared":
      return { ...state, logs: [] };
    case "warnings.cleared":
      return {
        ...state,
        notifications: state.notifications.filter(
          (notification) =>
            notification.severity !== "warning" &&
            notification.severity !== "error",
        ),
      };
    default:
      return state;
  }
}

function applyTwitchStatus(
  state: AppState,
  event: TwitchStatusEvent,
  snapshotRevision: number,
): AppState {
  const revision = event.revision ?? snapshotRevision;
  if (event.domain === "chat") {
    if (revision > 0 && revision < state.twitchConnectionRevision) return state;
    return appReducer(state, {
      type: "twitch.connectionStatus",
      status: event.status as TwitchChatConnectionStatus,
      revision,
    });
  }
  if (revision > 0 && revision < state.twitchAuthRevision) return state;
  const authStatus: AuthStatus | undefined =
    event.status === "connected"
      ? "authenticated"
      : event.status === "validating"
        ? "checking"
        : event.status === "authRequired"
          ? "expired"
          : event.status === "error"
            ? "error"
            : undefined;
  return authStatus
    ? appReducer(state, {
        type: "twitch.authStatus",
        status: authStatus,
        revision,
      })
    : state;
}

function inferQueuePhase(items: QueueItem[]): SpeechQueuePhase {
  if (items.some((item) => item.status === "speaking")) return "speaking";
  if (items.some((item) => item.status === "error")) return "error";
  return "idle";
}

export function chatStatusFromQueueStatus(
  status: QueueDisplayState,
): Extract<ChatMessage, { kind: "user" }>["status"] {
  return status === "speaking" ? "queued" : status;
}

function syncChatMessageStatuses(
  messages: ChatMessage[],
  queueItems: QueueItem[],
): ChatMessage[] {
  const statusByMessageId = queueStatusByMessageId(queueItems);
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

function syncChatMessageStatus(
  message: ChatMessage,
  queueItems: QueueItem[],
): ChatMessage {
  if (message.kind !== "user") {
    return message;
  }
  const status = queueStatusByMessageId(queueItems).get(message.id);
  return status && status !== message.status ? { ...message, status } : message;
}

function queueStatusByMessageId(queueItems: QueueItem[]) {
  return new Map(
    queueItems.flatMap((item) =>
      item.sourceMessageId
        ? [
            [
              item.sourceMessageId,
              chatStatusFromQueueStatus(item.status),
            ] as const,
          ]
        : [],
    ),
  );
}

export function warningNotifications(
  notifications: AppNotification[],
): AppNotification[] {
  return notifications
    .filter(
      (notification) =>
        notification.severity === "warning" ||
        notification.severity === "error",
    )
    .slice(0, 5);
}

function notificationId(notification: Omit<AppNotification, "id">): string {
  return `${notification.occurredAtMs}-${notification.severity}-${notification.source}-${notification.correlationId ?? notification.message}`;
}

function isDuplicateNotification(
  existing: AppNotification,
  incoming: AppNotification,
): boolean {
  if (existing.correlationId && incoming.correlationId) {
    return existing.correlationId === incoming.correlationId;
  }

  return (
    existing.message === incoming.message &&
    Math.abs(existing.occurredAtMs - incoming.occurredAtMs) <= 5_000
  );
}

function notificationSeverityRank(
  severity: AppNotification["severity"],
): number {
  return { info: 0, success: 1, warning: 2, error: 3 }[severity];
}

function uniqueLogId(
  log: AppLogEvent,
  existingLogs: StoredAppLogEvent[],
): string {
  const baseId = log.id ?? `${log.occurredAtMs}-${log.level}-${log.message}`;
  let id = baseId;
  let suffix = 1;

  while (existingLogs.some((existingLog) => existingLog.id === id)) {
    id = `${baseId}-${suffix}`;
    suffix += 1;
  }

  return id;
}
