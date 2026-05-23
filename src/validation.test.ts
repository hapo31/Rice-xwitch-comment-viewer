import { describe, expect, it } from "vitest";
import { isValidBouyomiVoice, isValidPort, isValidTwitchChannelLogin, parseRuleList } from "./validation";

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

  it("normalizes rule lists", () => {
    expect(parseRuleList(" alice\nbob, alice ,, ")).toEqual(["alice", "bob"]);
  });
});
