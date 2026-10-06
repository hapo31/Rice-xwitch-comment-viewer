import {
  AlertCircle,
  CheckCircle2,
  Circle,
  CircleDashed,
  CircleOff,
  type LucideIcon,
  Volume2,
} from "lucide-react";
import type { ChatDisplayState, QueueDisplayState, SpeechStatus } from "../types";

export interface StatusPresentation {
  icon: LucideIcon;
  label: string;
  className: string;
}

export function getQueueStatusPresentation(status: QueueDisplayState): StatusPresentation {
  return {
    queued: { icon: CircleDashed, label: "待機", className: "text-sky-400" },
    speaking: { icon: Volume2, label: "読み上げ中", className: "text-emerald-400" },
    spoken: { icon: CheckCircle2, label: "完了", className: "text-emerald-400" },
    skipped: { icon: CircleOff, label: "スキップ", className: "text-zinc-400" },
    blocked: { icon: CircleOff, label: "抑制", className: "text-amber-400" },
    error: { icon: AlertCircle, label: "エラー", className: "text-rose-400" },
  }[status];
}

export function getChatStatusPresentation(status: ChatDisplayState): StatusPresentation {
  if (status === "received") return { icon: Circle, label: "受信済み", className: "text-zinc-400" };
  return getQueueStatusPresentation(status);
}

export function queueStatusLabel(status: QueueDisplayState): string {
  return getQueueStatusPresentation(status).label;
}

export function speechStatusLabel(status: SpeechStatus): string {
  return {
    idle: "待機中",
    speaking: "読み上げ中",
    paused: "一時停止中",
    disconnected: "未接続",
    error: "接続エラー",
  }[status];
}

export function getChatMessagePresentation(message: { kind: "user" | "system" }): {
  userNameClassName: string;
  textClassName: string;
} {
  return message.kind === "system"
    ? { userNameClassName: "text-amber-300", textClassName: "text-zinc-300" }
    : { userNameClassName: "text-sky-300", textClassName: "text-zinc-200" };
}
