import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { formatLogTime } from "../../presentation/logs";
import type { AppState } from "../../stores/appStore";
import type { AppLogLevel } from "../../types";
import { routeHeadingId } from "../../routeAccessibility";

export function LogsView({ state }: { state: AppState }) {
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const rowVirtualizer = useVirtualizer({
    count: state.logs.length,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => 40,
    overscan: 12,
    getItemKey: (index) => state.logs[index]?.id ?? index,
  });

  return (
    <main className="col-start-3 row-start-2 flex min-w-0 flex-col overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 id={routeHeadingId} tabIndex={-1} className="truncate text-sm font-semibold text-zinc-100">Logs</h1>
          <p className="truncate text-xs text-zinc-400">認証、チャット受信、読み上げ連携の動作ログを確認します</p>
        </div>
        <div className="text-xs text-zinc-400">{state.logs.length} events</div>
      </header>

      <section ref={scrollParentRef} className="min-h-0 flex-1 overflow-auto">
        <div className="min-w-[480px]">
          <div className="sticky top-0 z-10 grid grid-cols-[96px_88px_minmax(0,1fr)] border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-400">
            <span>時刻</span>
            <span>種別</span>
            <span>メッセージ</span>
          </div>
          {state.logs.length === 0 ? (
            <div className="px-4 py-8 text-sm text-zinc-400">ログはまだありません。</div>
          ) : (
            <div
              className="relative"
              style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const log = state.logs[virtualRow.index];

                if (!log) return null;

                return (
                  <div
                    key={virtualRow.key}
                    ref={rowVirtualizer.measureElement}
                    data-index={virtualRow.index}
                    className="absolute left-0 top-0 w-full"
                    style={{ transform: `translateY(${virtualRow.start}px)` }}
                  >
                    <LogRow log={log} />
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </section>
    </main>
  );
}

function LogRow({ log }: { log: AppState["logs"][number] }) {
  return (
    <div className="grid min-h-10 grid-cols-[96px_88px_minmax(0,1fr)] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
      <span className="font-mono text-xs text-zinc-400">{formatLogTime(log.occurredAtMs)}</span>
      <span className={`text-xs ${logLevelClass(log.level)}`}>{logLevelLabel(log.level)}</span>
      <span className="break-words text-zinc-200">{log.message}</span>
    </div>
  );
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
