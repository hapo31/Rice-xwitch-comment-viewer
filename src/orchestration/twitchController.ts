import { type AuthFlowEvent, type AuthFlowState, authFlowTransition } from "../authFlow";
import { AuthOperationController } from "../authOperation";
import { getDeviceAuthRemainingSeconds } from "../features/auth/deviceAuthExpiry";
import { presentError } from "../presentation/errors";
import { autoConnectTimelineEvent, type SystemTimelineEvent } from "../presentation/systemTimeline";
import type { AppAction } from "../stores/appState";
import {
  appOpenExternalUrl,
  twitchConnect,
  twitchDisconnect,
  twitchGetStoredAuth,
  twitchPollAuth,
  twitchStartAuth,
  twitchStopChat,
  twitchValidateAuth,
} from "../tauri/client";
import type {
  AuthStatus,
  NotificationSeverity,
  NotificationSource,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";
import { routeAuthStorageWarning } from "./authWarnings";
import { restoreStartupAuth } from "./domainOrchestration";

export interface TwitchControllerDependencies {
  operations: AuthOperationController;
  dispatch: (action: AppAction) => void;
  getAuthPrompt: () => TwitchDeviceAuthStart | undefined;
  getAuthStatus: () => AuthStatus;
  getAuthProfile: () => TwitchUserProfile | undefined;
  getChannelLogin: () => string | undefined;
  getConfirmBeforeStopChat: () => boolean;
  waitForSettings: () => Promise<void>;
  reportSystemMessage: (message: string) => void;
  reportInfo: (message: string, source?: "command" | "event") => void;
  reportNotification: (
    severity: NotificationSeverity,
    source: NotificationSource,
    message: string,
  ) => void;
  reportError: (error: unknown, operation?: "auth" | "chat" | "externalUrl") => unknown;
  reportTechnicalError: (message: string) => void;
  routeAutoConnectTimeline: (event: SystemTimelineEvent) => void;
}

export function createTwitchController(deps: TwitchControllerDependencies) {
  function transitionAuth(event: AuthFlowEvent, quietWaiting = false) {
    const current: AuthFlowState = {
      status: deps.getAuthStatus(),
      prompt: deps.getAuthPrompt(),
      profile: deps.getAuthProfile(),
    };
    const transition = authFlowTransition(current, event);
    deps.dispatch({ type: "twitch.authStatus", status: transition.state.status });
    deps.dispatch({ type: "twitch.authPrompt", prompt: transition.state.prompt });
    deps.dispatch({ type: "twitch.profile", profile: transition.state.profile });
    for (const effect of transition.effects) {
      if (
        effect.type === "info" &&
        effect.message &&
        !(quietWaiting && event.type === "poll.waiting")
      ) {
        deps.reportInfo(effect.message, event.type === "poll.waiting" ? "event" : "command");
      } else if (effect.type === "warning") {
        deps.reportNotification(effect.severity, "event", effect.message);
      } else if (effect.type === "notification") {
        deps.reportNotification(
          effect.severity,
          event.type === "restore.failed" ? "command" : "event",
          effect.message,
        );
      } else if (effect.type === "failure") {
        deps.reportError(effect.error, "auth");
      }
    }
    return transition.state;
  }

  async function restore() {
    const operation = deps.operations.begin("restore");
    transitionAuth({ type: "restore.started" });
    try {
      const auth = await restoreStartupAuth({
        getStoredAuth: twitchGetStoredAuth,
        validateAuth: twitchValidateAuth,
        reportSystemMessage: (message) => {
          if (deps.operations.isCurrent(operation)) deps.reportSystemMessage(message);
        },
        reportTechnicalError: (message) => {
          if (deps.operations.isCurrent(operation)) deps.reportTechnicalError(message);
        },
      });
      if (!deps.operations.isCurrent(operation)) return;
      if (auth.status === "authenticated") {
        transitionAuth({ type: "restore.authenticated", profile: auth.result.profile });
        routeAuthStorageWarning(auth.result, deps.reportNotification, deps.reportSystemMessage);
      } else if (auth.status === "missing") {
        transitionAuth({ type: "restore.missing" });
      } else if (auth.status === "error") {
        transitionAuth({ type: "restore.failed", message: auth.error });
      }
    } finally {
      deps.operations.finishOperation(operation);
    }
  }

  async function startAuth() {
    const operation = deps.operations.begin("start");
    transitionAuth({ type: "prompt.requested" });
    try {
      const prompt = await twitchStartAuth();
      if (!deps.operations.isCurrent(operation)) return;
      transitionAuth({ type: "prompt.started", prompt });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      deps.reportInfo("Twitch の認証コードを発行しました。");
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      transitionAuth({ type: "prompt.failed", error });
    } finally {
      deps.operations.finishOperation(operation);
    }
  }

  async function pollAuth(options: { quietWaiting?: boolean; expectedGeneration?: number } = {}) {
    const operation = deps.operations.tryBeginPoll(options.expectedGeneration);
    if (operation === undefined) return;
    transitionAuth({ type: "poll.started" });
    try {
      const result = await twitchPollAuth();
      if (!deps.operations.isCurrent(operation)) return;
      if (result.status === "authorized") {
        transitionAuth({ type: "poll.authorized", profile: result.profile });
        deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
        routeAuthStorageWarning(result, deps.reportNotification, deps.reportSystemMessage);
      } else {
        if (result.status === "pending" || result.status === "slowDown")
          transitionAuth(
            { type: "poll.waiting", interval: result.interval, message: result.message },
            options.quietWaiting,
          );
        else
          transitionAuth({ type: "poll.denied", status: result.status, message: result.message });
      }
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      transitionAuth({ type: "poll.failed", error });
    } finally {
      deps.operations.finishPoll(operation);
    }
  }

  async function validateAuth(): Promise<boolean> {
    const operation = deps.operations.begin("validate");
    transitionAuth({ type: "validate.started" });
    try {
      const result = await twitchValidateAuth();
      if (!deps.operations.isCurrent(operation)) return false;
      transitionAuth({ type: "validate.valid", profile: result.profile });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      routeAuthStorageWarning(result, deps.reportNotification, deps.reportSystemMessage);
      return true;
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return false;
      transitionAuth({ type: "validate.invalid", error });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      return false;
    } finally {
      deps.operations.finishOperation(operation);
    }
  }

  async function connect({ automatic = false }: { automatic?: boolean } = {}) {
    try {
      await deps.waitForSettings();
      deps.dispatch({ type: "twitch.connectionStatus", status: "connecting" });
      if (automatic) {
        deps.routeAutoConnectTimeline(
          autoConnectTimelineEvent("started", "Twitch チャットの自動接続を開始します。"),
        );
      }
      await twitchConnect(deps.getChannelLogin());
      deps.reportInfo("Twitch チャット接続を開始しました。");
    } catch (error) {
      deps.dispatch({ type: "twitch.connectionStatus", status: "error" });
      deps.reportError(error, "chat");
      if (automatic) {
        deps.routeAutoConnectTimeline(
          autoConnectTimelineEvent(
            "failed",
            `Twitch チャットの自動接続に失敗しました: ${presentError(error, "chat").message}`,
          ),
        );
      }
    }
  }

  async function stopChat() {
    if (deps.getConfirmBeforeStopChat() && !window.confirm("Twitch チャット受信を停止しますか？"))
      return;
    try {
      await twitchStopChat();
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
    } catch (error) {
      deps.dispatch({ type: "twitch.connectionStatus", status: "error" });
      deps.reportError(error, "chat");
    }
  }

  async function disconnect() {
    if (!window.confirm("Twitch 連携を解除しますか？")) return;
    const operation = deps.operations.begin("disconnect");
    transitionAuth({ type: "disconnect.started" });
    try {
      await twitchDisconnect();
      if (!deps.operations.isCurrent(operation)) return;
      transitionAuth({ type: "disconnect.succeeded" });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      transitionAuth({ type: "disconnect.failed", error });
    } finally {
      deps.operations.finishOperation(operation);
    }
  }

  async function openExternalUrl(url: string) {
    try {
      await appOpenExternalUrl(url);
    } catch (error) {
      deps.reportError(error, "externalUrl");
    }
  }

  function schedulePoll(prompt: TwitchDeviceAuthStart | undefined): () => void {
    if (!prompt) return () => undefined;
    const generation = deps.operations.getState().generation;
    const remainingMs = Math.max(0, prompt.expiresAtMs - Date.now());
    if (remainingMs === 0) {
      expireAuthPrompt(prompt, generation);
      return () => undefined;
    }
    const pollDelayMs = Math.min(Math.max(prompt.interval, 1) * 1000, remainingMs);
    const timer = window.setTimeout(() => {
      const current = deps.getAuthPrompt();
      if (
        current?.userCode === prompt.userCode &&
        current.expiresAtMs === prompt.expiresAtMs &&
        current.interval === prompt.interval &&
        getDeviceAuthRemainingSeconds(current.expiresAtMs) > 0
      ) {
        void pollAuth({ quietWaiting: true, expectedGeneration: generation });
      } else if (current) {
        if (getDeviceAuthRemainingSeconds(current.expiresAtMs) === 0)
          expireAuthPrompt(prompt, generation);
      }
    }, pollDelayMs);
    return () => window.clearTimeout(timer);
  }

  function expireAuthPrompt(prompt: TwitchDeviceAuthStart, generation: number) {
    const operation = deps.operations.getState().activeOperation;
    const current = deps.getAuthPrompt();
    if (
      !deps.operations.isCurrent(generation) ||
      (operation !== undefined && operation !== "poll") ||
      current?.userCode !== prompt.userCode ||
      current.expiresAtMs !== prompt.expiresAtMs
    )
      return;
    transitionAuth({
      type: "prompt.expired",
      message: "Twitch の認証コードの有効期限が切れました。再度ログインしてください。",
    });
  }

  return {
    restore,
    startAuth,
    pollAuth,
    validateAuth,
    connect,
    stopChat,
    disconnect,
    openExternalUrl,
    schedulePoll,
  };
}
