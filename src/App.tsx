import { useEffect, useReducer } from "react";
import { ActivityBar } from "./components/ActivityBar";
import { MainView } from "./components/MainView";
import { SidePanel } from "./components/SidePanel";
import { StatusBar } from "./components/StatusBar";
import { appReducer, initialAppState } from "./stores/appStore";
import { getSettings, speechControl, speechHealthCheck, speechTest, updateSettings } from "./tauri/client";
import type { AppSettings } from "./types";

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState);

  useEffect(() => {
    getSettings()
      .then((settings) => dispatch({ type: "settings.loaded", settings }))
      .catch(() => dispatch({ type: "warning.added", warning: "設定の読み込みに失敗しました。" }));
  }, []);

  async function handleSpeechTest(text = "テスト発話です。") {
    try {
      await speechTest(text);
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  async function handleSpeechHealthCheck() {
    try {
      const message = await speechHealthCheck();
      dispatch({ type: "warning.added", warning: message });
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
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
    } catch (error) {
      dispatch({ type: "warning.added", warning: String(error) });
    }
  }

  return (
    <div className="grid h-full grid-cols-[48px_280px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_24px] bg-zinc-950 text-zinc-100">
      <ActivityBar activeView={state.activeView} onChange={(view) => dispatch({ type: "view.changed", view })} />
      <SidePanel state={state} onSpeechControl={handleSpeechControl} onSpeechTest={handleSpeechTest} />
      <MainView
        state={state}
        onSettingsUpdate={handleSettingsUpdate}
        onSpeechHealthCheck={handleSpeechHealthCheck}
        onSpeechTest={handleSpeechTest}
      />
      <StatusBar state={state} />
    </div>
  );
}
