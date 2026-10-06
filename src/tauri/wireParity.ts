import type { z } from "zod";
import type { WireContracts } from "../../bindings/wire";
import * as schemas from "./schemas";

// Every generated wire DTO must have a schema, with no extra manual DTO model.
export const wireSchemas = {
  AppBuildInfo: schemas.appBuildInfoSchema,
  AppEventEmitError: schemas.appEventEmitErrorSchema,
  AppEventsSnapshot: schemas.appEventsSnapshotWireSchema,
  AppLogEvent: schemas.appLogEventSchema,
  AppLogLevel: schemas.appLogLevelSchema,
  AppSettings: schemas.appSettingsWireSchema,
  BlockedReason: schemas.blockedReasonSchema,
  BouyomiConnectionAttempt: schemas.bouyomiConnectionAttemptSchema,
  BouyomiConnectionDiagnostics: schemas.bouyomiConnectionDiagnosticsSchema,
  BouyomiConnectionStatus: schemas.bouyomiConnectionAttemptSchema.shape.status,
  ChatBadge: schemas.twitchChatBadgeSchema,
  ChatCheermote: schemas.twitchChatCheermoteSchema,
  ChatEmote: schemas.twitchChatEmoteSchema,
  ChatMessage: schemas.twitchChatMessageWireSchema,
  FailureCode: schemas.failureCodeSchema,
  LauncherAddResult: schemas.launcherAddResultSchema,
  LauncherCapabilities: schemas.launcherCapabilitiesSchema,
  LauncherItem: schemas.launcherItemSchema,
  LauncherItemKind: schemas.launcherItemSchema.shape.kind,
  LauncherLaunchFailure: schemas.launcherLaunchFailureSchema,
  LauncherLaunchResult: schemas.launcherLaunchResultSchema,
  LauncherSettings: schemas.launcherSettingsSchema,
  MessageFragment: schemas.twitchMessageFragmentSchema,
  Platform: schemas.twitchChatMessageWireSchema.shape.platform,
  RecoveryAction: schemas.recoveryActionSchema,
  SettingsRecoveryNotice: schemas.settingsRecoveryNoticeSchema,
  SkippedReason: schemas.skippedReasonSchema,
  SpeechAdapterHealth: schemas.speechAdapterHealthSchema,
  SpeechAdapterKind: schemas.speechSettingsSchema.shape.adapter,
  SpeechQueueItemEvent: schemas.queueItemWireSchema,
  SpeechQueueItemStatus: schemas.queueDisplayStateSchema,
  SpeechQueueOutcome: schemas.speechQueueOutcomeWireSchema,
  SpeechQueuePhase: schemas.speechQueuePhaseSchema,
  SpeechQueueUpdatedEvent: schemas.speechQueueUpdatedWireSchema,
  SpeechSettings: schemas.speechSettingsSchema,
  SpeechStateSnapshot: schemas.speechStateSnapshotWireSchema,
  SpeechStatus: schemas.speechStatusSchema,
  SpeechStatusEvent: schemas.speechStatusWireSchema,
  TwitchActiveConnection: schemas.twitchActiveConnectionSchema,
  TwitchAuthPollResult: schemas.twitchAuthPollResultSchema,
  TwitchAuthRequiredReason: schemas.twitchAuthRequiredReasonSchema,
  TwitchAuthValidationResult: schemas.twitchAuthValidationResultSchema,
  TwitchDeviceAuthStart: schemas.twitchDeviceAuthStartSchema,
  TwitchSettings: schemas.twitchSettingsSchema,
  TwitchStatus: schemas.twitchConnectionStatusSchema,
  TwitchStatusDomain: schemas.twitchStatusWireSchema.shape.domain,
  TwitchStatusEvent: schemas.twitchStatusWireSchema,
  TwitchUserProfile: schemas.twitchUserProfileSchema,
  UrlHandling: schemas.speechSettingsSchema.shape.urlHandling,
  WindowPosition: schemas.windowPositionSchema,
  WindowSettings: schemas.windowSettingsSchema,
} satisfies Record<keyof WireContracts, z.ZodType>;

type SameShape<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
type AssertAll<T extends Record<keyof WireContracts, true>> = T;
export type WireSchemasMatchRust = AssertAll<{
  [K in keyof WireContracts]: SameShape<z.output<(typeof wireSchemas)[K]>, WireContracts[K]>;
}>;
