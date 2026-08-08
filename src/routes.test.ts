import { describe, expect, it } from "vitest";
import { appRoutes, settingsRoute } from "./routes";

describe("current navigation contract", () => {
  it("keeps Settings as the diagnostic destination and excludes legacy names", () => {
    expect(settingsRoute).toEqual({ path: "/settings", label: "Settings" });
    expect(appRoutes.map((route) => route.label)).toEqual([
      "Chat",
      "Launcher",
      "Queue",
      "Filter",
      "Settings",
      "Login",
      "Logs",
    ]);
    expect(appRoutes.map((route) => route.label)).not.toContain("Voices");
    expect(appRoutes.map((route) => route.label)).not.toContain("Rules");
  });
});
