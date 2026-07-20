import { CheckCircle2, Link2, LoaderCircle, LogOut, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppState } from "../../stores/appStore";
import type { AppSettings } from "../../types";
import { isValidTwitchChannelLogin } from "../../validation";
import { defaultTwitchSettings } from "../settings/defaults";

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
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
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
  const isChannelValid = isValidTwitchChannelLogin(channelLogin);
  const isAuthenticated = state.twitchAuthStatus === "authenticated";

  useEffect(() => {
    setChannelLogin(twitchSettings.channelLogin);
  }, [twitchSettings.channelLogin]);

  useEffect(() => {
    if (!isAuthenticated) {
      setAuthValidationNotice(undefined);
    }
  }, [isAuthenticated]);

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
              {state.twitchAuthPrompt && !isAuthenticated && (
                <button
                  type="button"
                  onClick={onTwitchPollAuth}
                  className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400"
                >
                  <ShieldCheck className="h-4 w-4" />
                  今すぐ確認
                </button>
              )}
              {isAuthenticated && (
                <button
                  type="button"
                  onClick={() => void validateAuth()}
                  disabled={isValidatingAuth}
                  className="flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 hover:border-sky-400 disabled:cursor-wait disabled:opacity-60"
                >
                  {isValidatingAuth ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
                  {isValidatingAuth ? "確認中..." : "有効性確認"}
                </button>
              )}
              <button
                type="button"
                onClick={isAuthenticated ? onTwitchDisconnect : onTwitchStartAuth}
                className={`flex items-center gap-2 border border-zinc-700 bg-zinc-850 px-3 py-1.5 text-sm text-zinc-100 ${
                  isAuthenticated ? "hover:border-rose-400" : "hover:border-sky-400"
                }`}
              >
                {isAuthenticated ? <LogOut className="h-4 w-4" /> : <Link2 className="h-4 w-4" />}
                {isAuthenticated ? "認証解除" : state.twitchAuthPrompt ? "認証をやり直す" : "認証開始"}
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
