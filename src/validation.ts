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
