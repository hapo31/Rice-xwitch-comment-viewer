import { createContext, useContext, useEffect, useState } from "react";
import { Controller } from "react-hook-form";
import { Network, PlugZap, Volume2 } from "lucide-react";
import {
  FieldError,
  RangeRow,
  SettingsSection,
  ToggleRow,
} from "../../components/SettingsFormControls";
import { focusIndicatorClass } from "../../presentation/focus";
import { presentError } from "../../presentation/errors";
import { useFormDraftContext } from "./useFormDraft";
import { authorizeSpeechEndpoint } from "../../tauri/client";
import type { AppSettings, AppSettingsPatch, BouyomiConnectionDiagnostics } from "../../types";
import {
  isValidBouyomiHost,
  isValidBouyomiVoice,
  isValidConfirmationText,
  isValidPort,
} from "../../validation";
import type { SettingsDraft } from "./formModels";

type SettingsActions = {
  twitch: AppSettings["twitch"];
  savedSpeech: AppSettings["speech"];
  isDirty: boolean;
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
};

const SettingsActionsContext = createContext<SettingsActions | undefined>(undefined);

export function SettingsActionsProvider({
  value,
  children,
}: {
  value: SettingsActions;
  children: React.ReactNode;
}) {
  return (
    <SettingsActionsContext.Provider value={value}>{children}</SettingsActionsContext.Provider>
  );
}

function useSettingsActions() {
  const actions = useContext(SettingsActionsContext);
  if (!actions) throw new Error("Settings actions are unavailable outside SettingsActionsProvider");
  return actions;
}

export function ChatReceptionSection() {
  const { twitch, onSettingsUpdate } = useSettingsActions();
  return (
    <SettingsSection id="chat-reception" title="チャット受信">
      <ToggleRow
        label="起動時にチャット受信を開始"
        checked={twitch.autoConnect}
        onChange={(enabled) => void onSettingsUpdate({ twitch: { autoConnect: enabled } })}
      />
      <ToggleRow
        label="チャット受信停止時に確認する"
        checked={twitch.confirmBeforeStopChat}
        onChange={(enabled) =>
          void onSettingsUpdate({ twitch: { confirmBeforeStopChat: enabled } })
        }
      />
      <ToggleRow
        label="新着チャットを支援技術へ通知する"
        checked={twitch.liveChatAnnouncements}
        onChange={(enabled) =>
          void onSettingsUpdate({ twitch: { liveChatAnnouncements: enabled } })
        }
      />
    </SettingsSection>
  );
}

export function AutomaticSpeechSection() {
  const { control, getValues } = useFormDraftContext<SettingsDraft>();
  return (
    <SettingsSection id="automatic-speech" title="自動読み上げ">
      <Controller
        control={control}
        name="autoSpeak"
        render={({ field }) => (
          <ToggleRow
            label="自動読み上げ"
            checked={getValues("autoSpeak")}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="readUserName"
        render={({ field }) => (
          <ToggleRow
            label="ユーザー名を読む"
            checked={getValues("readUserName")}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="readEmotes"
        render={({ field }) => (
          <ToggleRow
            label="emote を読む"
            checked={getValues("readEmotes")}
            onChange={field.onChange}
          />
        )}
      />
    </SettingsSection>
  );
}

export function SpeechConnectionSection() {
  const { control, getValues } = useFormDraftContext<SettingsDraft>();
  const { savedSpeech, isDirty } = useSettingsActions();
  const [consentMessage, setConsentMessage] = useState("");
  const [isAuthorizing, setIsAuthorizing] = useState(false);
  useEffect(
    () => setConsentMessage(""),
    [savedSpeech.bouyomiHost, savedSpeech.bouyomiPort, savedSpeech.bouyomiRemoteMode],
  );

  return (
    <SettingsSection id="bouyomi-connection" title="棒読みちゃん接続">
      <Controller
        control={control}
        name="remoteMode"
        render={({ field }) => (
          <>
            <ToggleRow
              label="外部接続モード（明示許可が必要）"
              checked={getValues("remoteMode")}
              onChange={(value) => {
                setConsentMessage("");
                field.onChange(value);
              }}
            />
            <p className="py-2 text-xs text-zinc-400">
              通常は127.0.0.0/8・::1だけに接続します。外部接続はprivate
              LAN/VPN限定で、Twitchユーザー名・チャット・テスト文を相手認証なしの平文TCPで送信します。信頼する相手だけを許可し、暗号化トンネル/VPNを使用してください。public・link-local宛先は接続しません。モード選択だけでは許可されません。
            </p>
            {getValues("remoteMode") && (
              <>
                <button
                  type="button"
                  disabled={isDirty || isAuthorizing || !savedSpeech.bouyomiRemoteMode}
                  onClick={() => {
                    setIsAuthorizing(true);
                    setConsentMessage("");
                    void authorizeSpeechEndpoint()
                      .then(() =>
                        setConsentMessage(
                          "この起動中だけ、確認した接続先を許可しました。接続確認・診断を実行してください。",
                        ),
                      )
                      .catch((error: unknown) =>
                        setConsentMessage(presentError(error, "settings").message),
                      )
                      .finally(() => setIsAuthorizing(false));
                  }}
                  className={`border border-zinc-700 px-3 py-1.5 text-xs text-zinc-100 disabled:text-zinc-400 ${focusIndicatorClass}`}
                >
                  保存済みの接続先をネイティブ確認で許可
                </button>
                <p className="py-2 text-xs text-zinc-400">
                  先に接続設定を保存してください。再起動・接続先/DNS結果変更後は再許可が必要です。
                </p>
              </>
            )}
            {consentMessage && (
              <p role="status" className="py-2 text-xs text-zinc-400">
                {consentMessage}
              </p>
            )}
          </>
        )}
      />
      <Controller
        control={control}
        name="host"
        render={({ field }) => {
          const host = getValues("host");
          const valid = isValidBouyomiHost(host);
          const message =
            host.trim().length === 0
              ? "棒読みちゃんのホストを入力してください。"
              : "IPv4、DNS名、または角括弧なしのIPv6アドレスを入力してください。";
          return (
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
              <label className="pt-2 text-sm text-zinc-400" htmlFor="bouyomi-host">
                ホスト
              </label>
              <div>
                <input
                  id="bouyomi-host"
                  value={host}
                  onChange={field.onChange}
                  aria-invalid={!valid}
                  aria-describedby={!valid ? "bouyomi-host-error" : undefined}
                  className={`h-9 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
                />
                {!valid && <FieldError id="bouyomi-host" message={message} />}
              </div>
            </div>
          );
        }}
      />
      <Controller
        control={control}
        name="port"
        render={({ field }) => {
          const port = getValues("port");
          const valid = isValidPort(port);
          return (
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="bouyomi-port">
                ポート
              </label>
              <div>
                <input
                  id="bouyomi-port"
                  inputMode="numeric"
                  value={port}
                  onChange={field.onChange}
                  aria-invalid={!valid}
                  aria-describedby={!valid ? "bouyomi-port-error" : undefined}
                  className={`h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
                />
                {!valid && (
                  <FieldError
                    id="bouyomi-port"
                    message="棒読みちゃんのポートは 1 から 65535 の範囲で入力してください。"
                  />
                )}
              </div>
            </div>
          );
        }}
      />
    </SettingsSection>
  );
}

export function ConnectionDiagnosticsSection() {
  const { onSpeechDiagnostics } = useSettingsActions();
  const [diagnostics, setDiagnostics] = useState<BouyomiConnectionDiagnostics>();
  const [isDiagnosing, setIsDiagnosing] = useState(false);
  async function runDiagnostics() {
    setIsDiagnosing(true);
    try {
      setDiagnostics(await onSpeechDiagnostics());
    } finally {
      setIsDiagnosing(false);
    }
  }
  return (
    <>
      <button
        type="button"
        onClick={() => void runDiagnostics()}
        disabled={isDiagnosing}
        className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-xs text-zinc-100 hover:border-sky-400 disabled:cursor-wait disabled:text-zinc-400"
      >
        <Network className="h-4 w-4" />
        診断
      </button>
      {diagnostics && (
        <SettingsSection id="connection-diagnostics" title="接続診断">
          <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
            <span className="text-sm text-zinc-400">診断結果</span>
            <div className="space-y-2">
              <p className="text-sm text-zinc-200">{diagnostics.recommendation}</p>
              <p className="font-mono text-xs text-zinc-400">
                configured: {diagnostics.configuredAddr}
              </p>
            </div>
          </div>
          <div className="divide-y divide-zinc-800">
            {diagnostics.attempted.map((attempt) => (
              <div
                key={attempt.addr}
                className="grid grid-cols-[180px_minmax(0,1fr)_72px] items-start py-3 text-xs"
              >
                <span
                  className={attempt.status === "connected" ? "text-emerald-400" : "text-rose-400"}
                >
                  {attempt.status === "connected" ? "接続成功" : "接続失敗"}
                </span>
                <div className="min-w-0">
                  <p className="font-mono text-zinc-200">{attempt.addr}</p>
                  <p className="mt-1 break-words text-zinc-400">{attempt.message}</p>
                </div>
                <span className="text-right font-mono text-zinc-400">{attempt.elapsedMs}ms</span>
              </div>
            ))}
          </div>
        </SettingsSection>
      )}
    </>
  );
}

export function SpeechHealthCheckButton() {
  const { onSpeechHealthCheck } = useSettingsActions();
  return (
    <button
      type="button"
      onClick={onSpeechHealthCheck}
      className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-xs text-zinc-100 hover:border-sky-400"
    >
      <PlugZap className="h-4 w-4" />
      接続確認
    </button>
  );
}

export function VoiceSettingsSection() {
  const { control, getValues } = useFormDraftContext<SettingsDraft>();
  return (
    <SettingsSection id="voice-settings" title="声質">
      <Controller
        control={control}
        name="speed"
        render={({ field }) => (
          <RangeRow
            id="bouyomi-speed"
            label="速度"
            value={getValues("speed")}
            min={-1}
            max={300}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="tone"
        render={({ field }) => (
          <RangeRow
            id="bouyomi-tone"
            label="音程"
            value={getValues("tone")}
            min={-1}
            max={200}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="volume"
        render={({ field }) => (
          <RangeRow
            id="bouyomi-volume"
            label="音量"
            value={getValues("volume")}
            min={-1}
            max={100}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="voice"
        render={({ field }) => {
          const voice = getValues("voice");
          const valid = isValidBouyomiVoice(voice);
          return (
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-t border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="bouyomi-voice">
                声質
              </label>
              <div>
                <input
                  id="bouyomi-voice"
                  inputMode="numeric"
                  value={voice}
                  onChange={field.onChange}
                  aria-invalid={!valid}
                  aria-describedby={!valid ? "bouyomi-voice-error" : undefined}
                  className={`h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
                />
                {!valid && (
                  <FieldError
                    id="bouyomi-voice"
                    message="棒読みちゃんの声質は 0 から 30000 の範囲で入力してください。"
                  />
                )}
              </div>
            </div>
          );
        }}
      />
    </SettingsSection>
  );
}

export function ConnectionSuccessSpeechSection() {
  const { control, getValues } = useFormDraftContext<SettingsDraft>();
  const enabled = getValues("connectionSuccessSpeechEnabled");
  return (
    <SettingsSection id="connection-success-speech" title="接続成功時の読み上げ">
      <Controller
        control={control}
        name="connectionSuccessSpeechEnabled"
        render={({ field }) => (
          <ToggleRow
            label="接続成功時に読み上げさせる"
            checked={getValues("connectionSuccessSpeechEnabled")}
            onChange={field.onChange}
          />
        )}
      />
      <Controller
        control={control}
        name="connectionSuccessSpeechText"
        render={({ field }) => {
          const text = getValues("connectionSuccessSpeechText");
          const valid = isValidConfirmationText(text);
          return (
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start py-3">
              <label
                className="pt-2 text-sm text-zinc-400"
                htmlFor="connection-success-speech-text"
              >
                接続成功時メッセージ
              </label>
              <div className="space-y-1">
                <input
                  id="connection-success-speech-text"
                  value={text}
                  aria-invalid={!valid}
                  aria-describedby={!valid ? "confirmation-text-error" : undefined}
                  disabled={!enabled}
                  placeholder="棒読みちゃんと接続しました"
                  onChange={field.onChange}
                  className={`h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 placeholder:text-zinc-400 disabled:cursor-not-allowed disabled:border-zinc-800 disabled:bg-zinc-950 disabled:text-zinc-400 ${focusIndicatorClass}`}
                />
                <div className="text-right text-xs text-zinc-400">
                  {Array.from(text).length}/120
                </div>
                {!valid && (
                  <FieldError
                    id="confirmation-text"
                    message="制御文字を含まない120文字・480 UTF-8バイト以内にしてください。"
                  />
                )}
              </div>
            </div>
          );
        }}
      />
    </SettingsSection>
  );
}

export function SpeechTestSection() {
  const { onSpeechTest } = useSettingsActions();
  const [testText, setTestText] = useState("テスト読み上げです。");
  return (
    <SettingsSection id="speech-test" title="テスト読み上げ">
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
            className={`h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
          />
          <div className="flex items-center justify-between text-xs text-zinc-400">
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
    </SettingsSection>
  );
}
