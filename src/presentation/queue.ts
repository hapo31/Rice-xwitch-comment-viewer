import type { QueueItem } from "../types";

const incompleteQueueStatuses = new Set<QueueItem["status"]>(["queued", "speaking", "error"]);
const visibleQueueStatuses = new Set<QueueItem["status"]>(["queued", "speaking", "error", "blocked"]);

export function countIncompleteQueueItems(items: QueueItem[]): number {
  return items.filter((item) => incompleteQueueStatuses.has(item.status)).length;
}

export function selectQueueItemsForDisplay(items: QueueItem[]): QueueItem[] {
  return items
    .filter((item) => visibleQueueStatuses.has(item.status))
    .sort((left, right) => queueSequence(right.id) - queueSequence(left.id));
}

function queueSequence(id: string): number {
  const sequence = id.match(/(\d+)$/)?.[1];
  return sequence ? Number(sequence) : 0;
}
