import { describe, expect, it } from "vitest";
import { getQueueStatusPresentation, queueStatusLabel, speechStatusLabel } from "./chat";

describe("queue status presentation", () => {
  it("maps every queue display state to a Japanese label and tone", () => {
    expect(getQueueStatusPresentation("queued")).toMatchObject({ label: "待機", className: "text-sky-400" });
    expect(getQueueStatusPresentation("speaking")).toMatchObject({ label: "読み上げ中", className: "text-emerald-400" });
    expect(getQueueStatusPresentation("spoken")).toMatchObject({ label: "完了", className: "text-emerald-400" });
    expect(getQueueStatusPresentation("skipped")).toMatchObject({ label: "スキップ", className: "text-zinc-500" });
    expect(getQueueStatusPresentation("blocked")).toMatchObject({ label: "抑制", className: "text-amber-400" });
    expect(getQueueStatusPresentation("error")).toMatchObject({ label: "エラー", className: "text-rose-400" });
  });

  it("maps queue states to Japanese labels", () => {
    expect(queueStatusLabel("speaking")).toBe("読み上げ中");
    expect(queueStatusLabel("skipped")).toBe("スキップ");
  });

  it("maps every speech state to a Japanese label", () => {
    expect(speechStatusLabel("idle")).toBe("待機中");
    expect(speechStatusLabel("speaking")).toBe("読み上げ中");
    expect(speechStatusLabel("paused")).toBe("一時停止中");
    expect(speechStatusLabel("disconnected")).toBe("未接続");
    expect(speechStatusLabel("error")).toBe("接続エラー");
  });
});
