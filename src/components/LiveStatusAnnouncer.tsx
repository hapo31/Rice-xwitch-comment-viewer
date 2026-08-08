import { useEffect, useRef, useState } from "react";
import { speechStatusLabel } from "../presentation/chat";
import type { AppState } from "../stores/appStore";
import type { AuthStatus, SpeechStatus, TwitchChatConnectionStatus } from "../types";

type AnnouncementPriority = "status" | "alert";

export interface LiveStatusSnapshot {
  twitchAuthStatus: AuthStatus;
  twitchConnectionStatus: TwitchChatConnectionStatus;
  speechStatus: SpeechStatus;
  latestWarning?: string;
}

export interface LiveStatusAnnouncement {
  message: string;
  priority: AnnouncementPriority;
}

const twitchAuthLabels: Record<AuthStatus, string> = {
  unauthenticated: "未認証",
  authenticated: "ログイン済み",
  expired: "再ログイン必要",
  error: "認証エラー",
};

const twitchConnectionLabels: Record<TwitchChatConnectionStatus, string> = {
  disconnected: "未接続",
  connecting: "接続中",
  connected: "受信中",
  reconnecting: "再接続中",
  authRequired: "再ログイン必要",
  error: "接続エラー",
};

export function toLiveStatusSnapshot(state: AppState): LiveStatusSnapshot {
  return {
    twitchAuthStatus: state.twitchAuthStatus,
    twitchConnectionStatus: state.twitchConnectionStatus,
    speechStatus: state.speechStatus,
    latestWarning: state.notifications.find((notification) => notification.severity === "warning")?.message,
  };
}

export function getLiveStatusAnnouncement(
  previous: LiveStatusSnapshot,
  current: LiveStatusSnapshot,
): LiveStatusAnnouncement | undefined {
  if (previous.twitchAuthStatus !== current.twitchAuthStatus && isAuthError(current.twitchAuthStatus)) {
    return { message: `Twitch 認証: ${twitchAuthLabels[current.twitchAuthStatus]}`, priority: "alert" };
  }

  if (
    previous.twitchConnectionStatus !== current.twitchConnectionStatus &&
    isConnectionError(current.twitchConnectionStatus)
  ) {
    return { message: `Twitch 接続: ${twitchConnectionLabels[current.twitchConnectionStatus]}`, priority: "alert" };
  }

  if (previous.speechStatus !== current.speechStatus && isSpeechError(current.speechStatus)) {
    return { message: `棒読みちゃん: ${speechStatusLabel(current.speechStatus)}`, priority: "alert" };
  }

  if (previous.latestWarning !== current.latestWarning && current.latestWarning) {
    return { message: `警告: ${current.latestWarning}`, priority: "status" };
  }

  if (previous.twitchAuthStatus !== current.twitchAuthStatus) {
    return { message: `Twitch 認証: ${twitchAuthLabels[current.twitchAuthStatus]}`, priority: "status" };
  }

  if (previous.twitchConnectionStatus !== current.twitchConnectionStatus) {
    return { message: `Twitch 接続: ${twitchConnectionLabels[current.twitchConnectionStatus]}`, priority: "status" };
  }

  if (previous.speechStatus !== current.speechStatus) {
    return { message: `棒読みちゃん: ${speechStatusLabel(current.speechStatus)}`, priority: "status" };
  }
}

export function LiveStatusAnnouncer({ state }: { state: AppState }) {
  const snapshot = toLiveStatusSnapshot(state);
  const previousSnapshot = useRef(snapshot);
  const [announcement, setAnnouncement] = useState<LiveStatusAnnouncement>();

  useEffect(() => {
    const nextAnnouncement = getLiveStatusAnnouncement(previousSnapshot.current, snapshot);
    previousSnapshot.current = snapshot;
    if (nextAnnouncement) {
      setAnnouncement(nextAnnouncement);
    }
  }, [snapshot.twitchAuthStatus, snapshot.twitchConnectionStatus, snapshot.speechStatus, snapshot.latestWarning]);

  return (
    <>
      <p className="sr-only" role="status" aria-atomic="true">
        {announcement?.priority === "status" ? announcement.message : ""}
      </p>
      <p className="sr-only" role="alert" aria-atomic="true">
        {announcement?.priority === "alert" ? announcement.message : ""}
      </p>
    </>
  );
}

function isAuthError(status: AuthStatus): boolean {
  return status === "expired" || status === "error";
}

function isConnectionError(status: TwitchChatConnectionStatus): boolean {
  return status === "authRequired" || status === "error";
}

function isSpeechError(status: SpeechStatus): boolean {
  return status === "disconnected" || status === "error";
}
