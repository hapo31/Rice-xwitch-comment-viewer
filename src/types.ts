import type { UtcTimestamp } from "./time";

export type AuthStatus =
  | "unauthenticated"
  | "authorizing"
  | "polling"
  | "checking"
  | "authenticated"
  | "expired"
  | "disconnecting"
  | "error";

export type SpeechStatus = "idle" | "speaking" | "paused" | "disconnected" | "error";

export type ChatDisplayState = "queued" | "spoken" | "skipped" | "blocked" | "error";
export type QueueDisplayState = ChatDisplayState | "speaking";

export interface AppSettings {
  twitch: {
    channelLogin: string;
    autoConnect: boolean;
    confirmBeforeStopChat: boolean;
    liveChatAnnouncements: boolean;
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
    autoSpeak: boolean;
    maxCommentLength: number;
    repeatSuppressionSeconds: number;
    blockedUsers: string[];
    blockedWords: string[];
    urlHandling: "replace" | "read" | "block";
    readEmotes: boolean;
    connectionSuccessSpeechEnabled: boolean;
    connectionSuccessSpeechText: string;
  };
  launcher: LauncherSettings;
}

export interface SettingsRecoveryNotice {
  message: string;
}

export interface LauncherSettings {
  items: LauncherItem[];
}

/**
 * `website` is reserved for the planned URL launcher support. The backend
 * currently creates and launches `application` items only.
 */
export type LauncherItemKind = "application" | "website";

export interface LauncherItem {
  id: string;
  kind: LauncherItemKind;
  target: string;
  displayName: string;
  iconDataUrl?: string;
  backgroundColor?: string;
  groupId?: string;
  order: number;
}

export interface LauncherLaunchFailure {
  itemId: string;
  displayName: string;
  message: string;
}

export interface LauncherLaunchResult {
  launchedCount: number;
  failures: LauncherLaunchFailure[];
}

export interface UserChatMessage {
  kind: "user";
  id: string;
  receivedAt: UtcTimestamp;
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

export interface SystemChatMessage {
  kind: "system";
  id: string;
  receivedAt: UtcTimestamp;
  userDisplayName: "system";
  text: string;
}

export type ChatMessage = UserChatMessage | SystemChatMessage;

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
  receivedAt: UtcTimestamp;
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
  expiresAtMs: number;
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
  id?: string;
  level: AppLogLevel;
  message: string;
  occurredAtMs: number;
}

export type NotificationSeverity = "info" | "success" | "warning" | "error";
export type NotificationSource = "command" | "event" | "log" | "system";

export interface AppNotification {
  id: string;
  severity: NotificationSeverity;
  source: NotificationSource;
  message: string;
  occurredAtMs: number;
  correlationId?: string;
}

export type TwitchConnectionStatus = "disconnected" | "connecting" | "connected" | "reconnecting" | "authRequired" | "error";

export type TwitchAuthRequiredReason = "missingRequiredScope";
export type TwitchStatusDomain = "auth" | "chat";

export type TwitchChatConnectionStatus = "disconnected" | "connecting" | "connected" | "reconnecting" | "authRequired" | "error";

export interface TwitchStatusEvent {
  domain: TwitchStatusDomain;
  status: TwitchConnectionStatus;
  reason?: TwitchAuthRequiredReason;
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
