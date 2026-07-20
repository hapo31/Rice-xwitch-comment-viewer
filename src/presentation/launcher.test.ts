import { describe, expect, it } from "vitest";
import {
  launcherLaunchSummary,
  launcherTileColor,
  partitionApplicationPaths,
  sortLauncherItems,
} from "./launcher";
import type { LauncherItem } from "../types";

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
    expect(launcherTileColor(item({ backgroundColor: "red; color: white" }))).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("accepts Windows executable and shortcut paths case-insensitively", () => {
    expect(partitionApplicationPaths(["C:\\App.EXE", "C:\\App.lnk", "C:\\note.txt"])).toEqual({
      accepted: ["C:\\App.EXE", "C:\\App.lnk"],
      rejected: ["C:\\note.txt"],
    });
  });

  it("summarizes partial bulk launch failures", () => {
    expect(launcherLaunchSummary({
      launchedCount: 2,
      failures: [{ itemId: "3", displayName: "Broken", message: "見つかりません" }],
    })).toBe("2 件を起動し、1 件は起動できませんでした（Broken）。");
  });
});
