import { describe, expect, it } from "vitest";
import {
  CHAT_GRID_TEMPLATE,
  getMinimumChatContentWidthPx,
  getMinimumChatMessageColumnWidthPx,
} from "./chatLayout";

describe("Chat の最小幅レイアウト", () => {
  it.each([
    ["100%", 1, 572, 416],
    ["125%", 1.25, 490, 295],
    ["150%", 1.5, 408, 174],
  ])("keeps the primary columns within the 900px window at %s", (_mode, scale, contentWidth, messageWidth) => {
    expect(getMinimumChatContentWidthPx(scale)).toBe(contentWidth);
    expect(getMinimumChatMessageColumnWidthPx(scale)).toBe(messageWidth);
    expect(getMinimumChatMessageColumnWidthPx(scale)).toBeGreaterThan(0);
  });

  it("uses shrinkable time and user columns instead of a fixed chat content width", () => {
    expect(CHAT_GRID_TEMPLATE).toBe("minmax(3.75rem, 5.5rem) minmax(4rem, 10rem) minmax(0, 1fr)");
  });
});
