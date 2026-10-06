import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { createHashRouter, createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it, vi } from "vitest";
import { AppShell } from "../AppShell";
import { appRoutes } from "../routes";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import outcomeFixture from "../tauri/fixtures/queue-outcomes.json";
import type { AppEventsSnapshot, AppSettingsPatch } from "../types";
import { defaultSettings, tauriMock } from "./tauriMock";

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

it("native settings-schema completion survives HashRouter without an unknown-route redirect", async () => {
  const originalUrl = window.location.href;
  window.location.hash = "/chat";
  const stores = createDomainStores();
  const router = createHashRouter([
    {
      path: "*",
      element: (
        <DomainProvider stores={stores}>
          <AppShell />
        </DomainProvider>
      ),
    },
  ]);
  const view = render(<RouterProvider router={router} />);
  try {
    await ready(stores);
    await act(async () => {
      window.location.hash = "rice-schema-ok";
    });
    await waitFor(() => expect(window.location.hash).toBe("#/chat"));
    for (const result of ["ok", "failed&stage=update"]) {
      const search = `?riceSchemaResult=${result}`;
      await act(async () => {
        window.location.hash = `/chat${search}`;
      });
      await waitFor(() => expect(router.state.location.search).toBe(search));
      expect(router.state.location.pathname).toBe("/chat");
      expect(window.location.hash).toBe(`#/chat${search}`);
    }
  } finally {
    view.unmount();
    router.dispose();
    window.history.replaceState(null, "", originalUrl);
  }
});

it("late startup and explicit reload preserve all item reasons and expose keyboard-operated skip history", async () => {
  let revision = 10;
  const items = outcomeFixture.map((outcome, index) => ({
    id: `speech-${index + 1}`,
    sourceMessageId: `chat-${index + 1}`,
    userDisplayName: "viewer",
    text: "bounded text",
    status: outcome.kind,
    outcome,
  }));
  tauriMock.setCommand("speech_queue_reload", () => ({
    revision,
    status: { revision, status: "idle", adapterHealth: "connected", occurredAtMs: 1 },
    queue: { revision: revision++, phase: "error", items, queuedCount: 0, occurredAtMs: 1 },
  }));
  const user = userEvent.setup();
  const { stores } = mountApp("/queue");
  await ready(stores);
  const table = screen.getByRole("table", { name: "読み上げキュー" });
  await waitFor(() => expect(within(table).getByText("blockedWord")).toBeInTheDocument());
  expect(within(table).queryByText("overflow")).not.toBeInTheDocument();
  const toggle = screen.getByRole("checkbox", { name: "スキップ履歴を表示" });
  toggle.focus();
  await user.keyboard("[Space]");
  expect(toggle).toBeChecked();
  for (const outcome of outcomeFixture) {
    expect(within(table).getByText(outcome.reasonCode)).toBeInTheDocument();
    expect(within(table).getByText(outcome.message)).toBeInTheDocument();
  }
  expect(within(table).getAllByRole("link", { name: "Filterを開く" })).toHaveLength(6);
  expect(within(table).getAllByRole("link", { name: "Settingsの診断を開く" })).toHaveLength(12);
  await user.click(screen.getByRole("button", { name: "キューを再読込" }));
  await waitFor(() => expect(stores.queue.getState().revision).toBe(11));
  expect(within(table).getByText("overflow")).toBeInTheDocument();
  expect(stores.queue.getState().items[0].outcome).toEqual(outcomeFixture[0]);
  const skippedRow = within(table).getByText("userSkip").closest('[role="row"]')!;
  expect(
    within(skippedRow as HTMLElement).getByRole("button", { name: /履歴から削除/ }),
  ).toBeEnabled();
  tauriMock.setCommand("speech_queue_dismiss", () => null);
  await user.click(within(skippedRow as HTMLElement).getByRole("button", { name: /履歴から削除/ }));
  await waitFor(() =>
    expect(tauriMock.invoke).toHaveBeenCalledWith("speech_queue_dismiss", { itemId: "speech-7" }),
  );
});

it("Chat exposes outcome and recovery in a stable-row detail pane, updates it and restores keyboard focus", async () => {
  const original = outcomeFixture.find((outcome) => outcome.reasonCode === "writeTimeout")!;
  const replacement = outcomeFixture.find((outcome) => outcome.reasonCode === "configuration")!;
  const item = {
    id: "speech-1",
    sourceMessageId: "detail-chat",
    userDisplayName: "Viewer",
    text: "bounded text",
    status: "error",
    outcome: original,
  };
  tauriMock.setCommand("speech_queue_reload", () => ({
    revision: 10,
    status: { revision: 10, status: "idle", adapterHealth: "connected", occurredAtMs: 1 },
    queue: { revision: 10, phase: "error", items: [item], queuedCount: 0, occurredAtMs: 1 },
  }));
  const user = userEvent.setup();
  const { stores } = mountApp("/chat");
  await ready(stores);
  await waitFor(() => expect(stores.queue.getState().items).toHaveLength(1));
  await act(async () =>
    tauriMock.emit("twitch://chat-message", {
      id: "detail-chat",
      platform: "twitch",
      channelId: "channel",
      channelLogin: "streamer",
      userId: "viewer",
      userLogin: "viewer",
      userDisplayName: "Viewer",
      text: "bounded text",
      fragments: [],
      badges: [],
      receivedAt: "2026-10-05T00:00:00Z",
    }),
  );
  const trigger = await screen.findByRole("button", {
    name: "Viewerの読み上げ結果の詳細、detail-chat",
  });
  trigger.focus();
  await user.keyboard("{Enter}");
  const details = screen.getByRole("complementary", { name: "読み上げ結果の詳細" });
  expect(details).toHaveFocus();
  expect(details).toHaveTextContent(original.message);
  expect(details).toHaveTextContent("speech-1");
  expect(within(details).getByRole("link", { name: "Settingsの診断を開く" })).toHaveAttribute(
    "href",
    "/settings",
  );
  expect(within(details).getByRole("link", { name: "Queueで再試行・履歴を確認" })).toHaveAttribute(
    "href",
    "/queue",
  );
  await act(async () =>
    tauriMock.emit("speech://queue-updated", {
      revision: 11,
      phase: "error",
      items: [{ ...item, outcome: replacement }],
      queuedCount: 0,
      occurredAtMs: 2,
    }),
  );
  expect(details).toHaveTextContent(replacement.message);
  expect(details).not.toHaveTextContent(original.message);
  await user.keyboard("{Escape}");
  expect(
    screen.queryByRole("complementary", { name: "読み上げ結果の詳細" }),
  ).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
  expect(screen.getAllByText("bounded text")).toHaveLength(1);
  await user.keyboard("{Enter}");
  expect(screen.getByRole("complementary", { name: "読み上げ結果の詳細" })).toHaveFocus();
  const { outcome: _oldOutcome, ...retryItem } = item;
  await act(async () =>
    tauriMock.emit("speech://queue-updated", {
      revision: 12,
      phase: "idle",
      items: [{ ...retryItem, status: "queued" }],
      queuedCount: 1,
      occurredAtMs: 3,
    }),
  );
  expect(
    screen.queryByRole("complementary", { name: "読み上げ結果の詳細" }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Chat", level: 1 })).toHaveFocus();
});
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

it("remote mode must be saved then explicitly sent to native consent, never auto-connects", async () => {
  allowSettingsSave();
  tauriMock.setCommand("speech_authorize_endpoint", () =>
    Promise.reject({
      field: "speech.bouyomiHost",
      code: "consentDeclined",
      message: "外部接続を許可しませんでした。送信していません。",
      recovery: "接続先を確認してください。",
    }),
  );
  const user = userEvent.setup();
  const { stores } = mountApp("/settings");
  await ready(stores);
  const mode = screen.getByRole("checkbox", { name: /外部接続モード/ });
  expect(mode).not.toBeChecked();
  await user.click(mode);
  expect(screen.getByText(/Twitchユーザー名・チャット・テスト文/)).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "保存済みの接続先をネイティブ確認で許可" }),
  ).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "設定を保存" }));
  await waitFor(() =>
    expect(stores.settings.getState().settings?.speech.bouyomiRemoteMode).toBe(true),
  );
  const authorize = screen.getByRole("button", { name: "保存済みの接続先をネイティブ確認で許可" });
  expect(authorize).toBeEnabled();
  expect(tauriMock.invoke).not.toHaveBeenCalledWith("speech_authorize_endpoint");
  const probesBeforeConsent = tauriMock.invoke.mock.calls.filter(
    ([command]) => command === "speech_health_probe",
  ).length;
  await user.click(authorize);
  await waitFor(() => expect(screen.getByText(/外部接続を許可しませんでした/)).toBeInTheDocument());
  expect(tauriMock.invoke).toHaveBeenCalledWith("speech_authorize_endpoint");
  expect(
    tauriMock.invoke.mock.calls.filter(([command]) => command === "speech_health_probe"),
  ).toHaveLength(probesBeforeConsent);
});

it("oversized confirmation and NG rules expose field errors and block saving", async () => {
  const { stores, router } = mountApp("/settings");
  await ready(stores);
  const confirmation = screen.getByLabelText("接続成功時メッセージ");
  fireEvent.change(confirmation, { target: { value: "😀".repeat(121) } });
  expect(confirmation).toHaveAttribute("aria-invalid", "true");
  expect(document.getElementById(confirmation.getAttribute("aria-describedby")!)).toHaveTextContent(
    /120文字/,
  );
  expect(screen.getByRole("button", { name: "設定を保存" })).toBeDisabled();
  fireEvent.change(confirmation, { target: { value: "" } });
  await act(() => router.navigate("/filter"));
  fireEvent.change(screen.getByLabelText("NG ワード", { exact: true }), {
    target: { value: "x".repeat(501) },
  });
  expect(
    within(screen.getByRole("region", { name: "除外リスト" })).getByRole("alert"),
  ).toHaveTextContent(/500文字/);
  expect(screen.getByRole("button", { name: "設定を保存" })).toBeDisabled();
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
  tauriMock.setCommand("launcher_add", { items, addedCount: 2 });
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
  expect(await screen.findByText("2 件を登録しました。")).toBeVisible();
  tauriMock.setCommand("launcher_add", { items, addedCount: 1 });
  await user.click(screen.getByRole("button", { name: "アプリをランチャーに追加" }));
  expect(await screen.findByText("1 件を登録しました。 1 件は登録済みです。")).toBeVisible();
  tauriMock.setCommand("launcher_add", { items, addedCount: 0 });
  await user.click(screen.getByRole("button", { name: "アプリをランチャーに追加" }));
  expect(await screen.findByText("選択したアプリはすでに登録されています。")).toBeVisible();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "有効なアプリ を起動" })).toBeEnabled(),
  );
  expect(tauriMock.invoke).toHaveBeenCalledWith("launcher_add", {
    paths: ["C:\\valid.exe", "C:\\missing.lnk"],
  });
  await user.click(screen.getByRole("button", { name: "一斉に起動" }));
  expect(
    await screen.findByText(/1 件の起動プロセスを開始し、1 件は起動できませんでした/),
  ).toHaveTextContent("壊れたアプリ");
  const failures = screen.getByRole("region", { name: "起動できなかったアプリ" });
  expect(failures).toHaveTextContent("壊れたアプリ");
  expect(failures).toHaveTextContent("ショートカットを修正してください。");
  expect(failures).toHaveTextContent("正しいアプリを再登録してください");
  expect(screen.getByText(/アプリの準備完了は未確認です/)).toBeVisible();
  tauriMock.setCommand("launcher_launch", {
    launchedCount: 0,
    failures: [
      { itemId: "app-2", displayName: "壊れたアプリ", message: "リンク先が見つかりません。" },
    ],
  });
  await user.click(screen.getByRole("button", { name: "壊れたアプリ を起動" }));
  await waitFor(() =>
    expect(screen.getByRole("region", { name: "起動できなかったアプリ" })).toHaveTextContent(
      "リンク先が見つかりません。",
    ),
  );
  expect(screen.getByRole("region", { name: "起動できなかったアプリ" })).toHaveTextContent(
    "ショートカットのプロパティ",
  );
  tauriMock.setCommand("launcher_launch", { launchedCount: 1, failures: [] });
  await user.click(screen.getByRole("button", { name: "有効なアプリ を起動" }));
  expect(
    await screen.findByText(
      "有効なアプリ の起動プロセスを開始しました。アプリの準備完了は未確認です。",
    ),
  ).toBeVisible();
  expect(screen.queryByRole("region", { name: "起動できなかったアプリ" })).not.toBeInTheDocument();
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

it("keeps paused queue state while periodic probes detect disconnect and recovery", async () => {
  const intervals = vi.spyOn(globalThis, "setInterval");
  tauriMock.setCommand("speech_queue_reload", {
    revision: 3,
    status: { revision: 2, status: "paused", adapterHealth: "connected", occurredAtMs: 1 },
    queue: { revision: 3, phase: "paused", items: [], queuedCount: 0, occurredAtMs: 1 },
  });
  tauriMock.setCommand("speech_health_probe", () => {
    tauriMock.emit("speech://status", {
      revision: 10,
      status: "disconnected",
      adapterHealth: "disconnected",
      occurredAtMs: 1,
      message: "棒読みちゃんが未接続です。",
    });
    throw new Error("native failure");
  });
  const { stores } = mountApp("/queue");
  await ready(stores);
  await waitFor(() =>
    expect(stores.connection.getState().speechAdapterHealth).toBe("disconnected"),
  );
  expect(stores.queue.getState().phase).toBe("paused");
  const footer = screen.getByRole("contentinfo");
  expect(footer).toHaveTextContent("棒読みちゃん: 未接続");
  expect(footer).toHaveTextContent("キュー: 一時停止中");
  tauriMock.setCommand("speech_health_probe", () => {
    tauriMock.emit("speech://status", {
      revision: 11,
      status: "idle",
      adapterHealth: "connected",
      occurredAtMs: 2,
      message: "棒読みちゃんの接続を確認しました。",
    });
    return "接続が復旧しました。";
  });
  const poll = intervals.mock.calls.find(([, delay]) => delay === 5000)?.[0];
  expect(typeof poll).toBe("function");
  await act(async () => {
    if (typeof poll === "function") poll();
  });
  await waitFor(() => expect(stores.connection.getState().speechAdapterHealth).toBe("connected"));
  expect(stores.queue.getState().phase).toBe("paused");
  expect(footer).toHaveTextContent("棒読みちゃん: 接続確認済み");
  expect(footer).toHaveTextContent("キュー: 一時停止中");
});

it.each([
  ["speech_health_check", "接続確認", "disconnected"],
  ["speech_health_check", "接続確認", "error"],
  ["speech_test", "テスト読み上げ", "disconnected"],
  ["speech_test", "テスト読み上げ", "error"],
  ["speech_pause", "一時停止", "disconnected"],
  ["speech_pause", "一時停止", "error"],
] as const)(
  "native %s (%s) failure retains classified %s state",
  async (command, button, status) => {
    tauriMock.setCommand(command, () => {
      tauriMock.emit("speech://status", {
        revision: 10,
        status,
        adapterHealth: status,
        occurredAtMs: 1,
        message: "型付きの原因と復旧案内",
      });
      throw new Error("型付きの原因と復旧案内");
    });
    const user = userEvent.setup();
    const { stores } = mountApp("/settings");
    await ready(stores);
    await user.click(screen.getByRole("button", { name: button }));
    await waitFor(() =>
      expect(stores.logs.getState().notifications.some((entry) => entry.severity === "error")).toBe(
        true,
      ),
    );
    expect(stores.connection.getState()).toMatchObject({
      speechStatus: status,
      speechAdapterHealth: status,
      speechRevision: 10,
    });
  },
);

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

it("keeps a current Twitch comment delivered before the startup snapshot", async () => {
  let resolveSnapshot!: (value: AppEventsSnapshot) => void;
  tauriMock.setCommand(
    "app_events_snapshot",
    () =>
      new Promise((resolve) => {
        resolveSnapshot = resolve;
      }),
  );
  const { stores } = mountApp("/chat");
  await ready(stores);
  await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("app_events_snapshot"));
  const message = {
    id: "startup-chat-1",
    platform: "twitch",
    channelId: "channel-7",
    channelLogin: "channel_7",
    userId: "viewer-1",
    userLogin: "viewer",
    userDisplayName: "Viewer",
    text: "snapshot 前に届いたコメント",
    fragments: [],
    badges: [],
    receivedAt: "2026-10-06T00:00:00Z",
    connectionGeneration: 7,
  };
  await act(async () => {
    tauriMock.emit("twitch://chat-message", message);
    tauriMock.emit("twitch://chat-message", message);
  });
  expect(stores.chat.getState().messages.filter((entry) => entry.kind === "user")).toEqual([]);

  await act(async () => {
    resolveSnapshot({
      revision: 10,
      logs: [],
      emitErrors: [],
      twitchStatuses: [
        {
          revision: 10,
          domain: "chat",
          status: "connected",
          occurredAtMs: 1,
          connectionGeneration: 7,
          activeConnection: {
            generation: 7,
            broadcasterUserId: "channel-7",
            broadcasterLogin: "channel_7",
          },
        },
      ],
    });
  });
  await waitFor(() =>
    expect(
      within(screen.getByRole("table", { name: "チャット一覧" })).getByText(
        "snapshot 前に届いたコメント",
      ),
    ).toBeInTheDocument(),
  );
  expect(
    stores.chat.getState().messages.filter((entry) => entry.id === "startup-chat-1"),
  ).toHaveLength(1);
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
