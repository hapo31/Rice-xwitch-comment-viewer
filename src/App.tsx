import { useEffect, useReducer, useRef } from "react";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { SidePanel } from "./components/SidePanel";
import { StatusBar } from "./components/StatusBar";
import { ResizeHandles, TitleBar } from "./components/TitleBar";
import { useDisplayScale } from "./hooks/useDisplayScale";
import { appReducer, initialAppState } from "./stores/appStore";
import {
  appOpenExternalUrl,
  getSettings,
  subscribeAppLogEvents,
  subscribeSpeechQueueUpdatedEvents,
  subscribeSpeechStatusEvents,
  subscribeTwitchChatMessageEvents,
  subscribeTwitchStatusEvents,
  speechConnectionDiagnostics,
  speechControl,
  speechHealthCheck,
  speechHealthProbe,
  speechQueueReload,
  speechQueueRemove,
  speechTest,
  twitchConnect,
  twitchDisconnect,
  twitchGetStoredAuth,
  twitchPollAuth,
  twitchStartAuth,
  twitchStopChat,
  twitchValidateAuth,
  updateSettings,
} from "./tauri/client";
import type { AppSettings, BouyomiConnectionDiagnostics } from "./types";

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const displayScale = useDisplayScale();
  const autoConnectAttempted = useRef(false);
  const speechRecoveryPollingEnabled = useRef(false);

  useEffect(() => {
    getSettings()
      .then((settings) => dispatch({ type: "settings.loaded", settings }))
      .catch(() => dispatch({ type: "warning.added", warning: "設定の読み込みに失敗しました。" }));
    twitchGetStoredAuth()
      .then((profile) => {
        if (!profile) {
          return;
        }
        dispatch({ type: "twitch.profile", profile });
        dispatch({ type: "twitch.authStatus", status: "authenticated" });
      })
      .catch(() => dispatch({ type: "warning.added", warning: "保存済み Twitch 認証の確認に失敗しました。" }));
  }, []);

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    void Promise.all([
      subscribeAppLogEvents((event) => {
        dispatch({ type: "log.added", log: event });
        if (event.level !== "info") {
          dispatch({ type: "warning.added", warning: event.message });
        }
      }),
      subscribeTwitchStatusEvents((event) => {
        const message = event.message ?? "";
        const isChatConnectionEvent =
          message.includes("チャンネル") ||
          message.includes("コメント受信") ||
          message.includes("EventSub");
        if (isChatConnectionEvent) {
          dispatch({ type: "twitch.connectionStatus", status: event.status });
        }
        if (event.status === "authRequired") {
          dispatch({ type: "twitch.authStatus", status: "expired" });
        }
        if (event.message && (event.status === "authRequired" || event.status === "error")) {
          dispatch({ type: "warning.added", warning: event.message });
        }
      }),
      subscribeTwitchChatMessageEvents((message) => {
        dispatch({
          type: "chat.message",
          message: {
            ...message,
            status: "queued",
          },
        });
      }),
      subscribeSpeechStatusEvents((event) => {
        dispatch({ type: "speech.status", status: event.status });
        if (event.message && (event.status === "disconnected" || event.status === "error")) {
          speechRecoveryPollingEnabled.current = true;
          dispatch({ type: "warning.added", warning: event.message });
        }
      }),
      subscribeSpeechQueueUpdatedEvents((event) => {
        dispatch({ type: "queue.changed", items: event.items ?? [] });
        if (event.warning) {
          dispatch({ type: "warning.added", warning: event.warning });
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
    void handleTwitchConnect();
  }, [state.settings?.twitch.autoConnect, state.twitchAuthStatus, state.twitchConnectionStatus]);

  useEffect(() => {
    const shouldPoll =
      speechRecoveryPollingEnabled.current &&
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
        speechRecoveryPollingEnabled.current = false;
        dispatch({ type: "speech.status", status: "idle" });
        dispatch({ type: "warning.added", warning: message });
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
      const speechText = typeof text === "string" ? text : "テスト発話です。";
      dispatch({ type: "speech.status", status: "speaking" });
      await speechTest(speechText);
      dispatch({ type: "speech.status", status: "idle" });
      dispatch({ type: "warning.added", warning: "テスト発話を送信しました。" });
    } catch (error) {
      speechRecoveryPollingEnabled.current = true;
      dispatch({ type: "speech.status", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleSpeechHealthCheck() {
    try {
      const message = await speechHealthCheck();
      dispatch({ type: "speech.status", status: "idle" });
      dispatch({ type: "warning.added", warning: message });
    } catch (error) {
      speechRecoveryPollingEnabled.current = true;
      dispatch({ type: "speech.status", status: "disconnected" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleSpeechDiagnostics(): Promise<BouyomiConnectionDiagnostics> {
    try {
      const diagnostics = await speechConnectionDiagnostics();
      dispatch({ type: "warning.added", warning: diagnostics.recommendation });
      return diagnostics;
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
      throw error;
    }
  }

  async function handleSettingsUpdate(patch: Partial<AppSettings>) {
    try {
      const settings = await updateSettings(patch);
      dispatch({ type: "settings.loaded", settings });
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleTwitchStartAuth() {
    try {
      const prompt = await twitchStartAuth();
      dispatch({ type: "twitch.authPrompt", prompt });
      dispatch({ type: "twitch.authStatus", status: "unauthenticated" });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      dispatch({ type: "warning.added", warning: "Twitch の認証コードを発行しました。" });
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  useEffect(() => {
    if (!state.twitchAuthPrompt) {
      return;
    }

    const delay = Math.max(state.twitchAuthPrompt.interval, 1) * 1000;
    const timer = window.setTimeout(() => {
      void handleTwitchPollAuth({ quietWaiting: true });
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
        dispatch({ type: "warning.added", warning: `Twitch に ${result.profile.login} としてログインしました。` });
        if (result.storageWarning) {
          dispatch({ type: "warning.added", warning: result.storageWarning });
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
          dispatch({ type: "warning.added", warning: result.message });
        }
        if (result.status === "expired" || result.status === "denied") {
          dispatch({ type: "twitch.authPrompt", prompt: undefined });
        }
      }
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleTwitchValidateAuth() {
    try {
      const result = await twitchValidateAuth();
      dispatch({ type: "twitch.authStatus", status: "authenticated" });
      dispatch({ type: "twitch.profile", profile: result.profile });
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
      dispatch({ type: "warning.added", warning: "Twitch 認証は有効です。" });
      if (result.storageWarning) {
        dispatch({ type: "warning.added", warning: result.storageWarning });
      }
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "expired" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleTwitchConnect() {
    try {
      const channelLogin = state.settings?.twitch.channelLogin;
      dispatch({ type: "twitch.connectionStatus", status: "connecting" });
      await twitchConnect(channelLogin);
      dispatch({ type: "warning.added", warning: "Twitch コメント接続を開始しました。" });
    } catch (error) {
      dispatch({ type: "twitch.connectionStatus", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleTwitchStopChat() {
    if (!window.confirm("Twitch コメント受信を停止しますか？")) {
      return;
    }

    try {
      await twitchStopChat();
      dispatch({ type: "twitch.connectionStatus", status: "disconnected" });
    } catch (error) {
      dispatch({ type: "twitch.connectionStatus", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
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
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleOpenExternalUrl(url: string) {
    try {
      await appOpenExternalUrl(url);
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleSpeechControl(command: "pause" | "resume" | "skip" | "clear") {
    if (command === "clear" && !window.confirm("読み上げキューをクリアしますか？")) {
      return;
    }

    try {
      await speechControl(command);
      dispatch({ type: "speech.status", status: command === "pause" ? "paused" : "idle" });
    } catch (error) {
      dispatch({ type: "speech.status", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleQueueReload() {
    try {
      await speechQueueReload();
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleQueueRemove(itemId: string) {
    try {
      await speechQueueRemove(itemId);
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  return (
    <div className="relative grid h-full grid-cols-[48px_280px_minmax(0,1fr)] grid-rows-[2rem_minmax(0,1fr)_24px] bg-zinc-950 text-zinc-100">
      <TitleBar scale={displayScale.scale} scaleMode={displayScale.mode} onScaleModeChange={displayScale.setMode} />
      <ActivityBar />
      <SidePanel
        state={state}
        onSpeechControl={handleSpeechControl}
        onSpeechTest={handleSpeechTest}
        onTwitchConnect={handleTwitchConnect}
        onTwitchStopChat={handleTwitchStopChat}
        onWarningsClear={() => dispatch({ type: "warnings.cleared" })}
      />
      <MainView
        state={state}
        onSettingsUpdate={handleSettingsUpdate}
        onSpeechHealthCheck={handleSpeechHealthCheck}
        onSpeechDiagnostics={handleSpeechDiagnostics}
        onSpeechTest={handleSpeechTest}
        onSpeechControl={handleSpeechControl}
        onQueueReload={handleQueueReload}
        onQueueRemove={handleQueueRemove}
        onTwitchStartAuth={handleTwitchStartAuth}
        onTwitchPollAuth={handleTwitchPollAuth}
        onTwitchValidateAuth={handleTwitchValidateAuth}
        onTwitchDisconnect={handleTwitchDisconnect}
        onOpenExternalUrl={handleOpenExternalUrl}
      />
      <StatusBar state={state} />
      <ResizeHandles />
    </div>
  );
}
