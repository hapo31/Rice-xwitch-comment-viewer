import { describe, expect, it } from "vitest";
import { launcherMenuItemIndex, launcherMenuKeyAction } from "./LauncherView";

describe("Launcher menu keyboard navigation", () => {
  it("maps arrow keys and Home/End to predictable menu item focus", () => {
    expect(launcherMenuItemIndex("ArrowDown", 0, 3)).toBe(1);
    expect(launcherMenuItemIndex("ArrowUp", 0, 3)).toBe(2);
    expect(launcherMenuItemIndex("Home", 2, 3)).toBe(0);
    expect(launcherMenuItemIndex("End", 0, 3)).toBe(2);
    expect(launcherMenuItemIndex("ArrowDown", 0, 1)).toBe(0);
  });

  it("closes on Escape with trigger restoration and closes on Tab without trapping focus", () => {
    expect(launcherMenuKeyAction("Escape")).toBe("close-and-focus-trigger");
    expect(launcherMenuKeyAction("Tab")).toBe("close");
    expect(launcherMenuKeyAction("ArrowDown")).toBe("move-focus");
  });
});
