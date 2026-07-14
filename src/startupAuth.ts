import type { TwitchAuthValidationResult, TwitchUserProfile } from "./types";

export type StartupAuthResult =
  | { status: "missing" }
  | { status: "authenticated"; result: TwitchAuthValidationResult }
  | { status: "error"; error: string };

interface StartupAuthDependencies {
  getStoredAuth: () => Promise<TwitchUserProfile | undefined>;
  validateAuth: () => Promise<TwitchAuthValidationResult>;
  reportSystemMessage: (message: string) => void;
}

export async function restoreAndValidateStartupAuth({
  getStoredAuth,
  validateAuth,
  reportSystemMessage,
}: StartupAuthDependencies): Promise<StartupAuthResult> {
  try {
    const storedProfile = await getStoredAuth();
    if (!storedProfile) {
      reportSystemMessage("保存済みの Twitch 認証情報はありません。");
      return { status: "missing" };
    }

    reportSystemMessage("保存済みの Twitch 認証情報を確認しています…");
    const result = await validateAuth();
    reportSystemMessage(`Twitch 認証の有効性を確認しました（${result.profile.login}）。`);
    return { status: "authenticated", result };
  } catch (error) {
    const message = String(error);
    reportSystemMessage(`Twitch 認証の確認に失敗しました。Login から再認証してください: ${message}`);
    return { status: "error", error: message };
  }
}
