import { describe, expect, it } from "vitest";
import { getQueueStatusPresentation, queueStatusLabel } from "./chat";

describe("queue status presentation", () => {
  it("maps queue display states to labels and tones", () => {
    expect(getQueueStatusPresentation("queued")).toMatchObject({
      label: "queued",
      className: "text-sky-400",
    });
    expect(getQueueStatusPresentation("speaking")).toMatchObject({
      label: "speaking",
      className: "text-emerald-400",
    });
    expect(getQueueStatusPresentation("spoken")).toMatchObject({
      label: "spoken",
      className: "text-emerald-400",
    });
    expect(getQueueStatusPresentation("blocked")).toMatchObject({
      label: "blocked",
      className: "text-amber-400",
    });
    expect(getQueueStatusPresentation("error")).toMatchObject({
      label: "error",
      className: "text-rose-400",
    });
  });

  it("maps queue states to Japanese labels", () => {
    expect(queueStatusLabel("speaking")).toBe("読み上げ中");
    expect(queueStatusLabel("skipped")).toBe("スキップ");
  });
});
