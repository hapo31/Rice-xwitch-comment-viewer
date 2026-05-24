import type { QueueItem } from "../types";

const incompleteQueueStatuses = new Set<QueueItem["status"]>(["queued", "speaking", "error"]);

export function countIncompleteQueueItems(items: QueueItem[]): number {
  return items.filter((item) => incompleteQueueStatuses.has(item.status)).length;
}
