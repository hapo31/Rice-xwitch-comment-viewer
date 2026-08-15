import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ChatRow, ChatView } from "./ChatView";
import { initialAppState } from "../../stores/appStore";
import type { ChatDisplayState, UserChatMessage } from "../../types";
import { utcTimestamp } from "../../time";

const virtualizerState = vi.hoisted(() => ({ indexes: [0], totalSize: 44 }));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: () => ({
    getTotalSize: () => virtualizerState.totalSize,
    getVirtualItems: () => virtualizerState.indexes.map((index) => ({
      index,
      key: `virtual-chat-${index}`,
      start: index * 44,
    })),
    measureElement: () => undefined,
    scrollToIndex: () => undefined,
    scrollToOffset: () => undefined,
  }),
}));

function chatMessage(status: ChatDisplayState, id: string = status): UserChatMessage {
  return {
    kind: "user",
    id,
    receivedAt: utcTimestamp("2026-08-12T00:00:00Z"),
    userDisplayName: "viewer",
    text: "こんにちは",
    status,
  };
}

describe("ChatRow read status", () => {
  beforeEach(() => {
    virtualizerState.indexes = [0];
    virtualizerState.totalSize = 44;
  });

  it("exposes a named table, column headers, and the total logical row count", () => {
    const markup = renderToStaticMarkup(
      <ChatView
        state={{ ...initialAppState, chatMessages: [chatMessage("queued")] }}
        showStartupGuide={false}
      />,
    );

    expect(markup).toContain('role="table"');
    expect(markup).toContain('aria-label="チャット一覧"');
    expect(markup).toContain('aria-colcount="3"');
    expect(markup).toContain('aria-rowcount="2"');
    expect(markup.match(/role="columnheader"/g)).toHaveLength(3);
  });

  it("keeps the virtualized logical row index aligned when messages are prepended", () => {
    const existingMessages = [
      chatMessage("queued", "newest"),
      chatMessage("spoken", "middle"),
      chatMessage("skipped", "anchor"),
    ];
    virtualizerState.indexes = [2];
    virtualizerState.totalSize = 132;

    const beforePrepend = renderToStaticMarkup(
      <ChatView
        state={{ ...initialAppState, chatMessages: existingMessages }}
        showStartupGuide={false}
      />,
    );

    expect(beforePrepend).toContain('aria-rowcount="4"');
    expect(beforePrepend).toContain('data-chat-message-id="anchor"');
    expect(beforePrepend).toContain('role="row" aria-rowindex="4"');

    virtualizerState.indexes = [3];
    virtualizerState.totalSize = 176;
    const afterPrepend = renderToStaticMarkup(
      <ChatView
        state={{
          ...initialAppState,
          chatMessages: [chatMessage("blocked", "prepended"), ...existingMessages],
        }}
        showStartupGuide={false}
      />,
    );

    expect(afterPrepend).toContain('aria-rowcount="5"');
    expect(afterPrepend).toContain('data-chat-message-id="anchor"');
    expect(afterPrepend).toContain('role="row" aria-rowindex="5"');
  });

  it.each([
    ["queued", "待機"],
    ["spoken", "完了"],
    ["skipped", "スキップ"],
    ["blocked", "抑制"],
    ["error", "エラー"],
  ] satisfies Array<[ChatDisplayState, string]>)("renders %s with a visible Japanese label", (status, label) => {
    const markup = renderToStaticMarkup(<ChatRow message={chatMessage(status)} rowIndex={37} />);

    expect(markup).toContain('role="row"');
    expect(markup).toContain('aria-rowindex="37"');
    expect(markup.match(/role="cell"/g)).toHaveLength(3);
    expect(markup).toContain('aria-colindex="3"');
    expect(markup).toContain(`>${label}</span>`);
    expect(markup).toContain('aria-hidden="true"');
  });
});
