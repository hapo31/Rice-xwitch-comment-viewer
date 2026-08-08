import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ChatBadges } from "./ChatBadges";

describe("ChatBadges", () => {
  it("renders multiple representative Twitch badges with readable labels and accessible names", () => {
    const markup = renderToStaticMarkup(
      <ChatBadges
        badges={[
          { setId: "moderator", id: "1", info: "" },
          { setId: "subscriber", id: "12", info: "12" },
        ]}
      />,
    );

    expect(markup).toContain(">モデ<");
    expect(markup).toContain(">購読<");
    expect(markup).toContain('aria-label="モデバッジ"');
    expect(markup).toContain('aria-label="購読バッジ"');
  });

  it("keeps an unknown badge bounded while exposing its set ID to assistive technology", () => {
    const markup = renderToStaticMarkup(<ChatBadges badges={[{ setId: "founder", id: "0", info: "" }]} />);

    expect(markup).toContain(">founder<");
    expect(markup).toContain('aria-label="不明な Twitch バッジ: founder"');
    expect(markup).toContain("max-w-20 truncate");
  });

  it("renders no badge markup for messages without badges", () => {
    expect(renderToStaticMarkup(<ChatBadges badges={[]} />)).toBe("");
  });
});
