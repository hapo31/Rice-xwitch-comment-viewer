import { describe, expect, it, vi } from "vitest";
import { subscribeWithCleanup } from "./subscriptions";

describe("subscribeWithCleanup", () => {
  it("removes listeners that resolve after cleanup", async () => {
    let resolve!: (unlisten: () => void) => void;
    const delayed = new Promise<() => void>((next) => { resolve = next; });
    const unlisten = vi.fn();

    const dispose = subscribeWithCleanup([() => delayed]);
    dispose();
    resolve(unlisten);
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("cleans successful registrations even when another registration fails", async () => {
    const unlisten = vi.fn();
    const onError = vi.fn();
    const dispose = subscribeWithCleanup([
      async () => unlisten,
      async () => Promise.reject(new Error("listen failed")),
    ], onError);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dispose();

    expect(onError).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
