import type { z } from "zod";
import * as schemas from "./tauri/schemas";
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

export type SpeechStatus = z.infer<typeof schemas.speechStatusSchema>;

export type ChatDisplayState = Exclude<QueueDisplayState, "speaking">;
export type QueueDisplayState = z.infer<typeof schemas.queueDisplayStateSchema>;

export const speechOutcomeReasonCodes = {
  blocked: schemas.blockedReasonSchema.options,
  skipped: schemas.skippedReasonSchema.options,
  error: schemas.failureCodeSchema.options,
} as const;
export const speechRecoveryActions = schemas.recoveryActionSchema.options;
export const retryableSpeechReasons = schemas.retryableSpeechReasonSchema.options;
export const diagnosableNonRetryableSpeechReasons =
  schemas.diagnosableNonRetryableSpeechReasonSchema.options;
export const twitchStatusesByDomain = {
  auth: schemas.twitchAuthConnectionStatusSchema.options,
  chat: schemas.twitchChatConnectionStatusSchema.options,
} as const;
export type TwitchAuthConnectionStatus = z.infer<typeof schemas.twitchAuthConnectionStatusSchema>;
export type SpeechRecoveryAction = z.infer<typeof schemas.recoveryActionSchema>;
export type SpeechQueueOutcome = z.infer<typeof schemas.speechQueueOutcomeSchema>;

export interface AppSettingsPatch {
  twitch?: Partial<AppSettings["twitch"]>;
  speech?: Partial<AppSettings["speech"]>;
  launcher?: { items?: LauncherItemEdit[] };
}

export type AppSettings = z.infer<typeof schemas.appSettingsSchema>;

export type SettingsRecoveryNotice = z.infer<typeof schemas.settingsRecoveryNoticeSchema>;

export type LauncherSettings = z.infer<typeof schemas.launcherSettingsSchema>;

/** Only metadata of existing items is editable; registration is backend-owned. */
export type LauncherItemEdit = Pick<
  LauncherItem,
  "id" | "displayName" | "backgroundColor" | "groupId" | "order"
>;

/**
 * `website` is reserved for the planned URL launcher support. The backend
 * currently creates and launches `application` items only.
 */
export type LauncherItemKind = LauncherItem["kind"];

export type LauncherItem = z.infer<typeof schemas.launcherItemSchema>;

export type LauncherAddResult = z.infer<typeof schemas.launcherAddResultSchema>;

export type LauncherLaunchFailure = z.infer<typeof schemas.launcherLaunchFailureSchema>;

export type LauncherLaunchResult = z.infer<typeof schemas.launcherLaunchResultSchema>;

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

export type TwitchChatMessageEvent = Omit<
  z.infer<typeof schemas.twitchChatMessageWireSchema>,
  "receivedAt"
> & { receivedAt: UtcTimestamp };

export type TwitchMessageFragment = z.infer<typeof schemas.twitchMessageFragmentSchema>;

export type TwitchChatEmote = z.infer<typeof schemas.twitchChatEmoteSchema>;

export type TwitchChatCheermote = z.infer<typeof schemas.twitchChatCheermoteSchema>;

export type TwitchChatBadge = z.infer<typeof schemas.twitchChatBadgeSchema>;

export type QueueItem = z.infer<typeof schemas.queueItemSchema>;

export type BouyomiConnectionDiagnostics = z.infer<
  typeof schemas.bouyomiConnectionDiagnosticsSchema
>;

export type BouyomiConnectionAttempt = z.infer<typeof schemas.bouyomiConnectionAttemptSchema>;

export type TwitchDeviceAuthStart = z.infer<typeof schemas.twitchDeviceAuthStartSchema>;

export type TwitchUserProfile = z.infer<typeof schemas.twitchUserProfileSchema>;

export type TwitchAuthValidationResult = z.infer<typeof schemas.twitchAuthValidationResultSchema>;

export type AppLogLevel = z.infer<typeof schemas.appLogLevelSchema>;

export type AppLogEvent = z.infer<typeof schemas.appLogEventSchema>;

export type NotificationSeverity = "info" | "success" | "warning" | "error";
export type NotificationSource = "command" | "event" | "log" | "system";

export type NotificationStatusDomain = "auth" | "chat" | "speech" | "queue";

export interface AppNotification {
  id: string;
  severity: NotificationSeverity;
  source: NotificationSource;
  message: string;
  occurredAtMs: number;
  correlationId?: string;
  /** Status summaries describing the same occurrence as this actionable notice. */
  announcementDomains?: NotificationStatusDomain[];
}

export type TwitchConnectionStatus = z.infer<typeof schemas.twitchConnectionStatusSchema>;

export type TwitchAuthRequiredReason = z.infer<typeof schemas.twitchAuthRequiredReasonSchema>;
export type TwitchStatusDomain = z.infer<typeof schemas.twitchStatusWireSchema>["domain"];

export type TwitchActiveConnection = z.infer<typeof schemas.twitchActiveConnectionSchema>;

export type TwitchChatConnectionStatus = Exclude<TwitchConnectionStatus, "validating">;

export type TwitchStatusEvent = z.infer<typeof schemas.twitchStatusSchema>;

export type SpeechStatusEvent = Omit<
  z.infer<typeof schemas.speechStatusEventSchema>,
  "adapterHealth"
> & { adapterHealth?: SpeechAdapterHealth };

export type SpeechQueueUpdatedEvent = z.infer<typeof schemas.speechQueueUpdatedSchema>;

export type SpeechQueuePhase = z.infer<typeof schemas.speechQueuePhaseSchema>;
export type SpeechAdapterHealth = z.infer<typeof schemas.speechAdapterHealthSchema>;

export type AppEventsSnapshot = z.infer<typeof schemas.appEventsSnapshotSchema>;

export type AppEventEmitError = z.infer<typeof schemas.appEventEmitErrorSchema>;

export type SpeechStateSnapshot = z.infer<typeof schemas.speechStateSnapshotSchema>;

export type TwitchAuthPollResult = z.infer<typeof schemas.twitchAuthPollResultSchema>;
export type LauncherCapabilities = z.infer<typeof schemas.launcherCapabilitiesSchema>;
