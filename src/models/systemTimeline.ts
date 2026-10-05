import type { SpeechStatus, TwitchStatusEvent } from "../types";

/** Operational Chat events retain the source-specific deduplication key. */
export type SystemTimelineEvent =
  | {
      source: "twitch-auth";
      transition: `${TwitchStatusEvent["status"]}:${string}`;
      message: string;
    }
  | {
      source: "twitch-connection";
      transition: TwitchStatusEvent["status"] | "auto-started" | "auto-failed";
      message: string;
    }
  | {
      source: "speech";
      transition: SpeechStatus;
      message: string;
    };
