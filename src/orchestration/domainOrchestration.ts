import type { AppAction } from "../stores/appStore";
import type { DomainStores } from "../stores/domainStores";
import type {
  AppLogEvent,
  AppEventsSnapshot,
  SpeechStateSnapshot,
  AuthStatus,
  AppNotification,
  AppSettings,
  ChatMessage,
  SpeechQueueUpdatedEvent,
  SpeechStatusEvent,
  TwitchChatMessageEvent,
  TwitchStatusEvent,
} from "../types";
import { subscribeWithCleanup } from "../tauri/subscriptions";
import { restoreAndValidateStartupAuth, type StartupAuthDependencies } from "../startupAuth";

/**
 * Transitional command boundary for the shell. The compatibility action names
 * make migration safe while every write is routed to exactly one domain store.
 */
export function dispatchDomainAction(stores: DomainStores, action: AppAction): void {
  switch (action.type) {
    case "settings.loaded": stores.settings.dispatch({ type: "settings.loaded", settings: action.settings }); break;
    case "twitch.authStatus": stores.connection.dispatch({ type: "auth.status.changed", status: action.status, revision: action.revision }); break;
    case "twitch.connectionStatus": stores.connection.dispatch({ type: "chat.status.changed", status: action.status, revision: action.revision, connectionGeneration: action.connectionGeneration, activeConnection: action.activeConnection }); break;
    case "twitch.authPrompt": stores.connection.dispatch({ type: "auth.prompt.changed", prompt: action.prompt }); break;
    case "twitch.profile": stores.connection.dispatch({ type: "auth.profile.changed", profile: action.profile }); break;
    case "speech.status": stores.connection.dispatch({ type: "speech.status.changed", status: action.status, revision: action.revision, adapterHealth: action.adapterHealth }); break;
    case "speech.snapshot":
      dispatchDomainAction(stores, { type: "speech.status", ...action.snapshot.status });
      dispatchDomainAction(stores, { type: "queue.changed", ...action.snapshot.queue });
      break;
    case "chat.message": stores.chat.dispatch({ type: "message.added", message: action.message, queueItems: stores.queue.getState().items }); break;
    case "queue.changed":
      stores.queue.dispatch({ type: "items.replaced", items: action.items, revision: action.revision, phase: action.phase });
      stores.chat.dispatch({ type: "queue.statuses.changed", items: stores.queue.getState().items });
      break;
    case "launcher.changed": stores.settings.dispatch({ type: "launcher.items.changed", items: action.items }); break;
    case "log.added": stores.logs.dispatch({ type: "log.added", log: action.log }); break;
    case "notification.added": stores.logs.dispatch({ type: "notification.added", notification: action.notification }); break;
    case "logs.cleared": stores.logs.dispatch({ type: "logs.cleared" }); break;
    case "warnings.cleared": stores.logs.dispatch({ type: "warnings.cleared" }); break;
  }
}

export interface DomainEventBridge {
  getAppEventsSnapshot?: () => Promise<AppEventsSnapshot | undefined>;
  speechQueueReload?: () => Promise<SpeechStateSnapshot | undefined>;
  subscribeAppLogEvents: (listener: (event: AppLogEvent & { id?: string }) => void) => Promise<() => void>;
  subscribeTwitchStatusEvents: (listener: (event: TwitchStatusEvent) => void) => Promise<() => void>;
  subscribeTwitchChatMessageEvents: (listener: (event: TwitchChatMessageEvent) => void) => Promise<() => void>;
  subscribeSpeechStatusEvents: (listener: (event: SpeechStatusEvent) => void) => Promise<() => void>;
  subscribeSpeechQueueUpdatedEvents: (listener: (event: SpeechQueueUpdatedEvent) => void) => Promise<() => void>;
}

export interface DomainEventSubscriptionOptions {
  stores: DomainStores;
  onRestored?: () => void;
  replaySystemLog?: (message: string) => void;
  bridge: DomainEventBridge;
  reportNotification: (severity: "warning" | "error", source: "event" | "log", message: string, correlationId?: string) => void;
  routeSystemTimelineEvent?: (event: { message: string }) => void;
  speechRecoveryMessage?: (message: string, status: SpeechStatusEvent["status"]) => { message: string };
  twitchTimelineEvent?: (event: TwitchStatusEvent) => { message: string } | undefined;
}

/** Register all backend event listeners as one cleanup-safe domain boundary. */
export function subscribeDomainEvents({
  stores, bridge, reportNotification, routeSystemTimelineEvent,
  speechRecoveryMessage, twitchTimelineEvent, onRestored, replaySystemLog,
}: DomainEventSubscriptionOptions): () => void {
  let disposed = false;
  const log = (event: AppLogEvent, replay = false) => {
    if (disposed || (event.id && stores.logs.getState().logs.some((log) => log.id === event.id))) return;
    dispatchDomainAction(stores, { type: "log.added", log: event });
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
    ) return;
    if (event.domain === "chat" && event.status !== "validating") {
      dispatchDomainAction(stores, { type: "twitch.connectionStatus", status: event.status, revision: event.revision, connectionGeneration: event.connectionGeneration, activeConnection: event.activeConnection });
    } else if (event.domain === "auth") {
      const statuses: Record<TwitchStatusEvent["status"], AuthStatus> = {
        disconnected: "unauthenticated", connecting: "checking", connected: "authenticated",
        validating: "checking", reconnecting: "checking", authRequired: "expired", error: "error",
      };
      dispatchDomainAction(stores, { type: "twitch.authStatus", status: statuses[event.status], revision: event.revision });
    }
    if (event.message && (event.status === "authRequired" || event.status === "error")) reportNotification("error", "event", event.message);
    const timeline = twitchTimelineEvent?.(event);
    if (timeline) routeSystemTimelineEvent?.(timeline);
  };
  const speech = (event: SpeechStatusEvent) => {
    if (disposed || (event.revision !== undefined && event.revision <= stores.connection.getState().speechRevision)) return;
    dispatchDomainAction(stores, { type: "speech.status", ...event });
    if (event.message && (event.status === "disconnected" || event.status === "error")) {
      reportNotification("error", "event", event.message);
      const timeline = speechRecoveryMessage?.(event.message, event.status);
      if (timeline) routeSystemTimelineEvent?.(timeline);
    }
  };
  const queue = (event: SpeechQueueUpdatedEvent) => {
    if (disposed || (event.revision !== undefined && event.revision <= stores.queue.getState().revision)) return;
    dispatchDomainAction(stores, { type: "queue.changed", ...event });
    if (event.warning) reportNotification("warning", "event", event.warning);
  };
  const cleanup = subscribeWithCleanup([
    () => bridge.subscribeAppLogEvents(log),
    () => bridge.subscribeTwitchStatusEvents(twitch),
    () => bridge.subscribeTwitchChatMessageEvents((event) => {
      if (disposed) return;
      const connection = stores.connection.getState();
      if (event.connectionGeneration !== undefined) {
        if (
          event.connectionGeneration < connection.twitchConnectionGeneration ||
          !connection.twitchActiveConnection ||
          event.connectionGeneration !== connection.twitchActiveConnection.generation ||
          event.channelId !== connection.twitchActiveConnection.broadcasterUserId ||
          event.channelLogin.toLowerCase() !== connection.twitchActiveConnection.broadcasterLogin.toLowerCase()
        ) return;
      }
      const message: ChatMessage = { ...event, kind: "user", status: "queued" };
      dispatchDomainAction(stores, { type: "chat.message", message });
    }),
    () => bridge.subscribeSpeechStatusEvents(speech),
    () => bridge.subscribeSpeechQueueUpdatedEvents(queue),
  ], () => reportNotification("warning", "event",
    "アプリ内イベントの購読に失敗しました。画面を再読み込みしてください。", "app-event-subscription"),
  () => {
    // Subscribe first, then reconcile each stream with its own revision.
    void Promise.all([bridge.getAppEventsSnapshot?.(), bridge.speechQueueReload?.()]).then(([events, state]) => {
      if (disposed) return;
      if (events) {
        for (const event of [...events.logs].reverse()) log(event, true);
        for (const error of [...events.emitErrors].reverse()) log({
          id: error.id, level: "error", occurredAtMs: error.occurredAtMs,
          message: `イベント送信に失敗しました（${error.event}）: ${error.error}`,
        }, true);
        for (const event of [...events.twitchStatuses].sort((a, b) => (a.revision ?? 0) - (b.revision ?? 0))) twitch(event);
        if (events.speechStatus) speech(events.speechStatus);
      }
      if (state) { speech(state.status); queue(state.queue); }
      onRestored?.();
    }).catch(() => {
      if (!disposed) reportNotification("error", "event", "アプリ状態を復元できませんでした。画面を再読み込みしてください。", "app-event-snapshot");
    });
  });
  return () => { disposed = true; cleanup(); };
}

export function restoreStartupAuth(dependencies: StartupAuthDependencies) {
  return restoreAndValidateStartupAuth(dependencies);
}

export interface SettingsMutationDependencies {
  updateSettings: (patch: Partial<{ twitch: Partial<AppSettings["twitch"]>; speech: Partial<AppSettings["speech"]>; launcher: Partial<AppSettings["launcher"]> }>) => Promise<AppSettings>;
  onSettingsLoaded: (settings: AppSettings) => void;
  onError: (error: unknown) => void;
}

/** Serialize settings writes and publish only the value accepted by backend. */
export function createSettingsMutationOrchestrator(dependencies: SettingsMutationDependencies) {
  let tail = Promise.resolve();
  return {
    mutate(patch: Parameters<SettingsMutationDependencies["updateSettings"]>[0]): Promise<boolean> {
      const operation = tail.then(async () => {
        try {
          const settings = await dependencies.updateSettings(patch);
          dependencies.onSettingsLoaded(settings);
          return true;
        } catch (error) {
          dependencies.onError(error);
          return false;
        }
      });
      tail = operation.then(() => undefined, () => undefined);
      return operation;
    },
    waitForIdle(): Promise<void> {
      return tail;
    },
  };
}
