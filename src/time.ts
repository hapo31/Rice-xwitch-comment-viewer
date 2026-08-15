declare const utcTimestampBrand: unique symbol;

/** RFC 3339 timestamp normalized to the UTC `Z` representation. */
export type UtcTimestamp = string & { readonly [utcTimestampBrand]: true };

const rfc3339WithOffset = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})$/;

export function parseUtcTimestamp(value: unknown): UtcTimestamp | undefined {
  if (typeof value !== "string") {
    return undefined;
  }

  const match = rfc3339WithOffset.exec(value);
  if (!match) {
    return undefined;
  }

  const [, yearText, monthText, dayText, hourText, minuteText, secondText, fraction, offset] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText);
  const offsetHours = offset === "Z" ? 0 : Number(offset.slice(1, 3));
  const offsetMinutes = offset === "Z" ? 0 : Number(offset.slice(4, 6));

  if (
    month < 1 || month > 12 ||
    day < 1 || day > daysInMonth(year, month) ||
    hour > 23 || minute > 59 || second > 59 ||
    offsetHours > 23 || offsetMinutes > 59
  ) {
    return undefined;
  }

  const epochMilliseconds = Date.parse(value);
  if (!Number.isFinite(epochMilliseconds)) {
    return undefined;
  }

  const utcSecond = new Date(epochMilliseconds).toISOString().slice(0, 19);
  return `${utcSecond}${fraction ? `.${fraction}` : ""}Z` as UtcTimestamp;
}

export function utcTimestamp(value: string): UtcTimestamp {
  const timestamp = parseUtcTimestamp(value);
  if (!timestamp) {
    throw new RangeError(`Invalid RFC 3339 timestamp: ${value}`);
  }
  return timestamp;
}

export function utcNow(): UtcTimestamp {
  return new Date().toISOString() as UtcTimestamp;
}

export function normalizeUtcTimestamp(
  value: unknown,
  fallback: UtcTimestamp = utcNow(),
): UtcTimestamp {
  return parseUtcTimestamp(value) ?? fallback;
}

export function formatLocalChatTime(value: unknown, timeZone?: string): string {
  const timestamp = parseUtcTimestamp(value);
  if (!timestamp) {
    return "--:--:--";
  }

  return new Intl.DateTimeFormat("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    ...(timeZone ? { timeZone } : {}),
  }).format(new Date(timestamp));
}

function daysInMonth(year: number, month: number): number {
  if (month === 2) {
    const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    return leapYear ? 29 : 28;
  }
  return [4, 6, 9, 11].includes(month) ? 30 : 31;
}
