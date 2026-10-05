import { getTwitchAuthLabel, getTwitchConnectionLabel } from "../presentation/twitch";
import { useEffect, useRef, useState } from "react";
import { speechHealthLabels, speechQueuePhaseLabels } from "../presentation/speech";
import type { AppState } from "../stores/appStore";
import type {
  AuthStatus,
  SpeechAdapterHealth,
  SpeechQueuePhase,
  TwitchChatConnectionStatus,
} from "../types";

type AnnouncementPriority = "status" | "alert";

export interface LiveStatusSnapshot {
  twitchAuthStatus: AuthStatus;
  twitchConnectionStatus: TwitchChatConnectionStatus;
  speechAdapterHealth: SpeechAdapterHealth;
  speechQueuePhase: SpeechQueuePhase;
  latestWarning?: string;
}

export interface LiveStatusAnnouncement {
  message: string;
  priority: AnnouncementPriority;
}

export function toLiveStatusSnapshot(state: AppState): LiveStatusSnapshot {
  return {
    twitchAuthStatus: state.twitchAuthStatus,
    twitchConnectionStatus: state.twitchConnectionStatus,
    speechAdapterHealth: state.speechAdapterHealth,
    speechQueuePhase: state.speechQueuePhase,
    latestWarning: state.notifications.find((notification) => notification.severity === "warning")
      ?.message,
  };
}

export function getLiveStatusAnnouncement(
  previous: LiveStatusSnapshot,
  current: LiveStatusSnapshot,
): LiveStatusAnnouncement | undefined {
  if (
    previous.twitchAuthStatus !== current.twitchAuthStatus &&
    isAuthError(current.twitchAuthStatus)
  ) {
    return {
      message: `Twitch 認証: ${getTwitchAuthLabel(current.twitchAuthStatus, "announcement")}`,
      priority: "alert",
    };
  }

  if (
    previous.twitchConnectionStatus !== current.twitchConnectionStatus &&
    isConnectionError(current.twitchConnectionStatus)
  ) {
    if (current.twitchConnectionStatus === "authRequired") {
      // The auth status transition emits the recovery instruction. Avoid
      // announcing the same revocation once as a connection failure first.
      return undefined;
    }
    return {
      message: `Twitch 接続: ${getTwitchConnectionLabel(current.twitchConnectionStatus)}`,
      priority: "alert",
    };
  }

  if (
    previous.speechAdapterHealth !== current.speechAdapterHealth &&
    isSpeechError(current.speechAdapterHealth)
  ) {
    return {
      message: `棒読みちゃん: ${speechHealthLabels[current.speechAdapterHealth]}`,
      priority: "alert",
    };
  }

  if (previous.latestWarning !== current.latestWarning && current.latestWarning) {
    return { message: `警告: ${current.latestWarning}`, priority: "status" };
  }

  if (previous.twitchAuthStatus !== current.twitchAuthStatus) {
    return {
      message: `Twitch 認証: ${getTwitchAuthLabel(current.twitchAuthStatus, "announcement")}`,
      priority: "status",
    };
  }

  if (previous.twitchConnectionStatus !== current.twitchConnectionStatus) {
    if (current.twitchConnectionStatus === "authRequired") {
      return undefined;
    }
    return {
      message: `Twitch 接続: ${getTwitchConnectionLabel(current.twitchConnectionStatus)}`,
      priority: "status",
    };
  }

  if (previous.speechAdapterHealth !== current.speechAdapterHealth) {
    return {
      message: `棒読みちゃん: ${speechHealthLabels[current.speechAdapterHealth]}`,
      priority: "status",
    };
  }

  if (previous.speechQueuePhase !== current.speechQueuePhase) {
    return {
      message: `読み上げキュー: ${speechQueuePhaseLabels[current.speechQueuePhase]}`,
      priority: current.speechQueuePhase === "error" ? "alert" : "status",
    };
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
  }, [
    snapshot.twitchAuthStatus,
    snapshot.twitchConnectionStatus,
    snapshot.speechAdapterHealth,
    snapshot.speechQueuePhase,
    snapshot.latestWarning,
  ]);

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

function isSpeechError(status: SpeechAdapterHealth): boolean {
  return status === "disconnected" || status === "error";
}
