import type { ChatMessage, QueueItem, SpeechQueueOutcome, UserChatMessage } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export interface ChatState {
  messages: ChatMessage[];
}

export type ChatAction =
  | { type: "message.added"; message: ChatMessage; queueItems?: QueueItem[] }
  | { type: "queue.statuses.changed"; items: QueueItem[] };

export const initialChatState: ChatState = { messages: [] };

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  switch (action.type) {
    case "message.added":
      return {
        messages: [
          syncChatMessageStatus(action.message, action.queueItems ?? []),
          ...state.messages,
        ].slice(0, 200),
      };
    case "queue.statuses.changed":
      return { messages: syncChatMessageStatuses(state.messages, action.items) };
    default:
      return state;
  }
}

export function addChatMessage(
  store: ExternalStore<ChatState, ChatAction>,
  message: ChatMessage,
  queueItems: QueueItem[] = [],
): void {
  store.dispatch({ type: "message.added", message: syncChatMessageStatus(message, queueItems) });
}

export function syncChatMessageStatuses(
  messages: ChatMessage[],
  queueItems: QueueItem[],
): ChatMessage[] {
  const itemByMessageId = queueItemByMessageId(queueItems);
  let changed = false;
  const updatedMessages = messages.map((message) => {
    if (message.kind !== "user") return message;
    const item = itemByMessageId.get(message.id);
    const updated = item ? applyQueueItem(message, item) : message;
    if (updated !== message) changed = true;
    return updated;
  });
  return changed ? updatedMessages : messages;
}

export function syncChatMessageStatus(message: ChatMessage, queueItems: QueueItem[]): ChatMessage {
  if (message.kind !== "user") return message;
  const item = queueItemByMessageId(queueItems).get(message.id);
  return item ? applyQueueItem(message, item) : message;
}

function sameOutcome(left?: SpeechQueueOutcome, right?: SpeechQueueOutcome): boolean {
  return (
    left === right ||
    Boolean(
      left &&
        right &&
        left.kind === right.kind &&
        left.reasonCode === right.reasonCode &&
        left.message === right.message &&
        left.retryable === right.retryable &&
        left.recoveryAction === right.recoveryAction &&
        left.occurredAtMs === right.occurredAtMs,
    )
  );
}

function applyQueueItem(message: UserChatMessage, item: QueueItem): UserChatMessage {
  const status = item.status === "speaking" ? "queued" : item.status;
  const itemId = item.outcome ? item.id : undefined;
  if (
    status === message.status &&
    sameOutcome(message.speechOutcome, item.outcome) &&
    message.speechQueueItemId === itemId
  )
    return message;
  const { speechOutcome: _previous, speechQueueItemId: _previousId, ...rest } = message;
  return {
    ...rest,
    status,
    ...(item.outcome ? { speechOutcome: item.outcome, speechQueueItemId: item.id } : {}),
  };
}

function queueItemByMessageId(queueItems: QueueItem[]) {
  return new Map(
    queueItems.flatMap((item) =>
      item.sourceMessageId ? [[item.sourceMessageId, item] as const] : [],
    ),
  );
}

export function createChatStore(): ExternalStore<ChatState, ChatAction> {
  return createExternalStore(chatReducer, initialChatState);
}
