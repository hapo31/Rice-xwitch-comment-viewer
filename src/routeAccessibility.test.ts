import { describe, expect, it } from "vitest";
import { NavigationType } from "react-router-dom";
import { getRouteDocumentTitle, routeHeadingId, shouldFocusRouteHeading } from "./routeAccessibility";

describe("route accessibility", () => {
  it("derives a document title for every application route", () => {
    expect(getRouteDocumentTitle("/chat")).toBe("Rice - Chat");
    expect(getRouteDocumentTitle("/launcher")).toBe("Rice - Launcher");
    expect(getRouteDocumentTitle("/queue")).toBe("Rice - Queue");
    expect(getRouteDocumentTitle("/filter")).toBe("Rice - Filter");
    expect(getRouteDocumentTitle("/settings")).toBe("Rice - Settings");
    expect(getRouteDocumentTitle("/auth")).toBe("Rice - Login");
    expect(getRouteDocumentTitle("/logs")).toBe("Rice - Logs");
  });

  it("only moves focus for user-initiated history pushes", () => {
    expect(shouldFocusRouteHeading(NavigationType.Push)).toBe(true);
    expect(shouldFocusRouteHeading(NavigationType.Pop)).toBe(false);
    expect(shouldFocusRouteHeading(NavigationType.Replace)).toBe(false);
    expect(routeHeadingId).toBe("main-view-heading");
  });
});
