import type { SpeechStatus, TwitchStatusEvent } from "../types";

export interface SystemTimelineEvent {
  source: "twitch-auth" | "twitch-connection" | "speech";
  transition: string;
  message: string;
}

/** Suppresses repeated state until that source changes state. */
export class SystemTimelineRouter {
  private previousTransitions = new Map<SystemTimelineEvent["source"], string>();

  shouldRecord(event: SystemTimelineEvent): boolean {
    if (this.previousTransitions.get(event.source) === event.transition) return false;
    this.previousTransitions.set(event.source, event.transition);
    return true;
  }
}

export function timelineEventFromTwitchStatus(event: TwitchStatusEvent): SystemTimelineEvent | undefined {
  const message = event.message?.trim();
  if (!message || /keepalive/i.test(message)) return undefined;

  const source = event.domain === "auth" ? "twitch-auth" : "twitch-connection";
  const isOperational = source === "twitch-auth" || /EventSub|チャット受信|Twitch チャンネル|チャンネル .*接続/.test(message);
  return isOperational
    ? { source, transition: source === "twitch-auth" ? `${event.status}:${message}` : event.status, message }
    : undefined;
}

export function speechRecoveryTimelineEvent(message: string, status: SpeechStatus): SystemTimelineEvent {
  return { source: "speech", transition: status, message };
}

export function autoConnectTimelineEvent(transition: "started" | "failed", message: string): SystemTimelineEvent {
  return { source: "twitch-connection", transition: `auto-${transition}`, message };
}
