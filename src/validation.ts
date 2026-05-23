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

export function parseRuleList(value: string): string[] {
  return Array.from(
    new Set(
      value
        .split(/\r?\n|,/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  ).slice(0, 200);
}

export function formatRuleList(items: string[] | undefined): string {
  return (items ?? []).join("\n");
}
