import { useVirtualizer } from "@tanstack/react-virtual";
import { KeyRound } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { ChatLiveAnnouncementController } from "../../presentation/chatLiveAnnouncements";
import { getChatMessagePresentation } from "../../presentation/chat";
import { getStartupGuideMessages, type StartupGuideMessage } from "../../presentation/startupGuide";
import { routeHeadingId } from "../../routeAccessibility";
import type { AppState } from "../../stores/appStore";
import type { ChatMessage } from "../../types";
import { ChatBadges } from "./ChatBadges";
import { CHAT_GRID_TEMPLATE } from "./chatLayout";
import { getPrependedMessageCount } from "./scrollAnchor";

export function ChatView({ state, showStartupGuide }: { state: AppState; showStartupGuide: boolean }) {
  const startupReceivedAt = useRef(new Date().toISOString());
  const startupMessages = showStartupGuide ? getStartupGuideMessages(state, startupReceivedAt.current) : [];
  const messages: Array<ChatMessage | StartupGuideMessage> = [...state.chatMessages, ...startupMessages];
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const previousMessagesRef = useRef(messages);
  const scrollAnchorRef = useRef<{ messageId: string; offset: number }>();
  const isAtTopRef = useRef(true);
  const liveAnnouncementController = useRef<ChatLiveAnnouncementController>();
  const hasInitializedLiveAnnouncements = useRef(false);
  const liveAnnouncementTimer = useRef<number>();
  const [liveAnnouncement, setLiveAnnouncement] = useState("");
  const [unseenMessageCount, setUnseenMessageCount] = useState(0);
  const liveChatAnnouncementsEnabled = state.settings?.twitch.liveChatAnnouncements ?? true;
  if (!liveAnnouncementController.current) {
    liveAnnouncementController.current = new ChatLiveAnnouncementController();
  }

  useEffect(() => {
    const controller = liveAnnouncementController.current!;
    if (!hasInitializedLiveAnnouncements.current) {
      controller.initialize(state.chatMessages);
      hasInitializedLiveAnnouncements.current = true;
      return;
    }

    if (!liveChatAnnouncementsEnabled) {
      controller.suppress(state.chatMessages);
      if (liveAnnouncementTimer.current !== undefined) {
        window.clearTimeout(liveAnnouncementTimer.current);
        liveAnnouncementTimer.current = undefined;
      }
      setLiveAnnouncement("");
      return;
    }

    if (!controller.queueNewMessages(state.chatMessages) || liveAnnouncementTimer.current !== undefined) {
      return;
    }

    setLiveAnnouncement("");
    liveAnnouncementTimer.current = window.setTimeout(() => {
      liveAnnouncementTimer.current = undefined;
      setLiveAnnouncement(controller.takeSummary() ?? "");
    }, 500);
  }, [liveChatAnnouncementsEnabled, state.chatMessages]);

  useEffect(() => () => {
    if (liveAnnouncementTimer.current !== undefined) {
      window.clearTimeout(liveAnnouncementTimer.current);
    }
  }, []);
  const rowVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => 44,
    overscan: 12,
    getItemKey: (index) => messages[index]?.id ?? index,
  });

  useLayoutEffect(() => {
    const previousMessages = previousMessagesRef.current;
    const prependedMessageCount = getPrependedMessageCount(previousMessages, messages);
    const scrollParent = scrollParentRef.current;

    if (prependedMessageCount > 0 && scrollParent) {
      if (isAtTopRef.current) {
        rowVirtualizer.scrollToOffset(0);
      } else {
        setUnseenMessageCount((count) => count + prependedMessageCount);

        const anchor = scrollAnchorRef.current;
        const anchorIndex = anchor && messages.findIndex((message) => message.id === anchor.messageId);
        if (anchor && anchorIndex !== undefined && anchorIndex >= 0) {
          rowVirtualizer.scrollToIndex(anchorIndex, { align: "start" });
          scrollParent.scrollTop += anchor.offset;
        }
      }
    }

    previousMessagesRef.current = messages;

    return () => {
      const parent = scrollParentRef.current;
      if (!parent || parent.scrollTop <= 1) {
        isAtTopRef.current = true;
        scrollAnchorRef.current = undefined;
        return;
      }

      isAtTopRef.current = false;
      const parentTop = parent.getBoundingClientRect().top;
      const anchorRow = Array.from(parent.querySelectorAll<HTMLElement>("[data-chat-message-id]")).find(
        (row) => row.getBoundingClientRect().bottom > parentTop,
      );

      scrollAnchorRef.current = anchorRow?.dataset.chatMessageId
        ? {
            messageId: anchorRow.dataset.chatMessageId,
            offset: anchorRow.getBoundingClientRect().top - parentTop,
          }
        : undefined;
    };
  }, [messages, rowVirtualizer]);

  const handleScroll = () => {
    const scrollParent = scrollParentRef.current;
    if (!scrollParent) {
      return;
    }

    isAtTopRef.current = scrollParent.scrollTop <= 1;
    if (isAtTopRef.current) {
      setUnseenMessageCount(0);
    }
  };

  const returnToLatest = () => {
    rowVirtualizer.scrollToOffset(0);
    setUnseenMessageCount(0);
  };
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
          <h1 id={routeHeadingId} tabIndex={-1} className="truncate text-sm font-semibold text-zinc-100">Chat</h1>
          <p className="truncate text-xs text-zinc-400">Twitch チャットの受信状況と読み上げ状態を確認します</p>
        </div>
        <div className="flex items-center gap-3 text-xs text-zinc-400">
          <div className="flex min-w-0 items-center gap-2">
            <span className={`h-2 w-2 rounded-full ${connectionDotClass}`} />
            <span className="max-w-40 truncate">{chatTarget} / {connectionLabel}</span>
          </div>
        </div>
      </header>

      <p className="sr-only" role="status" aria-atomic="true">
        {liveAnnouncement}
      </p>
      <section
        ref={scrollParentRef}
        role="log"
        aria-label="受信チャット"
        aria-live="off"
        aria-relevant="additions text"
        onScroll={handleScroll}
        className="relative h-[calc(100%-3rem)] overflow-x-hidden overflow-y-auto"
      >
        {unseenMessageCount > 0 && (
          <button
            type="button"
            onClick={returnToLatest}
            className="absolute left-1/2 top-2 z-20 -translate-x-1/2 rounded border border-sky-500/50 bg-zinc-800 px-3 py-1 text-xs font-medium text-sky-300 shadow hover:bg-zinc-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-sky-400"
          >
            新着 {unseenMessageCount} 件を表示
          </button>
        )}
        <div className="min-w-0">
          <div
            className="sticky top-0 z-10 grid border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-400"
            style={{ gridTemplateColumns: CHAT_GRID_TEMPLATE }}
          >
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
                  data-chat-message-id={message?.id}
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
  const presentation = getChatMessagePresentation(message);
  const time = new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(message.receivedAt));

  return (
    <div
      className="grid min-h-11 items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900"
      style={{ gridTemplateColumns: CHAT_GRID_TEMPLATE }}
    >
      <span className="font-mono text-xs text-zinc-400">{time}</span>
      <span className={`flex min-w-0 items-center gap-1 pr-3 font-medium ${presentation.userNameClassName}`}>
        <ChatBadges badges={"badges" in message ? message.badges : undefined} />
        <span className="truncate">{message.userDisplayName}</span>
      </span>
      <span className={`line-clamp-2 ${presentation.textClassName}`}>
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
