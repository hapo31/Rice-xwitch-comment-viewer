import type { SpeechStatus, TwitchStatusEvent } from "../types";

export type AuthTimelineStatus = Exclude<TwitchStatusEvent["status"], "reconnecting">;
export type ChatTimelineStatus = Exclude<TwitchStatusEvent["status"], "validating">;

/** Operational Chat events retain the source-specific deduplication key. */
export type SystemTimelineEvent =
  | {
      source: "twitch-auth";
      transition: AuthTimelineStatus;
      message: string;
    }
  | {
      source: "twitch-connection";
      transition: ChatTimelineStatus | "auto-started" | "auto-failed";
      message: string;
    }
  | {
      source: "speech";
      transition: SpeechStatus;
      message: string;
    };
