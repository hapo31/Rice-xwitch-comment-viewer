import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { nativeWindow, tauriMock } from "./tauriMock";

// jsdom has no native modal dialog support. Keep the open/close and focus return
// contract so AppShell DOM tests can exercise the same React lifecycle.
const dialogReturnFocus = new WeakMap<HTMLDialogElement, HTMLElement | null>();
HTMLDialogElement.prototype.showModal = function showModal() {
  dialogReturnFocus.set(
    this,
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );
  this.setAttribute("open", "");
};
HTMLDialogElement.prototype.close = function close() {
  this.removeAttribute("open");
  const previousFocus = dialogReturnFocus.get(this);
  if (previousFocus?.isConnected) previousFocus.focus();
  dialogReturnFocus.delete(this);
};

vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriMock.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: tauriMock.listen }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => nativeWindow }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (listener: Parameters<typeof tauriMock.listen>[1]) =>
      tauriMock.listen("tauri://drag-drop", listener),
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: tauriMock.open }));

Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });

beforeEach(() => {
  tauriMock.reset();
  window.sessionStorage.clear();
  window.localStorage.clear();
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
  );
  vi.stubGlobal(
    "ResizeObserver",
    class {
      // jsdom has no layout. This is a deterministic viewport/row contract,
      // not a pixel-layout or WebView2 test.
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        const height = target.hasAttribute("data-index") ? 40 : 600;
        this.callback(
          [
            {
              target,
              borderBoxSize: [{ inlineSize: 1000, blockSize: height }],
              contentRect: new DOMRect(0, 0, 1000, height),
            } as unknown as ResizeObserverEntry,
          ],
          this as unknown as ResizeObserver,
        );
      }
      unobserve() {}
      disconnect() {}
    },
  );
  HTMLElement.prototype.scrollIntoView = vi.fn();
  HTMLElement.prototype.scrollTo = vi.fn();
});

afterEach(async () => {
  cleanup();
  tauriMock.releaseSubscriptions();
  // Delayed Tauri registration promises must deliver their unlisten callbacks
  // after unmount. Never reset the registry before this assertion.
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(tauriMock.listenerCount(), "Native listener leaked after cleanup").toBe(0);
  expect(tauriMock.pendingCount).toBe(0);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
