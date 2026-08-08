import type { NavigationType } from "react-router-dom";
import { getRouteLabel } from "./routes";

export const routeHeadingId = "main-view-heading";

export function getRouteDocumentTitle(pathname: string): string {
  return `Rice - ${getRouteLabel(pathname)}`;
}

export function shouldFocusRouteHeading(navigationType: NavigationType): boolean {
  return navigationType === "PUSH";
}
