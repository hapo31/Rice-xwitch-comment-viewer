import { describe, expect, it, vi } from "vitest";
import {
  subscribeLauncherDragDrop,
  type LauncherDragDropEvent,
  type LauncherDragDropHandlers,
} from "./dragDropListener";

const handlers = (): LauncherDragDropHandlers => ({
  onEnter: vi.fn(),
  onOver: vi.fn(),
  onLeave: vi.fn(),
  onDrop: vi.fn(),
});

describe("subscribeLauncherDragDrop", () => {
  it("keeps one subscription while dispatching drops to handlers from the latest render", async () => {
    let listener: ((event: LauncherDragDropEvent) => void) | undefined;
    const unlisten = vi.fn();
    const subscribe = vi.fn(async (nextListener) => {
      listener = nextListener;
      return unlisten;
    });
    const initialHandlers = handlers();
    const handlersRef = { current: initialHandlers };

    const cleanup = subscribeLauncherDragDrop(subscribe, handlersRef);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const updatedHandlers = handlers();
    handlersRef.current = updatedHandlers;

    listener?.({ payload: { type: "drop", paths: ["C:\\Apps\\Stream.exe"] } });

    expect(subscribe).toHaveBeenCalledTimes(1);
    expect(initialHandlers.onDrop).not.toHaveBeenCalled();
    expect(updatedHandlers.onDrop).toHaveBeenCalledWith(["C:\\Apps\\Stream.exe"]);

    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("cleans up a listener whose asynchronous registration finishes after unmount", async () => {
    let resolveSubscription: ((unlisten: () => void) => void) | undefined;
    const unlisten = vi.fn();
    const subscribe = vi.fn(() => new Promise<() => void>((resolve) => {
      resolveSubscription = resolve;
    }));

    const cleanup = subscribeLauncherDragDrop(subscribe, { current: handlers() });
    await new Promise((resolve) => setTimeout(resolve, 0));
    cleanup();
    resolveSubscription?.(unlisten);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
