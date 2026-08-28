import type { QueueItem, SpeechStatus, TwitchChatConnectionStatus } from "./types";

const activeTwitchStatuses: TwitchChatConnectionStatus[] = ["connecting", "connected", "reconnecting"];
const pendingQueueStatuses: QueueItem["status"][] = ["queued", "speaking"];

export function hasActiveTwitchChat(status: TwitchChatConnectionStatus): boolean {
  return activeTwitchStatuses.includes(status);
}

export function hasPendingSpeechWork(speechStatus: SpeechStatus, queueItems: QueueItem[]): boolean {
  return speechStatus === "speaking" || queueItems.some((item) => pendingQueueStatuses.includes(item.status));
}

export function requiresExitConfirmation(
  twitchStatus: TwitchChatConnectionStatus,
  speechStatus: SpeechStatus,
  queueItems: QueueItem[],
  hasUnsavedChanges: boolean,
): boolean {
  return hasUnsavedChanges || hasActiveTwitchChat(twitchStatus) || hasPendingSpeechWork(speechStatus, queueItems);
}
