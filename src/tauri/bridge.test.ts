import { describe, expect, it } from "vitest";
import bridgeContractFixture from "./fixtures/bridge-contract.json";
import {
  parseSpeechQueueUpdatedEvent,
  parseSpeechStatusEvent,
  parseTwitchAuthPollResult,
  parseTwitchAuthValidationResult,
  parseTwitchChatMessageWireEvent,
  parseTwitchStatusEvent,
  rejectUnexpectedNulls,
} from "./bridge";

describe("Tauri bridge Option contract", () => {
  it("accepts the Rust-serialized omission fixture and rejects null fields that TypeScript treats as optional", () => {
    const fixture = bridgeContractFixture.optionOmissions;
    expect(parseSpeechQueueUpdatedEvent(fixture.queue).items[0].sourceMessageId).toBeUndefined();
    expect(() => parseSpeechQueueUpdatedEvent({
      queuedCount: 0,
      items: [{ id: "queue-1", sourceMessageId: null, userDisplayName: "viewer", text: "hello", status: "queued" }],
      occurredAtMs: 1,
    })).toThrow("sourceMessageId");

    expect(parseTwitchStatusEvent(fixture.twitchStatus).message).toBeUndefined();
    expect(() => parseTwitchStatusEvent({ domain: "chat", status: "connected", message: null, occurredAtMs: 1 })).toThrow("message");
    expect(parseSpeechStatusEvent(fixture.speechStatus).message).toBeUndefined();
    expect(parseSpeechQueueUpdatedEvent(fixture.queue).warning).toBeUndefined();
  });

  it("validates the camelCase Device Code warning fixture and exposes it to the caller", () => {
    const fixture = bridgeContractFixture.authorizedPoll;
    const result = parseTwitchAuthPollResult(fixture);
    expect(result).toMatchObject({ status: "authorized", storageWarning: fixture.storageWarning });
    const snakeCaseFixture: Record<string, unknown> = { ...fixture, storage_warning: fixture.storageWarning };
    delete snakeCaseFixture.storageWarning;
    expect(() => parseTwitchAuthPollResult(snakeCaseFixture)).toThrow("storage_warning");
    expect(parseTwitchAuthValidationResult(bridgeContractFixture.optionOmissions.authValidation).storageWarning).toBeUndefined();
  });

  it("rejects null fragment options while allowing omitted emote, cheermote, and ownerId", () => {
    const message = {
      id: "message-1",
      platform: "twitch",
      channelId: "channel-1",
      channelLogin: "streamer",
      userId: "user-1",
      userLogin: "viewer",
      userDisplayName: "Viewer",
      text: "hello",
      fragments: [bridgeContractFixture.optionOmissions.fragment, { type: "emote", text: "Kappa", emote: bridgeContractFixture.optionOmissions.emote }],
      badges: [],
    };
    const parsed = parseTwitchChatMessageWireEvent(message);
    expect(parsed.fragments[0].emote).toBeUndefined();
    expect(parsed.fragments[1].emote?.ownerId).toBeUndefined();
    expect(() => parseTwitchChatMessageWireEvent({ ...message, fragments: [{ type: "text", text: "hello", emote: null }] })).toThrow("emote");
  });

  it("uses the generic guard only to reject nested null values", () => {
    expect(() => rejectUnexpectedNulls({ launcher: [{ iconDataUrl: null }] }, "launcher_add")).toThrow("payload.launcher[0].iconDataUrl");
    expect(() => rejectUnexpectedNulls("not an object", "app_build_info")).not.toThrow();
  });
});
