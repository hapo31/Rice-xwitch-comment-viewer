import { Link } from "react-router-dom";
import { appRoutes, settingsRoute } from "../routes";
import type { SpeechQueueOutcome } from "../types";

const filterRoute = appRoutes.find((route) => route.path === "/filter")!;
const queueRoute = appRoutes.find((route) => route.path === "/queue")!;
const recovery = {
  reviewFilters: {
    route: filterRoute,
    text: "FilterでNG・URL・連投・emoteの設定を確認してください。",
    label: "Filterを開く",
  },
  reviewQueue: {
    route: filterRoute,
    text: "待機量を確認し、読み上げ対象を調整してください。",
    label: "Filterを開く",
  },
  diagnoseSpeech: {
    route: settingsRoute,
    text: "接続設定と［診断］を確認してから、Queueで明示的に再試行してください。",
    label: "Settingsの診断を開く",
  },
  confirmDelivery: {
    route: settingsRoute,
    text: "読み上げ先の発声・待機を確認してください。明示的な再試行も重複する可能性があります。",
    label: "Settingsの診断を開く",
  },
  none: { route: undefined, text: "復旧操作は不要です。この項目は自動で再送しません。", label: "" },
} as const;

export function SpeechOutcomeDetails({
  outcome,
  itemId,
  showQueueLink = false,
}: {
  outcome: SpeechQueueOutcome;
  itemId?: string;
  showQueueLink?: boolean;
}) {
  const action = recovery[outcome.recoveryAction];
  return (
    <div className="mt-1 space-y-1 text-xs text-zinc-400">
      <p className="text-zinc-200">{outcome.message}</p>
      <p className="break-words font-mono text-[10px]">
        {itemId && <span>{itemId} · </span>}
        <code>{outcome.reasonCode}</code> ·{" "}
        <time dateTime={new Date(outcome.occurredAtMs).toISOString()}>
          {new Date(outcome.occurredAtMs).toLocaleString("ja-JP")}
        </time>
      </p>
      {outcome.kind === "error" && (
        <p>
          {outcome.retryable
            ? "未送信の一時的失敗です。自動再試行の終了後は手動で判断してください。"
            : "安全な自動再送はできません。"}
        </p>
      )}
      <p>
        {action.text}
        {action.route && (
          <>
            {" "}
            <Link to={action.route.path} className="text-sky-400 underline">
              {action.label}
            </Link>
          </>
        )}
        {showQueueLink && outcome.kind === "error" && (
          <>
            {" "}
            <Link to={queueRoute.path} className="text-sky-400 underline">
              Queueで再試行・履歴を確認
            </Link>
          </>
        )}
      </p>
    </div>
  );
}
