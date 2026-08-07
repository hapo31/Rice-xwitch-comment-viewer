import { getAppShellPixelDimensions } from "../../layout/appShell";

export const MIN_APP_WINDOW_WIDTH_PX = 900;
export const CHAT_GRID_TEMPLATE = "minmax(3.75rem, 5.5rem) minmax(4rem, 10rem) minmax(0, 1fr)";

const CHAT_HORIZONTAL_PADDING_REM = 2;
const CHAT_TIME_COLUMN_MIN_REM = 3.75;
const CHAT_USER_COLUMN_MIN_REM = 4;

export function getMinimumChatContentWidthPx(scale: number, baseFontSize = 16) {
  const shell = getAppShellPixelDimensions(scale, baseFontSize);

  return MIN_APP_WINDOW_WIDTH_PX - shell.activityBarWidth - shell.sidePanelWidth;
}

export function getMinimumChatMessageColumnWidthPx(scale: number, baseFontSize = 16) {
  const reservedWidth =
    (CHAT_HORIZONTAL_PADDING_REM + CHAT_TIME_COLUMN_MIN_REM + CHAT_USER_COLUMN_MIN_REM) * baseFontSize * scale;

  return getMinimumChatContentWidthPx(scale, baseFontSize) - reservedWidth;
}
