import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FilterView } from "../filter/FilterView";
import { SettingsView } from "./SettingsView";

function headingList(markup: string): Array<{ level: string; text: string }> {
  return [...markup.matchAll(/<h([1-6])[^>]*>([^<]+)<\/h\1>/g)].map(([, level, text]) => ({ level, text }));
}

describe("Settings and Filter section headings", () => {
  it("provides a consistent h1/h2 heading outline for settings groups", () => {
    const settingsHeadings = headingList(
      renderToStaticMarkup(
        <SettingsView
          onSettingsUpdate={() => undefined}
          onSpeechHealthCheck={() => undefined}
          onSpeechDiagnostics={async () => ({ configuredAddr: "127.0.0.1:50001", attempted: [], recommendation: "" })}
          onSpeechTest={() => undefined}
        />,
      ),
    );
    const filterHeadings = headingList(renderToStaticMarkup(<FilterView onSettingsUpdate={() => undefined} />));

    expect(settingsHeadings).toEqual([
      { level: "1", text: "Settings" },
      { level: "2", text: "チャット受信" },
      { level: "2", text: "自動読み上げ" },
      { level: "2", text: "棒読みちゃん接続" },
      { level: "2", text: "声質" },
      { level: "2", text: "接続成功時の読み上げ" },
      { level: "2", text: "テスト読み上げ" },
    ]);
    expect(filterHeadings).toEqual([
      { level: "1", text: "Filter" },
      { level: "2", text: "読み上げ条件" },
      { level: "2", text: "除外リスト" },
    ]);
  });
});
