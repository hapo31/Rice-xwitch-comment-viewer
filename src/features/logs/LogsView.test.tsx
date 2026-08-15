import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { initialAppState } from "../../stores/appStore";
import { LogRow, LogsView } from "./LogsView";

describe("LogsView table semantics", () => {
  const log = {
    id: "log-1",
    level: "warning" as const,
    message: "再接続しています",
    occurredAtMs: Date.UTC(2026, 7, 15, 12, 34, 56),
  };

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

  it("exposes the logical virtual row position and each cell column", () => {
    const markup = renderToStaticMarkup(<LogRow log={log} rowIndex={402} />);

    expect(markup).toContain('role="row"');
    expect(markup).toContain('aria-rowindex="402"');
    expect(markup.match(/role="cell"/g)).toHaveLength(3);
    expect(markup).toContain('aria-colindex="3"');
  });

  it("represents the empty message as one logical table row", () => {
    const markup = renderToStaticMarkup(<LogsView state={initialAppState} />);

    expect(markup).toContain('aria-rowcount="2"');
    expect(markup).toContain('aria-rowindex="2"');
    expect(markup).toContain('role="cell" aria-colspan="3"');
  });
});
