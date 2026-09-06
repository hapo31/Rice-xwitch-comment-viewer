import type { AppAction } from "../stores/appStore";
import type { DomainStores } from "../stores/domainStores";
import type {
  AppLogEvent,
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
    case "twitch.authStatus": stores.connection.dispatch({ type: "auth.status.changed", status: action.status }); break;
    case "twitch.connectionStatus": stores.connection.dispatch({ type: "chat.status.changed", status: action.status }); break;
    case "twitch.authPrompt": stores.connection.dispatch({ type: "auth.prompt.changed", prompt: action.prompt }); break;
    case "twitch.profile": stores.connection.dispatch({ type: "auth.profile.changed", profile: action.profile }); break;
    case "speech.status": stores.connection.dispatch({ type: "speech.status.changed", status: action.status }); break;
    case "chat.message": stores.chat.dispatch({ type: "message.added", message: action.message, queueItems: stores.queue.getState().items }); break;
    case "queue.changed":
      stores.queue.dispatch({ type: "items.replaced", items: action.items });
      stores.chat.dispatch({ type: "queue.statuses.changed", items: action.items });
      break;
    case "launcher.changed": stores.settings.dispatch({ type: "launcher.items.changed", items: action.items }); break;
    case "log.added": stores.logs.dispatch({ type: "log.added", log: action.log }); break;
    case "notification.added": stores.logs.dispatch({ type: "notification.added", notification: action.notification }); break;
    case "logs.cleared": stores.logs.dispatch({ type: "logs.cleared" }); break;
    case "warnings.cleared": stores.logs.dispatch({ type: "warnings.cleared" }); break;
  }
}

export interface DomainEventBridge {
  subscribeAppLogEvents: (listener: (event: AppLogEvent & { id?: string }) => void) => Promise<() => void>;
  subscribeTwitchStatusEvents: (listener: (event: TwitchStatusEvent) => void) => Promise<() => void>;
  subscribeTwitchChatMessageEvents: (listener: (event: TwitchChatMessageEvent) => void) => Promise<() => void>;
  subscribeSpeechStatusEvents: (listener: (event: SpeechStatusEvent) => void) => Promise<() => void>;
  subscribeSpeechQueueUpdatedEvents: (listener: (event: SpeechQueueUpdatedEvent) => void) => Promise<() => void>;
}

export interface DomainEventSubscriptionOptions {
  stores: DomainStores;
  bridge: DomainEventBridge;
  reportNotification: (severity: "warning" | "error", source: "event" | "log", message: string, correlationId?: string) => void;
  routeSystemTimelineEvent?: (event: { message: string }) => void;
  speechRecoveryMessage?: (message: string, status: SpeechStatusEvent["status"]) => { message: string };
  twitchTimelineEvent?: (event: TwitchStatusEvent) => { message: string } | undefined;
}

/** Register all backend event listeners as one cleanup-safe domain boundary. */
export function subscribeDomainEvents({
  stores,
  bridge,
  reportNotification,
  routeSystemTimelineEvent,
  speechRecoveryMessage,
  twitchTimelineEvent,
}: DomainEventSubscriptionOptions): () => void {
  return subscribeWithCleanup([
    () => bridge.subscribeAppLogEvents((event) => {
      dispatchDomainAction(stores, { type: "log.added", log: event });
      if (event.level !== "info") reportNotification(event.level, "log", event.message, event.id);
    }),
    () => bridge.subscribeTwitchStatusEvents((event) => {
      if (event.domain === "chat") dispatchDomainAction(stores, { type: "twitch.connectionStatus", status: event.status });
      if (event.status === "authRequired") dispatchDomainAction(stores, { type: "twitch.authStatus", status: "expired" });
      if (event.message && (event.status === "authRequired" || event.status === "error")) reportNotification("error", "event", event.message);
      const timelineEvent = twitchTimelineEvent?.(event);
      if (timelineEvent) routeSystemTimelineEvent?.(timelineEvent);
    }),
    () => bridge.subscribeTwitchChatMessageEvents((event) => {
      const message: ChatMessage = { ...event, kind: "user", status: "queued" };
      dispatchDomainAction(stores, { type: "chat.message", message });
    }),
    () => bridge.subscribeSpeechStatusEvents((event) => {
      dispatchDomainAction(stores, { type: "speech.status", status: event.status });
      if (event.message && (event.status === "disconnected" || event.status === "error")) {
        reportNotification("error", "event", event.message);
        const timelineEvent = speechRecoveryMessage?.(event.message, event.status);
        if (timelineEvent) routeSystemTimelineEvent?.(timelineEvent);
      }
    }),
    () => bridge.subscribeSpeechQueueUpdatedEvents((event) => {
      dispatchDomainAction(stores, { type: "queue.changed", items: event.items ?? [] });
      if (event.warning) reportNotification("warning", "event", event.warning);
    }),
  ], () => reportNotification(
    "warning",
    "event",
    "アプリ内イベントの購読に失敗しました。画面を再読み込みしてください。",
    "app-event-subscription",
  ));
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
