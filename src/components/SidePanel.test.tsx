import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { SidePanel } from "./SidePanel";
import { initialAppState } from "../stores/appStore";

describe("SidePanel speech recovery", () => {
  it("links a disconnected speech status to the Settings diagnostic", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/chat"]}>
        <SidePanel
          state={initialAppState}
          onSpeechControl={() => undefined}
          onTwitchConnect={() => undefined}
          onTwitchStopChat={() => undefined}
          onWarningsClear={() => undefined}
        />
      </MemoryRouter>,
    );

    expect(markup).toContain('href="/settings"');
    expect(markup).toContain("Settings 画面の［診断］を開く");
  });
});
