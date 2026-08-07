import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { TitleBar } from "./TitleBar";

describe("TitleBar UI scale selector", () => {
  it("exposes a named radio group with its selected scale and current display scale", () => {
    const markup = renderToStaticMarkup(
      <TitleBar scale={1.25} scaleMode="1.25" onScaleModeChange={() => undefined} />,
    );

    expect(markup).toContain("<legend class=\"sr-only\">UI倍率</legend>");
    expect(markup.match(/type="radio" name="ui-scale"/g)).toHaveLength(4);
    expect(markup).toContain('value="auto"');
    expect(markup).toContain('value="1"');
    expect(markup).toContain('checked="" value="1.25"');
    expect(markup).toContain('value="1.5"');
    expect(markup).toContain('aria-label="現在の表示倍率">125%</output>');
  });
});
