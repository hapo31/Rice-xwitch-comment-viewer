import { useEffect, useReducer } from "react";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { SidePanel } from "./components/SidePanel";
import { StatusBar } from "./components/StatusBar";
import { appReducer, initialAppState } from "./stores/appStore";
import { getSettings, speechConnectionDiagnostics, speechControl, speechHealthCheck, speechTest, updateSettings } from "./tauri/client";
import type { AppSettings, BouyomiConnectionDiagnostics } from "./types";

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);

  useEffect(() => {
    getSettings()
      .then((settings) => dispatch({ type: "settings.loaded", settings }))
      .catch(() => dispatch({ type: "warning.added", warning: "設定の読み込みに失敗しました。" }));
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
    <div className="grid h-full grid-cols-[48px_280px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_24px] bg-zinc-950 text-zinc-100">
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
      />
      <StatusBar state={state} />
    </div>
  );
}
