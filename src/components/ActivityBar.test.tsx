import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { ActivityBar } from "./ActivityBar";

describe("ActivityBar", () => {
  it("provides an accessible Logs navigation link and marks it as current", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/logs"]}>
        <ActivityBar />
      </MemoryRouter>,
    );

    expect(markup).toMatch(
      /<a(?=[^>]*href="\/logs")(?=[^>]*aria-label="Logs")(?=[^>]*title="Logs")(?=[^>]*aria-current="page")[^>]*>/,
    );
  });
});
