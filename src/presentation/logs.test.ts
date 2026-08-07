import { afterEach, describe, expect, it, vi } from "vitest";

const dateTimeFormatDescriptor = Object.getOwnPropertyDescriptor(Intl, "DateTimeFormat");

describe("formatLogTime", () => {
  afterEach(() => {
    Object.defineProperty(Intl, "DateTimeFormat", dateTimeFormatDescriptor!);
    vi.restoreAllMocks();
    vi.resetModules();
  });

  it("reuses one formatter while formatting a full log buffer", async () => {
    const DateTimeFormat = Intl.DateTimeFormat;
    let formatterCreations = 0;
    Object.defineProperty(Intl, "DateTimeFormat", {
      configurable: true,
      writable: true,
      value: function (...args: any[]) {
        formatterCreations += 1;
        return new DateTimeFormat(...args);
      },
    });
    const { formatLogTime } = await import("./logs");

    const formattedTimes = Array.from({ length: 500 }, (_, index) => formatLogTime(index * 1_000));

    expect(formatterCreations).toBe(1);
    expect(formattedTimes).toHaveLength(500);
    expect(formattedTimes[0]).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
});
