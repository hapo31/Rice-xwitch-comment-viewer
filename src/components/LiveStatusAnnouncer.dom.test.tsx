import { act, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { expect, it, vi } from "vitest";
import { initialAppState } from "../stores/appState";
import type { AppNotification } from "../types";
import { LIVE_ANNOUNCEMENT_GAP_MS, LiveStatusAnnouncer } from "./LiveStatusAnnouncer";

const notice = (id: string, severity: "warning" | "error" = "error"): AppNotification => ({
  id,
  severity,
  message: "設定の保存に失敗しました。",
  source: "command",
  occurredAtMs: 1,
  correlationId: id,
});
const initial = {
  ...initialAppState,
  twitchAuthStatus: "authenticated" as const,
  speechAdapterHealth: "connected" as const,
};

it("publishes command errors and clears between different IDs with identical text", () => {
  vi.useFakeTimers();
  const view = render(
    <StrictMode>
      <LiveStatusAnnouncer state={initial} />
    </StrictMode>,
  );
  try {
    const alert = screen.getByRole("alert");
    view.rerender(
      <StrictMode>
        <LiveStatusAnnouncer state={{ ...initial, notifications: [notice("save-1")] }} />
      </StrictMode>,
    );
    expect(alert).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(LIVE_ANNOUNCEMENT_GAP_MS));
    expect(alert).toHaveTextContent("設定の保存に失敗しました。");
    view.rerender(
      <StrictMode>
        <LiveStatusAnnouncer state={initial} />
      </StrictMode>,
    );
    expect(alert).toBeEmptyDOMElement();
    view.rerender(
      <StrictMode>
        <LiveStatusAnnouncer state={{ ...initial, notifications: [notice("save-2")] }} />
      </StrictMode>,
    );
    expect(alert).toBeEmptyDOMElement();
    act(() => vi.advanceTimersByTime(LIVE_ANNOUNCEMENT_GAP_MS));
    expect(screen.getByRole("alert")).toBe(alert);
    expect(alert).toHaveTextContent("設定の保存に失敗しました。");
  } finally {
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});

it("delivers both simultaneous failures followed by a polite warning and stops after unmount", () => {
  vi.useFakeTimers();
  const view = render(<LiveStatusAnnouncer state={initial} />);
  try {
    view.rerender(
      <LiveStatusAnnouncer
        state={{
          ...initial,
          twitchAuthStatus: "error",
          speechAdapterHealth: "error",
          notifications: [notice("warning", "warning")],
        }}
      />,
    );
    act(() => vi.advanceTimersToNextTimer());
    expect(screen.getByRole("alert")).toHaveTextContent("Twitch 認証: 認証エラー");
    act(() => vi.advanceTimersToNextTimer());
    expect(screen.getByRole("alert")).toBeEmptyDOMElement();
    act(() => vi.advanceTimersToNextTimer());
    expect(screen.getByRole("alert")).toHaveTextContent("棒読みちゃん:");
    act(() => vi.advanceTimersToNextTimer());
    act(() => vi.advanceTimersToNextTimer());
    expect(screen.getByRole("status")).toHaveTextContent("警告:");
  } finally {
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  }
});
