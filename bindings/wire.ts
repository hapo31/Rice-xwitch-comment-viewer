// Generated from Rust Serde DTOs by ts-rs. Do not edit.
// Regenerate: RICE_UPDATE_WIRE_TYPES=1 cargo test --manifest-path src-tauri/Cargo.toml --no-default-features generated_wire_contracts_are_current

export type AppBuildInfo = { version: string, isDev: boolean, launcher: LauncherCapabilities, commitHash?: string, };
export type AppEventEmitError = { id: string, event: string, error: string, occurredAtMs: number, };
export type AppEventsSnapshot = { revision: number, logs: Array<AppLogEvent>, twitchStatuses: Array<TwitchStatusEvent>, speechStatus?: SpeechStatusEvent, emitErrors: Array<AppEventEmitError>, };
export type AppLogEvent = { id?: string, level: AppLogLevel, message: string, occurredAtMs: number, };
export type AppLogLevel = "info" | "warning" | "error";
export type AppSettings = { twitch: TwitchSettings, speech: SpeechSettings, launcher: LauncherSettings, window: WindowSettings, };
export type BlockedReason = "repeatSuppressed" | "blockedUser" | "blockedWord" | "blockedUrl" | "emptyAfterFormatting";
export type BouyomiConnectionAttempt = { addr: string, status: BouyomiConnectionStatus, message: string, elapsedMs: number, };
export type BouyomiConnectionDiagnostics = { configuredAddr: string, attempted: Array<BouyomiConnectionAttempt>, recommendation: string, };
export type BouyomiConnectionStatus = "connected" | "failed";
export type ChatBadge = { setId: string, id: string, info: string, };
export type ChatCheermote = { prefix: string, bits: number, tier: number, };
export type ChatEmote = { id: string, emoteSetId: string, ownerId?: string, };
export type ChatMessage = { id: string, platform: Platform, channelId: string, channelLogin: string, userId: string, userLogin: string, userDisplayName: string, text: string, fragments: Array<MessageFragment>, badges: Array<ChatBadge>, receivedAt: string, connectionGeneration?: number, };
export type FailureCode = "configuration" | "connectionRefused" | "connectTimeout" | "connectFailed" | "connectionLost" | "permissionDenied" | "writeTimeout" | "writeFailed" | "responseTimeout" | "responseFailed" | "protocolMismatch" | "unknown";
export type LauncherAddResult = { items: Array<LauncherItem>, addedCount: number, };
export type LauncherCapabilities = { canRegisterApplications: boolean, canLaunchApplications: boolean, reason?: string, };
export type LauncherItem = { id: string, kind: LauncherItemKind, target: string, displayName: string, iconDataUrl?: string, backgroundColor?: string, groupId?: string, order: number, };
export type LauncherItemKind = "application" | "website";
export type LauncherLaunchFailure = { itemId: string, displayName: string, message: string, };
export type LauncherLaunchResult = {
/**
 * Verified target process creation only; not shell acceptance/app readiness.
 */
launchedCount: number, failures: Array<LauncherLaunchFailure>, };
export type LauncherSettings = { items: Array<LauncherItem>, };
export type MessageFragment = { type: string, text: string, emote?: ChatEmote, cheermote?: ChatCheermote, };
export type Platform = "twitch";
export type RecoveryAction = "reviewFilters" | "reviewQueue" | "diagnoseSpeech" | "confirmDelivery" | "none";
export type SettingsRecoveryNotice = { message: string, };
export type SkippedReason = "overflow" | "userSkip" | "removed" | "cleared";
export type SpeechAdapterHealth = "unknown" | "connected" | "disconnected" | "error";
export type SpeechAdapterKind = "bouyomi";
export type SpeechQueueItemEvent = { id: string, sourceMessageId?: string, userDisplayName: string, text: string, status: SpeechQueueItemStatus, outcome?: SpeechQueueOutcome, };
export type SpeechQueueItemStatus = "queued" | "speaking" | "spoken" | "skipped" | "blocked" | "error";
export type SpeechQueueOutcome = { "kind": "blocked", reasonCode: BlockedReason, message: string,
/**
 * Failure category permits a new safe attempt, not an unused retry budget.
 * Terminal error history is never automatically resent.
 */
retryable: boolean, recoveryAction: RecoveryAction, occurredAtMs: number, } | { "kind": "skipped", reasonCode: SkippedReason, message: string,
/**
 * Failure category permits a new safe attempt, not an unused retry budget.
 * Terminal error history is never automatically resent.
 */
retryable: boolean, recoveryAction: RecoveryAction, occurredAtMs: number, } | { "kind": "error", reasonCode: FailureCode, message: string,
/**
 * Failure category permits a new safe attempt, not an unused retry budget.
 * Terminal error history is never automatically resent.
 */
retryable: boolean, recoveryAction: RecoveryAction, occurredAtMs: number, };
export type SpeechQueuePhase = "idle" | "speaking" | "paused" | "error";
export type SpeechQueueUpdatedEvent = { revision: number, queuedCount: number, items: Array<SpeechQueueItemEvent>, phase: SpeechQueuePhase, warning?: string, occurredAtMs: number, };
export type SpeechSettings = { adapter: SpeechAdapterKind, bouyomiHost: string, bouyomiPort: number,
/**
 * Opt-in request only. Native consent is never persisted in settings.
 */
bouyomiRemoteMode: boolean, bouyomiSpeed: number, bouyomiTone: number, bouyomiVolume: number, bouyomiVoice: number, readUserName: boolean, autoSpeak: boolean, maxCommentLength: number, repeatSuppressionSeconds: number, blockedUsers: Array<string>, blockedWords: Array<string>, urlHandling: UrlHandling, readEmotes: boolean, connectionSuccessSpeechEnabled: boolean, connectionSuccessSpeechText: string, };
export type SpeechStateSnapshot = { revision: number, status: SpeechStatusEvent, queue: SpeechQueueUpdatedEvent, };
export type SpeechStatus = "idle" | "speaking" | "paused" | "disconnected" | "error";
export type SpeechStatusEvent = { revision: number, status: SpeechStatus, adapterHealth: SpeechAdapterHealth, message?: string, occurredAtMs: number, };
export type TwitchActiveConnection = { generation: number, broadcasterUserId: string, broadcasterLogin: string, };
export type TwitchAuthPollResult = { "status": "pending", message: string, interval: number, } | { "status": "slowDown", message: string, interval: number, } | { "status": "authorized", profile: TwitchUserProfile, storageWarning?: string, } | { "status": "denied", message: string, } | { "status": "expired", message: string, };
export type TwitchAuthRequiredReason = "missingRequiredScope";
export type TwitchAuthValidationResult = { profile: TwitchUserProfile, storageWarning?: string, };
export type TwitchDeviceAuthStart = { userCode: string, verificationUri: string, expiresIn: number, expiresAtMs: number, interval: number, };
export type TwitchSettings = { channelLogin: string, autoConnect: boolean, confirmBeforeStopChat: boolean, liveChatAnnouncements: boolean, };
export type TwitchStatus = "disconnected" | "connecting" | "connected" | "validating" | "reconnecting" | "authRequired" | "error";
export type TwitchStatusDomain = "auth" | "chat";
export type TwitchStatusEvent = { revision: number, domain: TwitchStatusDomain, status: TwitchStatus, reason?: TwitchAuthRequiredReason, connectionGeneration?: number, activeConnection?: TwitchActiveConnection, message?: string, occurredAtMs: number, };
export type TwitchUserProfile = { userId: string, login: string, scopes: Array<string>, expiresIn: number, };
export type UrlHandling = "replace" | "read" | "block";
export type WindowPosition = { x: number, y: number, };
export type WindowSettings = { position?: WindowPosition, };

export type WireContracts = {
  AppBuildInfo: AppBuildInfo;
  AppEventEmitError: AppEventEmitError;
  AppEventsSnapshot: AppEventsSnapshot;
  AppLogEvent: AppLogEvent;
  AppLogLevel: AppLogLevel;
  AppSettings: AppSettings;
  BlockedReason: BlockedReason;
  BouyomiConnectionAttempt: BouyomiConnectionAttempt;
  BouyomiConnectionDiagnostics: BouyomiConnectionDiagnostics;
  BouyomiConnectionStatus: BouyomiConnectionStatus;
  ChatBadge: ChatBadge;
  ChatCheermote: ChatCheermote;
  ChatEmote: ChatEmote;
  ChatMessage: ChatMessage;
  FailureCode: FailureCode;
  LauncherAddResult: LauncherAddResult;
  LauncherCapabilities: LauncherCapabilities;
  LauncherItem: LauncherItem;
  LauncherItemKind: LauncherItemKind;
  LauncherLaunchFailure: LauncherLaunchFailure;
  LauncherLaunchResult: LauncherLaunchResult;
  LauncherSettings: LauncherSettings;
  MessageFragment: MessageFragment;
  Platform: Platform;
  RecoveryAction: RecoveryAction;
  SettingsRecoveryNotice: SettingsRecoveryNotice;
  SkippedReason: SkippedReason;
  SpeechAdapterHealth: SpeechAdapterHealth;
  SpeechAdapterKind: SpeechAdapterKind;
  SpeechQueueItemEvent: SpeechQueueItemEvent;
  SpeechQueueItemStatus: SpeechQueueItemStatus;
  SpeechQueueOutcome: SpeechQueueOutcome;
  SpeechQueuePhase: SpeechQueuePhase;
  SpeechQueueUpdatedEvent: SpeechQueueUpdatedEvent;
  SpeechSettings: SpeechSettings;
  SpeechStateSnapshot: SpeechStateSnapshot;
  SpeechStatus: SpeechStatus;
  SpeechStatusEvent: SpeechStatusEvent;
  TwitchActiveConnection: TwitchActiveConnection;
  TwitchAuthPollResult: TwitchAuthPollResult;
  TwitchAuthRequiredReason: TwitchAuthRequiredReason;
  TwitchAuthValidationResult: TwitchAuthValidationResult;
  TwitchDeviceAuthStart: TwitchDeviceAuthStart;
  TwitchSettings: TwitchSettings;
  TwitchStatus: TwitchStatus;
  TwitchStatusDomain: TwitchStatusDomain;
  TwitchStatusEvent: TwitchStatusEvent;
  TwitchUserProfile: TwitchUserProfile;
  UrlHandling: UrlHandling;
  WindowPosition: WindowPosition;
  WindowSettings: WindowSettings;
};
