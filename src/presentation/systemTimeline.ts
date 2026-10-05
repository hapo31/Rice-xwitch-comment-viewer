import type { SpeechStatus, TwitchStatusEvent } from "../types";
import type {
  AuthTimelineStatus,
  ChatTimelineStatus,
  SystemTimelineEvent,
} from "../models/systemTimeline";

/** Suppresses repeated state until that source changes state. */
export class SystemTimelineRouter {
  private previousTransitions = new Map<SystemTimelineEvent["source"], string>();

  shouldRecord(event: SystemTimelineEvent): boolean {
    // Auth can provide new guidance without changing its connection status.
    const key =
      event.source === "twitch-auth" ? `${event.transition}:${event.message}` : event.transition;
    if (this.previousTransitions.get(event.source) === key) return false;
    this.previousTransitions.set(event.source, key);
    return true;
  }
}

export function timelineEventFromTwitchStatus(
  event: TwitchStatusEvent,
): SystemTimelineEvent | undefined {
  const message = event.message?.trim();
  if (!message || /keepalive/i.test(message)) return undefined;

  if (event.domain === "auth") {
    return isAuthStatus(event.status)
      ? { source: "twitch-auth", transition: event.status, message }
      : undefined;
  }
  return isChatStatus(event.status)
    ? { source: "twitch-connection", transition: event.status, message }
    : undefined;
}

function isAuthStatus(status: TwitchStatusEvent["status"]): status is AuthTimelineStatus {
  return status !== "reconnecting";
}

function isChatStatus(status: TwitchStatusEvent["status"]): status is ChatTimelineStatus {
  return status !== "validating";
}

export function speechRecoveryTimelineEvent(
  message: string,
  status: SpeechStatus,
): SystemTimelineEvent {
  return { source: "speech", transition: status, message };
}

export function autoConnectTimelineEvent(
  transition: "started" | "failed",
  message: string,
): SystemTimelineEvent {
  return { source: "twitch-connection", transition: `auto-${transition}`, message };
}
