import type { QueueItem } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export interface QueueState {
  items: QueueItem[];
}

export type QueueAction = { type: "items.replaced"; items: QueueItem[] };

export const initialQueueState: QueueState = { items: [] };

export function queueReducer(state: QueueState, action: QueueAction): QueueState {
  switch (action.type) {
    case "items.replaced":
      return { items: action.items };
    default:
      return state;
  }
}

export function createQueueStore(): ExternalStore<QueueState, QueueAction> {
  return createExternalStore(queueReducer, initialQueueState);
}
