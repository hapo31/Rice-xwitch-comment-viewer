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
    const sections = host.split("::");
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
