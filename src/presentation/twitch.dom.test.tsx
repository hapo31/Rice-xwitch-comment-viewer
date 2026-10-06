import { act, render, screen, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { tauriMock } from "../testing/tauriMock";
import type { AuthStatus, TwitchChatConnectionStatus } from "../types";

async function mountChat() {
  vi.useFakeTimers();
  const stores = createDomainStores();
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
    { initialEntries: ["/chat"] },
  );
  render(<RouterProvider router={router} />);
  await act(async () => vi.advanceTimersByTimeAsync(10_000));
  await act(async () => vi.advanceTimersByTimeAsync(10_000));
  expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_get_stored_auth");
  expect(stores.settings.getState().settings).toBeDefined();
  return stores;
}

it("uses matching connection labels in Chat, SidePanel, StatusBar and live announcements", async () => {
  const stores = await mountChat();
  try {
    const cases = [
      ["connecting", "接続中"],
      ["reconnecting", "再接続中"],
      ["connected", "受信中"],
      ["error", "接続エラー"],
    ] satisfies [TwitchChatConnectionStatus, string][];
    for (const [status, label] of cases) {
      act(() => stores.connection.dispatch({ type: "chat.status.changed", status }));
      expect(within(screen.getByRole("complementary")).getByText(label)).toBeVisible();
      expect(screen.getByRole("contentinfo")).toHaveTextContent(`未認証 / ${label}`);
      expect(screen.getByRole("main").querySelector("header")).toHaveTextContent(
        `未設定 / ${label}`,
      );
      act(() => vi.advanceTimersByTime(100));
      expect(screen.getByText(`Twitch 接続: ${label}`)).toHaveAttribute(
        "role",
        status === "error" ? "alert" : "status",
      );
      act(() => vi.advanceTimersByTime(6000));
    }
  } finally {
    vi.useRealTimers();
  }
});

it("keeps explicit compact and spoken authentication wording for the same state", async () => {
  const stores = await mountChat();
  try {
    const cases = [
      ["authorizing", "認証開始中", "認証コードを発行中"],
      ["checking", "認証確認中", "有効性を確認中"],
      ["authenticated", "ログイン済み", "ログイン済み"],
      ["error", "認証エラー", "認証エラー"],
    ] satisfies [AuthStatus, string, string][];
    for (const [status, compact, spoken] of cases) {
      act(() => stores.connection.dispatch({ type: "auth.status.changed", status }));
      expect(within(screen.getByRole("complementary")).getByText(compact)).toBeVisible();
      expect(screen.getByRole("contentinfo")).toHaveTextContent(`${compact} / 未接続`);
      act(() => vi.advanceTimersByTime(100));
      expect(screen.getByText(`Twitch 認証: ${spoken}`)).toHaveAttribute(
        "role",
        status === "error" ? "alert" : "status",
      );
      act(() => vi.advanceTimersByTime(6000));
    }
  } finally {
    vi.useRealTimers();
  }
});
