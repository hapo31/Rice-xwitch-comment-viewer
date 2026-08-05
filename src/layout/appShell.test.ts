import { describe, expect, it } from "vitest";
import {
  APP_SHELL_CLASS_NAME,
  APP_SHELL_DIMENSIONS_REM,
  getAppShellPixelDimensions,
} from "./appShell";

describe("app shell display scaling", () => {
  it.each([
    ["100%", 1],
    ["125%", 1.25],
    ["150%", 1.5],
    ["auto", 1.2],
  ])("keeps shell tracks aligned in %s mode", (_mode, scale) => {
    const dimensions = getAppShellPixelDimensions(scale);

    expect(dimensions.activityBarWidth).toBe(48 * scale);
    expect(dimensions.sidePanelWidth).toBe(280 * scale);
    expect(dimensions.statusBarHeight).toBe(24 * scale);
    expect(dimensions.titleBarHeight).toBe(32 * scale);
    expect(dimensions.activityButtonSize).toBeLessThanOrEqual(dimensions.activityBarWidth);
  });

  it("keeps every shell track in rem so auto scaling uses the same dimensions", () => {
    expect(APP_SHELL_DIMENSIONS_REM).toEqual({
      titleBarHeight: 2,
      activityBarWidth: 3,
      activityButtonSize: 2.75,
      sidePanelWidth: 17.5,
      statusBarHeight: 1.5,
    });
    expect(APP_SHELL_CLASS_NAME).toContain("grid-cols-[3rem_17.5rem_minmax(0,1fr)]");
    expect(APP_SHELL_CLASS_NAME).toContain("grid-rows-[2rem_minmax(0,1fr)_1.5rem]");
  });
});
