import {
  AlertCircle,
  CheckCircle2,
  CircleDashed,
  CircleOff,
  type LucideIcon,
} from "lucide-react";
import type { ChatDisplayState, QueueDisplayState } from "../types";

export interface StatusPresentation {
  icon: LucideIcon;
  label: string;
  className: string;
}

export function getChatStatusPresentation(status: ChatDisplayState): StatusPresentation {
  return {
    queued: { icon: CircleDashed, label: "queued", className: "text-sky-400" },
    spoken: { icon: CheckCircle2, label: "spoken", className: "text-emerald-400" },
    skipped: { icon: CircleOff, label: "skipped", className: "text-zinc-500" },
    blocked: { icon: CircleOff, label: "blocked", className: "text-amber-400" },
    error: { icon: AlertCircle, label: "error", className: "text-rose-400" },
  }[status];
}

export function queueStatusLabel(status: QueueDisplayState): string {
  return {
    queued: "待機",
    speaking: "読み上げ中",
    spoken: "完了",
    skipped: "スキップ",
    blocked: "抑制",
    error: "エラー",
  }[status];
}
