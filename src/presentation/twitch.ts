import type { AuthStatus, TwitchChatConnectionStatus } from "../types";

const authLabels = {
  unauthenticated: "未認証",
  authorizing: "認証開始中",
  polling: "認証確認中",
  checking: "認証確認中",
  authenticated: "ログイン済み",
  expired: "再ログイン必要",
  disconnecting: "認証解除中",
  error: "認証エラー",
} as const satisfies Record<AuthStatus, string>;

// Spoken announcements distinguish operations that share a compact visual label.
const authAnnouncementLabels = {
  ...authLabels,
  authorizing: "認証コードを発行中",
  polling: "認証を確認中",
  checking: "有効性を確認中",
  disconnecting: "認証を解除中",
} as const satisfies Record<AuthStatus, string>;

const connectionLabels = {
  disconnected: "未接続",
  connecting: "接続中",
  connected: "受信中",
  reconnecting: "再接続中",
  authRequired: "再ログイン必要",
  error: "接続エラー",
} as const satisfies Record<TwitchChatConnectionStatus, string>;

export function getTwitchAuthLabel(
  status: AuthStatus,
  usage: "compact" | "announcement" = "compact",
) {
  return usage === "announcement" ? authAnnouncementLabels[status] : authLabels[status];
}

export function getTwitchConnectionLabel(status: TwitchChatConnectionStatus) {
  return connectionLabels[status];
}
