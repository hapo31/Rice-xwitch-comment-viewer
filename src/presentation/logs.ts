const logTimeFormatter = new Intl.DateTimeFormat("ja-JP", {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
});

export function formatLogTime(occurredAtMs: number): string {
  return logTimeFormatter.format(new Date(occurredAtMs));
}
