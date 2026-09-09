import type { QueueItem, SpeechQueuePhase } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export interface QueueState {
  items: QueueItem[];
  revision: number;
  phase: SpeechQueuePhase;
}

export type QueueAction = { type: "items.replaced"; items: QueueItem[]; revision?: number; phase?: SpeechQueuePhase };

export const initialQueueState: QueueState = { items: [], revision: 0, phase: "idle" };

export function queueReducer(state: QueueState, action: QueueAction): QueueState {
  switch (action.type) {
    case "items.replaced":
      if (action.revision !== undefined && action.revision <= state.revision) return state;
      return { items: action.items, revision: action.revision ?? state.revision, phase: action.phase ?? state.phase };
    default:
      return state;
  }
}

export function createQueueStore(): ExternalStore<QueueState, QueueAction> {
  return createExternalStore(queueReducer, initialQueueState);
}
