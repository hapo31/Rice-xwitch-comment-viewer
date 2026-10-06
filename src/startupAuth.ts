import { presentError } from "./presentation/errors";
import type { TwitchAuthValidationResult, TwitchUserProfile } from "./types";

export type StartupAuthResult =
  | { status: "superseded" }
  | { status: "missing" }
  | { status: "authenticated"; result: TwitchAuthValidationResult }
  | { status: "error"; error: string };

export interface StartupAuthDependencies {
  isCurrent?: () => boolean;
  getStoredAuth: () => Promise<TwitchUserProfile | undefined>;
  validateAuth: () => Promise<TwitchAuthValidationResult>;
  reportSystemMessage: (message: string) => void;
  reportTechnicalError?: (details: string) => void;
}

export async function restoreAndValidateStartupAuth({
  isCurrent = () => true,
  getStoredAuth,
  validateAuth,
  reportSystemMessage,
  reportTechnicalError,
}: StartupAuthDependencies): Promise<StartupAuthResult> {
  try {
    if (!isCurrent()) return { status: "superseded" };
    const storedProfile = await getStoredAuth();
    if (!isCurrent()) return { status: "superseded" };
    if (!storedProfile) {
      reportSystemMessage("保存済みの Twitch 認証情報はありません。");
      return { status: "missing" };
    }

    reportSystemMessage("保存済みの Twitch 認証情報を確認しています…");
    const result = await validateAuth();
    if (!isCurrent()) return { status: "superseded" };
    reportSystemMessage(`Twitch 認証の有効性を確認しました（${result.profile.login}）。`);
    return { status: "authenticated", result };
  } catch (error) {
    if (!isCurrent()) return { status: "superseded" };
    const { message, details } = presentError(error, "auth");
    reportTechnicalError?.(details);
    reportSystemMessage(message);
    return { status: "error", error: message };
  }
}
