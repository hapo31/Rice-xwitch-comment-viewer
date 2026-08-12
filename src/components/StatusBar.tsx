import { useEffect, useState } from "react";
import { countIncompleteQueueItems } from "../presentation/queue";
import { speechStatusLabel } from "../presentation/chat";
import { warningNotifications, type AppState } from "../stores/appStore";
import { getAppBuildInfo, type AppBuildInfo } from "../tauri/client";
import { formatBouyomiAddress } from "../validation";

interface StatusBarProps {
  state: AppState;
}

export function StatusBar({ state }: StatusBarProps) {
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo>();

  useEffect(() => {
    void getAppBuildInfo().then(setBuildInfo).catch(() => undefined);
  }, []);

  const host = state.settings?.speech.bouyomiHost ?? "127.0.0.1";
  const port = state.settings?.speech.bouyomiPort ?? 50001;
  const queuedCount = countIncompleteQueueItems(state.queueItems);
  const warningCount = warningNotifications(state.notifications).length;
  const twitchAuthLabel = {
    unauthenticated: "未認証",
    authorizing: "認証開始中",
    polling: "認証確認中",
    checking: "認証確認中",
    authenticated: "ログイン済み",
    expired: "再ログイン必要",
    disconnecting: "認証解除中",
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
        <StatusItem label="棒読みちゃん" value={`${speechStatusLabel(state.speechStatus)} ${formatBouyomiAddress(host, port)}`} />
        <StatusItem label="キュー" value={String(queuedCount)} />
        <StatusItem label="Warnings" value={String(warningCount)} tone={warningCount > 0 ? "warning" : "default"} />
      </div>
      <div className="text-zinc-400">{buildInfo ? formatBuildLabel(buildInfo) : "Rice"}</div>
    </footer>
  );
}

export function formatBuildLabel({ version, isDev, commitHash }: AppBuildInfo): string {
  if (!isDev) {
    return `Rice ${version}`;
  }

  return `Rice ${version} (dev${commitHash ? ` ${commitHash}` : ""})`;
}

function StatusItem({ label, value, tone = "default" }: { label: string; value: string; tone?: "default" | "warning" }) {
  return (
    <span className={`min-w-0 truncate ${tone === "warning" ? "text-amber-300" : ""}`}>
      <span className="text-zinc-400">{label}: </span>
      {value}
    </span>
  );
}
