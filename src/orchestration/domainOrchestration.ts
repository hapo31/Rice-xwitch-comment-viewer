import type { SystemTimelineEvent } from "../models/systemTimeline";
import { restoreAndValidateStartupAuth, type StartupAuthDependencies } from "../startupAuth";
import type { AppAction } from "../stores/appState";
import type { DomainStores } from "../stores/domainStores";
import { subscribeWithCleanup } from "../tauri/subscriptions";
import type {
  AppEventsSnapshot,
  AppLogEvent,
  AppNotification,
  AuthStatus,
  ChatMessage,
  SpeechQueueUpdatedEvent,
  SpeechStateSnapshot,
  SpeechStatusEvent,
  TwitchChatMessageEvent,
  TwitchStatusEvent,
} from "../types";

/**
 * Transitional command boundary for the shell. The compatibility action names
 * make migration safe while every write is routed to exactly one domain store.
 */
export function dispatchDomainAction(stores: DomainStores, action: AppAction): void {
  switch (action.type) {
    case "twitch.disconnectStarted":
      stores.connection.dispatch({
        type: "auth.disconnect.started",
        generation: action.generation,
      });
      break;
    case "twitch.disconnectFinished":
      stores.connection.dispatch({
        type: "auth.disconnect.finished",
        generation: action.generation,
      });
      break;
    case "settings.loaded":
      stores.settings.dispatch({ type: "settings.loaded", settings: action.settings });
      break;
    case "twitch.authStatus":
      stores.connection.dispatch({
        type: "auth.status.changed",
        status: action.status,
        revision: action.revision,
      });
      break;
    case "twitch.connectionStatus":
      stores.connection.dispatch({
        type: "chat.status.changed",
        status: action.status,
        revision: action.revision,
        connectionGeneration: action.connectionGeneration,
        activeConnection: action.activeConnection,
      });
      break;
    case "twitch.authPrompt":
      stores.connection.dispatch({ type: "auth.prompt.changed", prompt: action.prompt });
      break;
    case "twitch.profile":
      stores.connection.dispatch({ type: "auth.profile.changed", profile: action.profile });
      break;
    case "speech.status":
      stores.connection.dispatch({
        type: "speech.status.changed",
        status: action.status,
        revision: action.revision,
        adapterHealth: action.adapterHealth,
      });
      break;
    case "speech.snapshot":
      dispatchDomainAction(stores, { type: "speech.status", ...action.snapshot.status });
      dispatchDomainAction(stores, { type: "queue.changed", ...action.snapshot.queue });
      break;
    case "chat.message":
      stores.chat.dispatch({
        type: "message.added",
        message: action.message,
        queueItems: stores.queue.getState().items,
      });
      break;
    case "queue.changed":
      stores.queue.dispatch({
        type: "items.replaced",
        items: action.items,
        revision: action.revision,
        phase: action.phase,
      });
      stores.chat.dispatch({
        type: "queue.statuses.changed",
        items: stores.queue.getState().items,
      });
      break;
    case "launcher.changed":
      stores.settings.dispatch({ type: "launcher.items.changed", items: action.items });
      break;
    case "log.added":
      stores.logs.dispatch({ type: "log.added", log: action.log });
      break;
    case "notification.added":
      stores.logs.dispatch({ type: "notification.added", notification: action.notification });
      break;
    case "logs.cleared":
      stores.logs.dispatch({ type: "logs.cleared" });
      break;
    case "warnings.cleared":
      stores.logs.dispatch({ type: "warnings.cleared" });
      break;
  }
}

export interface DomainEventBridge {
  getAppEventsSnapshot?: () => Promise<AppEventsSnapshot | undefined>;
  speechQueueReload?: () => Promise<SpeechStateSnapshot | undefined>;
  subscribeAppLogEvents: (
    listener: (event: AppLogEvent & { id?: string }) => void,
  ) => Promise<() => void>;
  subscribeTwitchStatusEvents: (
    listener: (event: TwitchStatusEvent) => void,
  ) => Promise<() => void>;
  subscribeTwitchChatMessageEvents: (
    listener: (event: TwitchChatMessageEvent) => void,
  ) => Promise<() => void>;
  subscribeSpeechStatusEvents: (
    listener: (event: SpeechStatusEvent) => void,
  ) => Promise<() => void>;
  subscribeSpeechQueueUpdatedEvents: (
    listener: (event: SpeechQueueUpdatedEvent) => void,
  ) => Promise<() => void>;
}

export interface DomainEventSubscriptionOptions {
  stores: DomainStores;
  onRestored?: () => void;
  shouldRestoreAuth?: () => boolean;
  replaySystemLog?: (message: string) => void;
  bridge: DomainEventBridge;
  reportNotification: (
    severity: "warning" | "error",
    source: "event" | "log",
    message: string,
    correlationId?: string,
    announcementDomains?: AppNotification["announcementDomains"],
  ) => void;
  routeSystemTimelineEvent?: (event: SystemTimelineEvent) => void;
  speechRecoveryMessage?: (
    message: string,
    status: SpeechStatusEvent["status"],
  ) => SystemTimelineEvent;
  twitchTimelineEvent?: (event: TwitchStatusEvent) => SystemTimelineEvent | undefined;
}

const MAX_BUFFERED_CHAT_MESSAGES = 200;
const MAX_RECENT_CHAT_MESSAGE_IDS = 400;

function chatMessageMatchesConnection(
  event: TwitchChatMessageEvent,
  connection: ReturnType<DomainStores["connection"]["getState"]>,
): boolean {
  if (event.connectionGeneration === undefined) return true;
  const activeConnection = connection.twitchActiveConnection;
  return (
    !!activeConnection &&
    event.connectionGeneration === activeConnection.generation &&
    event.connectionGeneration === connection.twitchConnectionGeneration &&
    event.channelId === activeConnection.broadcasterUserId &&
    event.channelLogin.toLowerCase() === activeConnection.broadcasterLogin.toLowerCase()
  );
}

function isUnresolvedChatMessage(
  event: TwitchChatMessageEvent,
  connection: ReturnType<DomainStores["connection"]["getState"]>,
): boolean {
  if (event.connectionGeneration === undefined) return false;
  if (event.connectionGeneration < connection.twitchConnectionGeneration) return false;
  const activeConnection = connection.twitchActiveConnection;
  if (!activeConnection) return true;
  if (event.connectionGeneration < activeConnection.generation) return false;
  if (event.connectionGeneration > activeConnection.generation) return true;
  return false;
}

/** Register all backend event listeners as one cleanup-safe domain boundary. */
export function subscribeDomainEvents({
  stores,
  bridge,
  reportNotification,
  routeSystemTimelineEvent,
  speechRecoveryMessage,
  twitchTimelineEvent,
  onRestored,
  shouldRestoreAuth,
  replaySystemLog,
}: DomainEventSubscriptionOptions): () => void {
  let disposed = false;
  let restorationPending = true;
  const bufferedChatMessages: TwitchChatMessageEvent[] = [];
  const recentChatMessageIds = new Set<string>();
  const recentChatMessageIdOrder: string[] = [];
  for (const message of stores.chat.getState().messages) {
    if (message.kind === "user" && !recentChatMessageIds.has(message.id)) {
      recentChatMessageIds.add(message.id);
      recentChatMessageIdOrder.push(message.id);
    }
  }
  const rememberChatMessageId = (id: string): boolean => {
    if (recentChatMessageIds.has(id)) return false;
    recentChatMessageIds.add(id);
    recentChatMessageIdOrder.push(id);
    while (recentChatMessageIdOrder.length > MAX_RECENT_CHAT_MESSAGE_IDS) {
      const oldest = recentChatMessageIdOrder.shift();
      if (oldest !== undefined) recentChatMessageIds.delete(oldest);
    }
    return true;
  };
  const addChatMessage = (event: TwitchChatMessageEvent) => {
    const message: ChatMessage = { ...event, kind: "user", status: "received" };
    dispatchDomainAction(stores, { type: "chat.message", message });
  };
  const bufferChatMessage = (event: TwitchChatMessageEvent) => {
    if (!rememberChatMessageId(event.id)) return;
    if (bufferedChatMessages.length === MAX_BUFFERED_CHAT_MESSAGES) bufferedChatMessages.shift();
    bufferedChatMessages.push(event);
  };
  const restoreBufferedChatMessages = () => {
    if (disposed) return;
    const connection = stores.connection.getState();
    const pending = bufferedChatMessages.splice(0);
    for (const event of pending) {
      if (chatMessageMatchesConnection(event, connection)) addChatMessage(event);
    }
  };
  const log = (event: AppLogEvent, replay = false) => {
    if (disposed) return;
    const previousLogs = stores.logs.getState().logs;
    dispatchDomainAction(stores, { type: "log.added", log: event });
    if (stores.logs.getState().logs === previousLogs) return;
    if (event.level !== "info") reportNotification(event.level, "log", event.message, event.id);
    if (replay) replaySystemLog?.(event.message);
  };
  const twitch = (event: TwitchStatusEvent) => {
    if (disposed) return;
    const current = stores.connection.getState();
    const revision = event.domain === "auth" ? current.authRevision : current.chatRevision;
    if (event.revision !== undefined && event.revision <= revision) return;
    if (
      event.domain === "chat" &&
      event.connectionGeneration !== undefined &&
      event.connectionGeneration < current.twitchConnectionGeneration
    )
      return;
    if (event.domain === "chat") {
      dispatchDomainAction(stores, {
        type: "twitch.connectionStatus",
        status: event.status,
        revision: event.revision,
        connectionGeneration: event.connectionGeneration,
        activeConnection: event.activeConnection,
      });
      if (!restorationPending) restoreBufferedChatMessages();
    } else if (event.domain === "auth") {
      const statuses: Record<TwitchStatusEvent["status"], AuthStatus> = {
        disconnected: "unauthenticated",
        connecting: "checking",
        connected: "authenticated",
        validating: "checking",
        reconnecting: "checking",
        authRequired: "expired",
        error: "error",
      };
      dispatchDomainAction(stores, {
        type: "twitch.authStatus",
        status: statuses[event.status],
        revision: event.revision,
      });
    }
    if (event.message && (event.status === "authRequired" || event.status === "error"))
      reportNotification("error", "event", event.message, undefined, [event.domain]);
    const timeline = twitchTimelineEvent?.(event);
    if (timeline) routeSystemTimelineEvent?.(timeline);
  };
  const speech = (event: SpeechStatusEvent) => {
    if (
      disposed ||
      (event.revision !== undefined &&
        event.revision <= stores.connection.getState().speechRevision)
    )
      return;
    dispatchDomainAction(stores, { type: "speech.status", ...event });
    if (event.message && (event.status === "disconnected" || event.status === "error")) {
      reportNotification("error", "event", event.message, undefined, ["speech"]);
      const timeline = speechRecoveryMessage?.(event.message, event.status);
      if (timeline) routeSystemTimelineEvent?.(timeline);
    }
  };
  const queue = (event: SpeechQueueUpdatedEvent) => {
    if (
      disposed ||
      (event.revision !== undefined && event.revision <= stores.queue.getState().revision)
    )
      return;
    dispatchDomainAction(stores, { type: "queue.changed", ...event });
    if (event.warning) reportNotification("warning", "event", event.warning, undefined, ["queue"]);
  };
  const cleanup = subscribeWithCleanup(
    [
      () => bridge.subscribeAppLogEvents(log),
      () => bridge.subscribeTwitchStatusEvents(twitch),
      () =>
        bridge.subscribeTwitchChatMessageEvents((event) => {
          if (disposed) return;
          const connection = stores.connection.getState();
          if (event.connectionGeneration !== undefined) {
            const matchesConnection = chatMessageMatchesConnection(event, connection);
            if (
              restorationPending &&
              (matchesConnection || isUnresolvedChatMessage(event, connection))
            ) {
              bufferChatMessage(event);
              return;
            }
            if (matchesConnection) {
              if (rememberChatMessageId(event.id)) addChatMessage(event);
              return;
            }
            return;
          }
          if (rememberChatMessageId(event.id)) addChatMessage(event);
        }),
      () => bridge.subscribeSpeechStatusEvents(speech),
      () => bridge.subscribeSpeechQueueUpdatedEvents(queue),
    ],
    () =>
      reportNotification(
        "warning",
        "event",
        "アプリ内イベントの購読に失敗しました。画面を再読み込みしてください。",
        "app-event-subscription",
      ),
    () => {
      // Subscribe first, then reconcile each stream with its own revision.
      void Promise.all([bridge.getAppEventsSnapshot?.(), bridge.speechQueueReload?.()])
        .then(([events, state]) => {
          if (disposed) return;
          if (events) {
            for (const event of [...events.logs].reverse()) log(event, true);
            for (const error of [...events.emitErrors].reverse())
              log(
                {
                  id: error.id,
                  level: "error",
                  occurredAtMs: error.occurredAtMs,
                  message: `イベント送信に失敗しました（${error.event}）: ${error.error}`,
                },
                true,
              );
            for (const event of [...events.twitchStatuses].sort(
              (a, b) => (a.revision ?? 0) - (b.revision ?? 0),
            )) {
              if (event.domain !== "auth" || shouldRestoreAuth?.() !== false) twitch(event);
            }
            if (events.speechStatus) speech(events.speechStatus);
          }
          if (state) {
            speech(state.status);
            queue(state.queue);
          }
          restorationPending = false;
          restoreBufferedChatMessages();
          onRestored?.();
        })
        .catch(() => {
          if (!disposed) {
            restorationPending = false;
            bufferedChatMessages.length = 0;
            reportNotification(
              "error",
              "event",
              "アプリ状態を復元できませんでした。画面を再読み込みしてください。",
              "app-event-snapshot",
            );
          }
        });
    },
  );
  return () => {
    disposed = true;
    restorationPending = false;
    bufferedChatMessages.length = 0;
    cleanup();
  };
}

export function restoreStartupAuth(dependencies: StartupAuthDependencies) {
  return restoreAndValidateStartupAuth(dependencies);
}
