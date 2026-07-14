import { describe, expect, it } from "vitest";
import { countIncompleteQueueItems, selectQueueItemsForDisplay } from "./queue";
import type { QueueItem } from "../types";

function queueItem(id: string, status: QueueItem["status"]): QueueItem {
  return {
    id,
    userDisplayName: "viewer",
    text: `message ${id}`,
    status,
  };
}

describe("queue presentation", () => {
  it("counts only queue items that have not completed reading", () => {
    const items: QueueItem[] = [
      queueItem("queued", "queued"),
      queueItem("speaking", "speaking"),
      queueItem("error", "error"),
      queueItem("spoken", "spoken"),
      queueItem("skipped", "skipped"),
      queueItem("blocked", "blocked"),
    ];

    expect(countIncompleteQueueItems(items)).toBe(3);
  });

  it("shows only pending, error, and filter-blocked items with newest first", () => {
    const items: QueueItem[] = [
      queueItem("speech-2", "queued"),
      queueItem("speech-5", "spoken"),
      queueItem("speech-4", "error"),
      queueItem("speech-1", "skipped"),
      queueItem("speech-3", "blocked"),
      queueItem("speech-6", "speaking"),
    ];

    expect(selectQueueItemsForDisplay(items).map((item) => item.id)).toEqual([
      "speech-6",
      "speech-4",
      "speech-3",
      "speech-2",
    ]);
  });
});
