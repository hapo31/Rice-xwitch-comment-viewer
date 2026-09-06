import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useBlocker, useNavigate } from "react-router-dom";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { ResizeHandles, TitleBar } from "./components/TitleBar";
import { useDisplayScale } from "./hooks/useDisplayScale";
import { useStreamHotkeys } from "./hooks/useStreamHotkeys";
import { getDeviceAuthRemainingSeconds } from "./features/auth/deviceAuthExpiry";
import { APP_SHELL_CLASS_NAME } from "./layout/appShell";
import { claimStartupGuideForSession } from "./presentation/startupGuide";
import { autoConnectTimelineEvent, speechRecoveryTimelineEvent, SystemTimelineRouter, timelineEventFromTwitchStatus } from "./presentation/systemTimeline";
import { AuthOperationController } from "./authOperation";

import { hasActiveTwitchChat, hasPendingSpeechWork, requiresExitConfirmation } from "./exitSafety";
import { type AppAction, initialAppState } from "./stores/appStore";
import { useConnectionSelector, useDomainStores, useQueueSelector, useSettingsSelector } from "./stores/domainStores";
import { DomainLiveStatusAnnouncer, DomainSidePanel, DomainStatusBar } from "./components/domainShellViews";
import { utcNow } from "./time";
import { subscribeWithCleanup } from "./tauri/subscriptions";
import {
  ActiveOperationsExitDialog,
  createNativeCloseHandler,
  UnsavedChangesContext,
  UnsavedChangesDialog,
  type UnsavedChange,
} from "./unsavedChanges";
import {
  appExit,
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
  isDesktopRuntime,
} from "./tauri/client";
import { createSettingsMutationOrchestrator, dispatchDomainAction, restoreStartupAuth, subscribeDomainEvents } from "./orchestration/domainOrchestration";
import type { AppSettings, AppSettingsPatch, BouyomiConnectionDiagnostics, LauncherLaunchResult, NotificationSeverity, NotificationSource } from "./types";

const showStartupGuideForSession = claimStartupGuideForSession(window.sessionStorage);



export function AppShell() {
  const stores = useDomainStores();
  const connection = useConnectionSelector((value) => value);
  const settings = useSettingsSelector((value) => value.settings);
  const queueItems = useQueueSelector((value) => value.items);
  const state = useMemo(() => ({ ...initialAppState, ...connection, settings, queueItems }), [connection, settings, queueItems]);
  const dispatch = useCallback((action: AppAction) => dispatchDomainAction(stores, action), [stores]);
  const navigate = useNavigate();
  const displayScale = useDisplayScale();
  const autoConnectAttempted = useRef(false);
  const settingsMutation = useRef(createSettingsMutationOrchestrator({ updateSettings, onSettingsLoaded: (nextSettings) => { settingsSnapshot.current = nextSettings; dispatch({ type: "settings.loaded", settings: nextSettings }); }, onError: (error) => reportError(error) }));
  const settingsSnapshot = useRef<AppSettings>();
  const startupAuthAttempted = useRef(false);
  const authOperations = useRef(new AuthOperationController());
  const systemTimelineRouter = useRef(new SystemTimelineRouter());
  const unsavedChanges = useRef(new Map<string, UnsavedChange>());
  const closeConfirmationRequiredRef = useRef(false);
  const [, setUnsavedChangesVersion] = useState(0);
  const [closeRequested, setCloseRequested] = useState(false);
  const [isClosing, setIsClosing] = useState(false);

  const unsavedChangesRegistry = useMemo(() => ({
    register(id: string, change: UnsavedChange) {
      unsavedChanges.current.set(id, change);
      setUnsavedChangesVersion((version) => version + 1);
    },
    unregister(id: string) {
      unsavedChanges.current.delete(id);
      setUnsavedChangesVersion((version) => version + 1);
    },
  }), []);
  const activeUnsavedChange = [...unsavedChanges.current.values()].find((change) => change.isDirty);
  const hasActiveChat = hasActiveTwitchChat(state.twitchConnectionStatus);
  const hasPendingSpeech = hasPendingSpeechWork(state.speechStatus, state.queueItems);
  const exitConfirmationRequired = requiresExitConfirmation(
    state.twitchConnectionStatus,
    state.speechStatus,
    state.queueItems,
    Boolean(activeUnsavedChange),
  );
  closeConfirmationRequiredRef.current = exitConfirmationRequired;
  const blocker = useBlocker(Boolean(activeUnsavedChange));

  const completeWindowClose = useCallback(async () => {
    setCloseRequested(false);
    setIsClosing(true);
    const results = await Promise.allSettled([
      hasActiveChat ? twitchStopChat() : Promise.resolve(),
      hasPendingSpeech ? speechControl("clear") : Promise.resolve(),
    ]);
    if (hasActiveChat && results[0].status === "fulfilled") {
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
    }
    if (hasPendingSpeech && results[1].status === "fulfilled") {
      dispatch({ type: "speech.status", status: "idle" });
    }
    for (const result of results) {
      if (result.status === "rejected") reportError(result.reason);
    }
    try {
      await appExit();
    } catch (error) {
      setIsClosing(false);
      reportError(error);
    }
  }, [hasActiveChat, hasPendingSpeech]);

  const requestWindowClose = useCallback(() => {
    if (exitConfirmationRequired) {
      setCloseRequested(true);
      return;
    }
    void completeWindowClose();
  }, [completeWindowClose, exitConfirmationRequired]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;

    return subscribeWithCleanup([() => getCurrentWindow().onCloseRequested(createNativeCloseHandler(closeConfirmationRequiredRef, () => {
      setCloseRequested(true);
    }))], () => reportNotification(
      "warning",
      "event",
      "終了確認の監視に失敗しました。未保存の変更を確認してから終了してください。",
      "app-close-subscription",
    ));
  }, []);

  useEffect(() => {
    const preventUnload = (event: BeforeUnloadEvent) => {
      if (!activeUnsavedChange) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", preventUnload);
    return () => window.removeEventListener("beforeunload", preventUnload);
  }, [activeUnsavedChange]);

  useEffect(() => {
    Promise.all([getSettings(), takeSettingsRecoveryNotice()])
      .then(([settings, recoveryNotice]) => {
        settingsSnapshot.current = settings;
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

    const operation = authOperations.current.begin();
    dispatch({ type: "twitch.authStatus", status: "checking" });
    void restoreStartupAuth({
      getStoredAuth: twitchGetStoredAuth,
      validateAuth: twitchValidateAuth,
      reportSystemMessage: addSystemChatMessage,
    }).then((auth) => {
      if (!authOperations.current.isCurrent(operation)) return;
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
        receivedAt: utcNow(),
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

  useEffect(() => subscribeDomainEvents({
    stores,
    bridge: { subscribeAppLogEvents, subscribeTwitchStatusEvents, subscribeTwitchChatMessageEvents, subscribeSpeechStatusEvents, subscribeSpeechQueueUpdatedEvents },
    reportNotification,
    routeSystemTimelineEvent: (event) => routeSystemTimelineEvent(event as Parameters<SystemTimelineRouter["shouldRecord"]>[0]),
    speechRecoveryMessage: speechRecoveryTimelineEvent,
    twitchTimelineEvent: timelineEventFromTwitchStatus,
  }), []);
  useEffect(() => {
    if (autoConnectAttempted.current || !state.settings?.twitch.autoConnect || state.twitchAuthStatus !== "authenticated" || state.twitchConnectionStatus !== "disconnected") return;
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

  function handleSettingsUpdate(patch: AppSettingsPatch): Promise<boolean> {
    return settingsMutation.current.mutate(patch);
  }

  async function handleTwitchStartAuth() {
    const operation = authOperations.current.begin();
    dispatch({ type: "twitch.authStatus", status: "authorizing" });
    try {
      const prompt = await twitchStartAuth();
      if (!authOperations.current.isCurrent(operation)) return;
      dispatch({ type: "twitch.authPrompt", prompt });
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.profile", profile: undefined });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      reportInfo("Twitch の認証コードを発行しました。");
    } catch (error) {
      if (!authOperations.current.isCurrent(operation)) return;
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
    const operation = authOperations.current.tryBeginPoll();
    if (operation === undefined) return;
    dispatch({ type: "twitch.authStatus", status: "polling" });
    try {
      const result = await twitchPollAuth();
      if (!authOperations.current.isCurrent(operation)) return;
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
        dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
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
      if (!authOperations.current.isCurrent(operation)) return;
      dispatch({ type: "twitch.authStatus", status: "error" });
      reportError(error);
    } finally {
      authOperations.current.finishPoll(operation);
    }
  }

  async function handleTwitchValidateAuth() {
    const operation = authOperations.current.begin();
    dispatch({ type: "twitch.authStatus", status: "checking" });
    try {
      const result = await twitchValidateAuth();
      if (!authOperations.current.isCurrent(operation)) return false;
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
      if (!authOperations.current.isCurrent(operation)) return false;
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
      await settingsMutation.current.waitForIdle();
      const channelLogin = settingsSnapshot.current?.twitch.channelLogin ?? state.settings?.twitch.channelLogin;
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

    const operation = authOperations.current.begin();
    dispatch({ type: "twitch.authStatus", status: "disconnecting" });
    try {
      await twitchDisconnect();
      if (!authOperations.current.isCurrent(operation)) return;
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      dispatch({ type: "twitch.authPrompt", prompt: undefined });
      dispatch({ type: "twitch.profile", profile: undefined });
    } catch (error) {
      if (!authOperations.current.isCurrent(operation)) return;
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

  const handleLauncherAdd = useCallback(async (paths: string[]) => {
    try {
      const items = await launcherAdd(paths);
      dispatch({ type: "launcher.changed", items });
      return items;
    } catch (error) {
      reportError(error);
      throw error;
    }
  }, []);

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
    <UnsavedChangesContext.Provider value={unsavedChangesRegistry}>
    <div className={APP_SHELL_CLASS_NAME}>
      <TitleBar scale={displayScale.scale} scaleMode={displayScale.mode} onScaleModeChange={displayScale.setMode} onClose={requestWindowClose} />
      <ActivityBar />
      <DomainSidePanel
        onSpeechControl={handleSpeechControl}
        onTwitchConnect={handleTwitchConnect}
        onTwitchStopChat={handleTwitchStopChat}
        onWarningsClear={() => dispatch({ type: "warnings.cleared" })}
      />
      <MainView
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
      <DomainStatusBar />
      <DomainLiveStatusAnnouncer />
      <ResizeHandles />
      {(blocker.state === "blocked" || closeRequested) && activeUnsavedChange && (
        <UnsavedChangesDialog
          onCancel={() => {
            if (blocker.state === "blocked") blocker.reset();
            setCloseRequested(false);
          }}
          onDiscard={() => {
            activeUnsavedChange.discard();
            if (closeRequested) {
              void completeWindowClose();
            } else if (blocker.state === "blocked") {
              blocker.proceed();
            }
          }}
          onSave={() => {
            void activeUnsavedChange.save().then((saved) => {
              if (!saved) return;
              if (closeRequested) {
                void completeWindowClose();
              } else if (blocker.state === "blocked") {
                blocker.proceed();
              }
            });
          }}
        />
      )}
      {closeRequested && !activeUnsavedChange && (
        <ActiveOperationsExitDialog
          isClosing={isClosing}
          onCancel={() => setCloseRequested(false)}
          onConfirm={() => void completeWindowClose()}
        />
      )}
    </div>
    </UnsavedChangesContext.Provider>
  );
}
