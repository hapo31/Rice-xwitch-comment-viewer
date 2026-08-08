import { describe, expect, it } from "vitest";
import {
  getPrependedMessageCount,
  getRestoredScrollOffset,
  getVisibleChatAnchor,
} from "./scrollAnchor";

describe("chat virtual scroll prepend", () => {
  it("preserves the first visible message and its relative offset after prepending", () => {
    const previous = [{ id: "three" }, { id: "two" }, { id: "one" }];
    const anchor = getVisibleChatAnchor(
      previous,
      [
        { index: 0, start: 0, size: 44 },
        { index: 1, start: 44, size: 44 },
        { index: 2, start: 88, size: 44 },
      ],
      52,
    );
    const next = [{ id: "five" }, { id: "four" }, ...previous];

    expect(anchor).toEqual({ messageId: "two", offset: 8 });
    expect(getPrependedMessageCount(previous, next)).toBe(2);
    expect(getRestoredScrollOffset(anchor!, next, (index) => index * 44)).toBe(140);
  });

  it("does not treat a replacement as prepended messages", () => {
    expect(getPrependedMessageCount([{ id: "old" }], [{ id: "new" }])).toBe(0);
  });
});
