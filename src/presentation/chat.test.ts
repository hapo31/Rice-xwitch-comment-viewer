import { describe, expect, it } from "vitest";
import { getChatStatusPresentation, queueStatusLabel } from "./chat";

describe("chat status presentation", () => {
  it("maps chat display states to labels and tones", () => {
    expect(getChatStatusPresentation("queued")).toMatchObject({
      label: "queued",
      className: "text-sky-400",
    });
    expect(getChatStatusPresentation("spoken")).toMatchObject({
      label: "spoken",
      className: "text-emerald-400",
    });
    expect(getChatStatusPresentation("blocked")).toMatchObject({
      label: "blocked",
      className: "text-amber-400",
    });
    expect(getChatStatusPresentation("error")).toMatchObject({
      label: "error",
      className: "text-rose-400",
    });
  });

  it("maps queue states to Japanese labels", () => {
    expect(queueStatusLabel("speaking")).toBe("読み上げ中");
    expect(queueStatusLabel("skipped")).toBe("スキップ");
  });
});
