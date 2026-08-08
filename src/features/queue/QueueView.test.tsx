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
    expect(markup.match(/aria-label="履歴項目を削除"/g)).toHaveLength(2);
    expect(markup).not.toContain('aria-label="履歴項目を削除" disabled=""');
  });
});
