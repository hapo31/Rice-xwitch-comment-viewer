import { describe, expect, it } from "vitest";
import {
  formatLocalChatTime,
  normalizeUtcTimestamp,
  parseUtcTimestamp,
  utcTimestamp,
} from "./time";

describe("UTC timestamp contract", () => {
  it.each([
    ["UTC", "2026-08-15T12:34:56Z", "2026-08-15T12:34:56Z"],
    ["fractional seconds", "2026-08-15T12:34:56.789123456Z", "2026-08-15T12:34:56.789123456Z"],
    ["positive offset", "2026-08-15T21:34:56.789123456+09:00", "2026-08-15T12:34:56.789123456Z"],
    ["negative offset", "2026-08-15T05:04:56-07:30", "2026-08-15T12:34:56Z"],
  ])("normalizes %s to the UTC Z representation", (_caseName, value, expected) => {
    expect(parseUtcTimestamp(value)).toBe(expected);
  });

  it.each([
    ["missing", undefined],
    ["empty", ""],
    ["naive", "2026-08-15T12:34:56"],
    ["invalid calendar date", "2026-02-30T12:34:56Z"],
    ["invalid offset", "2026-08-15T12:34:56+24:00"],
    ["unsupported leap second", "2016-12-31T23:59:60Z"],
    ["unparseable", "invalid-timestamp"],
  ])("rejects %s input", (_caseName, value) => {
    expect(parseUtcTimestamp(value)).toBeUndefined();
  });

  it("uses the supplied receive-time fallback for an invalid bridge value", () => {
    const fallback = utcTimestamp("2026-08-15T12:34:56.789Z");

    expect(normalizeUtcTimestamp("invalid-timestamp", fallback)).toBe(fallback);
  });

  it("formats the normalized instant in the requested local timezone", () => {
    expect(formatLocalChatTime("2026-08-15T12:34:56Z", "Asia/Tokyo")).toBe("21:34:56");
  });

  it("returns a stable placeholder instead of throwing for invalid display input", () => {
    expect(formatLocalChatTime("invalid-timestamp")).toBe("--:--:--");
  });
});
