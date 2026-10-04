import { StrictMode } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { appRoutes } from "../routes";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { defaultSettings, tauriMock } from "./tauriMock";
import type { AppSettingsPatch } from "../types";

function mountApp(path = "/chat", strict = false) {
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
  const view = render(strict ? <StrictMode>{content}</StrictMode> : content);
  return { stores, router, ...view };
}
async function ready(stores: ReturnType<typeof createDomainStores>) {
  await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
  await waitFor(() => expect(tauriMock.listenerCount("speech://queue-updated")).toBe(1));
}
function allowSettingsSave() {
  tauriMock.setCommand("settings_update", (args?: Record<string, unknown>) => {
    const patch = args?.patch as AppSettingsPatch;
    return {
      ...defaultSettings,
      twitch: { ...defaultSettings.twitch, ...patch.twitch },
      speech: { ...defaultSettings.speech, ...patch.speech },
    };
  });
}

it("mounts every formal route via the Activity Bar and moves focus to its heading", async () => {
  const user = userEvent.setup();
  const { stores } = mountApp("/logs");
  await ready(stores);
  for (const route of appRoutes) {
    await user.click(screen.getByRole("link", { name: route.label }));
    const heading = await screen.findByRole("heading", { name: route.label, level: 1 });
    expect(screen.getByRole("link", { name: route.label })).toHaveAttribute("aria-current", "page");
    expect(heading).toHaveFocus();
    expect(document.title).toContain(route.label);
  }
});

it("Settings validates inputs, preserves Tab order, saves and cancels/discards navigation", async () => {
  allowSettingsSave();
  const user = userEvent.setup();
  const { stores } = mountApp("/settings");
  await ready(stores);
  const port = screen.getByRole("textbox", { name: "ポート" });
  expect(screen.queryByRole("button", { name: "設定を保存" })).not.toBeInTheDocument();
  port.focus();
  await user.tab();
  expect(screen.getByLabelText("速度", { exact: true })).toHaveFocus();
  await user.clear(port);
  await user.type(port, "0");
  expect(port).toHaveAttribute("aria-invalid", "true");
  expect(document.getElementById(port.getAttribute("aria-describedby")!)).toHaveTextContent(
    /ポート/,
  );
  expect(screen.getByRole("button", { name: "設定を保存" })).toBeDisabled();
  await user.clear(port);
  await user.type(port, "50002");
  await user.click(screen.getByRole("button", { name: "設定を保存" }));
  await waitFor(() => expect(stores.settings.getState().settings?.speech.bouyomiPort).toBe(50002));
  expect(tauriMock.invoke).toHaveBeenCalledWith("settings_update", {
    patch: { speech: { bouyomiPort: 50002 } },
  });
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "設定を保存" })).not.toBeInTheDocument(),
  );
  await user.clear(port);
  await user.type(port, "50003");
  await user.click(screen.getByRole("link", { name: "Logs" }));
  const dialog = await screen.findByRole("dialog", { name: "未保存の変更があります" });
  expect(within(dialog).getByRole("button", { name: "キャンセル" })).toHaveFocus();
  await user.click(within(dialog).getByRole("button", { name: "キャンセル" }));
  expect(screen.getByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
  expect(port).toHaveValue("50003");
  await user.click(screen.getByRole("link", { name: "Logs" }));
  await user.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "破棄して続ける" }),
  );
  expect(await screen.findByRole("heading", { name: "Logs", level: 1 })).toHaveFocus();
});

it("Filter uses real input/validation/save and associates field errors", async () => {
  allowSettingsSave();
  const user = userEvent.setup();
  const { stores } = mountApp("/filter");
  await ready(stores);
  const maximum = screen.getByLabelText("最大文字数", { exact: true });
  const repeat = screen.getByLabelText(/連投抑制秒/);
  maximum.focus();
  await user.tab();
  expect(repeat).toHaveFocus();
  await user.clear(maximum);
  await user.type(maximum, "0");
  expect(maximum).toHaveAttribute("aria-invalid", "true");
  expect(document.getElementById(maximum.getAttribute("aria-describedby")!)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "設定を保存" })).toBeDisabled();
  await user.clear(maximum);
  await user.type(maximum, "100");
  await user.type(screen.getByLabelText("NG ワード", { exact: true }), "testword");
  await user.click(screen.getByRole("button", { name: "設定を保存" }));
  await waitFor(() =>
    expect(stores.settings.getState().settings?.speech.maxCommentLength).toBe(100),
  );
  expect(stores.settings.getState().settings?.speech.blockedWords).toEqual(["testword"]);
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "設定を保存" })).not.toBeInTheDocument(),
  );
});

it("Filter cancellation preserves a draft and discard restores navigation without saving", async () => {
  const user = userEvent.setup();
  const { stores } = mountApp("/filter");
  await ready(stores);
  const maximum = screen.getByLabelText("最大文字数");
  await user.clear(maximum);
  await user.type(maximum, "150");
  await user.click(screen.getByRole("link", { name: "Chat" }));
  await user.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "キャンセル" }),
  );
  expect(maximum).toHaveValue("150");
  await user.click(screen.getByRole("link", { name: "Chat" }));
  await user.click(
    within(await screen.findByRole("dialog")).getByRole("button", { name: "破棄して続ける" }),
  );
  expect(await screen.findByRole("heading", { name: "Chat", level: 1 })).toHaveFocus();
  expect(stores.settings.getState().settings?.speech.maxCommentLength).toBe(120);
  expect(tauriMock.invoke.mock.calls.filter(([name]) => name === "settings_update")).toHaveLength(
    0,
  );
});

it("Windows Launcher selects applications and displays partial launch failures", async () => {
  const user = userEvent.setup();
  const items = [
    {
      id: "app-1",
      kind: "application",
      target: "C:\\valid.exe",
      displayName: "有効なアプリ",
      order: 0,
    },
    {
      id: "app-2",
      kind: "application",
      target: "C:\\missing.lnk",
      displayName: "壊れたアプリ",
      order: 1,
    },
  ];
  tauriMock.open.mockResolvedValue(["C:\\valid.exe", "C:\\missing.lnk"]);
  tauriMock.setCommand("launcher_add", items);
  tauriMock.setCommand("launcher_launch_all", {
    launchedCount: 1,
    failures: [
      {
        itemId: "app-2",
        displayName: "壊れたアプリ",
        message: "ショートカットを修正してください。",
      },
    ],
  });
  const { stores } = mountApp("/launcher");
  await ready(stores);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "アプリをランチャーに追加" })).toBeEnabled(),
  );
  await user.click(screen.getByRole("button", { name: "アプリをランチャーに追加" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "有効なアプリ を起動" })).toBeEnabled(),
  );
  expect(tauriMock.invoke).toHaveBeenCalledWith("launcher_add", {
    paths: ["C:\\valid.exe", "C:\\missing.lnk"],
  });
  await user.click(screen.getByRole("button", { name: "一斉に起動" }));
  expect(await screen.findByText(/1 件を起動し、1 件は起動できませんでした/)).toHaveTextContent(
    "壊れたアプリ",
  );
});

it("Queue restores a paused snapshot with item-specific accessible controls", async () => {
  tauriMock.setCommand("speech_queue_reload", {
    revision: 3,
    status: { revision: 2, status: "paused", adapterHealth: "connected", occurredAtMs: 1 },
    queue: {
      revision: 3,
      phase: "paused",
      queuedCount: 1,
      occurredAtMs: 1,
      items: [
        {
          id: "q-1",
          sourceMessageId: "chat-1",
          text: "待機テスト",
          userDisplayName: "Viewer",
          status: "queued",
        },
      ],
    },
  });
  const { stores } = mountApp("/queue");
  await ready(stores);
  await waitFor(() => expect(stores.queue.getState().phase).toBe("paused"));
  const table = screen.getByRole("table", { name: "読み上げキュー" });
  expect(within(table).getByText("待機テスト")).toBeInTheDocument();
  expect(
    within(table)
      .getAllByRole("button")
      .every((button) => button.getAttribute("aria-label")),
  ).toBe(true);
  expect(screen.getByRole("button", { name: "キューを再読込" })).toBeEnabled();
});

it("command rejection is handled and remains visible in Logs", async () => {
  tauriMock.rejectCommand(
    "speech_health_check",
    "接続テスト失敗: 棒読みちゃんを起動してください。",
  );
  const user = userEvent.setup();
  const { stores } = mountApp("/settings");
  await ready(stores);
  await user.click(screen.getByRole("button", { name: "接続確認" }));
  await waitFor(() =>
    expect(
      stores.logs.getState().logs.some((entry) => entry.message.includes("接続テスト失敗")),
    ).toBe(true),
  );
  await user.click(screen.getByRole("link", { name: "Logs" }));
  expect(
    within(screen.getByRole("table", { name: "アプリログ" })).getByText(/接続テスト失敗/),
  ).toBeInTheDocument();
});

it("StrictMode keeps one subscription and one update per event after delayed registration", async () => {
  tauriMock.delaySubscriptions();
  const { stores, unmount } = mountApp("/chat", true);
  await waitFor(() => expect(tauriMock.pendingCount).toBeGreaterThan(0));
  await act(async () => tauriMock.releaseSubscriptions());
  await ready(stores);
  expect(tauriMock.listenerCount("twitch://chat-message")).toBe(1);
  const changed = vi.fn();
  const unsubscribe = stores.chat.subscribe(changed);
  await act(async () =>
    tauriMock.emit("twitch://chat-message", {
      id: "dom-event-1",
      platform: "twitch",
      channelId: "channel",
      channelLogin: "streamer",
      userId: "viewer",
      userLogin: "viewer",
      userDisplayName: "Viewer",
      text: "DOM event",
      fragments: [],
      badges: [],
      receivedAt: "2026-10-05T00:00:00Z",
    }),
  );
  expect(
    stores.chat.getState().messages.filter((message) => message.id === "dom-event-1"),
  ).toHaveLength(1);
  expect(changed).toHaveBeenCalledOnce();
  expect(
    within(screen.getByRole("table", { name: "チャット一覧" })).getByText("DOM event"),
  ).toBeInTheDocument();
  unsubscribe();
  unmount();
  await waitFor(() => expect(tauriMock.listenerCount()).toBe(0));
});

it("subscription rejection reports recovery and unmount removes successful registrations", async () => {
  tauriMock.rejectNextSubscription("speech://queue-updated");
  const { stores, unmount } = mountApp();
  await waitFor(() =>
    expect(stores.logs.getState().notifications.some((entry) => entry.severity === "warning")).toBe(
      true,
    ),
  );
  expect(tauriMock.listenerCount("twitch://chat-message")).toBe(1);
  expect(tauriMock.listenerCount("speech://status")).toBe(1);
  unmount();
  await waitFor(() => expect(tauriMock.listenerCount()).toBe(0));
});

it("unmount before delayed subscription completion leaves no native listener", async () => {
  tauriMock.delaySubscriptions();
  const { unmount } = mountApp();
  await waitFor(() => expect(tauriMock.pendingCount).toBeGreaterThan(0));
  unmount();
  await act(async () => tauriMock.releaseSubscriptions());
  await waitFor(() => expect(tauriMock.listenerCount()).toBe(0));
});

it("non-Windows Launcher disables registration/launch and never registers DnD", async () => {
  tauriMock.setCommand("app_build_info", {
    version: "0.2.3",
    isDev: true,
    launcher: {
      canRegisterApplications: false,
      canLaunchApplications: false,
      reason: "Windows版でのみ利用できます。OS標準のランチャーを利用してください。",
    },
  });
  const { stores } = mountApp("/launcher");
  await ready(stores);
  expect(await screen.findByRole("note")).toHaveTextContent(/Windows版/);
  expect(screen.getByRole("button", { name: "アプリをランチャーに追加" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "一斉に起動" })).toBeDisabled();
  expect(tauriMock.listenerCount("tauri://drag-drop")).toBe(0);
  expect(tauriMock.open).not.toHaveBeenCalled();
});
