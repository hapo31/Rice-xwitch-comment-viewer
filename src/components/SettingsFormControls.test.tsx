import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { FilterView } from "../features/filter/FilterView";
import { SettingsView } from "../features/settings/SettingsView";
import { FloatingSaveButton } from "./SettingsFormControls";

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
});
