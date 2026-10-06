import { z } from "zod";

// Rust JSON integers must remain exact when transported through JavaScript.
export const unsignedInteger = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const timestamp = unsignedInteger.max(8_640_000_000_000_000);
const positiveInteger = unsignedInteger.min(1);
const u32 = unsignedInteger.max(4_294_967_295);
const text = z.string();
const nonempty = text.min(1);

export const launcherCapabilitiesSchema = z.object({
  canRegisterApplications: z.boolean(),
  canLaunchApplications: z.boolean(),
  reason: text.optional(),
});
export const appBuildInfoSchema = z.object({
  version: nonempty,
  isDev: z.boolean(),
  launcher: launcherCapabilitiesSchema,
  commitHash: text.optional(),
});
export const launcherItemSchema = z.object({
  id: nonempty,
  kind: z.enum(["application", "website"]),
  target: nonempty,
  displayName: text,
  iconDataUrl: text.optional(),
  backgroundColor: text.optional(),
  groupId: text.optional(),
  order: u32,
});
export const launcherSettingsSchema = z.object({ items: z.array(launcherItemSchema) });
export const launcherAddResultSchema = z.object({
  items: z.array(launcherItemSchema),
  addedCount: unsignedInteger,
});
export const launcherLaunchFailureSchema = z.object({
  itemId: text,
  displayName: text,
  message: text,
});
export const launcherLaunchResultSchema = z.object({
  launchedCount: unsignedInteger,
  failures: z.array(launcherLaunchFailureSchema),
});
export const twitchSettingsSchema = z.object({
  channelLogin: text,
  autoConnect: z.boolean(),
  confirmBeforeStopChat: z.boolean(),
  liveChatAnnouncements: z.boolean(),
});
export const speechSettingsSchema = z.object({
  adapter: z.literal("bouyomi"),
  bouyomiHost: nonempty,
  bouyomiPort: positiveInteger.max(65535),
  bouyomiRemoteMode: z.boolean(),
  bouyomiSpeed: z.number().int().min(-1).max(300),
  bouyomiTone: z.number().int().min(-1).max(200),
  bouyomiVolume: z.number().int().min(-1).max(100),
  bouyomiVoice: unsignedInteger.max(30000),
  readUserName: z.boolean(),
  autoSpeak: z.boolean(),
  maxCommentLength: positiveInteger.max(500),
  repeatSuppressionSeconds: unsignedInteger.max(30),
  blockedUsers: z.array(text).max(200),
  blockedWords: z.array(text).max(200),
  urlHandling: z.enum(["replace", "read", "block"]),
  readEmotes: z.boolean(),
  connectionSuccessSpeechEnabled: z.boolean(),
  connectionSuccessSpeechText: text,
});
export const windowPositionSchema = z.object({
  x: z.number().int().min(-2_147_483_648).max(2_147_483_647),
  y: z.number().int().min(-2_147_483_648).max(2_147_483_647),
});
export const windowSettingsSchema = z.object({ position: windowPositionSchema.optional() });
export const appSettingsWireSchema = z.object({
  twitch: twitchSettingsSchema,
  speech: speechSettingsSchema,
  launcher: launcherSettingsSchema,
  window: windowSettingsSchema,
});
// Older settings snapshots predate window persistence and remote consent.
// Only those documented additions may be absent; core sections stay required.
export const appSettingsSchema = appSettingsWireSchema.extend({
  speech: speechSettingsSchema.extend({ bouyomiRemoteMode: z.boolean().optional() }),
  window: windowSettingsSchema.optional(),
});
export const settingsRecoveryNoticeSchema = z.object({ message: text });
export const bouyomiConnectionAttemptSchema = z.object({
  addr: text,
  status: z.enum(["connected", "failed"]),
  message: text,
  elapsedMs: unsignedInteger,
});
export const bouyomiConnectionDiagnosticsSchema = z.object({
  configuredAddr: text,
  attempted: z.array(bouyomiConnectionAttemptSchema),
  recommendation: text,
});
export const twitchDeviceAuthStartSchema = z.object({
  userCode: nonempty,
  verificationUri: z.url({ protocol: /^https$/ }),
  expiresIn: positiveInteger,
  expiresAtMs: timestamp.min(1),
  interval: positiveInteger,
});
export const twitchUserProfileSchema = z.object({
  userId: nonempty,
  login: nonempty,
  scopes: z.array(text),
  expiresIn: unsignedInteger,
});
export const twitchAuthValidationResultSchema = z.object({
  profile: twitchUserProfileSchema,
  storageWarning: text.optional(),
});
export const twitchAuthPollResultSchema = z.discriminatedUnion("status", [
  z.strictObject({ status: z.literal("pending"), message: text, interval: positiveInteger }),
  z.strictObject({ status: z.literal("slowDown"), message: text, interval: positiveInteger }),
  z.strictObject({
    status: z.literal("authorized"),
    profile: twitchUserProfileSchema,
    storageWarning: text.optional(),
  }),
  z.strictObject({ status: z.literal("denied"), message: text }),
  z.strictObject({ status: z.literal("expired"), message: text }),
]);
export const twitchChatEmoteSchema = z.object({
  id: text,
  emoteSetId: text,
  ownerId: text.optional(),
});
export const twitchChatCheermoteSchema = z.object({ prefix: text, bits: u32, tier: u32 });
export const twitchMessageFragmentSchema = z.object({
  type: text,
  text,
  emote: twitchChatEmoteSchema.optional(),
  cheermote: twitchChatCheermoteSchema.optional(),
});
export const twitchChatBadgeSchema = z.object({ setId: text, id: text, info: text });
export const twitchChatMessageWireSchema = z.object({
  id: text,
  platform: z.literal("twitch"),
  channelId: text,
  channelLogin: text,
  userId: text,
  userLogin: text,
  userDisplayName: text,
  text,
  fragments: z.array(twitchMessageFragmentSchema),
  badges: z.array(twitchChatBadgeSchema),
  receivedAt: text,
  connectionGeneration: unsignedInteger.optional(),
});
// Timestamp normalization remains a domain conversion after shape validation.
export const twitchChatMessageInputSchema = twitchChatMessageWireSchema.extend({
  receivedAt: z.unknown().optional(),
});
export const twitchConnectionStatusSchema = z.enum([
  "disconnected",
  "connecting",
  "connected",
  "validating",
  "reconnecting",
  "authRequired",
  "error",
]);
export const twitchAuthRequiredReasonSchema = z.literal("missingRequiredScope");
export const twitchActiveConnectionSchema = z.object({
  generation: unsignedInteger,
  broadcasterUserId: text,
  broadcasterLogin: text,
});
export const twitchStatusWireSchema = z.object({
  revision: unsignedInteger,
  domain: z.enum(["auth", "chat"]),
  status: twitchConnectionStatusSchema,
  reason: twitchAuthRequiredReasonSchema.optional(),
  connectionGeneration: unsignedInteger.optional(),
  activeConnection: twitchActiveConnectionSchema.optional(),
  message: text.optional(),
  occurredAtMs: timestamp,
});
export const twitchAuthConnectionStatusSchema = twitchConnectionStatusSchema.exclude([
  "reconnecting",
]);
export const twitchChatConnectionStatusSchema = twitchConnectionStatusSchema.exclude([
  "validating",
]);
const twitchStatusDetailsSchema = twitchStatusWireSchema
  .pick({ revision: true, message: true, occurredAtMs: true })
  .extend({ revision: unsignedInteger.optional() });
const twitchAuthStatusDetailsSchema = twitchStatusDetailsSchema.extend({
  domain: z.literal("auth"),
  connectionGeneration: z.never().optional(),
  activeConnection: z.never().optional(),
});
export const twitchStatusSchema = z.union([
  twitchAuthStatusDetailsSchema.extend({
    status: z.literal("authRequired"),
    reason: twitchAuthRequiredReasonSchema.optional(),
  }),
  twitchAuthStatusDetailsSchema.extend({
    status: twitchAuthConnectionStatusSchema.exclude(["authRequired"]),
    reason: z.never().optional(),
  }),
  twitchStatusDetailsSchema.extend({
    domain: z.literal("chat"),
    status: twitchChatConnectionStatusSchema,
    reason: z.never().optional(),
    connectionGeneration: unsignedInteger.optional(),
    activeConnection: twitchActiveConnectionSchema.optional(),
  }),
]);
export const speechStatusSchema = z.enum(["idle", "speaking", "paused", "disconnected", "error"]);
export const speechAdapterHealthSchema = z.enum(["unknown", "connected", "disconnected", "error"]);
export const speechQueuePhaseSchema = z.enum(["idle", "speaking", "paused", "error"]);
export const queueDisplayStateSchema = z.enum([
  "queued",
  "speaking",
  "spoken",
  "skipped",
  "blocked",
  "error",
]);
export const speechStatusWireSchema = z.object({
  revision: unsignedInteger,
  status: speechStatusSchema,
  adapterHealth: speechAdapterHealthSchema,
  message: text.optional(),
  occurredAtMs: timestamp,
});
export const speechStatusEventSchema = speechStatusWireSchema.extend({
  revision: unsignedInteger.optional(),
});
export const blockedReasonSchema = z.enum([
  "repeatSuppressed",
  "blockedUser",
  "blockedWord",
  "blockedUrl",
  "emptyAfterFormatting",
]);
export const skippedReasonSchema = z.enum([
  "overflow",
  "userSkip",
  "removed",
  "cleared",
  "autoSpeakDisabled",
]);
export const failureCodeSchema = z.enum([
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
]);
export const recoveryActionSchema = z.enum([
  "reviewFilters",
  "reviewQueue",
  "diagnoseSpeech",
  "confirmDelivery",
  "none",
]);
export const outcomeDetailsSchema = z.object({
  message: text.refine((value) => value.trim().length > 0 && Array.from(value).length <= 200),
  occurredAtMs: timestamp,
  retryable: z.boolean(),
  recoveryAction: recoveryActionSchema,
});
export const speechQueueOutcomeWireSchema = z.discriminatedUnion("kind", [
  outcomeDetailsSchema.extend({ kind: z.literal("blocked"), reasonCode: blockedReasonSchema }),
  outcomeDetailsSchema.extend({ kind: z.literal("skipped"), reasonCode: skippedReasonSchema }),
  outcomeDetailsSchema.extend({ kind: z.literal("error"), reasonCode: failureCodeSchema }),
]);
export const retryableSpeechReasonSchema = failureCodeSchema.extract([
  "connectionRefused",
  "connectTimeout",
  "connectFailed",
  "connectionLost",
]);
export const diagnosableNonRetryableSpeechReasonSchema = failureCodeSchema.exclude([
  "writeTimeout",
  "writeFailed",
  "connectionLost",
  "unknown",
]);
const outcomeDetails = outcomeDetailsSchema.pick({ message: true, occurredAtMs: true });
export const speechQueueOutcomeSchema = z.union([
  outcomeDetails.extend({
    kind: z.literal("blocked"),
    reasonCode: blockedReasonSchema,
    retryable: z.literal(false),
    recoveryAction: z.literal("reviewFilters"),
  }),
  outcomeDetails.extend({
    kind: z.literal("skipped"),
    reasonCode: z.literal("overflow"),
    retryable: z.literal(false),
    recoveryAction: z.literal("reviewQueue"),
  }),
  outcomeDetails.extend({
    kind: z.literal("skipped"),
    reasonCode: skippedReasonSchema.exclude(["overflow"]),
    retryable: z.literal(false),
    recoveryAction: z.literal("none"),
  }),
  outcomeDetails.extend({
    kind: z.literal("error"),
    reasonCode: failureCodeSchema,
    retryable: z.literal(false),
    recoveryAction: z.literal("confirmDelivery"),
  }),
  outcomeDetails.extend({
    kind: z.literal("error"),
    reasonCode: diagnosableNonRetryableSpeechReasonSchema,
    retryable: z.literal(false),
    recoveryAction: z.literal("diagnoseSpeech"),
  }),
  outcomeDetails.extend({
    kind: z.literal("error"),
    reasonCode: retryableSpeechReasonSchema,
    retryable: z.literal(true),
    recoveryAction: z.literal("diagnoseSpeech"),
  }),
]);
export const queueItemWireSchema = z.object({
  id: text,
  sourceMessageId: text.optional(),
  userDisplayName: text,
  text,
  status: queueDisplayStateSchema,
  outcome: speechQueueOutcomeWireSchema.optional(),
});
export const queueItemSchema = queueItemWireSchema
  .extend({ outcome: speechQueueOutcomeSchema.optional() })
  .refine(
    ({ status, outcome }) =>
      !outcome ||
      (outcome.kind === "error"
        ? ["queued", "speaking", "error"].includes(status)
        : status === outcome.kind),
  );
export const speechQueueUpdatedWireSchema = z.object({
  revision: unsignedInteger,
  queuedCount: unsignedInteger,
  items: z.array(queueItemWireSchema),
  phase: speechQueuePhaseSchema,
  warning: text.optional(),
  occurredAtMs: timestamp,
});
export const speechQueueUpdatedSchema = speechQueueUpdatedWireSchema.extend({
  revision: unsignedInteger.optional(),
  phase: speechQueuePhaseSchema.optional(),
  items: z.array(queueItemSchema),
});
export const appLogLevelSchema = z.enum(["info", "warning", "error"]);
export const appLogEventSchema = z.object({
  id: text.optional(),
  level: appLogLevelSchema,
  message: text,
  occurredAtMs: timestamp,
});
export const appEventEmitErrorSchema = z.object({
  id: text,
  event: text,
  error: text,
  occurredAtMs: timestamp,
});
export const appEventsSnapshotWireSchema = z.object({
  revision: unsignedInteger,
  logs: z.array(appLogEventSchema),
  twitchStatuses: z.array(twitchStatusWireSchema),
  speechStatus: speechStatusWireSchema.optional(),
  emitErrors: z.array(appEventEmitErrorSchema),
});
export const appEventsSnapshotSchema = appEventsSnapshotWireSchema.extend({
  twitchStatuses: z.array(twitchStatusSchema),
  speechStatus: speechStatusEventSchema.optional(),
});
export const speechStateSnapshotWireSchema = z.object({
  revision: unsignedInteger,
  status: speechStatusWireSchema,
  queue: speechQueueUpdatedWireSchema,
});
export const speechStateSnapshotSchema = speechStateSnapshotWireSchema.extend({
  status: speechStatusEventSchema,
  queue: speechQueueUpdatedSchema,
});

/** Tauri's serialized unit result is JSON null; missing IPC payload is an error. */
export const unitResultSchema = z.null();

function issuePaths(issues: readonly z.core.$ZodIssue[]): string[] {
  return issues.flatMap((issue) => {
    if (issue.code === "invalid_union") return issue.errors.flatMap(issuePaths);
    return issue.code === "unrecognized_keys"
      ? issue.keys.map((key) => [...issue.path, key].join("."))
      : [issue.path.join(".")];
  });
}

export function parsePayload<S extends z.ZodType>(
  schema: S,
  value: unknown,
  contract: string,
): z.output<S> {
  const result = schema.safeParse(value);
  if (result.success) return result.data;
  // Never echo payload values: responses can contain authentication information.
  const path =
    [...new Set(issuePaths(result.error.issues))].filter(Boolean).slice(0, 8).join(", ") ||
    "payload";
  throw new Error(
    `Tauri bridge の ${contract} payload が不正です: ${path} の型・値を確認してください。`,
  );
}
