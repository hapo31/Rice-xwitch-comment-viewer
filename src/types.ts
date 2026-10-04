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

export const speechOutcomeReasonCodes = {
  blocked: ["repeatSuppressed", "blockedUser", "blockedWord", "blockedUrl", "emptyAfterFormatting"],
  skipped: ["overflow", "userSkip", "removed", "cleared"],
  error: [
    "configuration",
    "connectionRefused",
    "connectTimeout",
    "connectFailed",
    "connectionLost",
    "permissionDenied",
    "writeTimeout",
    "writeFailed",
    "responseTimeout",
    "responseFailed",
    "protocolMismatch",
    "unknown",
  ],
} as const;
export const speechRecoveryActions = [
  "reviewFilters",
  "reviewQueue",
  "diagnoseSpeech",
  "confirmDelivery",
  "none",
] as const;
export type SpeechRecoveryAction = (typeof speechRecoveryActions)[number];
type SpeechOutcomeDetails = {
  message: string;
  retryable: boolean;
  recoveryAction: SpeechRecoveryAction;
  occurredAtMs: number;
};
export type SpeechQueueOutcome = SpeechOutcomeDetails &
  (
    | { kind: "blocked"; reasonCode: (typeof speechOutcomeReasonCodes.blocked)[number] }
    | { kind: "skipped"; reasonCode: (typeof speechOutcomeReasonCodes.skipped)[number] }
    | { kind: "error"; reasonCode: (typeof speechOutcomeReasonCodes.error)[number] }
  );

export interface AppSettingsPatch {
  twitch?: Partial<AppSettings["twitch"]>;
  speech?: Partial<AppSettings["speech"]>;
  launcher?: { items?: LauncherItemEdit[] };
}

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
  window?: {
    position?: {
      x: number;
      y: number;
    };
  };
}

export interface SettingsRecoveryNotice {
  message: string;
}

export interface LauncherSettings {
  items: LauncherItem[];
}

/** Only metadata of existing items is editable; registration is backend-owned. */
export type LauncherItemEdit = Pick<
  LauncherItem,
  "id" | "displayName" | "backgroundColor" | "groupId" | "order"
>;

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
  speechOutcome?: SpeechQueueOutcome;
  speechQueueItemId?: string;
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
  connectionGeneration?: number;
}

export interface TwitchMessageFragment {
  type: string;
  text: string;
  emote?: TwitchChatEmote;
  cheermote?: TwitchChatCheermote;
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
  outcome?: SpeechQueueOutcome;
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

export type TwitchConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "validating"
  | "reconnecting"
  | "authRequired"
  | "error";

export type TwitchAuthRequiredReason = "missingRequiredScope";
export type TwitchStatusDomain = "auth" | "chat";

export interface TwitchActiveConnection {
  generation: number;
  broadcasterUserId: string;
  broadcasterLogin: string;
}

export type TwitchChatConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "authRequired"
  | "error";

export interface TwitchStatusEvent {
  revision?: number;
  domain: TwitchStatusDomain;
  status: TwitchConnectionStatus;
  reason?: TwitchAuthRequiredReason;
  connectionGeneration?: number;
  activeConnection?: TwitchActiveConnection;
  message?: string;
  occurredAtMs: number;
}

export interface SpeechStatusEvent {
  revision?: number;
  status: SpeechStatus;
  adapterHealth?: SpeechAdapterHealth;
  message?: string;
  occurredAtMs: number;
}

export interface SpeechQueueUpdatedEvent {
  revision?: number;
  queuedCount: number;
  items: QueueItem[];
  phase?: SpeechQueuePhase;
  warning?: string;
  occurredAtMs: number;
}

export type SpeechQueuePhase = "idle" | "speaking" | "paused" | "error";
export type SpeechAdapterHealth = "unknown" | "connected" | "disconnected" | "error";

export interface AppEventsSnapshot {
  revision: number;
  logs: AppLogEvent[];
  twitchStatuses: TwitchStatusEvent[];
  speechStatus?: SpeechStatusEvent;
  emitErrors: AppEventEmitError[];
}

export interface AppEventEmitError {
  id: string;
  event: string;
  error: string;
  occurredAtMs: number;
}

export interface SpeechStateSnapshot {
  revision: number;
  status: SpeechStatusEvent;
  queue: SpeechQueueUpdatedEvent;
}

export type TwitchAuthPollResult =
  | { status: "pending"; message: string; interval: number }
  | { status: "slowDown"; message: string; interval: number }
  | {
      status: "authorized";
      profile: TwitchUserProfile;
      storageWarning?: string;
    }
  | { status: "denied"; message: string }
  | { status: "expired"; message: string };
export interface LauncherCapabilities {
  canRegisterApplications: boolean;
  canLaunchApplications: boolean;
  reason?: string;
}
