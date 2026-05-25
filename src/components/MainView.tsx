import {
  CheckCircle2,
  FileText,
  Link2,
  ListTodo,
  LogOut,
  Network,
  PlugZap,
  RotateCcw,
  Save,
  ScrollText,
  ShieldCheck,
  SkipForward,
  Trash2,
  Volume2,
} from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, useState } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { getQueueStatusPresentation, queueStatusLabel } from "../presentation/chat";
import { countIncompleteQueueItems } from "../presentation/queue";
import type { AppRoutePath } from "../routes";
import type { AppState } from "../stores/appStore";
import type { AppLogLevel, AppSettings, BouyomiConnectionDiagnostics, ChatMessage, QueueDisplayState } from "../types";
import { formatRuleList, isValidBouyomiVoice, isValidPort, isValidTwitchChannelLogin, parseRuleList } from "../validation";

interface MainViewProps {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onQueueReload: () => void;
  onQueueRemove: (itemId: string) => void;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => void;
  onTwitchDisconnect: () => void;
  onOpenExternalUrl: (url: string) => void;
}

const sampleMessages: ChatMessage[] = [
  {
    id: "sample-1",
    receivedAt: new Date().toISOString(),
    userDisplayName: "viewer_01",
    text: "左ペインのチャット受信から Twitch EventSub へ接続できます。",
    status: "queued",
  },
  {
    id: "sample-2",
    receivedAt: new Date().toISOString(),
    userDisplayName: "system",
    text: "棒読みちゃんの既定接続先は 127.0.0.1:50001 です。",
    status: "spoken",
  },
];

const defaultTwitchSettings: AppSettings["twitch"] = {
  channelLogin: "",
  autoConnect: false,
};

const defaultSpeechSettings: AppSettings["speech"] = {
  adapter: "bouyomi",
  bouyomiHost: "127.0.0.1",
  bouyomiPort: 50001,
  bouyomiSpeed: -1,
  bouyomiTone: -1,
  bouyomiVolume: -1,
  bouyomiVoice: 0,
  readUserName: true,
  autoSpeak: true,
  maxCommentLength: 120,
  repeatSuppressionSeconds: 2,
  blockedUsers: [],
  blockedWords: [],
  urlHandling: "replace",
  readEmotes: false,
  connectionSuccessSpeechEnabled: true,
  connectionSuccessSpeechText: "",
};

const defaultConnectionSuccessMessage = "棒読みちゃんと接続しました";

export function MainView({
  state,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
  onSpeechControl,
  onQueueReload,
  onQueueRemove,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
  onOpenExternalUrl,
}: MainViewProps) {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/chat" replace />} />
      <Route path="/chat" element={<ChatView state={state} />} />
      <Route
        path="/queue"
        element={
          <QueueView
            state={state}
            onSpeechControl={onSpeechControl}
            onQueueReload={onQueueReload}
            onQueueRemove={onQueueRemove}
          />
        }
      />
      <Route
        path="/rules"
        element={
          <RulesView settings={state.settings} onSettingsUpdate={onSettingsUpdate} />
        }
      />
      <Route
        path="/settings"
        element={
          <SettingsView
            settings={state.settings}
            onSettingsUpdate={onSettingsUpdate}
            onSpeechHealthCheck={onSpeechHealthCheck}
            onSpeechDiagnostics={onSpeechDiagnostics}
            onSpeechTest={onSpeechTest}
          />
        }
      />
      <Route path="/voices" element={<Navigate to="/settings" replace />} />
      <Route
        path="/auth"
        element={
          <AuthView
            state={state}
            onSettingsUpdate={onSettingsUpdate}
            onTwitchStartAuth={onTwitchStartAuth}
            onTwitchPollAuth={onTwitchPollAuth}
            onTwitchValidateAuth={onTwitchValidateAuth}
            onTwitchDisconnect={onTwitchDisconnect}
            onOpenExternalUrl={onOpenExternalUrl}
          />
        }
      />
      <Route
        path="/logs"
        element={<LogsView state={state} />}
      />
      <Route path="*" element={<Navigate to="/chat" replace />} />
    </Routes>
  );
}

function PlaceholderView({
  path,
  title,
  subtitle,
  description,
  items,
}: {
  path: AppRoutePath;
  title: string;
  subtitle: string;
  description: string;
  items: string[];
}) {
  const Icon = {
    "/chat": FileText,
    "/queue": ListTodo,
    "/rules": ShieldCheck,
    "/settings": Volume2,
    "/auth": ShieldCheck,
    "/logs": ScrollText,
  }[path];

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">{title}</h1>
          <p className="truncate text-xs text-zinc-500">{subtitle}</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-amber-300">
          <span className="h-2 w-2 rounded-full bg-amber-400" />
          未実装
        </div>
      </header>

      <section className="h-[calc(100%-3rem)] overflow-auto p-4">
        <div className="max-w-3xl border-y border-zinc-800">
          <div className="flex items-start gap-3 border-b border-zinc-800 py-4">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center border border-zinc-700 bg-zinc-850 text-zinc-300">
              <Icon className="h-5 w-5" />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-medium text-zinc-100">{title} は準備中です</p>
              <p className="mt-1 text-sm text-zinc-400">{description}</p>
            </div>
          </div>
          <div className="divide-y divide-zinc-800">
            {items.map((item) => (
              <div key={item} className="grid grid-cols-[120px_minmax(0,1fr)] items-center py-3 text-sm">
                <span className="text-zinc-500">予定</span>
                <span className="text-zinc-200">{item}</span>
              </div>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}

function ChatView({ state }: { state: AppState }) {
  const messages = state.chatMessages.length > 0 ? state.chatMessages : sampleMessages;
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

function QueueView({
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

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Queue</h1>
          <p className="truncate text-xs text-zinc-500">読み上げ待ちのチャットを確認し、スキップや削除を操作します</p>
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
        {state.queueItems.length === 0 ? (
          <div className="px-4 py-8 text-sm text-zinc-500">待機中の読み上げはありません。</div>
        ) : (
          state.queueItems.map((item) => (
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

function RulesView({
  settings,
  onSettingsUpdate,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
}) {
  const speechSettings = {
    ...defaultSpeechSettings,
    ...settings?.speech,
  };
  const [blockedUsers, setBlockedUsers] = useState(formatRuleList(speechSettings.blockedUsers));
  const [blockedWords, setBlockedWords] = useState(formatRuleList(speechSettings.blockedWords));
  const [urlHandling, setUrlHandling] = useState(speechSettings.urlHandling);
  const [maxLength, setMaxLength] = useState(String(speechSettings.maxCommentLength));
  const [repeatSeconds, setRepeatSeconds] = useState(String(speechSettings.repeatSuppressionSeconds));

  useEffect(() => {
    setBlockedUsers(formatRuleList(speechSettings.blockedUsers));
    setBlockedWords(formatRuleList(speechSettings.blockedWords));
    setUrlHandling(speechSettings.urlHandling);
    setMaxLength(String(speechSettings.maxCommentLength));
    setRepeatSeconds(String(speechSettings.repeatSuppressionSeconds));
  }, [
    speechSettings.blockedUsers,
    speechSettings.blockedWords,
    speechSettings.urlHandling,
    speechSettings.maxCommentLength,
    speechSettings.repeatSuppressionSeconds,
  ]);

  const numericMaxLength = Number(maxLength);
  const numericRepeatSeconds = Number(repeatSeconds);
  const isMaxLengthValid = Number.isInteger(numericMaxLength) && numericMaxLength >= 1 && numericMaxLength <= 500;
  const isRepeatSecondsValid = Number.isInteger(numericRepeatSeconds) && numericRepeatSeconds >= 0 && numericRepeatSeconds <= 30;
  const isDirty =
    numericMaxLength !== speechSettings.maxCommentLength ||
    numericRepeatSeconds !== speechSettings.repeatSuppressionSeconds ||
    urlHandling !== speechSettings.urlHandling ||
    !stringArrayEqual(parseRuleList(blockedUsers), speechSettings.blockedUsers) ||
    !stringArrayEqual(parseRuleList(blockedWords), speechSettings.blockedWords);

  function saveRules() {
    if (!isMaxLengthValid || !isRepeatSecondsValid) {
      return;
    }

    onSettingsUpdate({
      speech: {
        ...speechSettings,
        maxCommentLength: numericMaxLength,
        repeatSuppressionSeconds: numericRepeatSeconds,
        blockedUsers: parseRuleList(blockedUsers),
        blockedWords: parseRuleList(blockedWords),
        urlHandling,
      },
    });
  }

  return (
    <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Rules</h1>
          <p className="truncate text-xs text-zinc-500">NG ユーザー、NG ワード、URL、長文の扱いを設定します</p>
        </div>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4 pb-20">
        <div className="max-w-3xl space-y-6">
          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="rule-url-handling">
                URL
              </label>
              <select
                id="rule-url-handling"
                value={urlHandling}
                onChange={(event) => setUrlHandling(event.target.value as AppSettings["speech"]["urlHandling"])}
                className="h-9 w-52 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
              >
                <option value="replace">URL省略</option>
                <option value="read">そのまま読む</option>
                <option value="block">読み上げない</option>
              </select>
            </div>
            <NumberRuleRow
              id="rule-max-length"
              label="最大文字数"
              value={maxLength}
              onChange={setMaxLength}
              valid={isMaxLengthValid}
              error="1 から 500 の範囲で入力してください。"
            />
            <NumberRuleRow
              id="rule-repeat-seconds"
              label="連投抑制秒"
              value={repeatSeconds}
              onChange={setRepeatSeconds}
              valid={isRepeatSecondsValid}
              error="0 から 30 の範囲で入力してください。"
            />
          </section>

          <section className="border-y border-zinc-800">
            <RuleTextArea label="NG ユーザー" value={blockedUsers} onChange={setBlockedUsers} />
            <RuleTextArea label="NG ワード" value={blockedWords} onChange={setBlockedWords} />
          </section>
        </div>
      </div>
      <FloatingSaveButton
        visible={isDirty}
        disabled={!isMaxLengthValid || !isRepeatSecondsValid}
        onClick={saveRules}
      />
    </main>
  );
}

function LogsView({ state }: { state: AppState }) {
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

function AuthView({
  state,
  onSettingsUpdate,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
  onOpenExternalUrl,
}: {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => void;
  onTwitchDisconnect: () => void;
  onOpenExternalUrl: (url: string) => void;
}) {
  const twitchSettings = {
    ...defaultTwitchSettings,
    ...state.settings?.twitch,
  };
  const [channelLogin, setChannelLogin] = useState(twitchSettings.channelLogin);
  const isChannelValid = isValidTwitchChannelLogin(channelLogin);

  useEffect(() => {
    setChannelLogin(twitchSettings.channelLogin);
  }, [twitchSettings.channelLogin]);

  function saveChannelLogin() {
    const trimmedChannelLogin = channelLogin.trim();
    if (!isValidTwitchChannelLogin(trimmedChannelLogin) || trimmedChannelLogin === twitchSettings.channelLogin) {
      return;
    }

    onSettingsUpdate({
      twitch: {
        ...twitchSettings,
        channelLogin: trimmedChannelLogin,
      },
    });
  }

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Login</h1>
          <p className="truncate text-xs text-zinc-500">Twitch 認証と接続先チャンネルを管理します</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          <span className={state.twitchAuthStatus === "authenticated" ? "h-2 w-2 rounded-full bg-emerald-400" : "h-2 w-2 rounded-full bg-zinc-600"} />
          {state.twitchProfile?.login ?? "未ログイン"}
        </div>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4">
        <div className="max-w-3xl space-y-6">
          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="twitch-channel">
                チャンネル
              </label>
              <div>
                <input
                  id="twitch-channel"
                  value={channelLogin}
                  onChange={(event) => setChannelLogin(event.target.value)}
                  onBlur={saveChannelLogin}
                  className="h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                {!isChannelValid && <p className="mt-1 text-xs text-rose-400">Twitch のログイン名を 3 から 25 文字で入力してください。</p>}
              </div>
            </div>
          </section>

          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
              <span className="text-sm text-zinc-400">認証状態</span>
              <div className="space-y-2 text-sm">
                <p className="text-zinc-200">{state.twitchProfile ? `${state.twitchProfile.login} / ${state.twitchProfile.userId}` : "未ログイン"}</p>
                <p className="text-xs text-zinc-500">必要スコープ: user:read:chat</p>
              </div>
            </div>
            {state.twitchAuthPrompt && (
              <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
                <span className="text-sm text-zinc-400">認証コード</span>
                <div className="space-y-2">
                  <p className="font-mono text-lg font-semibold text-zinc-100">{state.twitchAuthPrompt.userCode}</p>
                  <button
                    type="button"
                    onClick={() => onOpenExternalUrl(state.twitchAuthPrompt?.verificationUri ?? "")}
                    className="inline-flex items-center gap-2 text-sm text-sky-300 hover:text-sky-200"
                  >
                    <Link2 className="h-4 w-4" />
                    {state.twitchAuthPrompt.verificationUri}
                  </button>
                  <p className="text-xs text-zinc-500">期限 {Math.floor(state.twitchAuthPrompt.expiresIn / 60)} 分 / 自動確認間隔 {state.twitchAuthPrompt.interval} 秒</p>
                </div>
              </div>
            )}
            <div className="flex flex-wrap justify-end gap-2 py-3">
              <button
                type="button"
                onClick={onTwitchStartAuth}
                className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400"
              >
                <Link2 className="h-4 w-4" />
                認証開始
              </button>
              <button
                type="button"
                onClick={onTwitchPollAuth}
                className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400"
              >
                <ShieldCheck className="h-4 w-4" />
                認証確認
              </button>
              <button
                type="button"
                onClick={onTwitchValidateAuth}
                className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400"
              >
                <CheckCircle2 className="h-4 w-4" />
                有効性確認
              </button>
              <button
                type="button"
                onClick={onTwitchDisconnect}
                className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-rose-400"
              >
                <LogOut className="h-4 w-4" />
                解除
              </button>
            </div>
          </section>
        </div>
      </div>
    </main>
  );
}

function SettingsView({
  settings,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
}) {
  const twitchSettings = {
    ...defaultTwitchSettings,
    ...settings?.twitch,
  };
  const speechSettings = {
    ...defaultSpeechSettings,
    ...settings?.speech,
  };
  const [host, setHost] = useState(speechSettings.bouyomiHost);
  const [port, setPort] = useState(String(speechSettings.bouyomiPort));
  const [speed, setSpeed] = useState(speechSettings.bouyomiSpeed);
  const [tone, setTone] = useState(speechSettings.bouyomiTone);
  const [volume, setVolume] = useState(speechSettings.bouyomiVolume);
  const [voice, setVoice] = useState(String(speechSettings.bouyomiVoice));
  const [autoSpeak, setAutoSpeak] = useState(speechSettings.autoSpeak);
  const [readUserName, setReadUserName] = useState(speechSettings.readUserName);
  const [readEmotes, setReadEmotes] = useState(speechSettings.readEmotes);
  const [connectionSuccessSpeechEnabled, setConnectionSuccessSpeechEnabled] = useState(
    speechSettings.connectionSuccessSpeechEnabled,
  );
  const [connectionSuccessSpeechText, setConnectionSuccessSpeechText] = useState(
    speechSettings.connectionSuccessSpeechText,
  );
  const [testText, setTestText] = useState("テスト発話です。");
  const [diagnostics, setDiagnostics] = useState<BouyomiConnectionDiagnostics>();
  const [isDiagnosing, setIsDiagnosing] = useState(false);

  useEffect(() => {
    setHost(speechSettings.bouyomiHost);
    setPort(String(speechSettings.bouyomiPort));
    setSpeed(speechSettings.bouyomiSpeed);
    setTone(speechSettings.bouyomiTone);
    setVolume(speechSettings.bouyomiVolume);
    setVoice(String(speechSettings.bouyomiVoice));
    setAutoSpeak(speechSettings.autoSpeak);
    setReadUserName(speechSettings.readUserName);
    setReadEmotes(speechSettings.readEmotes);
    setConnectionSuccessSpeechEnabled(speechSettings.connectionSuccessSpeechEnabled);
    setConnectionSuccessSpeechText(speechSettings.connectionSuccessSpeechText);
  }, [
    speechSettings.bouyomiHost,
    speechSettings.bouyomiPort,
    speechSettings.bouyomiSpeed,
    speechSettings.bouyomiTone,
    speechSettings.bouyomiVolume,
    speechSettings.bouyomiVoice,
    speechSettings.autoSpeak,
    speechSettings.readUserName,
    speechSettings.readEmotes,
    speechSettings.connectionSuccessSpeechEnabled,
    speechSettings.connectionSuccessSpeechText,
  ]);

  const numericPort = Number(port);
  const numericVoice = Number(voice);
  const isPortValid = isValidPort(port);
  const isVoiceValid = isValidBouyomiVoice(voice);
  const isHostValid = host.trim().length > 0;
  const isDirty =
    host.trim() !== speechSettings.bouyomiHost ||
    numericPort !== speechSettings.bouyomiPort ||
    speed !== speechSettings.bouyomiSpeed ||
    tone !== speechSettings.bouyomiTone ||
    volume !== speechSettings.bouyomiVolume ||
    numericVoice !== speechSettings.bouyomiVoice ||
    autoSpeak !== speechSettings.autoSpeak ||
    readUserName !== speechSettings.readUserName ||
    readEmotes !== speechSettings.readEmotes ||
    connectionSuccessSpeechEnabled !== speechSettings.connectionSuccessSpeechEnabled ||
    connectionSuccessSpeechText !== speechSettings.connectionSuccessSpeechText;

  function saveBouyomiSettings() {
    if (!isHostValid || !isPortValid || !isVoiceValid) {
      return;
    }

    onSettingsUpdate({
      speech: {
        ...speechSettings,
        adapter: "bouyomi",
        bouyomiHost: host.trim(),
        bouyomiPort: numericPort,
        bouyomiSpeed: speed,
        bouyomiTone: tone,
        bouyomiVolume: volume,
        bouyomiVoice: numericVoice,
        autoSpeak,
        readUserName,
        readEmotes,
        connectionSuccessSpeechEnabled,
        connectionSuccessSpeechText,
      },
    });
  }

  function updateAutoConnect(enabled: boolean) {
    onSettingsUpdate({
      twitch: {
        ...twitchSettings,
        autoConnect: enabled,
      },
    });
  }

  async function runDiagnostics() {
    setIsDiagnosing(true);
    try {
      setDiagnostics(await onSpeechDiagnostics());
    } finally {
      setIsDiagnosing(false);
    }
  }

  return (
    <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Settings</h1>
          <p className="truncate text-xs text-zinc-500">起動時接続、棒読みちゃん接続、声質、自動読み上げの設定を調整します</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={runDiagnostics}
            disabled={isDiagnosing}
            className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-xs text-zinc-100 hover:border-sky-400 disabled:cursor-wait disabled:text-zinc-500"
          >
            <Network className="h-4 w-4" />
            診断
          </button>
          <button
            type="button"
            onClick={onSpeechHealthCheck}
            className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-xs text-zinc-100 hover:border-sky-400"
          >
            <PlugZap className="h-4 w-4" />
            接続確認
          </button>
        </div>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4 pb-20">
        <div className="max-w-3xl space-y-6">
          <section className="border-y border-zinc-800">
            <ToggleRow
              label="起動時にチャット受信を開始"
              checked={twitchSettings.autoConnect}
              onChange={updateAutoConnect}
            />
          </section>

          <section className="border-y border-zinc-800">
            <ToggleRow label="自動読み上げ" checked={autoSpeak} onChange={setAutoSpeak} />
            <ToggleRow label="ユーザー名を読む" checked={readUserName} onChange={setReadUserName} />
            <ToggleRow label="emote を読む" checked={readEmotes} onChange={setReadEmotes} />
          </section>

          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="bouyomi-host">
                ホスト
              </label>
              <input
                id="bouyomi-host"
                value={host}
                onChange={(event) => setHost(event.target.value)}
                className="h-9 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
              />
            </div>
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="bouyomi-port">
                ポート
              </label>
              <div>
                <input
                  id="bouyomi-port"
                  inputMode="numeric"
                  value={port}
                  onChange={(event) => setPort(event.target.value)}
                  className="h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                {!isPortValid && <p className="mt-1 text-xs text-rose-400">1 から 65535 の範囲で入力してください。</p>}
              </div>
            </div>
          </section>

          {diagnostics && (
            <section className="border-y border-zinc-800">
              <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
                <span className="text-sm text-zinc-400">診断結果</span>
                <div className="space-y-2">
                  <p className="text-sm text-zinc-200">{diagnostics.recommendation}</p>
                  <p className="font-mono text-xs text-zinc-500">configured: {diagnostics.configuredAddr}</p>
                </div>
              </div>
              <div className="divide-y divide-zinc-800">
                {diagnostics.attempted.map((attempt) => (
                  <div key={attempt.addr} className="grid grid-cols-[180px_minmax(0,1fr)_72px] items-start py-3 text-xs">
                    <span className={attempt.status === "connected" ? "text-emerald-400" : "text-rose-400"}>
                      {attempt.status === "connected" ? "接続成功" : "接続失敗"}
                    </span>
                    <div className="min-w-0">
                      <p className="font-mono text-zinc-200">{attempt.addr}</p>
                      <p className="mt-1 break-words text-zinc-500">{attempt.message}</p>
                    </div>
                    <span className="text-right font-mono text-zinc-500">{attempt.elapsedMs}ms</span>
                  </div>
                ))}
              </div>
            </section>
          )}

          <section className="border-y border-zinc-800">
            <RangeRow label="速度" value={speed} min={-1} max={300} onChange={setSpeed} />
            <RangeRow label="音程" value={tone} min={-1} max={200} onChange={setTone} />
            <RangeRow label="音量" value={volume} min={-1} max={100} onChange={setVolume} />
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-t border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="bouyomi-voice">
                声質
              </label>
              <div>
                <input
                  id="bouyomi-voice"
                  inputMode="numeric"
                  value={voice}
                  onChange={(event) => setVoice(event.target.value)}
                  className="h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                {!isVoiceValid && <p className="mt-1 text-xs text-rose-400">0 から 30000 の範囲で入力してください。</p>}
              </div>
            </div>
          </section>

          <section className="border-y border-zinc-800">
            <ToggleRow
              label="接続成功時に読み上げさせる"
              checked={connectionSuccessSpeechEnabled}
              onChange={setConnectionSuccessSpeechEnabled}
            />
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="connection-success-speech-text">
                接続成功時メッセージ
              </label>
              <div className="space-y-1">
                <input
                  id="connection-success-speech-text"
                  value={connectionSuccessSpeechText}
                  maxLength={120}
                  disabled={!connectionSuccessSpeechEnabled}
                  placeholder={defaultConnectionSuccessMessage}
                  onChange={(event) => setConnectionSuccessSpeechText(event.target.value)}
                  className="h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-400 disabled:cursor-not-allowed disabled:border-zinc-800 disabled:bg-zinc-950 disabled:text-zinc-600"
                />
                <div className="text-right text-xs text-zinc-500">{connectionSuccessSpeechText.length}/120</div>
              </div>
            </div>
          </section>

          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="speech-test-text">
                テスト文
              </label>
              <div className="space-y-2">
                <input
                  id="speech-test-text"
                  value={testText}
                  maxLength={120}
                  onChange={(event) => setTestText(event.target.value)}
                  className="h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                <div className="flex items-center justify-between text-xs text-zinc-500">
                  <span>{testText.length}/120</span>
                  <button
                    type="button"
                    onClick={() => onSpeechTest(testText)}
                    className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400"
                  >
                    <Volume2 className="h-4 w-4" />
                    テスト発話
                  </button>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
      <FloatingSaveButton
        visible={isDirty}
        disabled={!isHostValid || !isPortValid || !isVoiceValid}
        onClick={saveBouyomiSettings}
      />
    </main>
  );
}

function FloatingSaveButton({
  visible,
  disabled,
  onClick,
}: {
  visible: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <div
      className={`absolute bottom-4 right-4 z-10 transition-all duration-200 ease-out ${
        visible ? "translate-x-0 translate-y-0 opacity-100" : "pointer-events-none translate-x-6 translate-y-3 opacity-0"
      }`}
    >
      <button
        type="button"
        aria-label="設定を保存"
        title="設定を保存"
        disabled={disabled}
        onClick={onClick}
        className="flex h-10 items-center gap-2 border border-sky-500 bg-sky-500 px-4 text-sm font-medium text-zinc-950 shadow-lg shadow-zinc-950/40 hover:border-sky-300 hover:bg-sky-400 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
      >
        <Save className="h-4 w-4" />
        保存
      </button>
    </div>
  );
}

function stringArrayEqual(left: string[], right: string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3 last:border-b-0">
      <span className="text-sm text-zinc-400">{label}</span>
      <span className="flex items-center gap-2 text-sm text-zinc-200">
        <input
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(event.target.checked)}
          className="h-4 w-4 accent-sky-400"
        />
        {checked ? "ON" : "OFF"}
      </span>
    </label>
  );
}

function NumberRuleRow({
  id,
  label,
  value,
  valid,
  error,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  valid: boolean;
  error: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3 last:border-b-0">
      <label className="pt-2 text-sm text-zinc-400" htmlFor={id}>
        {label}
      </label>
      <div>
        <input
          id={id}
          inputMode="numeric"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
        />
        {!valid && <p className="mt-1 text-xs text-rose-400">{error}</p>}
      </div>
    </div>
  );
}

function RuleTextArea({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3 last:border-b-0">
      <label className="pt-2 text-sm text-zinc-400">{label}</label>
      <textarea
        value={value}
        rows={5}
        onChange={(event) => onChange(event.target.value)}
        className="resize-y border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-sky-400"
      />
    </div>
  );
}

function RangeRow({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)_64px] items-center border-b border-zinc-800 py-3 last:border-b-0">
      <label className="text-sm text-zinc-400">{label}</label>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        className="w-full accent-sky-400"
      />
      <span className="text-right font-mono text-xs text-zinc-300">{value === -1 ? "既定" : value}</span>
    </div>
  );
}

function ChatRow({ message }: { message: ChatMessage }) {
  const time = new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(message.receivedAt));

  return (
    <div className="grid min-h-11 grid-cols-[88px_160px_minmax(0,1fr)] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
      <span className="font-mono text-xs text-zinc-500">{time}</span>
      <span className="truncate pr-3 font-medium text-sky-300">{message.userDisplayName}</span>
      <span className="line-clamp-2 text-zinc-200">{message.text}</span>
    </div>
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
