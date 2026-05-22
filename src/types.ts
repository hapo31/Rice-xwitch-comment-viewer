export type ViewId = "chat" | "queue" | "rules" | "voices" | "settings" | "logs";

export type AuthStatus = "unauthenticated" | "authenticated" | "expired" | "error";

export type SpeechStatus = "idle" | "speaking" | "paused" | "disconnected" | "error";

export type ChatDisplayState = "queued" | "spoken" | "skipped" | "blocked" | "error";

export interface AppSettings {
  twitch: {
    clientId: string;
    channelLogin: string;
    autoConnect: boolean;
  };
  speech: {
    adapter: "bouyomi";
    bouyomiHost: string;
    bouyomiPort: number;
    bouyomiSpeed: number;
    bouyomiTone: number;
    bouyomiVolume: number;
    bouyomiVoice: number;
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

export interface BouyomiConnectionDiagnostics {
  configuredAddr: string;
  attempted: BouyomiConnectionAttempt[];
  recommendation: string;
}

export interface BouyomiConnectionAttempt {
  addr: string;
  status: "connected" | "failed";
  message: string;
  elapsedMs: number;
}

export interface TwitchDeviceAuthStart {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export interface TwitchUserProfile {
  userId: string;
  login: string;
  clientId: string;
  scopes: string[];
  expiresIn: number;
}

export interface TwitchAuthValidationResult {
  profile: TwitchUserProfile;
  storageWarning?: string;
}

export type AppLogLevel = "info" | "warning" | "error";

export interface AppLogEvent {
  level: AppLogLevel;
  message: string;
  occurredAtMs: number;
}

export type TwitchConnectionStatus = "disconnected" | "connecting" | "connected" | "reconnecting" | "authRequired" | "error";

export interface TwitchStatusEvent {
  status: TwitchConnectionStatus;
  message?: string;
  occurredAtMs: number;
}

export interface SpeechStatusEvent {
  status: SpeechStatus;
  message?: string;
  occurredAtMs: number;
}

export interface SpeechQueueUpdatedEvent {
  queuedCount: number;
  warning?: string;
  occurredAtMs: number;
}

export type TwitchAuthPollResult =
  | { status: "pending"; message: string; interval: number }
  | { status: "slowDown"; message: string; interval: number }
  | { status: "authorized"; profile: TwitchUserProfile; storageWarning?: string }
  | { status: "denied"; message: string }
  | { status: "expired"; message: string };
