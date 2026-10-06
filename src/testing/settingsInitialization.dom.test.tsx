import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import type { AppSettings } from "../types";
import { defaultSettings, tauriMock } from "./tauriMock";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function mount(path = "/settings", strict = false) {
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
    { initialEntries: [path] },
  );
  const content = <RouterProvider router={router} />;
  return { stores, router, ...render(strict ? <StrictMode>{content}</StrictMode> : content) };
}

it.each(["/settings", "/filter"])(
  "does not offer editable default values before %s initialization and can retry a failed load",
  async (path) => {
    const read = deferred<AppSettings>();
    tauriMock.setCommand("settings_get", () => read.promise);
    const h = mount(path);
    expect(screen.getByText(/読み込み完了後に編集できます/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "設定を保存" })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "ポート" })).not.toBeInTheDocument();
    await act(async () => read.reject(new Error("設定ファイルを確認して再試行してください。")));
    expect(h.stores.settings.getState().initialization.status).toBe("error");
    tauriMock.setCommand("settings_get", defaultSettings);
    fireEvent.click(screen.getByRole("button", { name: "設定を再読み込み" }));
    await waitFor(() => expect(h.stores.settings.getState().initialization.status).toBe("ready"));
    expect(screen.queryByRole("button", { name: "設定を再読み込み" })).not.toBeInTheDocument();
    expect(
      h.stores.chat
        .getState()
        .messages.some(
          (entry) => entry.kind === "system" && entry.text === "設定を読み込みました。",
        ),
    ).toBe(true);
  },
);

it("keeps a save and current connection settings when the first StrictMode read returns last", async () => {
  const old = deferred<AppSettings>();
  let calls = 0;
  tauriMock.setCommand("settings_get", () => (++calls === 1 ? old.promise : defaultSettings));
  tauriMock.setCommand("settings_take_recovery_notice", { message: "設定を復旧しました。" });
  tauriMock.setCommand("settings_update", {
    ...defaultSettings,
    speech: { ...defaultSettings.speech, bouyomiPort: 50002 },
  });
  const h = mount("/settings", true);
  await waitFor(() => expect(h.stores.settings.getState().initialization.status).toBe("ready"));
  expect(calls).toBe(2);
  expect(
    tauriMock.invoke.mock.calls.filter(([command]) => command === "settings_take_recovery_notice"),
  ).toHaveLength(1);
  await act(async () =>
    fireEvent.change(screen.getByRole("textbox", { name: "ポート" }), {
      target: { value: "50002" },
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "設定を保存" }));
  await waitFor(() =>
    expect(h.stores.settings.getState().settings?.speech.bouyomiPort).toBe(50002),
  );
  await act(async () => old.resolve(defaultSettings));
  expect(screen.getByRole("textbox", { name: "ポート" })).toHaveValue("50002");
  expect(h.stores.settings.getState().settings?.speech.bouyomiPort).toBe(50002);
  expect(
    h.stores.chat
      .getState()
      .messages.filter((entry) => entry.kind === "system" && entry.text === "設定を復旧しました。"),
  ).toHaveLength(1);
});

it("does not publish settings or recovery feedback after the provider unmounts", async () => {
  const read = deferred<AppSettings>();
  tauriMock.setCommand("settings_get", () => read.promise);
  tauriMock.setCommand("settings_take_recovery_notice", { message: "古い復旧通知" });
  const h = mount();
  const before = h.stores.chat.getState().messages;
  h.unmount();
  await act(async () => read.resolve(defaultSettings));
  expect(h.stores.settings.getState().settings).toBeUndefined();
  expect(h.stores.chat.getState().messages).toBe(before);
});
