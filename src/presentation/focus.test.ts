import { describe, expect, it } from "vitest";
import { focusIndicatorClass } from "./focus";

describe("focusIndicatorClass", () => {
  it("uses a keyboard-only ring with an offset", () => {
    expect(focusIndicatorClass).toContain("focus-visible:ring-2");
    expect(focusIndicatorClass).toContain("focus-visible:ring-offset-2");
    expect(focusIndicatorClass).not.toContain("focus:ring-");
  });

  it("uses a system focus outline in forced-colors mode", () => {
    expect(focusIndicatorClass).toContain("forced-colors:focus-visible:outline-[Highlight]");
  });
});
