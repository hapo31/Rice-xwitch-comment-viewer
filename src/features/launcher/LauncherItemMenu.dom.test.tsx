import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { createDomainStores, DomainProvider } from "../../stores/domainStores";
import type { LauncherItem } from "../../types";
import { LauncherView } from "./LauncherView";

function mountMenu(remove = async () => undefined, launch = vi.fn()) {
  function Harness() {
    const [items, setItems] = useState<LauncherItem[]>([
      { id: "first", displayName: "First", target: "C:\\first.exe", kind: "application", order: 0 },
      {
        id: "second",
        displayName: "Second",
        target: "C:\\second.exe",
        kind: "application",
        order: 1,
      },
    ]);
    return (
      <DomainProvider stores={createDomainStores()}>
        <LauncherView
          items={items}
          isReady
          onAdd={async () => ({ items, addedCount: 0 })}
          onRemove={async (id) => {
            await remove();
            const next = items.filter((item) => item.id !== id);
            setItems(next);
            return next;
          }}
          onLaunch={async () => {
            launch();
            return { launchedCount: 1, failures: [] };
          }}
          onLaunchAll={async () => ({ launchedCount: 0, failures: [] })}
        />
      </DomainProvider>
    );
  }
  return render(<Harness />);
}

it("opens by keyboard, loops arrows/Home/End, and restores the trigger on Escape", async () => {
  const user = userEvent.setup();
  mountMenu();
  const trigger = screen.getByRole("button", { name: "First のメニュー" });
  trigger.focus();
  await user.keyboard("{ArrowDown}");
  const item = await screen.findByRole("menuitem", { name: "削除" });
  await waitFor(() => expect(item).toHaveFocus());
  for (const key of ["{ArrowDown}", "{ArrowUp}", "{Home}", "{End}"]) {
    await user.keyboard(key);
    await waitFor(() => expect(item).toHaveFocus());
  }
  await user.keyboard("{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

it("leaves the menu on Tab and Shift+Tab in document order without trapping focus", async () => {
  const user = userEvent.setup();
  mountMenu();
  await waitFor(() => expect(screen.getByRole("button", { name: "Second を起動" })).toBeEnabled());
  const trigger = screen.getByRole("button", { name: "First のメニュー" });
  trigger.focus();
  await user.keyboard("{Enter}");
  await screen.findByRole("menuitem");
  await user.tab();
  await waitFor(() => expect(screen.getByRole("button", { name: "Second を起動" })).toHaveFocus());
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  trigger.focus();
  await user.keyboard(" ");
  await screen.findByRole("menuitem");
  await user.tab({ shift: true });
  await waitFor(() => expect(screen.getByRole("button", { name: "First を起動" })).toHaveFocus());
});

it("closes outside, keeps only one menu, and never launches from menu interaction", async () => {
  const user = userEvent.setup();
  const launch = vi.fn();
  mountMenu(undefined, launch);
  await user.click(screen.getByRole("button", { name: "First のメニュー" }));
  expect(await screen.findByRole("menu")).toHaveAccessibleName("First の操作");
  await user.click(screen.getByRole("button", { name: "Second のメニュー" }));
  await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(1));
  expect(screen.getByRole("menu")).toHaveAccessibleName("Second の操作");
  await user.click(screen.getByRole("heading", { name: "Launcher" }));
  await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  expect(launch).not.toHaveBeenCalled();
});

it("disables actions while removing, closes the menu and focuses the heading after trigger removal", async () => {
  const user = userEvent.setup();
  let release!: () => void;
  const remove = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        release = resolve;
      }),
  );
  const launch = vi.fn();
  mountMenu(remove, launch);
  const trigger = screen.getByRole("button", { name: "First のメニュー" });
  trigger.focus();
  await user.keyboard("{Enter}");
  await user.click(await screen.findByRole("menuitem", { name: "削除" }));
  expect(remove).toHaveBeenCalledOnce();
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Second のメニュー" })).toBeDisabled();
  await user.keyboard("{Enter}");
  expect(remove).toHaveBeenCalledOnce();
  await act(async () => release());
  await waitFor(() => expect(trigger).not.toBeInTheDocument());
  expect(screen.getByRole("heading", { name: "Launcher" })).toHaveFocus();
  expect(screen.getByRole("button", { name: "Second のメニュー" })).toBeEnabled();
  expect(launch).not.toHaveBeenCalled();
});
