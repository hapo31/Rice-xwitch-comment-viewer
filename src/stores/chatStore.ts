import type { ChatMessage, QueueItem } from "../types";
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
        messages: [syncChatMessageStatus(action.message, action.queueItems ?? []), ...state.messages].slice(0, 200),
      };
    case "queue.statuses.changed":
      return { messages: syncChatMessageStatuses(state.messages, action.items) };
    default:
      return state;
  }
}


export function addChatMessage(store: ExternalStore<ChatState, ChatAction>, message: ChatMessage, queueItems: QueueItem[] = []): void {
  store.dispatch({ type: "message.added", message: syncChatMessageStatus(message, queueItems) });
}

export function syncChatMessageStatuses(messages: ChatMessage[], queueItems: QueueItem[]): ChatMessage[] {
  const statusByMessageId = queueStatusByMessageId(queueItems);
  let changed = false;
  const updatedMessages = messages.map((message) => {
    if (message.kind !== "user") return message;
    const status = statusByMessageId.get(message.id);
    if (!status || status === message.status) return message;
    changed = true;
    return { ...message, status };
  });
  return changed ? updatedMessages : messages;
}

export function syncChatMessageStatus(message: ChatMessage, queueItems: QueueItem[]): ChatMessage {
  if (message.kind !== "user") return message;
  const status = queueStatusByMessageId(queueItems).get(message.id);
  return status && status !== message.status ? { ...message, status } : message;
}

function queueStatusByMessageId(queueItems: QueueItem[]) {
  return new Map(
    queueItems.flatMap((item) =>
      item.sourceMessageId
        ? [[item.sourceMessageId, item.status === "speaking" ? "queued" : item.status] as const]
        : [],
    ),
  );
}

export function createChatStore(): ExternalStore<ChatState, ChatAction> {
  return createExternalStore(chatReducer, initialChatState);
}
