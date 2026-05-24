export const appRoutes = [
  { path: "/chat", label: "Chat" },
  { path: "/queue", label: "Queue" },
  { path: "/rules", label: "Rules" },
  { path: "/settings", label: "Settings" },
  { path: "/auth", label: "Login" },
  { path: "/logs", label: "Logs" },
] as const;

export type AppRoutePath = (typeof appRoutes)[number]["path"];

export function getRouteLabel(pathname: string): string {
  return appRoutes.find((route) => route.path === pathname)?.label ?? "Chat";
}
