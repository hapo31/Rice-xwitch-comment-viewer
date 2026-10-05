import { act, render, screen, waitFor } from "@testing-library/react";
import { flushSync } from "react-dom";
import { expect, it } from "vitest";
import { createDomainStores, DomainProvider } from "../../stores/domainStores";
import type {
  AppSettings,
  AppSettingsPatch,
  BouyomiConnectionDiagnostics,
  LauncherAddResult,
  LauncherItem,
  LauncherLaunchResult,
} from "../../types";
import {
  DomainControllerActionsProvider,
  type DomainControllerActions,
} from "../../orchestration/domainControllerContext";
import { createLauncherController } from "../../orchestration/domainCommandControllers";
import { defaultSettings, tauriMock } from "../../testing/tauriMock";
import { DomainLauncherView } from "../domainViews";

function launcherItem(id: string, target: string, order: number): LauncherItem {
  return {
    id,
    kind: "application",
    target,
    displayName: id,
    order,
  };
}

function mountLauncher(stores: ReturnType<typeof createDomainStores>) {
  stores.settings.dispatch({
    type: "settings.loaded",
    settings: structuredClone(defaultSettings) as AppSettings,
  });
  const launcher = createLauncherController({
    reportError: () => undefined,
    reportInfo: () => undefined,
    dispatchLauncherItems: (items) => {
      flushSync(() => stores.settings.dispatch({ type: "launcher.items.changed", items }));
    },
  });
  const actions: DomainControllerActions = {
    updateSettings: async (_patch: AppSettingsPatch) => true,
    speechHealthCheck: () => undefined,
    speechDiagnostics: async (): Promise<BouyomiConnectionDiagnostics> => ({
      configuredAddr: "127.0.0.1:50001",
      attempted: [],
      recommendation: "",
    }),
    speechTest: () => undefined,
    speechControl: () => undefined,
    queueReload: () => undefined,
    queueRemove: () => undefined,
    queueDismiss: () => undefined,
    queueDismissHistory: () => undefined,
    queueRetry: () => undefined,
    launcherAdd: launcher.add,
    launcherRemove: async () => [],
    launcherLaunch: async (): Promise<LauncherLaunchResult> => ({ launchedCount: 0, failures: [] }),
    launcherLaunchAll: async (): Promise<LauncherLaunchResult> => ({
      launchedCount: 0,
      failures: [],
    }),
    twitchStartAuth: () => undefined,
    twitchPollAuth: () => undefined,
    twitchValidateAuth: async () => true,
    twitchDisconnect: () => undefined,
    twitchConnect: () => undefined,
    twitchStopChat: () => undefined,
    openExternalUrl: () => undefined,
    clearWarnings: () => undefined,
  };
  const view = render(
    <DomainProvider stores={stores}>
      <DomainControllerActionsProvider actions={actions}>
        <DomainLauncherView />
      </DomainControllerActionsProvider>
    </DomainProvider>,
  );
  return view;
}

async function waitForDragDropListener() {
  await waitFor(() => expect(tauriMock.listenerCount("tauri://drag-drop")).toBe(1));
}

function drop(paths: string[]) {
  act(() => tauriMock.emit("tauri://drag-drop", { type: "drop", paths }));
}

it("reports new, mixed and duplicate outcomes after the real settings store renders first", async () => {
  const stores = createDomainStores();
  const first = launcherItem("first", "C:\\first.exe", 0);
  const second = launcherItem("second", "C:\\second.exe", 1);
  let callCount = 0;
  tauriMock.setCommand("launcher_add", () => {
    callCount += 1;
    const result: LauncherAddResult =
      callCount === 1
        ? { items: [first], addedCount: 1 }
        : callCount === 2
          ? { items: [first, second], addedCount: 1 }
          : { items: [first, second], addedCount: 0 };
    return result;
  });
  mountLauncher(stores);
  await waitForDragDropListener();

  drop(["C:\\first.exe"]);
  expect(await screen.findByText("1 件を登録しました。")).toBeVisible();
  expect(stores.settings.getState().settings?.launcher.items).toHaveLength(1);

  drop(["C:\\first.exe", "C:\\second.exe"]);
  expect(await screen.findByText("1 件を登録しました。 1 件は登録済みです。")).toBeVisible();
  expect(stores.settings.getState().settings?.launcher.items).toHaveLength(2);

  drop(["C:\\first.exe"]);
  expect(await screen.findByText("選択したアプリはすでに登録されています。")).toBeVisible();
  expect(stores.settings.getState().settings?.launcher.items).toHaveLength(2);
});

it("keeps concurrent add notices tied to each operation result after shared state has updated", async () => {
  const stores = createDomainStores();
  const shared = launcherItem("shared", "C:\\shared.exe", 0);
  const pending: Array<{
    resolve: (result: LauncherAddResult) => void;
    paths: string[];
  }> = [];
  tauriMock.setCommand("launcher_add", (args?: Record<string, unknown>) => {
    const rawPaths = args?.paths;
    const paths = Array.isArray(rawPaths)
      ? rawPaths.filter((path: unknown): path is string => typeof path === "string")
      : [];
    return new Promise<LauncherAddResult>((resolve) => pending.push({ resolve, paths }));
  });
  mountLauncher(stores);
  await waitForDragDropListener();

  drop(["C:\\shared.exe"]);
  drop(["C:\\shared.exe"]);
  await waitFor(() => expect(pending).toHaveLength(2));
  expect(pending.map((operation) => operation.paths)).toEqual([
    ["C:\\shared.exe"],
    ["C:\\shared.exe"],
  ]);

  const committed = { items: [shared], addedCount: 1 };
  act(() => {
    pending[1].resolve(committed);
  });
  expect(await screen.findByText("1 件を登録しました。")).toBeVisible();

  act(() => {
    pending[0].resolve({ ...committed, addedCount: 0 });
  });
  expect(await screen.findByText("選択したアプリはすでに登録されています。")).toBeVisible();
  expect(stores.settings.getState().settings?.launcher.items).toEqual([shared]);
});
