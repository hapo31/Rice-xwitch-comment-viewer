import { act, render, screen, waitFor } from "@testing-library/react";
import { flushSync } from "react-dom";
import { expect, it } from "vitest";
import { createDomainStores, DomainProvider } from "../../stores/domainStores";
import type { AppSettings, LauncherAddResult, LauncherItem } from "../../types";
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

function mountLauncher(
  stores: ReturnType<typeof createDomainStores>,
  onAdd: (paths: string[]) => Promise<LauncherAddResult>,
) {
  stores.settings.dispatch({
    type: "settings.loaded",
    settings: structuredClone(defaultSettings) as AppSettings,
  });
  const view = render(
    <DomainProvider stores={stores}>
      <DomainLauncherView
        onAdd={onAdd}
        onRemove={async () => []}
        onLaunch={async () => ({ launchedCount: 0, failures: [] })}
        onLaunchAll={async () => ({ launchedCount: 0, failures: [] })}
      />
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
  mountLauncher(stores, async () => {
    callCount += 1;
    const result: LauncherAddResult =
      callCount === 1
        ? { items: [first], addedCount: 1 }
        : callCount === 2
          ? { items: [first, second], addedCount: 1 }
          : { items: [first, second], addedCount: 0 };
    flushSync(() => {
      stores.settings.dispatch({ type: "launcher.items.changed", items: result.items });
    });
    return result;
  });
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
  mountLauncher(
    stores,
    (paths) =>
      new Promise((resolve) => {
        pending.push({ resolve, paths });
      }),
  );
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
    flushSync(() => {
      stores.settings.dispatch({ type: "launcher.items.changed", items: committed.items });
    });
    pending[1].resolve(committed);
  });
  expect(await screen.findByText("1 件を登録しました。")).toBeVisible();

  act(() => {
    flushSync(() => {
      stores.settings.dispatch({ type: "launcher.items.changed", items: committed.items });
    });
    pending[0].resolve({ ...committed, addedCount: 0 });
  });
  expect(await screen.findByText("選択したアプリはすでに登録されています。")).toBeVisible();
  expect(stores.settings.getState().settings?.launcher.items).toEqual([shared]);
});
