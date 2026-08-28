import { CheckCircle2, Link2, LoaderCircle, LogOut, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { focusIndicatorClass } from "../../presentation/focus";
import { routeHeadingId } from "../../routeAccessibility";
import type { AppState } from "../../stores/appStore";
import type { AppSettingsPatch } from "../../types";
import { isValidTwitchChannelLogin } from "../../validation";
import { defaultTwitchSettings } from "../settings/defaults";
import { FieldError } from "../../components/SettingsFormControls";
import { formatDeviceAuthRemainingTime, getDeviceAuthRemainingSeconds } from "./deviceAuthExpiry";

export function AuthView({
  state,
  onSettingsUpdate,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
  onOpenExternalUrl,
}: {
  state: AppState;
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => Promise<boolean>;
  onTwitchDisconnect: () => void;
  onOpenExternalUrl: (url: string) => void;
}) {
  const twitchSettings = {
    ...defaultTwitchSettings,
    ...state.settings?.twitch,
  };
  const [channelLogin, setChannelLogin] = useState(twitchSettings.channelLogin);
  const [isValidatingAuth, setIsValidatingAuth] = useState(false);
  const [authValidationNotice, setAuthValidationNotice] = useState<string>();
  const [nowMs, setNowMs] = useState(Date.now());
  const isChannelValid = isValidTwitchChannelLogin(channelLogin);
  const channelError = "Twitch チャンネル名は 3 から 25 文字の英数字またはアンダースコアで入力してください。";
  const isAuthenticated = state.twitchAuthStatus === "authenticated";
  const canDisconnect = Boolean(state.twitchProfile) && state.twitchAuthStatus !== "authorizing" && state.twitchAuthStatus !== "disconnecting";
  const isAuthOperationInProgress = ["authorizing", "polling", "checking", "disconnecting"].includes(state.twitchAuthStatus);

  useEffect(() => {
    setChannelLogin(twitchSettings.channelLogin);
  }, [twitchSettings.channelLogin]);

  useEffect(() => {
    if (!isAuthenticated) {
      setAuthValidationNotice(undefined);
    }
  }, [isAuthenticated]);

  useEffect(() => {
    if (!state.twitchAuthPrompt) {
      return;
    }

    setNowMs(Date.now());
    const timer = window.setInterval(() => setNowMs(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [state.twitchAuthPrompt]);

  const remainingSeconds = state.twitchAuthPrompt
    ? getDeviceAuthRemainingSeconds(state.twitchAuthPrompt.expiresAtMs, nowMs)
    : 0;
  const isAuthPromptExpired = Boolean(state.twitchAuthPrompt) && remainingSeconds === 0;

  function saveChannelLogin() {
    const trimmedChannelLogin = channelLogin.trim();
    if (!isValidTwitchChannelLogin(trimmedChannelLogin) || trimmedChannelLogin === twitchSettings.channelLogin) {
      return;
    }

    void onSettingsUpdate({ twitch: { channelLogin: trimmedChannelLogin } });
  }

  async function validateAuth() {
    if (isValidatingAuth) {
      return;
    }

    setIsValidatingAuth(true);
    setAuthValidationNotice(undefined);
    try {
      const isValid = await onTwitchValidateAuth();
      if (isValid) {
        setAuthValidationNotice("Twitch 認証は有効です。");
      }
    } finally {
      setIsValidatingAuth(false);
    }
  }

  return (
    <main className="col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 id={routeHeadingId} tabIndex={-1} className="truncate text-sm font-semibold text-zinc-100">Login</h1>
          <p className="truncate text-xs text-zinc-400">Twitch 認証と接続先チャンネルを管理します</p>
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
                  aria-invalid={!isChannelValid}
                  aria-describedby={!isChannelValid ? "twitch-channel-error" : undefined}
                  className={`h-9 w-full border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
                />
                {!isChannelValid && <FieldError id="twitch-channel" message={channelError} />}
              </div>
            </div>
          </section>

          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
              <span className="text-sm text-zinc-400">認証状態</span>
              <div className="space-y-2 text-sm">
                <p className="text-zinc-200">{state.twitchProfile ? `${state.twitchProfile.login} / ${state.twitchProfile.userId}` : "未ログイン"}</p>
                <p className="text-xs text-zinc-400">必要スコープ: user:read:chat</p>
              </div>
            </div>
            {state.twitchAuthPrompt && (
              <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3">
                <span className="text-sm text-zinc-400">認証コード</span>
                <div className="space-y-2">
                  {isAuthPromptExpired ? (
                    <p className="text-sm text-amber-300" role="status">認証コードの期限が切れました。認証をやり直してください。</p>
                  ) : (
                    <>
                      <p className="font-mono text-lg font-semibold text-zinc-100">{state.twitchAuthPrompt.userCode}</p>
                      <button
                        type="button"
                        onClick={() => onOpenExternalUrl(state.twitchAuthPrompt?.verificationUri ?? "")}
                        className="inline-flex items-center gap-2 text-sm text-sky-300 hover:text-sky-200"
                      >
                        <Link2 className="h-4 w-4" />
                        {state.twitchAuthPrompt.verificationUri}
                      </button>
                      <p className="text-xs text-zinc-400">残り {formatDeviceAuthRemainingTime(remainingSeconds)} / 自動確認間隔 {state.twitchAuthPrompt.interval} 秒</p>
                    </>
                  )}
                </div>
              </div>
            )}
            <div className="flex flex-wrap justify-end gap-2 py-3">
              {state.twitchAuthPrompt && !isAuthenticated && (
                <button
                  type="button"
                  onClick={onTwitchPollAuth}
                  disabled={isAuthPromptExpired || isAuthOperationInProgress}
                  className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  <ShieldCheck className="h-4 w-4" />
                  今すぐ確認
                </button>
              )}
              {isAuthenticated && (
                <button
                  type="button"
                  onClick={() => void validateAuth()}
                  disabled={isValidatingAuth || isAuthOperationInProgress}
                  className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400 disabled:cursor-wait disabled:opacity-60"
                >
                  {isValidatingAuth ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
                  {isValidatingAuth ? "確認中..." : "有効性確認"}
                </button>
              )}
              <button
                type="button"
                onClick={canDisconnect ? onTwitchDisconnect : onTwitchStartAuth}
                disabled={state.twitchAuthStatus === "authorizing" || state.twitchAuthStatus === "disconnecting"}
                className={`flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 ${
                  canDisconnect ? "hover:border-rose-400" : "hover:border-sky-400"
                }`}
              >
                {canDisconnect ? <LogOut className="h-4 w-4" /> : <Link2 className="h-4 w-4" />}
                {canDisconnect ? "認証解除" : state.twitchAuthPrompt ? "認証をやり直す" : "認証開始"}
              </button>
            </div>
            {authValidationNotice && isAuthenticated && (
              <div className="flex items-center justify-end gap-2 border-t border-zinc-800 py-3 text-sm text-emerald-300" role="status">
                <CheckCircle2 className="h-4 w-4" />
                {authValidationNotice}
              </div>
            )}
          </section>
        </div>
      </div>
    </main>
  );
}
