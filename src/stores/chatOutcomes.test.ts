import { expect, it } from "vitest";
import { parseSpeechQueueOutcome } from "../tauri/bridge";
import fixture from "../tauri/fixtures/queue-outcomes.json";
import { utcTimestamp } from "../time";
import type { QueueItem, UserChatMessage } from "../types";
import { chatReducer, initialChatState, syncChatMessageStatuses } from "./chatStore";

const message: UserChatMessage = {
  kind: "user",
  id: "chat-1",
  receivedAt: utcTimestamp("2026-10-05T00:00:00Z"),
  userDisplayName: "viewer",
  text: "hello",
  status: "blocked",
};
const item: QueueItem = {
  id: "speech-1",
  sourceMessageId: message.id,
  userDisplayName: "viewer",
  text: "hello",
  status: "blocked",
  outcome: parseSpeechQueueOutcome(fixture[2]),
};

it("updates reasons even when status is unchanged and preserves referential identity for equivalent reloads", () => {
  const first = syncChatMessageStatuses([message], [item]);
  expect(first[0]).toMatchObject({ speechOutcome: item.outcome, speechQueueItemId: item.id });
  expect(syncChatMessageStatuses(first, [JSON.parse(JSON.stringify(item))])).toBe(first);
  const changed = { ...item, outcome: parseSpeechQueueOutcome(fixture[3]) };
  const second = syncChatMessageStatuses(first, [changed]);
  expect(second[0]).toMatchObject({ speechOutcome: changed.outcome, status: "blocked" });
  expect(syncChatMessageStatuses(second, [])).toBe(second);
  const retried = syncChatMessageStatuses(second, [
    { ...item, status: "queued", outcome: undefined },
  ]);
  expect(retried[0]).toMatchObject({ status: "queued" });
  expect("speechOutcome" in retried[0]).toBe(false);
});

it("keeps event-before-chat and queue reload on the production outcome contract", () => {
  const domain = chatReducer(initialChatState, {
    type: "message.added",
    message,
    queueItems: [item],
  });
  expect(domain.messages).toEqual([expect.objectContaining({ id: message.id, status: "blocked" })]);
  const error: QueueItem = {
    ...item,
    status: "error",
    outcome: parseSpeechQueueOutcome(fixture[10]),
  };
  expect(
    chatReducer(domain, { type: "queue.statuses.changed", items: [error] }).messages,
  ).toMatchObject([{ status: "error", speechOutcome: error.outcome }]);
});

it("retains only the existing bounded 200 chat messages, including their outcomes", () => {
  let state = initialChatState;
  for (let n = 0; n < 250; n++) {
    const next = { ...message, id: `chat-${n}` };
    state = chatReducer(state, {
      type: "message.added",
      message: next,
      queueItems: [{ ...item, sourceMessageId: next.id }],
    });
  }
  expect(state.messages).toHaveLength(200);
  expect(state.messages[0].id).toBe("chat-249");
  expect(
    state.messages.every(
      (entry) => entry.kind === "user" && entry.speechOutcome?.reasonCode === "blockedWord",
    ),
  ).toBe(true);
});
