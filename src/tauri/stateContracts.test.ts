import { expect, it } from "vitest";
import type { SpeechQueueOutcome, TwitchStatusEvent } from "../types";
import { speechOutcomeReasonCodes, twitchStatusesByDomain } from "../types";
import { parseSpeechQueueOutcome, parseTwitchStatusEvent } from "./bridge";

const details = { message: "状態の案内", occurredAtMs: 1 };

it("preserves the typed outcome variants and their existing recovery choices", () => {
  const cases = [
    {
      ...details,
      kind: "blocked",
      reasonCode: "blockedWord",
      retryable: false,
      recoveryAction: "reviewFilters",
    },
    {
      ...details,
      kind: "skipped",
      reasonCode: "overflow",
      retryable: false,
      recoveryAction: "reviewQueue",
    },
    {
      ...details,
      kind: "skipped",
      reasonCode: "userSkip",
      retryable: false,
      recoveryAction: "none",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "connectTimeout",
      retryable: true,
      recoveryAction: "diagnoseSpeech",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "configuration",
      retryable: false,
      recoveryAction: "diagnoseSpeech",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "connectionLost",
      retryable: false,
      recoveryAction: "confirmDelivery",
    },
  ] satisfies SpeechQueueOutcome[];
  for (const value of cases) expect(parseSpeechQueueOutcome(value)).toEqual(value);
});

it("preserves delivery confirmation for all errors after the adapter accepted the item", () => {
  // Rust SpeechQueueOutcome::error treats accepted=true as uncertain for every reason.
  for (const reasonCode of speechOutcomeReasonCodes.error) {
    const outcome = {
      ...details,
      kind: "error",
      reasonCode,
      retryable: false,
      recoveryAction: "confirmDelivery",
    } satisfies SpeechQueueOutcome;
    expect(parseSpeechQueueOutcome(outcome)).toEqual(outcome);
  }
});

it("rejects the same impossible recovery combinations in types and the runtime parser", () => {
  const cases = [
    {
      ...details,
      kind: "blocked",
      reasonCode: "blockedWord",
      retryable: true,
      recoveryAction: "reviewFilters",
    },
    {
      ...details,
      kind: "blocked",
      reasonCode: "blockedWord",
      retryable: false,
      recoveryAction: "confirmDelivery",
    },
    {
      ...details,
      kind: "skipped",
      reasonCode: "overflow",
      retryable: false,
      recoveryAction: "none",
    },
    {
      ...details,
      kind: "skipped",
      reasonCode: "userSkip",
      retryable: false,
      recoveryAction: "reviewQueue",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "writeTimeout",
      retryable: true,
      recoveryAction: "diagnoseSpeech",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "connectTimeout",
      retryable: true,
      recoveryAction: "confirmDelivery",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "writeTimeout",
      retryable: false,
      recoveryAction: "diagnoseSpeech",
    },
    {
      ...details,
      kind: "error",
      reasonCode: "connectionLost",
      retryable: false,
      recoveryAction: "diagnoseSpeech",
    },
  ] as const;
  // @ts-expect-error Blocked outcomes cannot be retried.
  const blockedRetry: SpeechQueueOutcome = cases[0];
  // @ts-expect-error Blocked outcomes require filter review.
  const blockedDelivery: SpeechQueueOutcome = cases[1];
  // @ts-expect-error Overflow requires queue review.
  const overflow: SpeechQueueOutcome = cases[2];
  // @ts-expect-error User skips have no recovery action.
  const skipped: SpeechQueueOutcome = cases[3];
  // @ts-expect-error A write timeout cannot be retried safely.
  const writeRetry: SpeechQueueOutcome = cases[4];
  // @ts-expect-error Delivery confirmation and retry are mutually exclusive.
  const ambiguousRetry: SpeechQueueOutcome = cases[5];
  // @ts-expect-error A non-retryable write failure always requires delivery confirmation.
  const uncertainWrite: SpeechQueueOutcome = cases[6];
  // @ts-expect-error A non-retryable lost connection also has uncertain delivery.
  const uncertainConnection: SpeechQueueOutcome = cases[7];
  for (const value of [
    uncertainWrite,
    uncertainConnection,
    blockedRetry,
    blockedDelivery,
    overflow,
    skipped,
    writeRetry,
    ambiguousRetry,
  ])
    expect(() => parseSpeechQueueOutcome(value)).toThrow(/SpeechQueueOutcome/);
});

it("uses the same status lists for domain types and parsing, including legacy omissions", () => {
  const cases: TwitchStatusEvent[] = [
    ...twitchStatusesByDomain.auth.map(
      (status): TwitchStatusEvent => ({ ...details, domain: "auth", status }),
    ),
    ...twitchStatusesByDomain.chat.map(
      (status): TwitchStatusEvent => ({ ...details, domain: "chat", status }),
    ),
    { ...details, domain: "auth", status: "authRequired", reason: "missingRequiredScope" },
    {
      ...details,
      domain: "chat",
      status: "connected",
      connectionGeneration: 3,
      activeConnection: { generation: 3, broadcasterUserId: "123", broadcasterLogin: "rice" },
    },
  ];
  for (const value of cases) expect(parseTwitchStatusEvent(value)).toEqual(value);
});

it("rejects the same cross-domain states and metadata in types and the runtime parser", () => {
  const cases = [
    { ...details, domain: "chat", status: "validating" },
    { ...details, domain: "auth", status: "reconnecting" },
    { ...details, domain: "auth", status: "connected", connectionGeneration: 1 },
    {
      ...details,
      domain: "auth",
      status: "connected",
      activeConnection: { generation: 1, broadcasterUserId: "1", broadcasterLogin: "rice" },
    },
    { ...details, domain: "chat", status: "authRequired", reason: "missingRequiredScope" },
    { ...details, domain: "auth", status: "connected", reason: "missingRequiredScope" },
  ] as const;
  // @ts-expect-error Token validation belongs to Auth.
  const chatValidation: TwitchStatusEvent = cases[0];
  // @ts-expect-error Socket reconnection belongs to Chat.
  const authReconnection: TwitchStatusEvent = cases[1];
  // @ts-expect-error Auth cannot carry Chat generation metadata.
  const authGeneration: TwitchStatusEvent = cases[2];
  // @ts-expect-error Auth cannot carry an active Chat identity.
  const authConnection: TwitchStatusEvent = cases[3];
  // @ts-expect-error Missing scope is the Auth recovery reason.
  const chatReason: TwitchStatusEvent = cases[4];
  // @ts-expect-error An authenticated state cannot require a missing scope.
  const connectedReason: TwitchStatusEvent = cases[5];
  for (const value of [
    chatValidation,
    authReconnection,
    authGeneration,
    authConnection,
    chatReason,
    connectedReason,
  ])
    expect(() => parseTwitchStatusEvent(value)).toThrow(/TwitchStatusEvent/);
});
