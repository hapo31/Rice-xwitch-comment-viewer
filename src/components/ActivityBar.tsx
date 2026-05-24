import { KeyRound, ListFilter, MessageSquareText, Radio, ScrollText, SlidersHorizontal } from "lucide-react";
import { NavLink } from "react-router-dom";
import type { AppRoutePath } from "../routes";

interface NavItem {
  path: AppRoutePath;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const navItems: NavItem[] = [
  { path: "/chat", label: "Chat", icon: MessageSquareText },
  { path: "/queue", label: "Queue", icon: Radio },
  { path: "/rules", label: "Rules", icon: ListFilter },
  { path: "/settings", label: "Settings", icon: SlidersHorizontal },
  { path: "/auth", label: "Auth", icon: KeyRound },
  { path: "/logs", label: "Logs", icon: ScrollText },
];

export function ActivityBar() {
  return (
    <nav className="col-start-1 row-start-2 flex flex-col items-center border-r border-zinc-800 bg-zinc-900 py-2">
      {navItems.map((item) => {
        const Icon = item.icon;

        return (
          <NavLink
            key={item.path}
            to={item.path}
            aria-label={item.label}
            title={item.label}
            className={({ isActive }) => [
              "mb-1 flex h-11 w-11 items-center justify-center border-l-2 transition-colors",
              isActive
                ? "border-sky-400 bg-zinc-850 text-zinc-100"
                : "border-transparent text-zinc-400 hover:bg-zinc-850 hover:text-zinc-100",
            ].join(" ")}
          >
            <Icon className="h-5 w-5" />
          </NavLink>
        );
      })}
      <div className="mt-auto">
        <SlidersHorizontal className="h-4 w-4 text-zinc-600" />
      </div>
    </nav>
  );
}
