import type { SpeechStatus, TwitchStatusEvent } from "../types";
import type { SystemTimelineEvent } from "../models/systemTimeline";

/** Suppresses repeated state until that source changes state. */
export class SystemTimelineRouter {
  private previousTransitions = new Map<SystemTimelineEvent["source"], string>();

  shouldRecord(event: SystemTimelineEvent): boolean {
    if (this.previousTransitions.get(event.source) === event.transition) return false;
    this.previousTransitions.set(event.source, event.transition);
    return true;
  }
}

export function timelineEventFromTwitchStatus(
  event: TwitchStatusEvent,
): SystemTimelineEvent | undefined {
  const message = event.message?.trim();
  if (!message || /keepalive/i.test(message)) return undefined;

  return event.domain === "auth"
    ? { source: "twitch-auth", transition: `${event.status}:${message}`, message }
    : { source: "twitch-connection", transition: event.status, message };
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
