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
    await new Promise((next) => setTimeout(next, 0));

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

  it("continues registering after a synchronous subscription failure", async () => {
    const unlisten = vi.fn();
    const onError = vi.fn();
    const dispose = subscribeWithCleanup([
      () => { throw new Error("sync listen failed"); },
      async () => unlisten,
    ], onError);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dispose();

    expect(onError).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("continues cleanup when an unlisten function throws", async () => {
    const failingUnlisten = vi.fn(() => { throw new Error("unlisten failed"); });
    const succeedingUnlisten = vi.fn();
    const onError = vi.fn();
    const dispose = subscribeWithCleanup([
      async () => failingUnlisten,
      async () => succeedingUnlisten,
    ], onError);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dispose();

    expect(failingUnlisten).toHaveBeenCalledOnce();
    expect(succeedingUnlisten).toHaveBeenCalledOnce();
    expect(onError).toHaveBeenCalledOnce();
  });

  it("reports a failing unlisten that resolves after cleanup", async () => {
    let resolve!: (unlisten: () => void) => void;
    const delayed = new Promise<() => void>((next) => { resolve = next; });
    const onError = vi.fn();
    const dispose = subscribeWithCleanup([() => delayed], onError);
    dispose();
    resolve(() => { throw new Error("late unlisten failed"); });
    await new Promise((next) => setTimeout(next, 0));

    expect(onError).toHaveBeenCalledOnce();
  });

  it("does not notify an unmounted consumer when delayed registration fails", async () => {
    let reject!: (error: Error) => void;
    const delayed = new Promise<() => void>((_resolve, nextReject) => { reject = nextReject; });
    const onError = vi.fn();
    const dispose = subscribeWithCleanup([() => delayed], onError);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dispose();
    reject(new Error("late registration failed"));
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onError).not.toHaveBeenCalled();
  });

  it("cleans every registration even when they return the same unlisten function", async () => {
    const unlisten = vi.fn();
    const dispose = subscribeWithCleanup([async () => unlisten, async () => unlisten]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    dispose();

    expect(unlisten).toHaveBeenCalledTimes(2);
  });
});
