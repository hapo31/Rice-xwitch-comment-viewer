export type AuthStatus = "unauthenticated" | "authenticated" | "expired" | "error";

export type SpeechStatus = "idle" | "speaking" | "paused" | "disconnected" | "error";

export type ChatDisplayState = "queued" | "spoken" | "skipped" | "blocked" | "error";
export type QueueDisplayState = ChatDisplayState | "speaking";

export interface AppSettings {
  twitch: {
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
  platform?: "twitch";
  channelId?: string;
  channelLogin?: string;
  userId?: string;
  userLogin?: string;
  fragments?: TwitchMessageFragment[];
  badges?: TwitchChatBadge[];
}

export interface TwitchChatMessageEvent {
  id: string;
  platform: "twitch";
  channelId: string;
  channelLogin: string;
  userId: string;
  userLogin: string;
  userDisplayName: string;
  text: string;
  fragments: TwitchMessageFragment[];
  badges: TwitchChatBadge[];
  receivedAt: string;
}

export interface TwitchMessageFragment {
  type: string;
  text: string;
  emote?: TwitchChatEmote | null;
  cheermote?: TwitchChatCheermote | null;
}

export interface TwitchChatEmote {
  id: string;
  emoteSetId: string;
  ownerId?: string;
}

export interface TwitchChatCheermote {
  prefix: string;
  bits: number;
  tier: number;
}

export interface TwitchChatBadge {
  setId: string;
  id: string;
  info: string;
}

export interface QueueItem {
  id: string;
  sourceMessageId?: string;
  userDisplayName: string;
  text: string;
  status: QueueDisplayState;
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

export type TwitchChatConnectionStatus = "disconnected" | "connecting" | "connected" | "reconnecting" | "authRequired" | "error";

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
  items: QueueItem[];
  warning?: string;
  occurredAtMs: number;
}

export type TwitchAuthPollResult =
  | { status: "pending"; message: string; interval: number }
  | { status: "slowDown"; message: string; interval: number }
  | { status: "authorized"; profile: TwitchUserProfile; storageWarning?: string }
  | { status: "denied"; message: string }
  | { status: "expired"; message: string };
