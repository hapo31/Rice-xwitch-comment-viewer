import { act, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import type { AppEventsSnapshot, TwitchDeviceAuthStart, TwitchUserProfile } from "../types";
import { tauriMock } from "./tauriMock";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
const profile: TwitchUserProfile = {
  userId: "1",
  login: "viewer",
  scopes: ["user:read:chat"],
  expiresIn: 3600,
};
function prompt(): TwitchDeviceAuthStart {
  return {
    userCode: "MANUAL-CODE",
    verificationUri: "https://www.twitch.tv/activate",
    expiresIn: 60,
    expiresAtMs: Date.now() + 60_000,
    interval: 30,
  };
}
const snapshot: AppEventsSnapshot = {
  revision: 3,
  logs: [],
  emitErrors: [],
  twitchStatuses: [{ domain: "auth", status: "disconnected", revision: 3, occurredAtMs: 1 }],
};
function mountAuth(strict = false) {
  const stores = createDomainStores();
  const element = (
    <DomainProvider stores={stores}>
      <AppShell />
    </DomainProvider>
  );
  const router = createMemoryRouter(
    [{ path: "*", element: strict ? <StrictMode>{element}</StrictMode> : element }],
    { initialEntries: ["/auth"] },
  );
  const view = render(<RouterProvider router={router} />);
  return {
    stores,
    close: () => {
      view.unmount();
      router.dispose();
    },
  };
}
async function startManual() {
  await waitFor(() => expect(screen.getByRole("button", { name: "認証開始" })).toBeEnabled());
  await act(async () => screen.getByRole("button", { name: "認証開始" }).click());
}

describe("startup restoration preserves later manual auth intent", () => {
  it.each([false, true])(
    "preserves a pending start when snapshot completes (StrictMode=%s)",
    async (strict) => {
      const pendingSnapshot = deferred<AppEventsSnapshot>();
      const pendingStart = deferred<TwitchDeviceAuthStart>();
      tauriMock.setCommand("app_events_snapshot", () => pendingSnapshot.promise);
      tauriMock.setCommand("twitch_start_auth", () => pendingStart.promise);
      const app = mountAuth(strict);
      await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("app_events_snapshot"));
      await startManual();
      await act(async () => pendingSnapshot.resolve(snapshot));
      expect(tauriMock.invoke).not.toHaveBeenCalledWith("twitch_get_stored_auth");
      await act(async () => pendingStart.resolve(prompt()));
      expect(await screen.findByText("MANUAL-CODE")).toBeVisible();
      expect(app.stores.connection.getState().twitchAuthPrompt?.userCode).toBe("MANUAL-CODE");
      app.close();
    },
  );

  it("does not replay an old auth snapshot over an already authorized manual poll", async () => {
    const pendingSnapshot = deferred<AppEventsSnapshot>();
    tauriMock.setCommand("app_events_snapshot", () => pendingSnapshot.promise);
    tauriMock.setCommand("twitch_start_auth", prompt());
    tauriMock.setCommand("twitch_poll_auth", { status: "authorized", profile });
    const app = mountAuth();
    await startManual();
    await screen.findByText("MANUAL-CODE");
    await act(async () => screen.getByRole("button", { name: "今すぐ確認" }).click());
    await waitFor(() =>
      expect(app.stores.connection.getState().twitchAuthStatus).toBe("authenticated"),
    );
    await act(async () => pendingSnapshot.resolve(snapshot));
    expect(app.stores.connection.getState().twitchAuthStatus).toBe("authenticated");
    expect(app.stores.connection.getState().twitchProfile).toEqual(profile);
    expect(tauriMock.invoke).not.toHaveBeenCalledWith("twitch_get_stored_auth");
    app.close();
  });

  it.each(["resolve", "reject"] as const)(
    "ignores delayed stored auth %s after manual start",
    async (completion) => {
      const pendingStored = deferred<TwitchUserProfile>();
      tauriMock.setCommand("twitch_get_stored_auth", () => pendingStored.promise);
      tauriMock.setCommand("twitch_start_auth", prompt());
      const app = mountAuth();
      await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_get_stored_auth"));
      await startManual();
      await screen.findByText("MANUAL-CODE");
      const before = app.stores.connection.getState();
      const chatBefore = app.stores.chat.getState().messages;
      await act(async () => {
        if (completion === "resolve") pendingStored.resolve(profile);
        else pendingStored.reject(new Error("old restore failed"));
      });
      expect(app.stores.connection.getState()).toEqual(before);
      expect(app.stores.chat.getState().messages).toEqual(chatBefore);
      expect(tauriMock.invoke).not.toHaveBeenCalledWith("twitch_validate_auth");
      app.close();
    },
  );

  it.each(["resolve", "reject"] as const)(
    "ignores old validation %s after manual start and poll",
    async (completion) => {
      const pendingValidation = deferred<{ profile: TwitchUserProfile }>();
      tauriMock.setCommand("twitch_get_stored_auth", profile);
      tauriMock.setCommand("twitch_validate_auth", () => pendingValidation.promise);
      tauriMock.setCommand("twitch_start_auth", prompt());
      const freshProfile = { ...profile, userId: "2", login: "freshviewer" };
      tauriMock.setCommand("twitch_poll_auth", { status: "authorized", profile: freshProfile });
      const app = mountAuth();
      await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_validate_auth"));
      await startManual();
      await screen.findByText("MANUAL-CODE");
      await act(async () => screen.getByRole("button", { name: "今すぐ確認" }).click());
      await waitFor(() =>
        expect(app.stores.connection.getState().twitchProfile).toEqual(freshProfile),
      );
      const chatBefore = app.stores.chat.getState().messages;
      await act(async () => {
        if (completion === "resolve") pendingValidation.resolve({ profile });
        else pendingValidation.reject(new Error("stale validation failed"));
      });
      expect(app.stores.connection.getState().twitchProfile).toEqual(freshProfile);
      expect(app.stores.connection.getState().twitchAuthStatus).toBe("authenticated");
      expect(app.stores.chat.getState().messages).toEqual(chatBefore);
      app.close();
    },
  );
});
