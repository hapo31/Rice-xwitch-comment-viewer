import { afterEach, expect, it, vi } from "vitest";
import { startSpeechHealthMonitor } from "./speechHealthMonitor";
import type { SpeechAdapterHealth } from "../types";

afterEach(() => vi.useRealTimers());

it("keeps probing while connected/paused, avoids overlap and reports only recovery", async () => {
  vi.useFakeTimers();
  let health: SpeechAdapterHealth = "connected";
  const queuePhase = "paused";
  let resolve!: (message: string) => void;
  const probe = vi.fn(
    () =>
      new Promise<string>((done) => {
        resolve = done;
      }),
  );
  const onRecovered = vi.fn();
  const stop = startSpeechHealthMonitor({ probe, getHealth: () => health, onRecovered });
  expect(probe).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(10000);
  expect(probe).toHaveBeenCalledTimes(1);
  resolve("接続確認済み");
  await Promise.resolve();
  expect(onRecovered).not.toHaveBeenCalled();
  health = "disconnected";
  await vi.advanceTimersByTimeAsync(5000);
  expect(probe).toHaveBeenCalledTimes(2);
  health = "connected";
  resolve("復旧");
  await Promise.resolve();
  expect(onRecovered).toHaveBeenCalledExactlyOnceWith("復旧");
  expect(queuePhase).toBe("paused");
  stop();
  await vi.advanceTimersByTimeAsync(10000);
  expect(probe).toHaveBeenCalledTimes(2);
});

it("does not fabricate state on rejection or invoke callbacks after cleanup", async () => {
  vi.useFakeTimers();
  const onRecovered = vi.fn();
  let resolve!: (message: string) => void;
  const probe = vi
    .fn()
    .mockRejectedValueOnce(new Error("typed native failure"))
    .mockImplementation(
      () =>
        new Promise<string>((done) => {
          resolve = done;
        }),
    );
  const stop = startSpeechHealthMonitor({ probe, getHealth: () => "error", onRecovered });
  await Promise.resolve();
  expect(onRecovered).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(5000);
  expect(probe).toHaveBeenCalledTimes(2);
  stop();
  resolve("遅延した成功");
  await Promise.resolve();
  expect(onRecovered).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
});
