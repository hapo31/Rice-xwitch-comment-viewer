export interface ChatScrollItem {
  id: string;
}

export interface VirtualChatRow {
  index: number;
  start: number;
  size: number;
}

export interface ChatScrollAnchor {
  messageId: string;
  offset: number;
}

export function getPrependedMessageCount(previous: ChatScrollItem[], next: ChatScrollItem[]): number {
  const previousFirstId = previous[0]?.id;

  if (!previousFirstId) {
    return 0;
  }

  const previousFirstIndex = next.findIndex((message) => message.id === previousFirstId);
  return previousFirstIndex > 0 ? previousFirstIndex : 0;
}

export function getVisibleChatAnchor(
  messages: ChatScrollItem[],
  virtualRows: VirtualChatRow[],
  scrollOffset: number,
): ChatScrollAnchor | undefined {
  const row = virtualRows.find((candidate) => candidate.start + candidate.size > scrollOffset);
  const message = row && messages[row.index];

  return message && row
    ? { messageId: message.id, offset: scrollOffset - row.start }
    : undefined;
}

export function getRestoredScrollOffset(
  anchor: ChatScrollAnchor,
  messages: ChatScrollItem[],
  getOffsetForIndex: (index: number) => number,
): number | undefined {
  const index = messages.findIndex((message) => message.id === anchor.messageId);
  return index >= 0 ? getOffsetForIndex(index) + anchor.offset : undefined;
}
