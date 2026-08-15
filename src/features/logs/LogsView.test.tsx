import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { initialAppState } from "../../stores/appStore";
import { LogsView } from "./LogsView";

const virtualizerState = vi.hoisted(() => ({ indexes: [0], totalSize: 40 }));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: () => ({
    getTotalSize: () => virtualizerState.totalSize,
    getVirtualItems: () => virtualizerState.indexes.map((index) => ({
      index,
      key: `virtual-log-${index}`,
      start: index * 40,
    })),
    measureElement: () => undefined,
  }),
}));

describe("LogsView table semantics", () => {
  const log = logEvent("log-1", "再接続しています");

  beforeEach(() => {
    virtualizerState.indexes = [0];
    virtualizerState.totalSize = 40;
  });

  function logEvent(id: string, message: string) {
    return {
      id,
      level: "warning" as const,
      message,
      occurredAtMs: Date.UTC(2026, 7, 15, 12, 34, 56),
    };
  }

  it("exposes a named table, column headers, and the virtualized total row count", () => {
    const markup = renderToStaticMarkup(
      <LogsView state={{ ...initialAppState, logs: [log] }} />,
    );

    expect(markup).toContain('role="table"');
    expect(markup).toContain('aria-label="アプリログ"');
    expect(markup).toContain('aria-colcount="3"');
    expect(markup).toContain('aria-rowcount="2"');
    expect(markup.match(/role="columnheader"/g)).toHaveLength(3);
  });

  it("keeps the virtualized logical row index aligned when logs are prepended", () => {
    const existingLogs = [
      logEvent("newest", "最新"),
      logEvent("middle", "中間"),
      logEvent("anchor", "基準ログ"),
    ];
    virtualizerState.indexes = [2];
    virtualizerState.totalSize = 120;

    const beforePrepend = renderToStaticMarkup(
      <LogsView state={{ ...initialAppState, logs: existingLogs }} />,
    );

    expect(beforePrepend).toContain('aria-rowcount="4"');
    expect(beforePrepend).toContain('role="row" aria-rowindex="4"');
    expect(beforePrepend).toContain("基準ログ");
    expect(beforePrepend.match(/role="cell"/g)).toHaveLength(3);
    expect(beforePrepend).toContain('aria-colindex="3"');

    virtualizerState.indexes = [3];
    virtualizerState.totalSize = 160;
    const afterPrepend = renderToStaticMarkup(
      <LogsView
        state={{
          ...initialAppState,
          logs: [logEvent("prepended", "追加ログ"), ...existingLogs],
        }}
      />,
    );

    expect(afterPrepend).toContain('aria-rowcount="5"');
    expect(afterPrepend).toContain('role="row" aria-rowindex="5"');
    expect(afterPrepend).toContain("基準ログ");
  });

  it("represents the empty message as one logical table row", () => {
    const markup = renderToStaticMarkup(<LogsView state={initialAppState} />);

    expect(markup).toContain('aria-rowcount="2"');
    expect(markup).toContain('aria-rowindex="2"');
    expect(markup).toContain('role="cell" aria-colspan="3"');
  });
});
