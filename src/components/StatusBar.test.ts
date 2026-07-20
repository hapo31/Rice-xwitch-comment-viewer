import { describe, expect, it } from "vitest";
import { formatBuildLabel } from "./StatusBar";

describe("formatBuildLabel", () => {
  it("shows only the version for a release build", () => {
    expect(formatBuildLabel({ version: "1.2.3", isDev: false, commitHash: "abcdef0" })).toBe(
      "Rice 1.2.3",
    );
  });

  it("shows the dev marker and commit hash for a development build", () => {
    expect(formatBuildLabel({ version: "1.2.3", isDev: true, commitHash: "abcdef0" })).toBe(
      "Rice 1.2.3 (dev abcdef0)",
    );
  });

  it("still identifies a development build when the commit is unavailable", () => {
    expect(formatBuildLabel({ version: "1.2.3", isDev: true })).toBe("Rice 1.2.3 (dev)");
  });
});
