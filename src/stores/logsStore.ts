import type { AppLogEvent, AppNotification } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export type StoredAppLogEvent = AppLogEvent & { id: string; sourceEventId?: string };

export interface LogsState {
  logs: StoredAppLogEvent[];
  /** Unresolved actionable notices have a capacity independent of informational history. */
  notifications: AppNotification[];
  notificationHistory: AppNotification[];
}

export type LogsAction =
  | { type: "log.added"; log: AppLogEvent }
  | { type: "notification.added"; notification: Omit<AppNotification, "id"> & { id?: string } }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialLogsState: LogsState = { logs: [], notifications: [], notificationHistory: [] };
const notificationLimits = { notifications: 100, notificationHistory: 100 } as const;

function notificationBucket(notification: AppNotification): keyof typeof notificationLimits {
  return notification.severity === "warning" || notification.severity === "error"
    ? "notifications"
    : "notificationHistory";
}

export function logsReducer(state: LogsState, action: LogsAction): LogsState {
  switch (action.type) {
    case "log.added":
      if (action.log.id && state.logs.some((log) => log.sourceEventId === action.log.id))
        return state;
      return {
        ...state,
        logs: [
          {
            ...action.log,
            id: uniqueLogId(action.log, state.logs),
            ...(action.log.id ? { sourceEventId: action.log.id } : {}),
          },
          ...state.logs,
        ]
          .sort((a, b) => b.occurredAtMs - a.occurredAtMs)
          .slice(0, 500),
      };
    case "notification.added": {
      const notification = {
        ...action.notification,
        id: action.notification.id ?? notificationId(action.notification),
      };
      const existing = [...state.notifications, ...state.notificationHistory].find((entry) =>
        isDuplicateNotification(entry, notification),
      );
      const target = notificationBucket(notification);
      if (existing) {
        if (
          notificationSeverityRank(notification.severity) <=
          notificationSeverityRank(existing.severity)
        )
          return state;
        const source = notificationBucket(existing);
        const promoted = { ...existing, severity: notification.severity };
        if (source === target) {
          return {
            ...state,
            [target]: state[target].map((entry) => (entry === existing ? promoted : entry)),
          };
        }
        return {
          ...state,
          [source]: state[source].filter((entry) => entry !== existing),
          [target]: [promoted, ...state[target]].slice(0, notificationLimits[target]),
        };
      }
      return {
        ...state,
        [target]: [notification, ...state[target]].slice(0, notificationLimits[target]),
      };
    }
    case "logs.cleared":
      return { ...state, logs: [] };
    case "warnings.cleared":
      return { ...state, notifications: [] };
    default:
      return state;
  }
}

export function warningNotifications(notifications: AppNotification[]): AppNotification[] {
  return notifications
    .filter(
      (notification) => notification.severity === "warning" || notification.severity === "error",
    )
    .slice(0, 5);
}

function notificationId(notification: Omit<AppNotification, "id">): string {
  return `${notification.occurredAtMs}-${notification.severity}-${notification.source}-${notification.correlationId ?? notification.message}`;
}
function isDuplicateNotification(existing: AppNotification, incoming: AppNotification): boolean {
  if (existing.correlationId && incoming.correlationId)
    return existing.correlationId === incoming.correlationId;
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
    id = `${baseId}-${suffix++}`;
  }
  return id;
}

export function createLogsStore(): ExternalStore<LogsState, LogsAction> {
  return createExternalStore(logsReducer, initialLogsState);
}
