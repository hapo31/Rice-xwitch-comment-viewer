import type { SpeechAdapterHealth } from "../types";

/** Poll independently of queue phase, with at most one outstanding probe. */
export function startSpeechHealthMonitor(options: {
  probe: () => Promise<string>;
  getHealth: () => SpeechAdapterHealth;
  onRecovered: (message: string) => void;
  schedule?: (poll: () => void) => () => void;
}): () => void {
  let stopped = false;
  let inFlight = false;
  const poll = async () => {
    if (stopped || inFlight) return;
    inFlight = true;
    const previousHealth = options.getHealth();
    try {
      const message = await options.probe();
      if (!stopped && previousHealth !== "connected") options.onRecovered(message);
    } catch {
      // Native typed health events own failure state and details. Never turn a
      // rejected command into a fabricated frontend status/queue transition.
    } finally {
      inFlight = false;
    }
  };
  const schedule =
    options.schedule ??
    ((tick: () => void) => {
      const timer = globalThis.setInterval(tick, 5000);
      return () => globalThis.clearInterval(timer);
    });
  const cancelTimer = schedule(() => {
    void poll();
  });
  void poll();
  return () => {
    stopped = true;
    cancelTimer();
  };
}
