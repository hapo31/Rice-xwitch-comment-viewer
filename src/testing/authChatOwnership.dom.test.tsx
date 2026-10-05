import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import type { TwitchStatusEvent, TwitchUserProfile } from "../types";
import { tauriMock } from "./tauriMock";

const profile: TwitchUserProfile = {
  userId: "viewer-1",
  login: "viewer",
  scopes: ["user:read:chat"],
  expiresIn: 3600,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((complete, fail) => {
    resolve = complete;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function chatStatus(
  generation: number,
  revision: number,
): Extract<TwitchStatusEvent, { domain: "chat" }> {
  return {
    domain: "chat",
    status: "connected",
    revision,
    connectionGeneration: generation,
    activeConnection: {
      generation,
      broadcasterUserId: `channel-${generation}`,
      broadcasterLogin: `channel_${generation}`,
    },
    occurredAtMs: revision,
  };
}

async function mountConnected() {
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
    { initialEntries: ["/auth"] },
  );
  render(<RouterProvider router={router} />);
  await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_get_stored_auth"));
  await act(async () => undefined);
  act(() => {
    tauriMock.emit("twitch://status", {
      domain: "auth",
      status: "connected",
      revision: 1,
      occurredAtMs: 1,
    });
    stores.connection.dispatch({ type: "auth.profile.changed", profile });
    tauriMock.emit("twitch://status", chatStatus(7, 10));
  });
  await screen.findByRole("button", { name: "有効性確認" });
  return stores;
}

it.each([
  { succeeds: true, newerEvent: false },
  { succeeds: false, newerEvent: false },
  { succeeds: true, newerEvent: true },
  { succeeds: false, newerEvent: true },
])("keeps backend chat identity after validation ($succeeds, newer=$newerEvent)", async (test) => {
  const validation = deferred<{ profile: TwitchUserProfile }>();
  tauriMock.setCommand("twitch_validate_auth", () => validation.promise);
  const stores = await mountConnected();
  fireEvent.click(screen.getByRole("button", { name: "有効性確認" }));
  await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_validate_auth"));
  if (test.newerEvent) {
    act(() => tauriMock.emit("twitch://status", chatStatus(8, 11)));
  }
  const expected = test.newerEvent ? chatStatus(8, 11) : chatStatus(7, 10);
  await act(async () => {
    if (test.succeeds) validation.resolve({ profile });
    else validation.reject(new Error("Twitch 認証の確認がタイムアウトしました。"));
  });
  await waitFor(() =>
    expect(stores.connection.getState().twitchAuthStatus).toBe(
      test.succeeds ? "authenticated" : "unauthenticated",
    ),
  );
  expect(stores.connection.getState()).toMatchObject({
    twitchConnectionStatus: "connected",
    twitchActiveConnection: expected.activeConnection,
    twitchConnectionGeneration: expected.connectionGeneration,
    chatRevision: expected.revision,
  });
  expect(screen.getByTitle("Login 画面でチャンネルを設定")).toHaveTextContent(
    expected.activeConnection?.broadcasterLogin ?? "missing channel",
  );
  expect(tauriMock.invoke.mock.calls.some(([command]) => command === "twitch_stop_chat")).toBe(
    false,
  );
  if (!test.succeeds) {
    expect(
      stores.logs.getState().logs.some((entry) => entry.message.includes("タイムアウト")),
    ).toBe(true);
  }

  // Auth completion must not weaken later revision/generation rejection either.
  act(() => tauriMock.emit("twitch://status", { ...chatStatus(6, 9), status: "disconnected" }));
  expect(stores.connection.getState().twitchActiveConnection).toEqual(expected.activeConnection);
});

it.each([true, false])(
  "preserves a newer backend connection after an old stop completes (%s)",
  async (succeeds) => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const stop = deferred<null>();
    tauriMock.setCommand("twitch_stop_chat", () => stop.promise);
    const stores = await mountConnected();
    fireEvent.click(screen.getByRole("button", { name: "停止" }));
    await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_stop_chat"));
    act(() => {
      tauriMock.emit("twitch://status", { ...chatStatus(7, 11), status: "disconnected" });
      tauriMock.emit("twitch://status", chatStatus(8, 12));
    });
    await act(async () => {
      if (succeeds) stop.resolve(null);
      else stop.reject(new Error("古いチャット停止要求が失敗しました。"));
    });
    expect(stores.connection.getState()).toMatchObject({
      twitchConnectionStatus: "connected",
      twitchConnectionGeneration: 8,
      twitchActiveConnection: chatStatus(8, 12).activeConnection,
      chatRevision: 12,
    });
    expect(screen.getByRole("button", { name: "停止" })).toBeEnabled();
    if (!succeeds) {
      expect(stores.logs.getState().logs.some((entry) => entry.message.includes("停止要求"))).toBe(
        true,
      );
    }
  },
);
