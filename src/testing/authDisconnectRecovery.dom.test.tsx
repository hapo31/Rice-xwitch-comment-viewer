import { act, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import type { TwitchUserProfile } from "../types";
import { tauriMock } from "./tauriMock";

const profile: TwitchUserProfile = {
  userId: "1",
  login: "viewer",
  scopes: ["user:read:chat"],
  expiresIn: 3600,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
async function mountAuthenticated() {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  tauriMock.setCommand("twitch_get_stored_auth", profile);
  tauriMock.setCommand("twitch_validate_auth", { profile });
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
  const view = render(<RouterProvider router={router} />);
  await waitFor(() => expect(stores.connection.getState().twitchAuthStatus).toBe("authenticated"));
  return {
    stores,
    close: () => {
      view.unmount();
      router.dispose();
    },
  };
}
async function disconnect() {
  await act(async () => screen.getByRole("button", { name: "認証解除" }).click());
}
const failure =
  "保存済みの Twitch 認証情報を削除できませんでした。OS の資格情報ストアを確認してください。";

describe("auth disconnect recovery in AppShell", () => {
  it("clears the old profile when the backend success event precedes the command reply", async () => {
    const pending = deferred<null>();
    tauriMock.setCommand("twitch_disconnect", () => pending.promise);
    const app = await mountAuthenticated();
    await disconnect();
    await act(async () =>
      tauriMock.emit("twitch://status", {
        domain: "auth",
        status: "disconnected",
        revision: 20,
        occurredAtMs: 1,
      }),
    );
    await act(async () => pending.resolve(null));
    expect(app.stores.connection.getState().authRevision).toBe(20);
    expect(app.stores.connection.getState().twitchProfile).toBeUndefined();
    expect(screen.getByRole("button", { name: "認証開始" })).toBeEnabled();
    app.close();
  });

  it("retains the current auth while pending and enables retry after a keyring failure", async () => {
    const pending = deferred<null>();
    tauriMock.setCommand("twitch_disconnect", () => pending.promise);
    const app = await mountAuthenticated();
    await disconnect();
    expect(screen.getByRole("button", { name: "解除中…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "有効性確認" })).toBeDisabled();
    expect(app.stores.connection.getState().twitchAuthStatus).toBe("authenticated");
    await act(async () => pending.reject(new Error(failure)));
    await waitFor(() => expect(screen.getByRole("button", { name: "認証解除" })).toBeEnabled());
    expect(screen.getByRole("button", { name: "有効性確認" })).toBeEnabled();
    expect(app.stores.connection.getState().twitchProfile).toEqual(profile);
    expect(
      app.stores.logs
        .getState()
        .notifications.some((notice) => notice.message.includes("削除できません")),
    ).toBe(true);
    expect(
      app.stores.chat
        .getState()
        .messages.some((message) => message.text === "Twitch 連携を解除しました。"),
    ).toBe(false);
    tauriMock.setCommand("twitch_disconnect", null);
    await disconnect();
    await waitFor(() => expect(screen.getByRole("button", { name: "認証開始" })).toBeEnabled());
    expect(app.stores.connection.getState().twitchAuthStatus).toBe("unauthenticated");
    expect(app.stores.connection.getState().twitchProfile).toBeUndefined();
    app.close();
  });

  it("reconciles a partial failure with missing backend auth instead of restoring the old profile", async () => {
    const pending = deferred<null>();
    tauriMock.setCommand("twitch_disconnect", () => pending.promise);
    const app = await mountAuthenticated();
    await disconnect();
    tauriMock.setCommand("twitch_get_stored_auth", null);
    await act(async () => pending.reject(new Error(failure)));
    await waitFor(() =>
      expect(app.stores.connection.getState().twitchDisconnectRequest).toBeUndefined(),
    );
    expect(app.stores.connection.getState().twitchAuthStatus).toBe("unauthenticated");
    expect(app.stores.connection.getState().twitchProfile).toBeUndefined();
    expect(screen.getByRole("button", { name: "認証開始" })).toBeEnabled();
    app.close();
  });

  it.each(["resolve", "reject"] as const)(
    "preserves a newer backend auth event after disconnect %s",
    async (completion) => {
      const pending = deferred<null>();
      tauriMock.setCommand("twitch_disconnect", () => pending.promise);
      const app = await mountAuthenticated();
      await disconnect();
      await act(async () =>
        tauriMock.emit("twitch://status", {
          domain: "auth",
          status: "authRequired",
          revision: 50,
          occurredAtMs: 1,
          message: "再ログインしてください。",
        }),
      );
      const before = app.stores.connection.getState();
      await act(async () => {
        if (completion === "resolve") pending.resolve(null);
        else pending.reject(new Error(failure));
      });
      expect(app.stores.connection.getState().authRevision).toBe(50);
      expect(app.stores.connection.getState().twitchAuthStatus).toBe(before.twitchAuthStatus);
      expect(app.stores.connection.getState().twitchProfile).toEqual(before.twitchProfile);
      expect(app.stores.connection.getState().twitchDisconnectRequest).toBeUndefined();
      app.close();
    },
  );

  it("ignores a reconciliation response superseded by a backend event", async () => {
    const pending = deferred<TwitchUserProfile>();
    const app = await mountAuthenticated();
    tauriMock.rejectCommand("twitch_disconnect", failure);
    tauriMock.setCommand("twitch_get_stored_auth", () => pending.promise);
    await disconnect();
    await waitFor(() =>
      expect(
        tauriMock.invoke.mock.calls.filter(([name]) => name === "twitch_get_stored_auth"),
      ).toHaveLength(2),
    );
    await act(async () =>
      tauriMock.emit("twitch://status", {
        domain: "auth",
        status: "authRequired",
        revision: 51,
        occurredAtMs: 2,
      }),
    );
    const before = { ...app.stores.connection.getState(), twitchDisconnectRequest: undefined };
    await act(async () => pending.resolve(profile));
    expect(app.stores.connection.getState()).toEqual(before);
    app.close();
  });

  it("releases the pending UI when reconciliation itself fails", async () => {
    const app = await mountAuthenticated();
    tauriMock.rejectCommand("twitch_disconnect", failure);
    tauriMock.rejectCommand("twitch_get_stored_auth", "read failed");
    await disconnect();
    await waitFor(() => expect(screen.getByRole("button", { name: "認証解除" })).toBeEnabled());
    expect(app.stores.connection.getState().twitchProfile).toEqual(profile);
    expect(
      app.stores.chat
        .getState()
        .messages.some((message) => message.text.includes("現在の状態を確認できません")),
    ).toBe(true);
    app.close();
  });
});
