export type ViewId = "chat" | "queue" | "rules" | "voices" | "settings" | "logs";

export type AuthStatus = "unauthenticated" | "authenticated" | "expired" | "error";

export type SpeechStatus = "idle" | "speaking" | "paused" | "disconnected" | "error";

export type ChatDisplayState = "queued" | "spoken" | "skipped" | "blocked" | "error";

export interface AppSettings {
  twitch: {
    channelLogin: string;
    autoConnect: boolean;
  };
  speech: {
    adapter: "bouyomi";
    bouyomiHost: string;
    bouyomiPort: number;
    readUserName: boolean;
    maxCommentLength: number;
    repeatSuppressionSeconds: number;
  };
}

export interface ChatMessage {
  id: string;
  receivedAt: string;
  userDisplayName: string;
  text: string;
  status: ChatDisplayState;
}

export interface QueueItem {
  id: string;
  sourceMessageId?: string;
  text: string;
  status: ChatDisplayState;
}
