import { Bot, ListFilter, MessageSquareText, Radio, ScrollText, Settings, SlidersHorizontal } from "lucide-react";
import type { ViewId } from "../types";

interface NavItem {
  id: ViewId;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}

const navItems: NavItem[] = [
  { id: "chat", label: "Chat", icon: MessageSquareText },
  { id: "queue", label: "Queue", icon: Radio },
  { id: "rules", label: "Rules", icon: ListFilter },
  { id: "voices", label: "Voices", icon: Bot },
  { id: "settings", label: "Settings", icon: Settings },
  { id: "logs", label: "Logs", icon: ScrollText },
];

interface ActivityBarProps {
  activeView: ViewId;
  onChange: (view: ViewId) => void;
}

export function ActivityBar({ activeView, onChange }: ActivityBarProps) {
  return (
    <nav className="col-start-1 row-start-2 flex flex-col items-center border-r border-zinc-800 bg-zinc-900 py-2">
      {navItems.map((item) => {
        const Icon = item.icon;
        const isActive = item.id === activeView;

        return (
          <button
            key={item.id}
            type="button"
            aria-label={item.label}
            title={item.label}
            onClick={() => onChange(item.id)}
            className={[
              "mb-1 flex h-11 w-11 items-center justify-center border-l-2 transition-colors",
              isActive
                ? "border-sky-400 bg-zinc-850 text-zinc-100"
                : "border-transparent text-zinc-400 hover:bg-zinc-850 hover:text-zinc-100",
            ].join(" ")}
          >
            <Icon className="h-5 w-5" />
          </button>
        );
      })}
      <div className="mt-auto">
        <SlidersHorizontal className="h-4 w-4 text-zinc-600" />
      </div>
    </nav>
  );
}
