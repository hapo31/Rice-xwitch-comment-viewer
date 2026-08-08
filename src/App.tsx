import { useEffect, useReducer, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { SidePanel } from "./components/SidePanel";
import { StatusBar } from "./components/StatusBar";
import { LiveStatusAnnouncer } from "./components/LiveStatusAnnouncer";
import { ResizeHandles, TitleBar } from "./components/TitleBar";
import { useDisplayScale } from "./hooks/useDisplayScale";
import { useStreamHotkeys } from "./hooks/useStreamHotkeys";
import { getDeviceAuthRemainingSeconds } from "./features/auth/deviceAuthExpiry";
import { APP_SHELL_CLASS_NAME } from "./layout/appShell";
import { claimStartupGuideForSession } from "./presentation/startupGuide";
import { autoConnectTimelineEvent, speechRecoveryTimelineEvent, SystemTimelineRouter, timelineEventFromTwitchStatus } from "./presentation/systemTimeline";
import { restoreAndValidateStartupAuth } from "./startupAuth";
import { appReducer, initialAppState } from "./stores/appStore";
import {
  appOpenExternalUrl,
  getSettings,
  launcherAdd,
  launcherLaunch,
  launcherLaunchAll,
  launcherRemove,
  subscribeAppLogEvents,
  subscribeSpeechQueueUpdatedEvents,
  subscribeSpeechStatusEvents,
  subscribeTwitchChatMessageEvents,
  subscribeTwitchStatusEvents,
  speechConnectionDiagnostics,
  speechControl,
  speechQueueDismiss,
  speechQueueDismissHistory,
  speechHealthCheck,
  speechHealthProbe,
  speechQueueReload,
  speechQueueRemove,
  speechQueueRetry,
  speechTest,
  takeSettingsRecoveryNotice,
  twitchConnect,
  twitchDisconnect,
  twitchGetStoredAuth,
  twitchPollAuth,
  twitchStartAuth,
  twitchStopChat,
  twitchValidateAuth,
  updateSettings,
} from "./tauri/client";
import type { AppSettings, BouyomiConnectionDiagnostics, LauncherLaunchResult, NotificationSeverity, NotificationSource } from "./types";

const showStartupGuideForSession = claimStartupGuideForSession(window.sessionStorage);

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const navigate = useNavigate();
  const displayScale = useDisplayScale();
  const autoConnectAttempted = useRef(false);
  const startupAuthAttempted = useRef(false);
  const systemTimelineRouter = useRef(new SystemTimelineRouter());

  useEffect(() => {
    Promise.all([getSettings(), takeSettingsRecoveryNotice()])
      .then(([settings, recoveryNotice]) => {
        dispatch({ type: "settings.loaded", settings });
        if (recoveryNotice) {
          addSystemChatMessage(recoveryNotice.message);
          dispatch({
            type: "log.added",
            log: {
              level: "warning",
              message: recoveryNotice.message,
              occurredAtMs: Date.now(),
            },
          });
          reportNotification("warning", "system", recoveryNotice.message);
        }
      })
      .catch(() => reportNotification("error", "command", "設定の読み込みに失敗しました。"));

    if (startupAuthAttempted.current) {
      return;
    }
    startupAuthAttempted.current = true;

    void restoreAndValidateStartupAuth({
      getStoredAuth: twitchGetStoredAuth,
      validateAuth: twitchValidateAuth,
      reportSystemMessage: addSystemChatMessage,
    }).then((auth) => {
      if (auth.status === "authenticated") {
        dispatch({ type: "twitch.profile", profile: auth.result.profile });
        dispatch({ type: "twitch.authStatus", status: "authenticated" });
        dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
        if (auth.result.storageWarning) {
          reportNotification("warning", "system", auth.result.storageWarning);
          addSystemChatMessage(auth.result.storageWarning);
        }
        return;
      }

      if (auth.status === "error") {
        dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
        dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
        dispatch({ type: "twitch.profile", profile: undefined });
        reportNotification("error", "command", auth.error);
      }
    });
  }, []);

  function addSystemChatMessage(text: string) {
    dispatch({
      type: "chat.message",
      message: {
        kind: "system",
        id: `system-${Date.now()}-${crypto.randomUUID()}`,
        receivedAt: new Date().toISOString(),
        userDisplayName: "system",
        text,
      },
    });
  }

  function reportNotification(
    severity: NotificationSeverity,
    source: NotificationSource,
    message: string,
    correlationId?: string,
  ) {
    dispatch({
      type: "notification.added",
      notification: { severity, source, message, occurredAtMs: Date.now(), correlationId },
    });
  }

  function reportError(error: unknown, source: NotificationSource = "command", correlationId?: string) {
    reportNotification("error", source, String(error), correlationId);
  }

  function reportInfo(message: string, source: NotificationSource = "command") {
    reportNotification("success", source, message);
    dispatch({ type: "log.added", log: { level: "info", message, occurredAtMs: Date.now() } });
    addSystemChatMessage(message);
  }

  function routeSystemTimelineEvent(event: Parameters<SystemTimelineRouter["shouldRecord"]>[0]) {
    if (systemTimelineRouter.current.shouldRecord(event)) addSystemChatMessage(event.message);
  }

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    void Promise.all([
      subscribeAppLogEvents((event) => {
        dispatch({ type: "log.added", log: event });
        if (event.level !== "info") {
          reportNotification(event.level, "log", event.message, event.id);
        }
      }),
      subscribeTwitchStatusEvents((event) => {
        const message = event.message ?? "";
        const isChatConnectionEvent =
          message.includes("チャンネル") ||
          message.includes("チャット受信") ||
          message.includes("EventSub");
        if (isChatConnectionEvent) {
          dispatch({ type: "twitch.connectionStatus", status: event.status });
        }
        if (event.status === "authRequired") {
          dispatch({ type: "twitch.authStatus", status: "expired" });
        }
        if (event.message && (event.status === "authRequired" || event.status === "error")) {
          reportNotification("error", "event", event.message);
        }
        const timelineEvent = timelineEventFromTwitchStatus(event);
        if (timelineEvent) routeSystemTimelineEvent(timelineEvent);
      }),
      subscribeTwitchChatMessageEvents((message) => {
        dispatch({
          type: "chat.message",
          message: {
            ...message,
            kind: "user",
            status: "queued",
          },
        });
      }),
      subscribeSpeechStatusEvents((event) => {
        dispatch({ type: "speech.status", status: event.status });
        if (event.message && (event.status === "disconnected" || event.status === "error")) {
          reportNotification("error", "event", event.message);
          routeSystemTimelineEvent(speechRecoveryTimelineEvent(event.message, event.status));
        }
      }),
      subscribeSpeechQueueUpdatedEvents((event) => {
        dispatch({ type: "queue.changed", items: event.items ?? [] });
        if (event.warning) {
          reportNotification("warning", "event", event.warning);
        }
      }),
    ]).then((listeners) => {
      unlisten.push(...listeners);
    });

    return () => {
      for (const dispose of unlisten) {
        dispose();
      }
    };
  }, []);

  useEffect(() => {
    if (
      autoConnectAttempted.current ||
      !state.settings?.twitch.autoConnect ||
      state.twitchAuthStatus !== "authenticated" ||
      state.twitchConnectionStatus !== "disconnected"
    ) {
      return;
    }

    autoConnectAttempted.current = true;
    void handleTwitchConnect({ automatic: true });
  }, [state.settings?.twitch.autoConnect, state.twitchAuthStatus, state.twitchConnectionStatus]);

  useEffect(() => {
    const shouldPoll =
      state.settings &&
      (state.speechStatus === "disconnected" || state.speechStatus === "error");

    if (!shouldPoll) {
      return;
    }

    let cancelled = false;
    const pollSpeechHealth = async () => {
      try {
        const message = await speechHealthProbe();
        if (cancelled) {
          return;
        }
        dispatch({ type: "speech.status", status: "idle" });
        reportInfo(message, "event");
        routeSystemTimelineEvent(speechRecoveryTimelineEvent(message, "idle"));
      } catch {
        // Keep the existing error visible while waiting for BouyomiChan to become reachable.
      }
    };

    void pollSpeechHealth();
    const intervalId = window.setInterval(() => {
      void pollSpeechHealth();
    }, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [state.settings, state.speechStatus]);

  async function handleSpeechTest(text?: string) {
    try {
      const speechText = typeof text === "string" ? text : "テスト読み上げです。";
      dispatch({ type: "speech.status", status: "speaking" });
      await speechTest(speechText);
      dispatch({ type: "speech.status", status: "idle" });
      reportInfo("テスト読み上げを送信しました。");
    } catch (error) {
      dispatch({ type: "speech.status", status: "error" });
      reportError(error);
    }
  }

  async function handleSpeechHealthCheck() {
    try {
      const message = await speechHealthCheck();
      dispatch({ type: "speech.status", status: "idle" });
      reportInfo(message);
    } catch (error) {
      dispatch({ type: "speech.status", status: "disconnected" });
      reportError(error);
    }
  }

  async function handleSpeechDiagnostics(): Promise<BouyomiConnectionDiagnostics> {
    try {
      const diagnostics = await speechConnectionDiagnostics();
      reportInfo(diagnostics.recommendation);
      return diagnostics;
    } catch (error) {
      reportError(error);
      throw error;
    }
  }

  async function handleSettingsUpdate(patch: Partial<AppSettings>) {
    try {
      const settings = await updateSettings(patch);
      dispatch({ type: "settings.loaded", settings });
    } catch (error) {
      reportError(error);
    }
  }

  async function handleTwitchStartAuth() {
    try {
      const prompt = await twitchStartAuth();
      dispatch({ type: "twitch.authPrompt", prompt });
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      reportInfo("Twitch の認証コードを発行しました。");
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "error" });
      reportError(error);
    }
  }

  useEffect(() => {
    if (!state.twitchAuthPrompt) {
      return;
    }

    if (getDeviceAuthRemainingSeconds(state.twitchAuthPrompt.expiresAtMs) === 0) {
      return;
    }

    const delay = Math.max(state.twitchAuthPrompt.interval, 1) * 1000;
    const timer = window.setTimeout(() => {
      if (state.twitchAuthPrompt && getDeviceAuthRemainingSeconds(state.twitchAuthPrompt.expiresAtMs) > 0) {
        void handleTwitchPollAuth({ quietWaiting: true });
      }
    }, delay);

    return () => window.clearTimeout(timer);
  }, [state.twitchAuthPrompt]);

  async function handleTwitchPollAuth(options: { quietWaiting?: boolean } = {}) {
    try {
      const result = await twitchPollAuth();
      if (result.status === "authorized") {
        dispatch({ type: "twitch.authStatus", status: "authenticated" });
        dispatch({ type: "twitch.authPrompt", prompt: undefined });
        dispatch({ type: "twitch.profile", profile: result.profile });
        dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
        reportInfo(`Twitch に ${result.profile.login} としてログインしました。`);
        if (result.storageWarning) {
          reportNotification("warning", "system", result.storageWarning);
          addSystemChatMessage(result.storageWarning);
        }
      } else {
        if (state.twitchAuthPrompt && (result.status === "pending" || result.status === "slowDown")) {
          dispatch({
            type: "twitch.authPrompt",
            prompt: {
              ...state.twitchAuthPrompt,
              interval: result.interval,
            },
          });
        }
        if (!options.quietWaiting || (result.status !== "pending" && result.status !== "slowDown")) {
          if (result.status === "pending" || result.status === "slowDown") {
            reportInfo(result.message, "event");
          } else {
            reportNotification(result.status === "denied" || result.status === "expired" ? "warning" : "info", "event", result.message);
          }
        }
        if (result.status === "expired" || result.status === "denied") {
          dispatch({ type: "twitch.authPrompt", prompt: undefined });
        }
      }
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "error" });
      reportError(error);
    }
  }

  async function handleTwitchValidateAuth() {
    try {
      const result = await twitchValidateAuth();
      dispatch({ type: "twitch.authStatus", status: "authenticated" });
      dispatch({ type: "twitch.profile", profile: result.profile });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      reportInfo("Twitch 認証は有効です。");
      if (result.storageWarning) {
        reportNotification("warning", "system", result.storageWarning);
        addSystemChatMessage(result.storageWarning);
      }
      return true;
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      dispatch({ type: "twitch.authPrompt", prompt: undefined });
      dispatch({ type: "twitch.profile", profile: undefined });
      reportError(error);
      return false;
    }
  }

  async function handleTwitchConnect({ automatic = false }: { automatic?: boolean } = {}) {
    try {
      const channelLogin = state.settings?.twitch.channelLogin;
      dispatch({ type: "twitch.connectionStatus", status: "connecting" });
      if (automatic) routeSystemTimelineEvent(autoConnectTimelineEvent("started", "Twitch チャットの自動接続を開始します。"));
      await twitchConnect(channelLogin);
      reportInfo("Twitch チャット接続を開始しました。");
    } catch (error) {
      dispatch({ type: "twitch.connectionStatus", status: "error" });
      reportError(error);
      if (automatic) routeSystemTimelineEvent(autoConnectTimelineEvent("failed", `Twitch チャットの自動接続に失敗しました: ${String(error)}`));
    }
  }

  async function handleTwitchStopChat() {
    const shouldConfirm = state.settings?.twitch.confirmBeforeStopChat ?? true;
    if (shouldConfirm && !window.confirm("Twitch チャット受信を停止しますか？")) {
      return;
    }

    try {
      await twitchStopChat();
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
    } catch (error) {
      dispatch({ type: "twitch.connectionStatus", status: "error" });
      reportError(error);
    }
  }

  async function handleTwitchDisconnect() {
    if (!window.confirm("Twitch 連携を解除しますか？")) {
      return;
    }

    try {
      await twitchDisconnect();
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      dispatch({ type: "twitch.authPrompt", prompt: undefined });
      dispatch({ type: "twitch.profile", profile: undefined });
    } catch (error) {
      reportError(error);
    }
  }

  async function handleOpenExternalUrl(url: string) {
    try {
      await appOpenExternalUrl(url);
    } catch (error) {
      reportError(error);
    }
  }

  async function handleSpeechControl(command: "pause" | "resume" | "skip" | "clear") {
    if (command === "clear" && !window.confirm("待機中の読み上げをクリアしますか？")) {
      return;
    }

    try {
      await speechControl(command);
      dispatch({ type: "speech.status", status: command === "pause" ? "paused" : "idle" });
    } catch (error) {
      dispatch({ type: "speech.status", status: "error" });
      reportError(error);
    }
  }

  useStreamHotkeys({
    onToggleSpeech: () => {
      void handleSpeechControl(state.speechStatus === "paused" ? "resume" : "pause");
    },
    onSkipSpeech: () => {
      void handleSpeechControl("skip");
    },
    onOpenSettings: () => navigate("/settings"),
  });

  async function handleQueueReload() {
    try {
      await speechQueueReload();
    } catch (error) {
      reportError(error);
    }
  }

  async function handleQueueRemove(itemId: string) {
    try {
      await speechQueueRemove(itemId);
    } catch (error) {
      reportError(error);
    }
  }

  async function handleQueueDismiss(itemId: string) {
    try {
      await speechQueueDismiss(itemId);
    } catch (error) {
      reportError(error);
    }
  }

  async function handleQueueDismissHistory() {
    if (!window.confirm("表示中の読み上げ履歴をクリアしますか？")) {
      return;
    }

    try {
      await speechQueueDismissHistory();
    } catch (error) {
      reportError(error);
    }
  }

  async function handleQueueRetry(itemId: string) {
    try {
      await speechQueueRetry(itemId);
    } catch (error) {
      reportError(error);
    }
  }

  async function handleLauncherAdd(paths: string[]) {
    try {
      const items = await launcherAdd(paths);
      dispatch({ type: "launcher.changed", items });
      return items;
    } catch (error) {
      reportError(error);
      throw error;
    }
  }

  async function handleLauncherRemove(itemId: string) {
    try {
      const items = await launcherRemove(itemId);
      dispatch({ type: "launcher.changed", items });
      return items;
    } catch (error) {
      reportError(error);
      throw error;
    }
  }

  async function reportLauncherResult(result: LauncherLaunchResult) {
    if (result.failures.length > 0) {
      const firstFailure = result.failures[0];
      reportNotification("error", "command", `${firstFailure.displayName} を起動できませんでした: ${firstFailure.message}`);
    }
    return result;
  }

  async function handleLauncherLaunch(itemId: string) {
    try {
      return reportLauncherResult(await launcherLaunch(itemId));
    } catch (error) {
      reportError(error);
      throw error;
    }
  }

  async function handleLauncherLaunchAll() {
    try {
      return reportLauncherResult(await launcherLaunchAll());
    } catch (error) {
      reportError(error);
      throw error;
    }
  }

  return (
    <div className={APP_SHELL_CLASS_NAME}>
      <TitleBar scale={displayScale.scale} scaleMode={displayScale.mode} onScaleModeChange={displayScale.setMode} />
      <ActivityBar />
      <SidePanel
        state={state}
        onSpeechControl={handleSpeechControl}
        onTwitchConnect={handleTwitchConnect}
        onTwitchStopChat={handleTwitchStopChat}
        onWarningsClear={() => dispatch({ type: "warnings.cleared" })}
      />
      <MainView
        state={state}
        showStartupGuide={showStartupGuideForSession}
        onSettingsUpdate={handleSettingsUpdate}
        onSpeechHealthCheck={handleSpeechHealthCheck}
        onSpeechDiagnostics={handleSpeechDiagnostics}
        onSpeechTest={handleSpeechTest}
        onSpeechControl={handleSpeechControl}
        onQueueReload={handleQueueReload}
        onQueueRemove={handleQueueRemove}
        onQueueDismiss={handleQueueDismiss}
        onQueueDismissHistory={handleQueueDismissHistory}
        onQueueRetry={handleQueueRetry}
        onLauncherAdd={handleLauncherAdd}
        onLauncherRemove={handleLauncherRemove}
        onLauncherLaunch={handleLauncherLaunch}
        onLauncherLaunchAll={handleLauncherLaunchAll}
        onTwitchStartAuth={handleTwitchStartAuth}
        onTwitchPollAuth={handleTwitchPollAuth}
        onTwitchValidateAuth={handleTwitchValidateAuth}
        onTwitchDisconnect={handleTwitchDisconnect}
        onOpenExternalUrl={handleOpenExternalUrl}
      />
      <StatusBar state={state} />
      <LiveStatusAnnouncer state={state} />
      <ResizeHandles />
    </div>
  );
}
