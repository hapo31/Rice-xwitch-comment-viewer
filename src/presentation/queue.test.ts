import { describe, expect, it } from "vitest";
import { countIncompleteQueueItems } from "./queue";
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
});
