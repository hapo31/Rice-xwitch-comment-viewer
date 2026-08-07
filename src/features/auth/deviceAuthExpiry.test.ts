import { afterEach, describe, expect, it, vi } from "vitest";
import { formatDeviceAuthRemainingTime, getDeviceAuthRemainingSeconds } from "./deviceAuthExpiry";

describe("Device Code の期限表示", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("fake timerで期限境界までの残り時間を実時間どおりに計算する", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-07T00:00:00.000Z"));
    const expiresAtMs = Date.now() + 61_000;

    expect(getDeviceAuthRemainingSeconds(expiresAtMs)).toBe(61);
    expect(formatDeviceAuthRemainingTime(getDeviceAuthRemainingSeconds(expiresAtMs))).toBe("1分 01秒");

    vi.advanceTimersByTime(60_999);
    expect(getDeviceAuthRemainingSeconds(expiresAtMs)).toBe(1);

    vi.advanceTimersByTime(1);
    expect(getDeviceAuthRemainingSeconds(expiresAtMs)).toBe(0);
  });
});
