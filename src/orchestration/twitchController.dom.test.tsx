import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthOperationController } from "../authOperation";
import { createTwitchController } from "./twitchController";
import { initialAppState, appReducer, type AppAction } from "../stores/appStore";
import type { TwitchDeviceAuthStart, TwitchUserProfile } from "../types";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { tauriMock } from "../testing/tauriMock";

const prompt: TwitchDeviceAuthStart = {
  userCode: "ABCD-EFGH",
  verificationUri: "https://www.twitch.tv/activate",
  expiresIn: 60,
  expiresAtMs: Date.now() + 60_000,
  interval: 1,
};
const profile: TwitchUserProfile = {
  userId: "1",
  login: "viewer",
  scopes: ["user:read:chat"],
  expiresIn: 3600,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function makeController(state = initialAppState) {
  let current = state;
  const operations = new AuthOperationController();
  const reportInfo = vi.fn();
  const reportNotification = vi.fn();
  const reportError = vi.fn();
  const controller = createTwitchController({
    operations,
    dispatch: (action: AppAction) => {
      current = appReducer(current, action);
    },
    getAuthPrompt: () => current.twitchAuthPrompt,
    getAuthStatus: () => current.twitchAuthStatus,
    getAuthProfile: () => current.twitchProfile,
    getChannelLogin: () => undefined,
    getConfirmBeforeStopChat: () => false,
    waitForSettings: async () => undefined,
    reportSystemMessage: vi.fn(),
    reportInfo,
    reportNotification,
    reportError,
    reportTechnicalError: vi.fn(),
    routeAutoConnectTimeline: vi.fn(),
  });
  return {
    controller,
    operations,
    getState: () => current,
    reportInfo,
    reportNotification,
    reportError,
  };
}

afterEach(() => vi.useRealTimers());

describe("Twitch controller auth-operation lifecycle", () => {
  it.each(["start", "validate"] as const)(
    "does not let a due expiry timer preempt deferred manual %s",
    async (operation) => {
      vi.useFakeTimers();
      vi.setSystemTime(1_000_000);
      const expiringPrompt = {
        ...prompt,
        expiresAtMs: Date.now() + 1000,
        interval: 5,
      };
      const pendingStart = deferred<TwitchDeviceAuthStart>();
      const pendingValidation = deferred<{ profile: TwitchUserProfile }>();
      tauriMock.setCommand("twitch_start_auth", () => pendingStart.promise);
      tauriMock.setCommand("twitch_validate_auth", () => pendingValidation.promise);
      const harness = makeController({
        ...initialAppState,
        twitchAuthPrompt: expiringPrompt,
      });
      harness.controller.schedulePoll(expiringPrompt);

      const manual =
        operation === "start" ? harness.controller.startAuth() : harness.controller.validateAuth();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1000);
      });

      expect(harness.getState().twitchAuthStatus).not.toBe("expired");
      expect(harness.reportNotification).not.toHaveBeenCalledWith(
        "warning",
        "event",
        "Twitch の認証コードの有効期限が切れました。再度ログインしてください。",
      );

      if (operation === "start") pendingStart.resolve({ ...prompt, userCode: "NEW-CODE" });
      else pendingValidation.resolve({ profile });
      await act(async () => manual);

      expect(harness.getState().twitchAuthStatus).not.toBe("expired");
      if (operation === "start") {
        expect(harness.getState().twitchAuthPrompt?.userCode).toBe("NEW-CODE");
      } else {
        expect(harness.getState()).toMatchObject({
          twitchAuthStatus: "authenticated",
          twitchProfile: profile,
        });
      }
    },
  );

  it("does not expire an already-expired retained prompt during manual start", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    const pendingStart = deferred<TwitchDeviceAuthStart>();
    tauriMock.setCommand("twitch_start_auth", () => pendingStart.promise);
    const expiredPrompt = { ...prompt, expiresAtMs: Date.now() - 1 };
    const harness = makeController({ ...initialAppState, twitchAuthPrompt: expiredPrompt });

    const manual = harness.controller.startAuth();
    harness.controller.schedulePoll(expiredPrompt);

    expect(harness.getState().twitchAuthStatus).not.toBe("expired");
    expect(harness.getState().twitchAuthPrompt).toEqual(expiredPrompt);
    expect(harness.reportNotification).not.toHaveBeenCalledWith(
      "warning",
      "event",
      "Twitch の認証コードの有効期限が切れました。再度ログインしてください。",
    );

    pendingStart.resolve({ ...prompt, userCode: "NEW-CODE" });
    await act(async () => manual);
    expect(harness.getState().twitchAuthPrompt?.userCode).toBe("NEW-CODE");
  });

  it.each(["start", "validate", "disconnect"] as const)(
    "prevents a due Device Code timer from overtaking deferred manual %s",
    async (operation) => {
      vi.useFakeTimers();
      vi.spyOn(window, "confirm").mockReturnValue(true);
      const pendingStart = deferred<TwitchDeviceAuthStart>();
      const pendingValidation = deferred<{ profile: TwitchUserProfile }>();
      const pendingDisconnect = deferred<null>();
      tauriMock.setCommand("twitch_start_auth", () => pendingStart.promise);
      tauriMock.setCommand("twitch_validate_auth", () => pendingValidation.promise);
      tauriMock.setCommand("twitch_disconnect", () => pendingDisconnect.promise);
      const harness = makeController({
        ...initialAppState,
        twitchAuthPrompt: prompt,
      });
      harness.controller.schedulePoll(prompt);

      let manual: Promise<unknown>;
      if (operation === "start") manual = harness.controller.startAuth();
      else if (operation === "validate") manual = harness.controller.validateAuth();
      else manual = harness.controller.disconnect();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1000);
      });
      expect(tauriMock.invoke).not.toHaveBeenCalledWith("twitch_poll_auth");
      expect(harness.operations.getState().activeOperation).toBe(operation);

      if (operation === "start") pendingStart.resolve({ ...prompt, userCode: "NEW-CODE" });
      else if (operation === "validate") pendingValidation.resolve({ profile });
      else pendingDisconnect.resolve(null);
      await act(async () => manual);

      expect(tauriMock.invoke).not.toHaveBeenCalledWith("twitch_poll_auth");
      if (operation === "start") {
        expect(harness.getState()).toMatchObject({
          twitchAuthStatus: "unauthenticated",
          twitchAuthPrompt: { userCode: "NEW-CODE" },
        });
      } else if (operation === "validate") {
        expect(harness.getState()).toMatchObject({
          twitchAuthStatus: "authenticated",
          twitchProfile: profile,
        });
      } else {
        expect(harness.getState()).toMatchObject({
          twitchAuthStatus: "unauthenticated",
          twitchAuthPrompt: undefined,
        });
      }
    },
  );

  it("invalidates deferred manual auth results when AppShell unmounts", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const pendingStart = deferred<TwitchDeviceAuthStart>();
    tauriMock.setCommand("twitch_start_auth", () => pendingStart.promise);
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
    await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
    await waitFor(() => expect(screen.getByRole("button", { name: "認証開始" })).toBeEnabled());
    await act(async () => {
      screen.getByRole("button", { name: "認証開始" }).click();
    });
    await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_start_auth"));
    const beforeUnmount = stores.connection.getState();
    const chatBeforeUnmount = stores.chat.getState().messages;
    const logsBeforeUnmount = stores.logs.getState().logs;
    view.unmount();
    router.dispose();

    pendingStart.resolve(prompt);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(stores.connection.getState()).toEqual(beforeUnmount);
    expect(stores.chat.getState().messages).toEqual(chatBeforeUnmount);
    expect(stores.logs.getState().logs).toEqual(logsBeforeUnmount);
  });

  it("suppresses delayed restore callbacks when AppShell unmounts", async () => {
    const pendingStoredAuth = deferred<TwitchUserProfile | undefined>();
    tauriMock.setCommand("twitch_get_stored_auth", () => pendingStoredAuth.promise);
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
    await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
    await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("twitch_get_stored_auth"));
    const chatBeforeUnmount = stores.chat.getState().messages;
    view.unmount();
    router.dispose();

    pendingStoredAuth.resolve(undefined);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(stores.chat.getState().messages).toEqual(chatBeforeUnmount);
    expect(stores.chat.getState().messages).toHaveLength(0);
    expect(stores.logs.getState().logs).toEqual([]);
  });
});
