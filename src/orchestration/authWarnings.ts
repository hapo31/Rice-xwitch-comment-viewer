import type { TwitchAuthPollResult, TwitchAuthValidationResult } from "../types";

type AuthResultWithStorageWarning = Extract<TwitchAuthPollResult, { status: "authorized" }> | TwitchAuthValidationResult;

/** Route an auth persistence warning through both visible recovery paths. */
export function routeAuthStorageWarning(
  result: AuthResultWithStorageWarning,
  reportNotification: (severity: "warning", source: "system", message: string) => void,
  addSystemChatMessage: (message: string) => void,
): void {
  if (!result.storageWarning) return;
  reportNotification("warning", "system", result.storageWarning);
  addSystemChatMessage(result.storageWarning);
}
