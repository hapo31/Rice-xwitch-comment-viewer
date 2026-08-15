import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ChatRow, ChatView } from "./ChatView";
import { initialAppState } from "../../stores/appStore";
import type { ChatDisplayState, UserChatMessage } from "../../types";

function chatMessage(status: ChatDisplayState): UserChatMessage {
  return {
    kind: "user",
    id: status,
    receivedAt: "2026-08-12T00:00:00Z",
    userDisplayName: "viewer",
    text: "こんにちは",
    status,
  };
}

describe("ChatRow read status", () => {
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
