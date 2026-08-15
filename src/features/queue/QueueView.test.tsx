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
            { id: "speech-1", userDisplayName: "viewer", text: "同じ本文", status: "queued" },
            { id: "speech-2", userDisplayName: "viewer", text: "同じ本文", status: "error" },
            { id: "speech-3", userDisplayName: "viewer", text: "同じ本文", status: "blocked" },
            { id: "speech-4", userDisplayName: "viewer", text: "表示対象外", status: "spoken" },
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
    expect(markup).not.toContain("表示対象外");
    expect(markup).toContain('aria-label="キュー3行目、viewerの「同じ本文」を再試行"');
    expect(markup).toContain('aria-label="キュー4行目、viewerの「同じ本文」を待機キューから削除"');
    expect(markup).toContain('aria-label="キュー3行目、viewerの「同じ本文」を履歴から削除"');
    expect(markup).toContain('aria-label="キュー2行目、viewerの「同じ本文」を履歴から削除"');
    expect(markup).not.toContain('aria-label="キュー2行目、viewerの「同じ本文」を履歴から削除" disabled=""');
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

  it("keeps operation names concise when the reading text is long", () => {
    const markup = renderToStaticMarkup(
      <QueueView
        state={{
          ...initialAppState,
          queueItems: [
            {
              id: "speech-1",
              userDisplayName: "viewer",
              text: "1234567890123456789012345",
              status: "error",
            },
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

    expect(markup).toContain(
      'aria-label="キュー2行目、viewerの「123456789012345678901234…」を再試行"',
    );
  });
});
