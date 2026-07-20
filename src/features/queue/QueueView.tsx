import { RotateCcw, SkipForward, Trash2 } from "lucide-react";
import { getQueueStatusPresentation, queueStatusLabel } from "../../presentation/chat";
import { countIncompleteQueueItems, selectQueueItemsForDisplay } from "../../presentation/queue";
import type { AppState } from "../../stores/appStore";
import type { QueueDisplayState } from "../../types";

export function QueueView({
  state,
  onSpeechControl,
  onQueueReload,
  onQueueRemove,
}: {
  state: AppState;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onQueueReload: () => void;
  onQueueRemove: (itemId: string) => void;
}) {
  const queuedCount = countIncompleteQueueItems(state.queueItems);
  const displayItems = selectQueueItemsForDisplay(state.queueItems);

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Queue</h1>
          <p className="truncate text-xs text-zinc-500">読み上げ待ち、エラー、フィルターで読み飛ばしたチャットを確認します</p>
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            aria-label="キューを再読込"
            title="キューを再読込"
            onClick={onQueueReload}
            className="flex h-8 w-8 items-center justify-center border border-zinc-700 bg-zinc-850 text-zinc-200 hover:border-sky-400"
          >
            <RotateCcw className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="現在の読み上げをスキップ"
            title="現在の読み上げをスキップ"
            onClick={() => onSpeechControl("skip")}
            className="flex h-8 w-8 items-center justify-center border border-zinc-700 bg-zinc-850 text-zinc-200 hover:border-sky-400"
          >
            <SkipForward className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="キューをクリア"
            title="キューをクリア"
            onClick={() => onSpeechControl("clear")}
            className="flex h-8 w-8 items-center justify-center border border-zinc-700 bg-zinc-850 text-zinc-200 hover:border-rose-400"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </header>

      <section className="h-[calc(100%-3rem)] overflow-auto">
        <div className="grid grid-cols-[140px_96px_minmax(0,1fr)_72px] border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-500">
          <span>ユーザー</span>
          <span>状態</span>
          <span>読み上げ文</span>
          <span className="text-right">操作</span>
        </div>
        {displayItems.length === 0 ? (
          <div className="px-4 py-8 text-sm text-zinc-500">確認が必要な読み上げはありません。</div>
        ) : (
          displayItems.map((item) => (
            <div key={item.id} className="grid min-h-11 grid-cols-[140px_96px_minmax(0,1fr)_72px] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
              <span className="truncate pr-3 font-medium text-sky-300">{item.userDisplayName}</span>
              <span className="flex items-center gap-2 text-xs text-zinc-400">
                <StatusIcon status={item.status} />
                {queueStatusLabel(item.status)}
              </span>
              <span className="line-clamp-2 pr-4 text-zinc-200">{item.text}</span>
              <span className="flex justify-end">
                <button
                  type="button"
                  aria-label="キュー項目を削除"
                  title="キュー項目を削除"
                  disabled={!["queued", "error"].includes(item.status)}
                  onClick={() => onQueueRemove(item.id)}
                  className="flex h-7 w-7 items-center justify-center border border-zinc-800 bg-zinc-850 text-zinc-400 hover:border-rose-400 hover:text-rose-200 disabled:cursor-not-allowed disabled:text-zinc-700"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </span>
            </div>
          ))
        )}
      </section>

      <div className="pointer-events-none absolute bottom-8 right-4 border border-zinc-800 bg-zinc-900 px-3 py-1 text-xs text-zinc-400">
        待機 {queuedCount}
      </div>
    </main>
  );
}

function StatusIcon({ status }: { status: QueueDisplayState }) {
  const props = getQueueStatusPresentation(status);
  const Icon = props.icon;

  return (
    <span title={props.label} aria-label={props.label}>
      <Icon className={`h-4 w-4 ${props.className}`} />
    </span>
  );
}
