import type { ChatMessage } from "../types";

const MAX_REMEMBERED_MESSAGE_IDS = 400;

export class ChatLiveAnnouncementController {
  private readonly announcedMessageIds = new Set<string>();
  private pendingCount = 0;

  initialize(messages: readonly ChatMessage[]) {
    this.remember(messages);
  }

  queueNewMessages(messages: readonly ChatMessage[]): boolean {
    const previousCount = this.pendingCount;
    this.remember(messages, true);
    return this.pendingCount > previousCount;
  }

  suppress(messages: readonly ChatMessage[]) {
    this.remember(messages);
    this.pendingCount = 0;
  }

  takeSummary(): string | undefined {
    if (this.pendingCount === 0) {
      return undefined;
    }

    const count = this.pendingCount;
    this.pendingCount = 0;
    return `新しいチャットが${count}件届きました。`;
  }

  private remember(messages: readonly ChatMessage[], queueNew = false) {
    for (const message of messages) {
      if (message.platform !== "twitch" || this.announcedMessageIds.has(message.id)) {
        continue;
      }

      this.announcedMessageIds.add(message.id);
      if (queueNew) {
        this.pendingCount += 1;
      }
    }

    while (this.announcedMessageIds.size > MAX_REMEMBERED_MESSAGE_IDS) {
      const oldestId = this.announcedMessageIds.values().next().value;
      if (!oldestId) {
        return;
      }
      this.announcedMessageIds.delete(oldestId);
    }
  }
}
