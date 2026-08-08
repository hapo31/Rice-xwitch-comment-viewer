export function isValidTwitchChannelLogin(value: string): boolean {
  const channel = value.trim();
  return channel.length === 0 || /^[a-zA-Z0-9_]{3,25}$/.test(channel);
}

export function isValidPort(value: string | number): boolean {
  const port = Number(value);
  return Number.isInteger(port) && port > 0 && port <= 65535;
}

export function isValidBouyomiVoice(value: string | number): boolean {
  const voice = Number(value);
  return Number.isInteger(voice) && voice >= 0 && voice <= 30000;
}

export function isValidBouyomiHost(value: string): boolean {
  const host = value.trim();
  if (!host || /[\s\[\]]/.test(host)) {
    return false;
  }

  if (host.includes(":")) {
    const lastColon = host.lastIndexOf(":");
    const ipv4Tail = host.slice(lastColon + 1);
    const ipv4Octets = ipv4Tail.split(".");
    if (
      ipv4Tail.includes(".") &&
      (ipv4Octets.length !== 4 ||
        ipv4Octets.some((octet) => !/^\d+$/.test(octet) || Number(octet) > 255))
    ) {
      return false;
    }

    const normalizedHost = ipv4Tail.includes(".")
      ? `${host.slice(0, lastColon + 1)}${((Number(ipv4Octets[0]) << 8) | Number(ipv4Octets[1])).toString(16)}:${((Number(ipv4Octets[2]) << 8) | Number(ipv4Octets[3])).toString(16)}`
      : host;

    const sections = normalizedHost.split("::");
    if (sections.length > 2) {
      return false;
    }

    const labels = sections.flatMap((section) => (section ? section.split(":") : []));
    if (!labels.every((label) => /^[0-9a-fA-F]{1,4}$/.test(label))) {
      return false;
    }

    return sections.length === 2 ? labels.length < 8 : labels.length === 8;
  }

  return host.split(".").every((label) => /^[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?$/.test(label));
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

function parseRuleList(value: string, normalizeItem: (item: string) => string): RuleListParseResult {
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
