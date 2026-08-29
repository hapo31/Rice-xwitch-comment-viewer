import type { AppLogEvent, AppNotification } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export type StoredAppLogEvent = AppLogEvent & { id: string };

export interface LogsState {
  logs: StoredAppLogEvent[];
  notifications: AppNotification[];
}

export type LogsAction =
  | { type: "log.added"; log: AppLogEvent }
  | { type: "notification.added"; notification: Omit<AppNotification, "id"> & { id?: string } }
  | { type: "logs.cleared" }
  | { type: "warnings.cleared" };

export const initialLogsState: LogsState = { logs: [], notifications: [] };

export function logsReducer(state: LogsState, action: LogsAction): LogsState {
  switch (action.type) {
    case "log.added":
      return { ...state, logs: [{ ...action.log, id: uniqueLogId(action.log, state.logs) }, ...state.logs].slice(0, 500) };
    case "notification.added": {
      const notification = { ...action.notification, id: action.notification.id ?? notificationId(action.notification) };
      const duplicateIndex = state.notifications.findIndex((existing) => isDuplicateNotification(existing, notification));
      if (duplicateIndex >= 0) {
        const existing = state.notifications[duplicateIndex];
        if (notificationSeverityRank(notification.severity) <= notificationSeverityRank(existing.severity)) return state;
        const notifications = [...state.notifications];
        notifications[duplicateIndex] = { ...existing, severity: notification.severity };
        return { ...state, notifications };
      }
      return { ...state, notifications: [notification, ...state.notifications].slice(0, 100) };
    }
    case "logs.cleared": return { ...state, logs: [] };
    case "warnings.cleared": return {
      ...state,
      notifications: state.notifications.filter((notification) => notification.severity !== "warning" && notification.severity !== "error"),
    };
    default: return state;
  }
}

export function warningNotifications(notifications: AppNotification[]): AppNotification[] {
  return notifications.filter((notification) => notification.severity === "warning" || notification.severity === "error").slice(0, 5);
}

function notificationId(notification: Omit<AppNotification, "id">): string {
  return `${notification.occurredAtMs}-${notification.severity}-${notification.source}-${notification.correlationId ?? notification.message}`;
}
function isDuplicateNotification(existing: AppNotification, incoming: AppNotification): boolean {
  if (existing.correlationId && incoming.correlationId) return existing.correlationId === incoming.correlationId;
  return existing.message === incoming.message && Math.abs(existing.occurredAtMs - incoming.occurredAtMs) <= 5_000;
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
