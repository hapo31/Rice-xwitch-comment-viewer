import { act, fireEvent, render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { tauriMock } from "./tauriMock";

async function mount(path: string) {
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
  const view = render(<RouterProvider router={router} />);
  await act(async () => vi.advanceTimersByTimeAsync(10_000));
  await act(async () => vi.advanceTimersByTimeAsync(10_000));
  expect(stores.settings.getState().settings).toBeDefined();
  const alert = document.querySelector('p.sr-only[role="alert"]');
  expect(alert).toBeInTheDocument();
  return { ...view, stores, router, alert };
}

it.each(["settings", "launcher"] as const)(
  "announces a real %s command failure and a separate repeated failure",
  async (operation) => {
    vi.useFakeTimers();
    const h = await mount(`/${operation}`);
    try {
      const message =
        operation === "settings"
          ? "保存先を確認してもう一度保存してください。"
          : "登録先を確認してもう一度追加してください。";
      tauriMock.setCommand(operation === "settings" ? "settings_update" : "launcher_add", () => {
        throw new Error(message);
      });
      tauriMock.open.mockResolvedValue(["C:\\app.exe"]);
      if (operation === "settings") {
        await act(async () =>
          fireEvent.change(screen.getByRole("textbox", { name: "ポート" }), {
            target: { value: "50002" },
          }),
        );
      }
      const action = screen.getByRole("button", {
        name: operation === "settings" ? "設定を保存" : "アプリをランチャーに追加",
      });
      for (let repeat = 1; repeat <= 2; repeat += 1) {
        await act(async () => fireEvent.click(action));
        await act(async () => vi.advanceTimersByTimeAsync(100));
        expect(h.alert).toHaveTextContent(message);
        expect(
          h.stores.logs.getState().notifications.filter((notice) => notice.message === message),
        ).toHaveLength(repeat);
        // Consume this occurrence, then invoke the same operation independently.
        await act(async () => vi.advanceTimersByTimeAsync(6000));
      }
    } finally {
      h.unmount();
      h.router.dispose();
      vi.useRealTimers();
    }
  },
);

it("coalesces a status and its event/log notice while preserving a simultaneous unrelated error", async () => {
  vi.useFakeTimers();
  const h = await mount("/chat");
  try {
    await act(async () => {
      tauriMock.emit("twitch://status", {
        domain: "chat",
        status: "error",
        revision: 100,
        connectionGeneration: 10,
        message: "接続先を確認してください。",
        occurredAtMs: 1,
      });
      tauriMock.emit("app://log", {
        id: "chat-log",
        level: "error",
        message: "接続先を確認してください。",
        occurredAtMs: 1,
      });
      tauriMock.emit("speech://status", {
        status: "error",
        adapterHealth: "error",
        revision: 100,
        message: "棒読みちゃんを起動してください。",
        occurredAtMs: 1,
      });
    });
    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(h.alert).toHaveTextContent("接続先を確認してください。");
    await act(async () => vi.advanceTimersByTimeAsync(2500));
    expect(h.alert).toBeEmptyDOMElement();
    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(h.alert).toHaveTextContent("棒読みちゃんを起動してください。");
    await act(async () => vi.advanceTimersByTimeAsync(6000));
    expect(h.alert).toBeEmptyDOMElement();
    expect(h.stores.logs.getState().notifications).toHaveLength(2);
  } finally {
    h.unmount();
    h.router.dispose();
    vi.useRealTimers();
  }
});

it("does not repeat a revoked-session failure when its auth event follows the chat notice", async () => {
  vi.useFakeTimers();
  const h = await mount("/chat");
  try {
    const message = "Twitch に再ログインしてください。";
    await act(async () =>
      tauriMock.emit("twitch://status", {
        domain: "chat",
        status: "authRequired",
        revision: 100,
        connectionGeneration: 10,
        message,
        occurredAtMs: 1,
      }),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(h.alert).toHaveTextContent(message);
    await act(async () =>
      tauriMock.emit("twitch://status", {
        domain: "auth",
        status: "authRequired",
        revision: 101,
        message,
        occurredAtMs: 2,
      }),
    );
    await act(async () => vi.advanceTimersByTimeAsync(2500));
    expect(h.alert).toBeEmptyDOMElement();
    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(h.alert).toBeEmptyDOMElement();
    expect(h.stores.logs.getState().notifications).toHaveLength(1);
  } finally {
    h.unmount();
    h.router.dispose();
    vi.useRealTimers();
  }
});
