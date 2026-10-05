import { presentError } from "../presentation/errors";
import { getDeviceAuthRemainingSeconds } from "../features/auth/deviceAuthExpiry";
import { AuthOperationController } from "../authOperation";
import { autoConnectTimelineEvent, type SystemTimelineEvent } from "../presentation/systemTimeline";
import { routeAuthStorageWarning } from "./authWarnings";
import { restoreStartupAuth } from "./domainOrchestration";
import type { AppAction } from "../stores/appStore";
import type { NotificationSeverity, NotificationSource, TwitchDeviceAuthStart } from "../types";
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

export interface TwitchControllerDependencies {
  operations: AuthOperationController;
  dispatch: (action: AppAction) => void;
  getAuthPrompt: () => TwitchDeviceAuthStart | undefined;
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
  async function restore() {
    const operation = deps.operations.begin("restore");
    deps.dispatch({ type: "twitch.authStatus", status: "checking" });
    const auth = await restoreStartupAuth({
      getStoredAuth: twitchGetStoredAuth,
      validateAuth: twitchValidateAuth,
      reportSystemMessage: deps.reportSystemMessage,
      reportTechnicalError: deps.reportTechnicalError,
    });
    if (!deps.operations.isCurrent(operation)) return;
    if (auth.status === "authenticated") {
      deps.dispatch({ type: "twitch.profile", profile: auth.result.profile });
      deps.dispatch({ type: "twitch.authStatus", status: "authenticated" });
      routeAuthStorageWarning(auth.result, deps.reportNotification, deps.reportSystemMessage);
    } else if (auth.status === "missing") {
      deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
    } else if (auth.status === "error") {
      deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      deps.dispatch({ type: "twitch.profile", profile: undefined });
      deps.reportNotification("error", "command", auth.error);
    }
  }

  async function startAuth() {
    const operation = deps.operations.begin("start");
    deps.dispatch({ type: "twitch.authStatus", status: "authorizing" });
    try {
      const prompt = await twitchStartAuth();
      if (!deps.operations.isCurrent(operation)) return;
      deps.dispatch({ type: "twitch.authPrompt", prompt });
      deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      deps.dispatch({ type: "twitch.profile", profile: undefined });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      deps.reportInfo("Twitch の認証コードを発行しました。");
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      deps.dispatch({ type: "twitch.authStatus", status: "error" });
      deps.reportError(error, "auth");
    }
  }

  async function pollAuth(options: { quietWaiting?: boolean } = {}) {
    const operation = deps.operations.tryBeginPoll();
    if (operation === undefined) return;
    deps.dispatch({ type: "twitch.authStatus", status: "polling" });
    try {
      const result = await twitchPollAuth();
      if (!deps.operations.isCurrent(operation)) return;
      if (result.status === "authorized") {
        deps.dispatch({ type: "twitch.authStatus", status: "authenticated" });
        deps.dispatch({ type: "twitch.authPrompt", prompt: undefined });
        deps.dispatch({ type: "twitch.profile", profile: result.profile });
        deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
        deps.reportInfo(`Twitch に ${result.profile.login} としてログインしました。`);
        routeAuthStorageWarning(result, deps.reportNotification, deps.reportSystemMessage);
      } else {
        deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
        const prompt = deps.getAuthPrompt();
        if (prompt && (result.status === "pending" || result.status === "slowDown")) {
          deps.dispatch({
            type: "twitch.authPrompt",
            prompt: { ...prompt, interval: result.interval },
          });
        }
        if (
          !options.quietWaiting ||
          (result.status !== "pending" && result.status !== "slowDown")
        ) {
          if (result.status === "pending" || result.status === "slowDown") {
            deps.reportInfo(result.message, "event");
          } else {
            deps.reportNotification(
              result.status === "denied" || result.status === "expired" ? "warning" : "info",
              "event",
              result.message,
            );
          }
        }
        if (result.status === "expired" || result.status === "denied") {
          deps.dispatch({ type: "twitch.authPrompt", prompt: undefined });
        }
      }
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      deps.dispatch({ type: "twitch.authStatus", status: "error" });
      deps.reportError(error, "auth");
    } finally {
      deps.operations.finishPoll(operation);
    }
  }

  async function validateAuth(): Promise<boolean> {
    const operation = deps.operations.begin("validate");
    deps.dispatch({ type: "twitch.authStatus", status: "checking" });
    try {
      const result = await twitchValidateAuth();
      if (!deps.operations.isCurrent(operation)) return false;
      deps.dispatch({ type: "twitch.authStatus", status: "authenticated" });
      deps.dispatch({ type: "twitch.profile", profile: result.profile });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      deps.reportInfo("Twitch 認証は有効です。");
      routeAuthStorageWarning(result, deps.reportNotification, deps.reportSystemMessage);
      return true;
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return false;
      deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      deps.dispatch({ type: "twitch.authPrompt", prompt: undefined });
      deps.dispatch({ type: "twitch.profile", profile: undefined });
      deps.reportError(error, "auth");
      return false;
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
    deps.dispatch({ type: "twitch.authStatus", status: "disconnecting" });
    try {
      await twitchDisconnect();
      if (!deps.operations.isCurrent(operation)) return;
      deps.dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      deps.dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      deps.dispatch({ type: "twitch.authPrompt", prompt: undefined });
      deps.dispatch({ type: "twitch.profile", profile: undefined });
    } catch (error) {
      if (!deps.operations.isCurrent(operation)) return;
      deps.reportError(error, "auth");
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
    if (!prompt || getDeviceAuthRemainingSeconds(prompt.expiresAtMs) === 0) return () => undefined;
    const timer = window.setTimeout(() => {
      const current = deps.getAuthPrompt();
      if (current && getDeviceAuthRemainingSeconds(current.expiresAtMs) > 0) {
        void pollAuth({ quietWaiting: true });
      }
    }, Math.max(prompt.interval, 1) * 1000);
    return () => window.clearTimeout(timer);
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
