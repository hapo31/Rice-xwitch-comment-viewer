import { describe, expect, it } from "vitest";
import {
  formatBouyomiAddress,
  isValidBouyomiHost,
  isValidBouyomiVoice,
  isValidPort,
  isValidRepeatSuppressionSeconds,
  isValidTwitchChannelLogin,
  parseBlockedUserList,
  parseBlockedWordList,
  RULE_LIST_LIMIT,
} from "./validation";

describe("settings validation", () => {
  it("validates Twitch channel logins", () => {
    expect(isValidTwitchChannelLogin("")).toBe(true);
    expect(isValidTwitchChannelLogin("rice_123")).toBe(true);
    expect(isValidTwitchChannelLogin("ab")).toBe(false);
    expect(isValidTwitchChannelLogin("invalid-name")).toBe(false);
  });

  it("validates Bouyomi port and voice values", () => {
    expect(isValidPort("50001")).toBe(true);
    expect(isValidPort("0")).toBe(false);
    expect(isValidPort("65536")).toBe(false);
    expect(isValidBouyomiVoice("10001")).toBe(true);
    expect(isValidBouyomiVoice("-1")).toBe(false);
  });

  it.each<[string, boolean]>([
    ["0", true],
    ["1", true],
    ["2", true],
    ["-1", false],
    ["31", false],
    ["1.5", false],
  ])("validates repeat suppression boundary %s", (value, expected) => {
    expect(isValidRepeatSuppressionSeconds(value)).toBe(expected);
  });

  it("validates and formats Bouyomi IPv4, DNS, and IPv6 hosts", () => {
    expect(isValidBouyomiHost("127.0.0.1")).toBe(true);
    expect(isValidBouyomiHost("localhost")).toBe(true);
    expect(isValidBouyomiHost("::1")).toBe(true);
    expect(isValidBouyomiHost("::ffff:127.0.0.1")).toBe(true);
    expect(isValidBouyomiHost("[::1]")).toBe(false);
    expect(isValidBouyomiHost("::ffff:127.0.0.999")).toBe(false);
    expect(isValidBouyomiHost("::1:50001")).toBe(false);
    expect(formatBouyomiAddress("::1", 50001)).toBe("[::1]:50001");
  });

  it("normalizes case-insensitive duplicate rule lists", () => {
    expect(parseBlockedUserList(" @Alice\nalice, bob ,, ")).toEqual({
      items: ["Alice", "bob"],
      duplicateCount: 1,
      overflowCount: 0,
    });
    expect(parseBlockedWordList("BadWord\nbadword")).toEqual({
      items: ["BadWord"],
      duplicateCount: 1,
      overflowCount: 0,
    });
  });

  it.each([199, 200, 201])("reports rule-list limits for %i items", (count) => {
    const result = parseBlockedWordList(
      Array.from({ length: count }, (_, index) => `word-${index}`).join("\n"),
    );

    expect(result.items).toHaveLength(count);
    expect(result.overflowCount).toBe(Math.max(0, count - RULE_LIST_LIMIT));
  });
});
