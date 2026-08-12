import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ChatRow } from "./ChatView";
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
  it.each([
    ["queued", "待機"],
    ["spoken", "完了"],
    ["skipped", "スキップ"],
    ["blocked", "抑制"],
    ["error", "エラー"],
  ] satisfies Array<[ChatDisplayState, string]>)("renders %s with a visible Japanese label", (status, label) => {
    const markup = renderToStaticMarkup(<ChatRow message={chatMessage(status)} />);

    expect(markup).toContain(`>${label}</span>`);
    expect(markup).toContain('aria-hidden="true"');
  });
});
