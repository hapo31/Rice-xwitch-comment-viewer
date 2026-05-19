import { AlertCircle, CheckCircle2, CircleDashed, CircleOff, PlugZap, Volume2 } from "lucide-react";
import { useState } from "react";
import type { AppState } from "../stores/appStore";
import type { AppSettings, ChatDisplayState, ChatMessage } from "../types";

interface MainViewProps {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechTest: (text?: string) => void;
}

const sampleMessages: ChatMessage[] = [
  {
    id: "sample-1",
    receivedAt: new Date().toISOString(),
    userDisplayName: "viewer_01",
    text: "コメント受信の準備中です。Twitch接続は次フェーズで実装します。",
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

export function MainView({ state, onSettingsUpdate, onSpeechHealthCheck, onSpeechTest }: MainViewProps) {
  if (state.activeView === "voices") {
    return (
      <VoicesView
        settings={state.settings}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={onSpeechHealthCheck}
        onSpeechTest={onSpeechTest}
      />
    );
  }

  const messages = state.chatMessages.length > 0 ? state.chatMessages : sampleMessages;

  return (
    <main className="col-start-3 row-start-1 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Chat</h1>
          <p className="truncate text-xs text-zinc-500">Twitch EventSub WebSocket</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          <span className="h-2 w-2 rounded-full bg-zinc-600" />
          未接続
        </div>
      </header>

      <section className="h-[calc(100%-3rem)] overflow-auto">
        <div className="min-w-[640px]">
          <div className="grid grid-cols-[88px_160px_minmax(0,1fr)_72px] border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-xs font-medium text-zinc-500">
            <span>時刻</span>
            <span>ユーザー</span>
            <span>コメント</span>
            <span className="text-right">状態</span>
          </div>
          {messages.map((message) => (
            <ChatRow key={message.id} message={message} />
          ))}
        </div>
      </section>
    </main>
  );
}

function VoicesView({
  settings,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechTest,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechTest: (text?: string) => void;
}) {
  const speechSettings = settings?.speech ?? {
    adapter: "bouyomi" as const,
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    readUserName: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
  };
  const [host, setHost] = useState(speechSettings.bouyomiHost);
  const [port, setPort] = useState(String(speechSettings.bouyomiPort));
  const [testText, setTestText] = useState("テスト発話です。");
  const numericPort = Number(port);
  const isPortValid = Number.isInteger(numericPort) && numericPort > 0 && numericPort <= 65535;
  const isHostValid = host.trim().length > 0;

  function saveBouyomiSettings() {
    if (!isHostValid || !isPortValid) {
      return;
    }

    onSettingsUpdate({
      speech: {
        ...speechSettings,
        adapter: "bouyomi",
        bouyomiHost: host.trim(),
        bouyomiPort: numericPort,
      },
    });
  }

  return (
    <main className="col-start-3 row-start-1 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Voices</h1>
          <p className="truncate text-xs text-zinc-500">BouyomiChan TCP</p>
        </div>
        <button
          type="button"
          onClick={onSpeechHealthCheck}
          className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-xs text-zinc-100 hover:border-sky-400"
        >
          <PlugZap className="h-4 w-4" />
          接続確認
        </button>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4">
        <div className="max-w-3xl space-y-6">
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
            <div className="flex justify-end py-3">
              <button
                type="button"
                disabled={!isHostValid || !isPortValid}
                onClick={saveBouyomiSettings}
                className="border border-sky-500 bg-sky-500 px-3 py-1.5 text-sm font-medium text-zinc-950 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
              >
                保存
              </button>
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
    </main>
  );
}

function ChatRow({ message }: { message: ChatMessage }) {
  const time = new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(new Date(message.receivedAt));

  return (
    <div className="grid min-h-11 grid-cols-[88px_160px_minmax(0,1fr)_72px] items-start border-b border-zinc-900 px-4 py-2 text-sm hover:bg-zinc-900">
      <span className="font-mono text-xs text-zinc-500">{time}</span>
      <span className="truncate pr-3 font-medium text-sky-300">{message.userDisplayName}</span>
      <span className="line-clamp-2 pr-4 text-zinc-200">{message.text}</span>
      <span className="flex justify-end">
        <StatusIcon status={message.status} />
      </span>
    </div>
  );
}

function StatusIcon({ status }: { status: ChatDisplayState }) {
  const props = {
    queued: { icon: CircleDashed, label: "queued", className: "text-sky-400" },
    spoken: { icon: CheckCircle2, label: "spoken", className: "text-emerald-400" },
    skipped: { icon: CircleOff, label: "skipped", className: "text-zinc-500" },
    blocked: { icon: CircleOff, label: "blocked", className: "text-amber-400" },
    error: { icon: AlertCircle, label: "error", className: "text-rose-400" },
  }[status];
  const Icon = props.icon;

  return (
    <span title={props.label} aria-label={props.label}>
      <Icon className={`h-4 w-4 ${props.className}`} />
    </span>
  );
}
