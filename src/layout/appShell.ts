export const APP_SHELL_DIMENSIONS_REM = {
  titleBarHeight: 2,
  activityBarWidth: 3,
  activityButtonSize: 2.75,
  sidePanelWidth: 17.5,
  statusBarHeight: 1.5,
} as const;

export const APP_SHELL_CLASS_NAME =
  "relative grid h-full grid-cols-[3rem_17.5rem_minmax(0,1fr)] grid-rows-[2rem_minmax(0,1fr)_1.5rem] bg-zinc-950 text-zinc-100";

export function getAppShellPixelDimensions(scale: number, baseFontSize = 16) {
  const pixelsPerRem = baseFontSize * scale;

  return Object.fromEntries(
    Object.entries(APP_SHELL_DIMENSIONS_REM).map(([key, value]) => [key, value * pixelsPerRem]),
  ) as Record<keyof typeof APP_SHELL_DIMENSIONS_REM, number>;
}
