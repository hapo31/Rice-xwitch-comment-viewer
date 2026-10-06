import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { initialAppState } from "../../stores/appState";
import { utcTimestamp } from "../../time";
import type { UserChatMessage } from "../../types";
import { ChatView } from "./ChatView";

const heights = new Map<string, number>();
const observers = new Map<Element, () => void>();
function height(element: HTMLElement) {
  return element.dataset.chatMessageId ? (heights.get(element.dataset.chatMessageId) ?? 44) : 200;
}
function rowTop(element: HTMLElement) {
  return Number(element.style.transform.match(/translateY\(([-\d.]+)px\)/)?.[1] ?? 0);
}
function message(id: string): UserChatMessage {
  return {
    kind: "user",
    id,
    receivedAt: utcTimestamp("2026-10-06T00:00:00Z"),
    userDisplayName: "viewer",
    text: id,
    status: "received",
  };
}
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
function mount(messages = Array.from({ length: 40 }, (_, i) => message(`m${i}`))) {
  const tree = (items: UserChatMessage[]) => (
    <ChatView state={{ ...initialAppState, chatMessages: items }} showStartupGuide={false} />
  );
  const view = render(tree(messages));
  const viewport = screen.getByRole("log", { name: "受信チャット" });
  return {
    messages,
    viewport,
    async update(items: UserChatMessage[]) {
      view.rerender(tree(items));
      await settle();
    },
    async scroll(offset: number) {
      viewport.scrollTop = offset;
      fireEvent.scroll(viewport);
      await settle();
    },
    offset(id: string) {
      const row = viewport.querySelector<HTMLElement>(`[data-chat-message-id="${id}"]`);
      expect(row).not.toBeNull();
      return rowTop(row!) - viewport.scrollTop;
    },
  };
}

beforeEach(() => {
  heights.clear();
  observers.clear();
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return height(this);
  });
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(200);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    const body = this.querySelector<HTMLElement>('[role="rowgroup"][style]');
    return body ? Number.parseFloat(body.style.height) : 200;
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const viewport = this.closest<HTMLElement>('[role="log"]');
    const top = this.dataset.chatMessageId ? rowTop(this) - (viewport?.scrollTop ?? 0) : 0;
    return new DOMRect(0, top, 800, height(this));
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        const notify = () =>
          this.callback(
            [
              {
                target,
                borderBoxSize: [{ inlineSize: 800, blockSize: height(target as HTMLElement) }],
              } as unknown as ResizeObserverEntry,
            ],
            this as unknown as ResizeObserver,
          );
        observers.set(target, notify);
        notify();
      }
      unobserve(target: Element) {
        observers.delete(target);
      }
      disconnect() {}
    },
  );
  HTMLElement.prototype.scrollTo = vi.fn(function (
    this: HTMLElement,
    options?: ScrollToOptions | number,
    y?: number,
  ) {
    const next =
      typeof options === "number" ? (y ?? this.scrollTop) : (options?.top ?? this.scrollTop);
    if (next === this.scrollTop) return;
    this.scrollTop = next;
    queueMicrotask(() => {
      if (this.isConnected) this.dispatchEvent(new Event("scroll"));
    });
  });
});

describe("ChatView with the installed virtualizer", () => {
  it("keeps a partially hidden row after one and several prepends", async () => {
    const h = mount();
    await settle();
    await h.scroll(228);
    expect(h.offset("m5")).toBe(-8);
    await h.update([message("n1"), ...h.messages]);
    expect(h.offset("m5")).toBe(-8);
    await h.update([message("n3"), message("n2"), message("n1"), ...h.messages]);
    expect(h.offset("m5")).toBe(-8);
    expect(screen.getByRole("button", { name: "新着 3 件を表示" })).toBeInTheDocument();
  });
  it("uses measured variable heights for prepended rows", async () => {
    heights.set("n1", 68);
    heights.set("n2", 28);
    heights.set("m2", 60);
    const h = mount();
    await settle();
    await h.scroll(244);
    expect(h.offset("m5")).toBe(-8);
    await h.update([message("n2"), message("n1"), ...h.messages]);
    expect(h.offset("m5")).toBe(-8);
  });
  it("keeps the anchor when old history is removed at the 200 message limit", async () => {
    const h = mount(Array.from({ length: 200 }, (_, i) => message(`m${i}`)));
    await settle();
    await h.scroll(448);
    expect(h.offset("m10")).toBe(-8);
    await h.update([message("n2"), message("n1"), ...h.messages].slice(0, 200));
    expect(h.offset("m10")).toBe(-8);
  });
  it("keeps the anchor when a prepended row is measured again later", async () => {
    const h = mount();
    await settle();
    await h.scroll(228);
    await h.update([message("n1"), ...h.messages]);
    expect(h.offset("m5")).toBe(-8);
    const row = h.viewport.querySelector('[data-chat-message-id="n1"]')!;
    heights.set("n1", 80);
    await act(async () => observers.get(row)!());
    await settle();
    expect(h.offset("m5")).toBe(-8);
  });
  it("follows only at the top and clears the unread count when returning to latest", async () => {
    const h = mount();
    await settle();
    await h.update([message("n1"), ...h.messages]);
    expect(h.viewport.scrollTop).toBe(0);
    expect(screen.queryByRole("button", { name: /新着/ })).not.toBeInTheDocument();
    await h.scroll(228);
    await h.update([message("n2"), message("n1"), ...h.messages]);
    fireEvent.click(screen.getByRole("button", { name: "新着 1 件を表示" }));
    await settle();
    expect(h.viewport.scrollTop).toBe(0);
    expect(screen.queryByRole("button", { name: /新着/ })).not.toBeInTheDocument();
    await h.update([message("n3"), message("n2"), message("n1"), ...h.messages]);
    expect(h.viewport.scrollTop).toBe(0);
  });
});
