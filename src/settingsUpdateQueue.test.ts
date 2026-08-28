import { describe, expect, it } from "vitest";
import { SettingsUpdateQueue } from "./settingsUpdateQueue";

describe("SettingsUpdateQueue", () => {
  it("serializes concurrent leaf updates so both backend responses include prior changes", async () => {
    const queue = new SettingsUpdateQueue();
    const completed: string[] = [];
    let releaseFirst: (() => void) | undefined;
    const firstGate = new Promise<void>((resolve) => { releaseFirst = resolve; });

    const first = queue.enqueue(async () => {
      await firstGate;
      completed.push("first");
      return { autoConnect: true };
    });
    const second = queue.enqueue(async () => {
      completed.push("second");
      return { autoConnect: true, confirmBeforeStopChat: false };
    });

    await Promise.resolve();
    expect(completed).toEqual([]);
    releaseFirst?.();
    await expect(first).resolves.toEqual({ autoConnect: true });
    await expect(second).resolves.toEqual({ autoConnect: true, confirmBeforeStopChat: false });
    expect(completed).toEqual(["first", "second"]);
  });

  it("continues with a later update after a failed save", async () => {
    const queue = new SettingsUpdateQueue();
    const failed = queue.enqueue(async () => { throw new Error("save failed"); });
    const later = queue.enqueue(async () => "saved");

    await expect(failed).rejects.toThrow("save failed");
    await expect(later).resolves.toBe("saved");
    await expect(queue.waitForIdle()).resolves.toBeUndefined();
  });
});
