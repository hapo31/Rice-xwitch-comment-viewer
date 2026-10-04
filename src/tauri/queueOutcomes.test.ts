import { expect, it } from "vitest";
import fixture from "./fixtures/queue-outcomes.json";
import {
  parseSpeechQueueOutcome,
  parseSpeechQueueUpdatedEvent,
  parseSpeechStateSnapshot,
} from "./bridge";
import { speechOutcomeReasonCodes } from "../types";

it("uses the exact Rust contract for all 21 safe outcome codes", () => {
  expect(fixture.map(parseSpeechQueueOutcome)).toEqual(fixture);
  for (const kind of ["blocked", "skipped", "error"] as const)
    expect(fixture.filter((value) => value.kind === kind).map((value) => value.reasonCode)).toEqual(
      speechOutcomeReasonCodes[kind],
    );
});

function snapshot(outcome: unknown, status = "blocked") {
  return {
    revision: 10,
    status: { revision: 10, status: "idle", adapterHealth: "connected", occurredAtMs: 1 },
    queue: {
      revision: 10,
      queuedCount: 0,
      phase: "idle",
      occurredAtMs: 2,
      items: [
        {
          id: "speech-1",
          sourceMessageId: "chat-1",
          userDisplayName: "viewer",
          text: "text",
          status,
          outcome,
        },
      ],
    },
  };
}

it.each(fixture)(
  "preserves $kind/$reasonCode across snapshot and reload without a warning",
  (outcome) => {
    const parsed = parseSpeechStateSnapshot(snapshot(outcome, outcome.kind));
    expect(parsed.queue.items[0].outcome).toEqual(outcome);
    expect(parsed.queue.warning).toBeUndefined();
    expect(
      parseSpeechStateSnapshot(JSON.parse(JSON.stringify(parsed))).queue.items[0].outcome,
    ).toEqual(outcome);
  },
);

it.each([
  { reasonCode: "unknown-new-code" },
  { kind: "unknown" },
  { retryable: "false" },
  { retryable: true },
  { message: "" },
  { message: "x".repeat(201) },
  { recoveryAction: "https://evil.example" },
  { occurredAtMs: -1 },
  { occurredAtMs: 1.2 },
  { occurredAtMs: Number.MAX_SAFE_INTEGER },
  { occurredAtMs: Infinity },
  { occurredAtMs: NaN },
  { recoveryAction: "none" },
])("rejects invalid bounded outcome fields: %j", (patch) => {
  expect(() => parseSpeechQueueOutcome({ ...fixture[0], ...patch })).toThrow(/SpeechQueueOutcome/);
});

it("rejects null, status/kind mismatch and unsafe retry contracts while accepting old omission", () => {
  expect(() => parseSpeechStateSnapshot(snapshot(null))).toThrow();
  expect(() => parseSpeechStateSnapshot(snapshot(fixture[0], "spoken"))).toThrow();
  const error = fixture.find((value) => value.reasonCode === "writeTimeout")!;
  expect(() => parseSpeechQueueOutcome({ ...error, retryable: true })).toThrow();
  const legacy = snapshot(undefined);
  delete (legacy.queue.items[0] as { outcome?: unknown }).outcome;
  expect(parseSpeechQueueUpdatedEvent(legacy.queue).items[0].outcome).toBeUndefined();
  const retry = fixture.find((value) => value.reasonCode === "connectTimeout")!;
  expect(parseSpeechStateSnapshot(snapshot(retry, "queued")).queue.items[0].outcome).toEqual(retry);
});
