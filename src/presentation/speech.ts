import type { AppState } from "../stores/appState";
import type { SpeechAdapterHealth, SpeechQueuePhase } from "../types";

export const speechHealthLabels: Record<SpeechAdapterHealth, string> = {
  unknown: "未確認",
  connected: "接続確認済み",
  disconnected: "未接続",
  error: "接続エラー",
};
export const speechQueuePhaseLabels: Record<SpeechQueuePhase, string> = {
  idle: "待機中",
  speaking: "処理中",
  paused: "一時停止中",
  error: "手動再試行待ち",
};

export function isSpeechReady(
  state: Pick<AppState, "speechAdapterHealth" | "speechQueuePhase" | "settings">,
): boolean {
  return (
    state.speechAdapterHealth === "connected" &&
    (state.speechQueuePhase === "idle" || state.speechQueuePhase === "speaking") &&
    state.settings?.speech.autoSpeak === true
  );
}
