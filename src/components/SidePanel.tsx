import { Pause, Play, RotateCcw, SkipForward, Volume2 } from "lucide-react";
import type { AppState } from "../stores/appStore";

interface SidePanelProps {
  state: AppState;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onSpeechTest: () => void;
}

export function SidePanel({ state, onSpeechControl, onSpeechTest }: SidePanelProps) {
  const channel = state.settings?.twitch.channelLogin || "未設定";
  const queueCount = state.queueItems.length;
  const twitchAuthLabel = {
    unauthenticated: "未認証",
    authenticated: "ログイン済み",
    expired: "再ログイン必要",
    error: "認証エラー",
  }[state.twitchAuthStatus];
  const twitchAuthTone = state.twitchAuthStatus === "authenticated" ? "ok" : state.twitchAuthStatus === "error" ? "danger" : "muted";

  return (
    <aside className="col-start-2 row-start-1 overflow-hidden border-r border-zinc-800 bg-zinc-900">
      <div className="border-b border-zinc-800 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">
        {state.activeView}
      </div>
      <div className="space-y-5 p-3 text-sm">
        <section>
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">接続</h2>
          <div className="space-y-2">
            <PanelRow label="Twitch" value={twitchAuthLabel} tone={twitchAuthTone} />
            <PanelRow label="チャンネル" value={channel} />
            <PanelRow label="読み上げ" value={state.speechStatus} tone={state.speechStatus === "idle" ? "ok" : "muted"} />
          </div>
        </section>

        <section>
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

        <section>
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">警告</h2>
          {state.warnings.length === 0 ? (
            <p className="text-xs text-zinc-500">現在の警告はありません。</p>
          ) : (
            <div className="space-y-2">
              {state.warnings.map((warning) => (
                <p key={warning} className="border-l-2 border-amber-400 bg-zinc-850 px-2 py-1 text-xs text-amber-200">
                  {warning}
                </p>
              ))}
            </div>
          )}
        </section>

        <section>
          <h2 className="mb-2 text-xs font-semibold text-zinc-400">音声</h2>
          <button
            type="button"
            onClick={onSpeechTest}
            className="flex w-full items-center justify-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-2 text-sm text-zinc-100 hover:border-sky-400"
          >
            <Volume2 className="h-4 w-4" />
            テスト発話
          </button>
        </section>
      </div>
    </aside>
  );
}

function PanelRow({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "ok" | "muted" | "danger";
}) {
  const dotClass =
    tone === "ok" ? "bg-emerald-400" : tone === "muted" ? "bg-zinc-600" : tone === "danger" ? "bg-rose-400" : "bg-sky-400";

  return (
    <div className="flex items-center justify-between gap-3 border border-zinc-800 bg-zinc-850 px-3 py-2">
      <span className="text-zinc-400">{label}</span>
      <span className="flex min-w-0 items-center gap-2 text-right text-zinc-100">
        <span className={`h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
        <span className="truncate">{value}</span>
      </span>
    </div>
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
