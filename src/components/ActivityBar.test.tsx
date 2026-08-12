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

    expect(markup).toContain('href="/logs"');
    expect(markup).toContain('aria-label="Logs"');
    expect(markup).toContain('title="Logs"');
    expect(markup).toContain('aria-current="page"');
  });
});
