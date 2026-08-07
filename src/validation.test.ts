import { describe, expect, it } from "vitest";
import { formatBouyomiAddress, isValidBouyomiHost, isValidBouyomiVoice, isValidPort, isValidTwitchChannelLogin, parseRuleList } from "./validation";

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

  it("validates and formats Bouyomi IPv4, DNS, and IPv6 hosts", () => {
    expect(isValidBouyomiHost("127.0.0.1")).toBe(true);
    expect(isValidBouyomiHost("localhost")).toBe(true);
    expect(isValidBouyomiHost("::1")).toBe(true);
    expect(isValidBouyomiHost("[::1]")).toBe(false);
    expect(isValidBouyomiHost("::1:50001")).toBe(false);
    expect(formatBouyomiAddress("::1", 50001)).toBe("[::1]:50001");
  });

  it("normalizes rule lists", () => {
    expect(parseRuleList(" alice\nbob, alice ,, ")).toEqual(["alice", "bob"]);
  });
});
