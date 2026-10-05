import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import type { AppSettings } from "../types";
import { defaultSettings, tauriMock } from "./tauriMock";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function settingsWithPort(port: number): AppSettings {
  return { ...defaultSettings, speech: { ...defaultSettings.speech, bouyomiPort: port } };
}

async function mountSettings() {
  tauriMock.setCommand("app_exit", null);
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
    { initialEntries: ["/settings"] },
  );
  const view = render(<RouterProvider router={router} />);
  await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
  fireEvent.change(screen.getByRole("textbox", { name: "ポート" }), {
    target: { value: "50002" },
  });
  return { stores, router, user: userEvent.setup(), ...view };
}

async function requestContinuation(
  kind: "exit" | "navigate",
  user: ReturnType<typeof userEvent.setup>,
) {
  await user.click(
    kind === "exit"
      ? screen.getByRole("button", { name: "閉じる" })
      : screen.getByRole("link", { name: "Logs" }),
  );
  return within(await screen.findByRole("dialog", { name: "未保存の変更があります" }));
}

it.each(["exit", "navigate"] as const)(
  "does not %s after cancelling a pending save",
  async (kind) => {
    const pending = deferred<AppSettings>();
    tauriMock.setCommand("settings_update", () => pending.promise);
    const { stores, router, user } = await mountSettings();
    const dialog = await requestContinuation(kind, user);
    await user.click(dialog.getByRole("button", { name: "保存して続ける" }));
    await user.click(dialog.getByRole("button", { name: "キャンセル" }));
    await act(async () => pending.resolve(settingsWithPort(50002)));
    await waitFor(() =>
      expect(stores.settings.getState().settings?.speech.bouyomiPort).toBe(50002),
    );
    expect(router.state.location.pathname).toBe("/settings");
    expect(tauriMock.invoke).not.toHaveBeenCalledWith("app_exit");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  },
);

it.each(["exit", "navigate"] as const)(
  "keeps a new %s attempt independent from the cancelled save",
  async (kind) => {
    const first = deferred<AppSettings>();
    const second = deferred<AppSettings>();
    let calls = 0;
    tauriMock.setCommand("settings_update", () => (++calls === 1 ? first.promise : second.promise));
    const { router, user } = await mountSettings();
    let dialog = await requestContinuation(kind, user);
    await user.click(dialog.getByRole("button", { name: "保存して続ける" }));
    await user.click(dialog.getByRole("button", { name: "キャンセル" }));
    fireEvent.change(screen.getByRole("textbox", { name: "ポート" }), {
      target: { value: "50003" },
    });
    dialog = await requestContinuation(kind, user);
    const save = dialog.getByRole("button", { name: "保存して続ける" });
    await user.click(save);
    await user.click(save);
    expect(save).toBeDisabled();
    await act(async () => first.resolve(settingsWithPort(50002)));
    await waitFor(() => expect(calls).toBe(2));
    expect(router.state.location.pathname).toBe("/settings");
    expect(tauriMock.invoke).not.toHaveBeenCalledWith("app_exit");
    expect(screen.getByRole("button", { name: "保存して続ける" })).toBeDisabled();
    await act(async () => second.resolve(settingsWithPort(50003)));
    if (kind === "exit")
      await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("app_exit"));
    else await waitFor(() => expect(router.state.location.pathname).toBe("/logs"));
    expect(calls).toBe(2);
  },
);

it.each(["exit", "navigate"] as const)(
  "allows retry after a failed %s save and blocks duplicate saves",
  async (kind) => {
    const first = deferred<AppSettings>();
    const second = deferred<AppSettings>();
    let calls = 0;
    tauriMock.setCommand("settings_update", () => (++calls === 1 ? first.promise : second.promise));
    const { router, user } = await mountSettings();
    const dialog = await requestContinuation(kind, user);
    const save = dialog.getByRole("button", { name: "保存して続ける" });
    fireEvent.click(save);
    fireEvent.click(save);
    await waitFor(() => expect(calls).toBe(1));
    expect(save).toBeDisabled();
    await act(async () => first.reject(new Error("保存に失敗しました")));
    await waitFor(() => expect(save).toBeEnabled());
    expect(router.state.location.pathname).toBe("/settings");
    expect(tauriMock.invoke).not.toHaveBeenCalledWith("app_exit");
    await user.click(save);
    await waitFor(() => expect(calls).toBe(2));
    await act(async () => second.resolve(settingsWithPort(50002)));
    if (kind === "exit")
      await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("app_exit"));
    else await waitFor(() => expect(router.state.location.pathname).toBe("/logs"));
  },
);

it("does not run a save continuation after the AppShell unmounts", async () => {
  const pending = deferred<AppSettings>();
  tauriMock.setCommand("settings_update", () => pending.promise);
  const { unmount, user } = await mountSettings();
  const dialog = await requestContinuation("exit", user);
  await user.click(dialog.getByRole("button", { name: "保存して続ける" }));
  unmount();
  await act(async () => pending.resolve(settingsWithPort(50002)));
  expect(tauriMock.invoke).not.toHaveBeenCalledWith("app_exit");
});

it("does not use an old save to approve a different blocked destination", async () => {
  const pending = deferred<AppSettings>();
  tauriMock.setCommand("settings_update", () => pending.promise);
  const { router, user } = await mountSettings();
  const dialog = await requestContinuation("navigate", user);
  await user.click(dialog.getByRole("button", { name: "保存して続ける" }));
  await act(async () => {
    await router.navigate("/queue");
  });
  await act(async () => pending.resolve(settingsWithPort(50002)));
  const confirmation = within(
    await screen.findByRole("dialog", { name: "移動または終了しますか？" }),
  );
  expect(router.state.location.pathname).toBe("/settings");
  await user.click(confirmation.getByRole("button", { name: "続ける" }));
  await waitFor(() => expect(router.state.location.pathname).toBe("/queue"));
  expect(
    tauriMock.invoke.mock.calls.filter(([command]) => command === "settings_update"),
  ).toHaveLength(1);
});

it.each(["exit", "navigate"] as const)(
  "requires a fresh choice for a new %s request after a cancelled save finishes",
  async (kind) => {
    const pending = deferred<AppSettings>();
    tauriMock.setCommand("settings_update", () => pending.promise);
    const { router, user } = await mountSettings();
    const dialog = await requestContinuation(kind, user);
    await user.click(dialog.getByRole("button", { name: "保存して続ける" }));
    await user.click(dialog.getByRole("button", { name: "キャンセル" }));
    await requestContinuation(kind, user);
    await act(async () => pending.resolve(settingsWithPort(50002)));
    const confirmation = within(
      await screen.findByRole("dialog", { name: "移動または終了しますか？" }),
    );
    expect(router.state.location.pathname).toBe("/settings");
    expect(tauriMock.invoke).not.toHaveBeenCalledWith("app_exit");
    await user.click(confirmation.getByRole("button", { name: "キャンセル" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/settings");
  },
);
