import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FilterView } from "../features/filter/FilterView";
import { SettingsView } from "../features/settings/SettingsView";
import { FloatingSaveButton, RangeRow, RuleTextArea } from "./SettingsFormControls";

describe("FloatingSaveButton accessibility", () => {
  it("removes the unchanged Filter and Settings save controls from the keyboard and accessibility tree", () => {
    const filterMarkup = renderToStaticMarkup(<FilterView onSettingsUpdate={() => undefined} />);
    const settingsMarkup = renderToStaticMarkup(
      <SettingsView
        onSettingsUpdate={() => undefined}
        onSpeechHealthCheck={() => undefined}
        onSpeechDiagnostics={async () => ({ configuredAddr: "127.0.0.1:50001", attempted: [], recommendation: "" })}
        onSpeechTest={() => undefined}
      />,
    );

    expect(filterMarkup).not.toContain('aria-label="設定を保存"');
    expect(settingsMarkup).not.toContain('aria-label="設定を保存"');
  });

  it("renders the save button as an operable button only when it is visible", () => {
    const markup = renderToStaticMarkup(
      <FloatingSaveButton visible disabled={false} onClick={() => undefined} />,
    );

    expect(markup).toContain('<button type="button" aria-label="設定を保存" title="設定を保存"');
    expect(markup).toContain(">保存</button>");
  });

  it("associates rule textarea labels with stable control IDs", () => {
    const markup = renderToStaticMarkup(
      <RuleTextArea id="rule-blocked-users" label="NG ユーザー" value="" onChange={() => undefined} />,
    );

    expect(markup).toContain('<label class="pt-2 text-sm text-zinc-400" for="rule-blocked-users">NG ユーザー</label>');
    expect(markup).toContain('<textarea id="rule-blocked-users"');
  });

  it("exposes the range label and its default value to assistive technology", () => {
    const markup = renderToStaticMarkup(
      <RangeRow id="bouyomi-speed" label="速度" value={-1} min={-1} max={300} onChange={() => undefined} />,
    );

    expect(markup).toContain('<label class="text-sm text-zinc-400" for="bouyomi-speed">速度</label>');
    expect(markup).toContain('id="bouyomi-speed"');
    expect(markup).toContain('aria-valuetext="既定"');
  });

  it("announces a numeric range value when it is explicitly set", () => {
    const markup = renderToStaticMarkup(
      <RangeRow id="bouyomi-volume" label="音量" value={80} min={-1} max={100} onChange={() => undefined} />,
    );

    expect(markup).toContain('aria-valuetext="80"');
  });
});
