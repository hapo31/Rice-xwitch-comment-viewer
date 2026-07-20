import type { AppState } from "../../stores/appStore";
import type { AppLogLevel } from "../../types";

export function LogsView({ state }: { state: AppState }) {
  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Logs</h1>
          <p className="truncate text-xs text-zinc-500">認証、チャット受信、読み上げ連携の動作ログを確認します</p>
        </div>
        <div className="text-xs text-zinc-400">{state.logs.length} events</div>
      </header>

      <section className="h-[calc(100%-3rem)] overflow-auto">
        <div className="grid grid-cols-[96px_88px_minmax(0,1fr)] border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-500">
          <span>時刻</span>
          <span>種別</span>
          <span>メッセージ</span>
        </div>
        {state.logs.length === 0 ? (
          <div className="px-4 py-8 text-sm text-zinc-500">ログはまだありません。</div>
        ) : (
          state.logs.map((log) => (
            <div key={log.id} className="grid min-h-10 grid-cols-[96px_88px_minmax(0,1fr)] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
              <span className="font-mono text-xs text-zinc-500">{formatLogTime(log.occurredAtMs)}</span>
              <span className={`text-xs ${logLevelClass(log.level)}`}>{logLevelLabel(log.level)}</span>
              <span className="break-words text-zinc-200">{log.message}</span>
            </div>
          ))
        )}
      </section>
    </main>
  );
}

function formatLogTime(occurredAtMs: number): string {
  return new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(occurredAtMs));
}

function logLevelLabel(level: AppLogLevel): string {
  return {
    info: "情報",
    warning: "警告",
    error: "エラー",
  }[level];
}

function logLevelClass(level: AppLogLevel): string {
  return {
    info: "text-zinc-400",
    warning: "text-amber-300",
    error: "text-rose-300",
  }[level];
}
