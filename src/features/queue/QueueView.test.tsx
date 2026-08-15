import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { QueueView } from "./QueueView";
import { initialAppState } from "../../stores/appStore";

describe("QueueView", () => {
  it("offers separate pending-speech and history dismiss controls, including blocked items", () => {
    const markup = renderToStaticMarkup(
      <QueueView
        state={{
          ...initialAppState,
          queueItems: [
            { id: "speech-1", userDisplayName: "queued", text: "待機", status: "queued" },
            { id: "speech-2", userDisplayName: "error", text: "失敗", status: "error" },
            { id: "speech-3", userDisplayName: "blocked", text: "抑制", status: "blocked" },
          ],
        }}
        onSpeechControl={() => undefined}
        onQueueReload={() => undefined}
        onQueueRemove={() => undefined}
        onQueueDismiss={() => undefined}
        onQueueDismissHistory={() => undefined}
        onQueueRetry={() => undefined}
      />,
    );

    expect(markup).toContain('aria-label="待機中の読み上げをクリア"');
    expect(markup).toContain('aria-label="表示履歴をクリア"');
    expect(markup).toContain('role="table"');
    expect(markup).toContain('aria-label="読み上げキュー"');
    expect(markup).toContain('aria-colcount="4"');
    expect(markup).toContain('aria-rowcount="4"');
    expect(markup.match(/role="columnheader"/g)).toHaveLength(4);
    expect(markup.match(/role="cell"/g)).toHaveLength(12);
    expect(markup).toContain('aria-rowindex="4"');
    expect(markup).toContain('aria-label="errorの読み上げを再試行"');
    expect(markup).toContain('aria-label="queuedの待機中の読み上げを削除"');
    expect(markup).toContain('aria-label="errorの履歴項目を削除"');
    expect(markup).toContain('aria-label="blockedの履歴項目を削除"');
    expect(markup).not.toContain('aria-label="blockedの履歴項目を削除" disabled=""');
  });

  it("keeps the empty state inside the named table as one logical row", () => {
    const markup = renderToStaticMarkup(
      <QueueView
        state={initialAppState}
        onSpeechControl={() => undefined}
        onQueueReload={() => undefined}
        onQueueRemove={() => undefined}
        onQueueDismiss={() => undefined}
        onQueueDismissHistory={() => undefined}
        onQueueRetry={() => undefined}
      />,
    );

    expect(markup).toContain('aria-rowcount="2"');
    expect(markup).toContain('aria-rowindex="2"');
    expect(markup).toContain('role="cell" aria-colspan="4"');
  });
});
