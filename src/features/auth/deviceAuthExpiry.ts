export function getDeviceAuthRemainingSeconds(expiresAtMs: number, nowMs = Date.now()): number {
  return Math.max(0, Math.ceil((expiresAtMs - nowMs) / 1000));
}

export function formatDeviceAuthRemainingTime(remainingSeconds: number): string {
  const minutes = Math.floor(remainingSeconds / 60);
  const seconds = remainingSeconds % 60;
  return `${minutes}分 ${seconds.toString().padStart(2, "0")}秒`;
}
