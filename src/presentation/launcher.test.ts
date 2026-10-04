import { describe, expect, it } from "vitest";
import type { LauncherItem } from "../types";
import {
  launcherLaunchSummary,
  launcherTileColor,
  partitionApplicationPaths,
  sortLauncherItems,
} from "./launcher";

const item = (overrides: Partial<LauncherItem>): LauncherItem => ({
  id: "item",
  kind: "application",
  target: "C:\\Apps\\Example.exe",
  displayName: "Example",
  order: 0,
  ...overrides,
});

describe("launcher presentation", () => {
  it("sorts by the reserved order and then by display name", () => {
    const items = [
      item({ id: "b", displayName: "Beta", order: 2 }),
      item({ id: "z", displayName: "Zulu", order: 1 }),
      item({ id: "a", displayName: "Alpha", order: 1 }),
    ];

    expect(sortLauncherItems(items).map(({ id }) => id)).toEqual(["a", "z", "b"]);
  });

  it("uses a valid custom tile color and rejects unsafe values", () => {
    expect(launcherTileColor(item({ backgroundColor: "#123abc" }))).toBe("#123abc");
    expect(launcherTileColor(item({ backgroundColor: "red; color: white" }))).toMatch(
      /^#[0-9a-f]{6}$/i,
    );
  });

  it("accepts Windows executable and shortcut paths case-insensitively", () => {
    expect(partitionApplicationPaths(["C:\\App.EXE", "C:\\App.lnk", "C:\\note.txt"])).toEqual({
      accepted: ["C:\\App.EXE", "C:\\App.lnk"],
      rejected: ["C:\\note.txt"],
    });
  });

  it("summarizes partial bulk launch failures", () => {
    expect(
      launcherLaunchSummary({
        launchedCount: 2,
        failures: [{ itemId: "3", displayName: "Broken", message: "見つかりません" }],
      }),
    ).toBe(
      "2 件の起動プロセスを開始し、1 件は起動できませんでした（Broken）。アプリの準備完了は未確認です。",
    );
  });

  it("does not describe process creation as application readiness", () => {
    expect(launcherLaunchSummary({ launchedCount: 1, failures: [] })).toContain("準備完了は未確認");
    expect(launcherLaunchSummary({ launchedCount: 0, failures: [] })).toBe(
      "起動するアプリがありません。",
    );
    expect(
      launcherLaunchSummary({
        launchedCount: 0,
        failures: [{ itemId: "a", displayName: "失敗", message: "原因" }],
      }),
    ).toBe("1 件のアプリを起動できませんでした（失敗）。");
  });
});
