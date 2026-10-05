import type { ProfilerOnRenderCallback } from "react";
import { act, render, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { DomainProvider, createDomainStores } from "../stores/domainStores";
import { tauriMock } from "./tauriMock";

describe("AppShell domain render boundaries", () => {
  it.each(["/settings", "/logs", "/launcher"])(
    "keeps the real %s route body committed while queue revisions change",
    async (path) => {
      const stores = createDomainStores();
      let commits = 0;
      const onRender: ProfilerOnRenderCallback = () => {
        commits += 1;
      };
      const router = createMemoryRouter(
        [
          {
            path: "*",
            element: (
              <DomainProvider stores={stores}>
                <AppShell onRouteCommit={onRender} />
              </DomainProvider>
            ),
          },
        ],
        { initialEntries: [path] },
      );
      const view = render(<RouterProvider router={router} />);
      try {
        await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
        await waitFor(() => expect(tauriMock.listenerCount("speech://queue-updated")).toBe(1));
        const baseline = commits;
        for (let revision = 1; revision <= 4; revision += 1) {
          act(() => stores.queue.dispatch({ type: "items.replaced", items: [], revision }));
        }
        expect(commits).toBe(baseline);
      } finally {
        view.unmount();
        router.dispose();
      }
    },
  );

  it("polls Device Code at its interval, reschedules pending/slowDown, expires, and clears timers", async () => {
    const stores = createDomainStores();
    let polls = 0;
    tauriMock.setCommand("twitch_poll_auth", () => {
      polls += 1;
      return polls === 1
        ? { status: "pending", message: "waiting", interval: 2 }
        : polls === 2
          ? { status: "slowDown", message: "slower", interval: 3 }
          : { status: "pending", message: "waiting", interval: 2 };
    });
    const router = createMemoryRouter(
      [
        {
          path: "*",
          element: (
            <DomainProvider stores={stores}>
              <AppShell />
            </DomainProvider>
          ),
        },
      ],
      { initialEntries: ["/settings"] },
    );
    const view = render(<RouterProvider router={router} />);
    try {
      await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
      await waitFor(() => expect(tauriMock.listenerCount("speech://queue-updated")).toBe(1));
      vi.useFakeTimers();
      const prompt = {
        userCode: "ABCD-EFGH",
        verificationUri: "https://www.twitch.tv/activate",
        expiresIn: 8,
        expiresAtMs: Date.now() + 8000,
        interval: 1,
      };
      act(() => {
        stores.connection.dispatch({ type: "auth.prompt.changed", prompt });
        stores.connection.dispatch({ type: "auth.status.changed", status: "unauthenticated" });
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1000);
      });
      expect(polls).toBe(1);
      expect(stores.connection.getState().twitchAuthPrompt?.interval).toBe(2);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1999);
      });
      expect(polls).toBe(1);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(polls).toBe(2);
      expect(stores.connection.getState().twitchAuthPrompt?.interval).toBe(3);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2999);
      });
      expect(polls).toBe(2);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(polls).toBe(3);
      expect(stores.connection.getState().twitchAuthPrompt?.interval).toBe(2);

      // An explicit operation cancels the current timer; restoring the idle state re-arms it.
      act(() => stores.connection.dispatch({ type: "auth.status.changed", status: "polling" }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000);
      });
      expect(polls).toBe(3);
      const expiringPrompt = { ...prompt, expiresAtMs: Date.now() + 500, interval: 1 };
      act(() => {
        stores.connection.dispatch({ type: "auth.prompt.changed", prompt: expiringPrompt });
        stores.connection.dispatch({ type: "auth.status.changed", status: "unauthenticated" });
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });
      expect(stores.connection.getState().twitchAuthPrompt).toBeUndefined();
      expect(stores.connection.getState().twitchAuthStatus).toBe("expired");
      expect(polls).toBe(3);

      act(() => {
        stores.connection.dispatch({
          type: "auth.prompt.changed",
          prompt: { ...prompt, expiresAtMs: Date.now() + 5000 },
        });
        stores.connection.dispatch({ type: "auth.status.changed", status: "unauthenticated" });
      });
      view.unmount();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });
      expect(polls).toBe(3);
    } finally {
      view.unmount();
      router.dispose();
      vi.useRealTimers();
    }
  });
});
