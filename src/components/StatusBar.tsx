import { countIncompleteQueueItems } from "../presentation/queue";
import type { AppState } from "../stores/appStore";

interface StatusBarProps {
  state: AppState;
}

export function StatusBar({ state }: StatusBarProps) {
  const host = state.settings?.speech.bouyomiHost ?? "127.0.0.1";
  const port = state.settings?.speech.bouyomiPort ?? 50001;
  const queuedCount = countIncompleteQueueItems(state.queueItems);
  const twitchAuthLabel = {
    unauthenticated: "未認証",
    authenticated: "ログイン済み",
    expired: "再ログイン必要",
    error: "認証エラー",
  }[state.twitchAuthStatus];
  const twitchConnectionLabel = {
    disconnected: "未接続",
    connecting: "接続中",
    connected: "受信中",
    reconnecting: "再接続中",
    authRequired: "再ログイン必要",
    error: "接続エラー",
  }[state.twitchConnectionStatus];

  return (
    <footer className="col-span-3 row-start-3 flex items-center justify-between border-t border-zinc-800 bg-zinc-900 px-2 text-xs text-zinc-300">
      <div className="flex min-w-0 items-center gap-4">
        <StatusItem label="Twitch" value={`${twitchAuthLabel} / ${twitchConnectionLabel}`} />
        <StatusItem label="Bouyomi" value={`${state.speechStatus} ${host}:${port}`} />
        <StatusItem label="Queue" value={String(queuedCount)} />
        <StatusItem label="Warnings" value={String(state.warnings.length)} tone={state.warnings.length > 0 ? "warning" : "default"} />
      </div>
      <div className="text-zinc-500">Rice 0.1.0</div>
    </footer>
  );
}

function StatusItem({ label, value, tone = "default" }: { label: string; value: string; tone?: "default" | "warning" }) {
  return (
    <span className={`min-w-0 truncate ${tone === "warning" ? "text-amber-300" : ""}`}>
      <span className="text-zinc-500">{label}: </span>
      {value}
    </span>
  );
}
