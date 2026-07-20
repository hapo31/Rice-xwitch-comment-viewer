import { Network, PlugZap, Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import {
  FloatingSaveButton,
  RangeRow,
  ToggleRow,
} from "../../components/SettingsFormControls";
import type { AppSettings, BouyomiConnectionDiagnostics } from "../../types";
import { isValidBouyomiVoice, isValidPort } from "../../validation";
import { defaultSpeechSettings, defaultTwitchSettings } from "./defaults";

const defaultConnectionSuccessMessage = "棒読みちゃんと接続しました";

export function SettingsView({
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
  const [testText, setTestText] = useState("テスト読み上げです。");
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

  function updateConfirmBeforeStopChat(enabled: boolean) {
    onSettingsUpdate({
      twitch: {
        ...twitchSettings,
        confirmBeforeStopChat: enabled,
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
            <ToggleRow
              label="チャット受信停止時に確認する"
              checked={twitchSettings.confirmBeforeStopChat}
              onChange={updateConfirmBeforeStopChat}
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
                    テスト読み上げ
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
