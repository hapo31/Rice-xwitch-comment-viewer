import { describe, expect, it } from "vitest";
import { appReducer, initialAppState } from "./appStore";
import type { ChatMessage, QueueItem } from "../types";

function chatMessage(id: string): ChatMessage {
  return {
    id,
    receivedAt: "2026-05-23T00:00:00Z",
    userDisplayName: "viewer",
    text: `comment ${id}`,
    status: "queued",
  };
}

describe("appReducer", () => {
  it("prepends chat messages and keeps the latest 200", () => {
    const state = Array.from({ length: 205 }, (_, index) => index).reduce(
      (current, index) =>
        appReducer(current, {
          type: "chat.message",
          message: chatMessage(String(index)),
        }),
      initialAppState,
    );

    expect(state.chatMessages).toHaveLength(200);
    expect(state.chatMessages[0]?.id).toBe("204");
    expect(state.chatMessages[state.chatMessages.length - 1]?.id).toBe("5");
  });

  it("replaces queue items from speech queue events", () => {
    const items: QueueItem[] = [
      {
        id: "speech-1",
        sourceMessageId: "message-1",
        userDisplayName: "viewer",
        text: "viewerさん。こんにちは",
        status: "speaking",
      },
    ];

    const state = appReducer(initialAppState, { type: "queue.changed", items });

    expect(state.queueItems).toEqual(items);
  });

  it("keeps only the latest five warnings", () => {
    const state = Array.from({ length: 8 }, (_, index) => index).reduce(
      (current, index) =>
        appReducer(current, {
          type: "warning.added",
          warning: `warning ${index}`,
        }),
      initialAppState,
    );

    expect(state.warnings).toEqual([
      "warning 7",
      "warning 6",
      "warning 5",
      "warning 4",
      "warning 3",
    ]);
  });

  it("stores application logs for the Logs view", () => {
    const state = appReducer(initialAppState, {
      type: "log.added",
      log: {
        level: "warning",
        message: "Twitch EventSub が切断されました。",
        occurredAtMs: 1,
      },
    });

    expect(state.logs).toHaveLength(1);
    expect(state.logs[0]).toMatchObject({
      id: "1-warning-Twitch EventSub が切断されました。",
      level: "warning",
    });
  });
});
