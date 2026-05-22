import { useEffect, useReducer } from "react";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { SidePanel } from "./components/SidePanel";
import { StatusBar } from "./components/StatusBar";
import { ResizeHandles, TitleBar } from "./components/TitleBar";
import { useDisplayScale } from "./hooks/useDisplayScale";
import { appReducer, initialAppState } from "./stores/appStore";
import {
  getSettings,
  speechConnectionDiagnostics,
  speechControl,
  speechHealthCheck,
  speechTest,
  twitchDisconnect,
  twitchGetStoredAuth,
  twitchPollAuth,
  twitchStartAuth,
  twitchValidateAuth,
  updateSettings,
} from "./tauri/client";
import type { AppSettings, BouyomiConnectionDiagnostics } from "./types";

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const displayScale = useDisplayScale();

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

  async function handleSpeechTest(text?: string) {
    try {
      const speechText = typeof text === "string" ? text : "テスト発話です。";
      dispatch({ type: "speech.status", status: "speaking" });
      await speechTest(speechText);
      dispatch({ type: "speech.status", status: "idle" });
      dispatch({ type: "warning.added", warning: "テスト発話を送信しました。" });
    } catch (error) {
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
      dispatch({ type: "warning.added", warning: "Twitch の認証コードを発行しました。" });
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "error" });
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleTwitchPollAuth() {
    try {
      const result = await twitchPollAuth();
      if (result.status === "authorized") {
        dispatch({ type: "twitch.authStatus", status: "authenticated" });
        dispatch({ type: "twitch.authPrompt", prompt: undefined });
        dispatch({ type: "twitch.profile", profile: result.profile });
        dispatch({ type: "warning.added", warning: `Twitch に ${result.profile.login} としてログインしました。` });
        if (result.storageWarning) {
          dispatch({ type: "warning.added", warning: result.storageWarning });
        }
      } else {
        dispatch({ type: "warning.added", warning: result.message });
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
      dispatch({ type: "warning.added", warning: "Twitch 認証は有効です。" });
      if (result.storageWarning) {
        dispatch({ type: "warning.added", warning: result.storageWarning });
      }
    } catch (error) {
      dispatch({ type: "twitch.authStatus", status: "expired" });
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
      dispatch({ type: "twitch.authPrompt", prompt: undefined });
      dispatch({ type: "twitch.profile", profile: undefined });
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

  return (
    <div className="relative grid h-full grid-cols-[48px_280px_minmax(0,1fr)] grid-rows-[2rem_minmax(0,1fr)_24px] bg-zinc-950 text-zinc-100">
      <TitleBar scale={displayScale.scale} scaleMode={displayScale.mode} onScaleModeChange={displayScale.setMode} />
      <ActivityBar activeView={state.activeView} onChange={(view) => dispatch({ type: "view.changed", view })} />
      <SidePanel
        state={state}
        onSpeechControl={handleSpeechControl}
        onSpeechTest={handleSpeechTest}
        onWarningsClear={() => dispatch({ type: "warnings.cleared" })}
      />
      <MainView
        state={state}
        onSettingsUpdate={handleSettingsUpdate}
        onSpeechHealthCheck={handleSpeechHealthCheck}
        onSpeechDiagnostics={handleSpeechDiagnostics}
        onSpeechTest={handleSpeechTest}
        onTwitchStartAuth={handleTwitchStartAuth}
        onTwitchPollAuth={handleTwitchPollAuth}
        onTwitchValidateAuth={handleTwitchValidateAuth}
        onTwitchDisconnect={handleTwitchDisconnect}
      />
      <StatusBar state={state} />
      <ResizeHandles />
    </div>
  );
}
