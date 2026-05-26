import { Pause, Play, Radio, RotateCcw, SkipForward, Square, Trash2 } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { countIncompleteQueueItems } from "../presentation/queue";
import { getRouteLabel } from "../routes";
import type { AppState } from "../stores/appStore";

interface SidePanelProps {
  state: AppState;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onTwitchConnect: () => void;
  onTwitchStopChat: () => void;
  onWarningsClear: () => void;
}

export function SidePanel({
  state,
  onSpeechControl,
  onTwitchConnect,
  onTwitchStopChat,
  onWarningsClear,
}: SidePanelProps) {
  const location = useLocation();
  const channel = state.settings?.twitch.channelLogin || "未設定";
  const queueCount = countIncompleteQueueItems(state.queueItems);
  const twitchAuthLabel = {
    unauthenticated: "未認証",
    authenticated: "ログイン済み",
    expired: "再ログイン必要",
    error: "認証エラー",
  }[state.twitchAuthStatus];
  const twitchAuthTone =
    state.twitchAuthStatus === "authenticated" ? "ok" : state.twitchAuthStatus === "error" ? "danger" : "muted";
  const twitchConnectionLabel = {
    disconnected: "未接続",
    connecting: "接続中",
    connected: "受信中",
    reconnecting: "再接続中",
    authRequired: "再ログイン必要",
    error: "接続エラー",
  }[state.twitchConnectionStatus];
  const twitchConnectionTone =
    state.twitchConnectionStatus === "connected"
      ? "ok"
      : state.twitchConnectionStatus === "connecting" || state.twitchConnectionStatus === "reconnecting"
        ? "active"
        : state.twitchConnectionStatus === "error" || state.twitchConnectionStatus === "authRequired"
          ? "danger"
          : "muted";
  const canStartChat =
    state.twitchAuthStatus === "authenticated" &&
    !["connecting", "connected", "reconnecting"].includes(state.twitchConnectionStatus);
  const canStopChat = ["connecting", "connected", "reconnecting", "error"].includes(state.twitchConnectionStatus);

  return (
    <aside className="col-start-2 row-start-2 flex min-h-0 flex-col overflow-hidden border-r border-zinc-800 bg-zinc-900">
      <div className="shrink-0 border-b border-zinc-800 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">
        {getRouteLabel(location.pathname)}
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-5 p-3 text-sm">
        <section className="shrink-0">
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">接続</h2>
          <div className="space-y-2">
            <PanelRow label="Twitch" value={twitchAuthLabel} tone={twitchAuthTone} />
            <PanelRow label="チャンネル" value={channel} to="/auth" title="Login 画面でチャンネルを設定" />
            <PanelRow label="サーバー接続" value={twitchConnectionLabel} tone={twitchConnectionTone} />
            <PanelRow label="読み上げ" value={state.speechStatus} tone={state.speechStatus === "idle" ? "ok" : "muted"} />
          </div>
        </section>

        <section className="shrink-0">
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">チャット受信</h2>
          <div className="grid grid-cols-2 gap-1">
            <CommandButton label="開始" icon={Radio} disabled={!canStartChat} onClick={onTwitchConnect} />
            <CommandButton label="停止" icon={Square} disabled={!canStopChat} onClick={onTwitchStopChat} danger />
          </div>
        </section>

        <section className="shrink-0">
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">キュー</h2>
          <div className="flex items-center justify-between border border-zinc-800 bg-zinc-850 px-3 py-2">
            <span className="text-zinc-300">待機中</span>
            <span className="font-mono text-zinc-100">{queueCount}</span>
          </div>
          <div className="mt-2 grid grid-cols-4 gap-1">
            <IconButton label="再開" icon={Play} onClick={() => onSpeechControl("resume")} />
            <IconButton label="一時停止" icon={Pause} onClick={() => onSpeechControl("pause")} />
            <IconButton label="スキップ" icon={SkipForward} onClick={() => onSpeechControl("skip")} />
            <IconButton label="クリア" icon={RotateCcw} onClick={() => onSpeechControl("clear")} />
          </div>
        </section>

        <section className="min-h-0 flex-1">
          <div className="mb-2 flex items-center justify-between gap-2">
            <h2 className="text-xs font-semibold text-zinc-400">警告</h2>
            <button
              type="button"
              aria-label="警告をクリア"
              title="警告をクリア"
              disabled={state.warnings.length === 0}
              onClick={onWarningsClear}
              className="flex h-7 w-7 items-center justify-center border border-zinc-800 bg-zinc-850 text-zinc-400 hover:border-zinc-600 hover:text-zinc-100 disabled:cursor-not-allowed disabled:text-zinc-700"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
          {state.warnings.length === 0 ? (
            <p className="text-xs text-zinc-500">現在の警告はありません。</p>
          ) : (
            <div className="max-h-full space-y-2 overflow-y-auto pr-1">
              {state.warnings.map((warning, index) => (
                <p key={`${index}-${warning}`} className="border-l-2 border-amber-400 bg-zinc-850 px-2 py-1 text-xs text-amber-200">
                  {warning}
                </p>
              ))}
            </div>
          )}
        </section>
      </div>
    </aside>
  );
}

function PanelRow({
  label,
  value,
  tone = "default",
  to,
  title,
}: {
  label: string;
  value: string;
  tone?: "default" | "ok" | "muted" | "danger" | "active";
  to?: string;
  title?: string;
}) {
  const dotClass =
    tone === "ok"
      ? "bg-emerald-400"
      : tone === "muted"
        ? "bg-zinc-600"
        : tone === "danger"
          ? "bg-rose-400"
          : tone === "active"
            ? "bg-sky-400"
            : "bg-sky-400";

  const className = `flex items-center justify-between gap-3 border border-zinc-800 bg-zinc-850 px-3 py-2 ${
    to ? "transition-colors hover:border-sky-400 hover:text-zinc-100" : ""
  }`;
  const content = (
    <>
      <span className="text-zinc-400">{label}</span>
      <span className="flex min-w-0 items-center gap-2 text-right text-zinc-100">
        <span className={`h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
        <span className="truncate">{value}</span>
      </span>
    </>
  );

  if (to) {
    return (
      <Link to={to} title={title} className={className}>
        {content}
      </Link>
    );
  }

  return (
    <div title={title} className={className}>
      {content}
    </div>
  );
}

function CommandButton({
  label,
  icon: Icon,
  disabled,
  danger = false,
  onClick,
}: {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  disabled?: boolean;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`flex h-9 items-center justify-center gap-2 border bg-zinc-850 px-3 text-sm disabled:cursor-not-allowed disabled:border-zinc-800 disabled:text-zinc-700 ${
        danger
          ? "border-zinc-800 text-zinc-300 hover:border-rose-400 hover:text-rose-200"
          : "border-zinc-700 text-zinc-100 hover:border-sky-400"
      }`}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  );
}

function IconButton({
  label,
  icon: Icon,
  onClick,
}: {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex h-9 items-center justify-center border border-zinc-800 bg-zinc-850 text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
    >
      <Icon className="h-4 w-4" />
    </button>
  );
}
