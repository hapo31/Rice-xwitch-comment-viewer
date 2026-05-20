import { AlertCircle, CheckCircle2, CircleDashed, CircleOff, Link2, LogOut, Network, PlugZap, ShieldCheck, Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppState } from "../stores/appStore";
import type { AppSettings, BouyomiConnectionDiagnostics, ChatDisplayState, ChatMessage } from "../types";

interface MainViewProps {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => void;
  onTwitchDisconnect: () => void;
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

export function MainView({
  state,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
}: MainViewProps) {
  if (state.activeView === "voices") {
    return (
      <VoicesView
        settings={state.settings}
        onSettingsUpdate={onSettingsUpdate}
        onSpeechHealthCheck={onSpeechHealthCheck}
        onSpeechDiagnostics={onSpeechDiagnostics}
        onSpeechTest={onSpeechTest}
      />
    );
  }

  if (state.activeView === "settings") {
    return (
      <SettingsView
        state={state}
        onSettingsUpdate={onSettingsUpdate}
        onTwitchStartAuth={onTwitchStartAuth}
        onTwitchPollAuth={onTwitchPollAuth}
        onTwitchValidateAuth={onTwitchValidateAuth}
        onTwitchDisconnect={onTwitchDisconnect}
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

function SettingsView({
  state,
  onSettingsUpdate,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
}: {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => void;
  onTwitchDisconnect: () => void;
}) {
  const twitchSettings = state.settings?.twitch ?? { clientId: "", channelLogin: "", autoConnect: false };
  const [clientId, setClientId] = useState(twitchSettings.clientId);
  const [channelLogin, setChannelLogin] = useState(twitchSettings.channelLogin);
  const isClientIdValid = clientId.trim().length >= 20 || clientId.trim().length === 0;
  const isChannelValid = channelLogin.trim().length === 0 || /^[a-zA-Z0-9_]{3,25}$/.test(channelLogin.trim());

  useEffect(() => {
    setClientId(twitchSettings.clientId);
    setChannelLogin(twitchSettings.channelLogin);
  }, [twitchSettings.clientId, twitchSettings.channelLogin]);

  function saveTwitchSettings() {
    if (!isClientIdValid || !isChannelValid) {
      return;
    }

    onSettingsUpdate({
      twitch: {
        ...twitchSettings,
        clientId: clientId.trim(),
        channelLogin: channelLogin.trim(),
      },
    });
  }

  return (
    <main className="col-start-3 row-start-1 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Settings</h1>
          <p className="truncate text-xs text-zinc-500">Twitch OAuth Device Code Flow</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-400">
          <span className={state.twitchAuthStatus === "authenticated" ? "h-2 w-2 rounded-full bg-emerald-400" : "h-2 w-2 rounded-full bg-zinc-600"} />
          {state.twitchProfile?.login ?? "未ログイン"}
        </div>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4">
        <div className="max-w-3xl space-y-6">
          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="twitch-client-id">
                Client ID
              </label>
              <div>
                <input
                  id="twitch-client-id"
                  value={clientId}
                  onChange={(event) => setClientId(event.target.value)}
                  className="h-9 w-full border border-zinc-700 bg-zinc-900 px-3 font-mono text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                {!isClientIdValid && <p className="mt-1 text-xs text-rose-400">Twitch Developer Console の Client ID を入力してください。</p>}
              </div>
            </div>
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="twitch-channel">
                チャンネル
              </label>
              <div>
                <input
                  id="twitch-channel"
                  value={channelLogin}
                  onChange={(event) => setChannelLogin(event.target.value)}
                  className="h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
                />
                {!isChannelValid && <p className="mt-1 text-xs text-rose-400">Twitch のログイン名を 3 から 25 文字で入力してください。</p>}
              </div>
            </div>
            <div className="flex justify-end py-3">
              <button
                type="button"
                disabled={!isClientIdValid || !isChannelValid}
                onClick={saveTwitchSettings}
                className="border border-sky-500 bg-sky-500 px-3 py-1.5 text-sm font-medium text-zinc-950 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
              >
                保存
              </button>
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
                  <a
                    href={state.twitchAuthPrompt.verificationUri}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex items-center gap-2 text-sm text-sky-300 hover:text-sky-200"
                  >
                    <Link2 className="h-4 w-4" />
                    {state.twitchAuthPrompt.verificationUri}
                  </a>
                  <p className="text-xs text-zinc-500">期限 {Math.floor(state.twitchAuthPrompt.expiresIn / 60)} 分 / 確認間隔 {state.twitchAuthPrompt.interval} 秒</p>
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

function VoicesView({
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
  const speechSettings = settings?.speech ?? {
    adapter: "bouyomi" as const,
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    bouyomiSpeed: -1,
    bouyomiTone: -1,
    bouyomiVolume: -1,
    bouyomiVoice: 0,
    readUserName: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
  };
  const [host, setHost] = useState(speechSettings.bouyomiHost);
  const [port, setPort] = useState(String(speechSettings.bouyomiPort));
  const [speed, setSpeed] = useState(speechSettings.bouyomiSpeed);
  const [tone, setTone] = useState(speechSettings.bouyomiTone);
  const [volume, setVolume] = useState(speechSettings.bouyomiVolume);
  const [voice, setVoice] = useState(String(speechSettings.bouyomiVoice));
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
  }, [
    speechSettings.bouyomiHost,
    speechSettings.bouyomiPort,
    speechSettings.bouyomiSpeed,
    speechSettings.bouyomiTone,
    speechSettings.bouyomiVolume,
    speechSettings.bouyomiVoice,
  ]);

  const numericPort = Number(port);
  const numericVoice = Number(voice);
  const isPortValid = Number.isInteger(numericPort) && numericPort > 0 && numericPort <= 65535;
  const isVoiceValid = Number.isInteger(numericVoice) && numericVoice >= 0 && numericVoice <= 30000;
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
        bouyomiSpeed: speed,
        bouyomiTone: tone,
        bouyomiVolume: volume,
        bouyomiVoice: numericVoice,
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
    <main className="col-start-3 row-start-1 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Voices</h1>
          <p className="truncate text-xs text-zinc-500">BouyomiChan TCP</p>
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
                disabled={!isHostValid || !isPortValid || !isVoiceValid}
                onClick={saveBouyomiSettings}
                className="border border-sky-500 bg-sky-500 px-3 py-1.5 text-sm font-medium text-zinc-950 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
              >
                保存
              </button>
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
