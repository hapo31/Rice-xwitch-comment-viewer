import { z } from "zod";

const ipv6Address = z.ipv6();

export function isValidTwitchChannelLogin(value: string): boolean {
  const channel = value.trim();
  return (
    utf8Bytes(value) <= 128 &&
    !hasControl(value) &&
    (channel.length === 0 || /^[a-zA-Z0-9_]{3,25}$/.test(channel))
  );
}

function utf8Bytes(value: string): number {
  return new TextEncoder().encode(value).length;
}
function hasControl(value: string): boolean {
  return Array.from(value).some((character) => {
    const point = character.codePointAt(0) ?? 0;
    return point < 32 || (point >= 127 && point <= 159);
  });
}
export function isValidBlockedWord(value: string): boolean {
  return (
    Boolean(value.trim()) &&
    !hasControl(value) &&
    Array.from(value).length <= 500 &&
    utf8Bytes(value) <= 2048
  );
}
export function isValidConfirmationText(value: string): boolean {
  return !hasControl(value) && Array.from(value).length <= 120 && utf8Bytes(value) <= 480;
}
export function blockedRulesError(
  users: readonly string[],
  words: readonly string[],
): string | undefined {
  if (users.length > 200 || words.length > 200) return "NGルールは各200件以内にしてください。";
  if (users.some((user) => !user.trim() || !isValidTwitchChannelLogin(user)))
    return "NGユーザーは英数字・_の3〜25文字で入力してください。";
  if (words.some((word) => !isValidBlockedWord(word)))
    return "NGワードは制御文字を含まない1〜500文字・2048 UTF-8バイト以内にしてください。";
  if ([...users, ...words].reduce((sum, value) => sum + utf8Bytes(value), 0) > 64 * 1024)
    return "NGユーザーとNGワードの合計は64KiB UTF-8以内にしてください。";
  return undefined;
}

export function isValidPort(value: string | number): boolean {
  const port = Number(value);
  return Number.isInteger(port) && port > 0 && port <= 65535;
}

export function isValidBouyomiVoice(value: string | number): boolean {
  const voice = Number(value);
  return Number.isInteger(voice) && voice >= 0 && voice <= 30000;
}

export function isValidRepeatSuppressionSeconds(value: string | number): boolean {
  if (typeof value === "string" && value.trim().length === 0) {
    return false;
  }
  const seconds = Number(value);
  return Number.isInteger(seconds) && seconds >= 0 && seconds <= 30;
}

export function isValidBouyomiHost(value: string): boolean {
  const host = value.trim();
  if (!host || utf8Bytes(value) > 253 || hasControl(value) || /[\s\[\]]/.test(host)) {
    return false;
  }

  // Host policy above is application-specific; IPv6 grammar belongs to Zod.
  if (host.includes(":")) {
    return ipv6Address.safeParse(host).success;
  }

  return host
    .split(".")
    .every(
      (label) => label.length <= 63 && /^[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?$/.test(label),
    );
}

export function formatBouyomiAddress(host: string, port: number): string {
  const normalizedHost = host.trim();
  return normalizedHost.includes(":") ? `[${normalizedHost}]:${port}` : `${normalizedHost}:${port}`;
}

export const RULE_LIST_LIMIT = 200;

export type RuleListParseResult = {
  items: string[];
  duplicateCount: number;
  overflowCount: number;
};

export function parseBlockedUserList(value: string): RuleListParseResult {
  return parseRuleList(value, (item) => item.replace(/^@+/, ""));
}

export function parseBlockedWordList(value: string): RuleListParseResult {
  return parseRuleList(value, (item) => item);
}

function parseRuleList(
  value: string,
  normalizeItem: (item: string) => string,
): RuleListParseResult {
  const seen = new Set<string>();
  const items: string[] = [];
  let duplicateCount = 0;

  for (const rawItem of value.split(/\r?\n|,/)) {
    const item = normalizeItem(rawItem.trim());
    if (!item) {
      continue;
    }

    const key = asciiLowercase(item);
    if (seen.has(key)) {
      duplicateCount += 1;
      continue;
    }

    seen.add(key);
    items.push(item);
  }

  return {
    items,
    duplicateCount,
    overflowCount: Math.max(0, items.length - RULE_LIST_LIMIT),
  };
}

export function formatRuleList(items: string[] | undefined): string {
  return (items ?? []).join("\n");
}

function asciiLowercase(value: string): string {
  return value.replace(/[A-Z]/g, (character) => character.toLowerCase());
}
