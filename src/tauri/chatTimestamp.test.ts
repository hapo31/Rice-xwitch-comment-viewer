import { describe, expect, it } from "vitest";
import { utcTimestamp } from "../time";
import { normalizeTwitchChatMessageEvent } from "./client";

const payload = {
  id: "message-1",
  platform: "twitch" as const,
  channelId: "channel-1",
  channelLogin: "streamer",
  userId: "user-1",
  userLogin: "viewer",
  userDisplayName: "Viewer",
  text: "こんにちは",
  fragments: [],
  badges: [],
};

describe("Twitch chat timestamp bridge", () => {
  it("keeps the Rust serde field name and normalizes an offset instant", () => {
    const message = normalizeTwitchChatMessageEvent({
      ...payload,
      receivedAt: "2026-08-15T21:34:56.789123456+09:00",
    });

    expect(message.receivedAt).toBe("2026-08-15T12:34:56.789123456Z");
    expect("received_at" in message).toBe(false);
  });

  it.each([
    ["missing", undefined],
    ["empty", ""],
    ["naive", "2026-08-15T12:34:56"],
    ["invalid", "invalid-timestamp"],
  ])("uses the receive-time fallback for %s input", (_caseName, receivedAt) => {
    const fallback = utcTimestamp("2026-08-15T12:34:56.789Z");
    const message = normalizeTwitchChatMessageEvent(
      { ...payload, receivedAt },
      fallback,
    );

    expect(message.receivedAt).toBe(fallback);
  });
});
