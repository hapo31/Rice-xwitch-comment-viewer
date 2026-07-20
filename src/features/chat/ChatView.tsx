import { useVirtualizer } from "@tanstack/react-virtual";
import { KeyRound } from "lucide-react";
import { useRef } from "react";
import { Link } from "react-router-dom";
import { getStartupGuideMessages, type StartupGuideMessage } from "../../presentation/startupGuide";
import type { AppState } from "../../stores/appStore";
import type { ChatMessage } from "../../types";

export function ChatView({ state, showStartupGuide }: { state: AppState; showStartupGuide: boolean }) {
  const startupReceivedAt = useRef(new Date().toISOString());
  const startupMessages = showStartupGuide ? getStartupGuideMessages(state, startupReceivedAt.current) : [];
  const messages: Array<ChatMessage | StartupGuideMessage> = [...state.chatMessages, ...startupMessages];
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const rowVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => 44,
    overscan: 12,
    getItemKey: (index) => messages[index]?.id ?? index,
  });
  const chatTarget = state.settings?.twitch.channelLogin || state.twitchProfile?.login || "未設定";
  const connectionLabel = {
    disconnected: "未接続",
    connecting: "接続中",
    connected: "受信中",
    reconnecting: "再接続中",
    authRequired: "再ログイン必要",
    error: "接続エラー",
  }[state.twitchConnectionStatus];
  const connectionDotClass =
    state.twitchConnectionStatus === "connected"
      ? "bg-emerald-400"
      : state.twitchConnectionStatus === "connecting" || state.twitchConnectionStatus === "reconnecting"
        ? "bg-sky-400"
        : state.twitchConnectionStatus === "error" || state.twitchConnectionStatus === "authRequired"
          ? "bg-rose-400"
          : "bg-zinc-600";

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Chat</h1>
          <p className="truncate text-xs text-zinc-500">Twitch チャットの受信状況と読み上げ状態を確認します</p>
        </div>
        <div className="flex items-center gap-3 text-xs text-zinc-400">
          <div className="flex min-w-0 items-center gap-2">
            <span className={`h-2 w-2 rounded-full ${connectionDotClass}`} />
            <span className="max-w-40 truncate">{chatTarget} / {connectionLabel}</span>
          </div>
        </div>
      </header>

      <section ref={scrollParentRef} className="h-[calc(100%-3rem)] overflow-auto">
        <div className="min-w-[640px]">
          <div className="sticky top-0 z-10 grid grid-cols-[88px_160px_minmax(0,1fr)] border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-500">
            <span>時刻</span>
            <span>ユーザー</span>
            <span>チャット</span>
          </div>
          <div
            className="relative"
            style={{
              height: `${rowVirtualizer.getTotalSize()}px`,
            }}
          >
            {rowVirtualizer.getVirtualItems().map((virtualRow) => {
              const message = messages[virtualRow.index];

              return (
                <div
                  key={virtualRow.key}
                  ref={rowVirtualizer.measureElement}
                  data-index={virtualRow.index}
                  className="absolute left-0 top-0 w-full"
                  style={{
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <ChatRow message={message} />
                </div>
              );
            })}
          </div>
        </div>
      </section>
    </main>
  );
}

function ChatRow({ message }: { message: ChatMessage | StartupGuideMessage }) {
  const time = new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(message.receivedAt));

  return (
    <div className="grid min-h-11 grid-cols-[88px_160px_minmax(0,1fr)] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
      <span className="font-mono text-xs text-zinc-500">{time}</span>
      <span className="truncate pr-3 font-medium text-sky-300">{message.userDisplayName}</span>
      <span className="line-clamp-2 text-zinc-200">
        {"action" in message && message.action === "login" && (
          <Link
            to="/auth"
            aria-label="Loginを開く"
            title="Login"
            className="mr-1 inline-flex align-text-bottom text-sky-400 hover:text-sky-300"
          >
            <KeyRound className="h-4 w-4" />
          </Link>
        )}
        {message.text}
      </span>
    </div>
  );
}
