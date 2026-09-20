import type {
  AppEventEmitError,
  AppEventsSnapshot,
  AppLogEvent,
  AppLogLevel,
  QueueItem,
  SpeechAdapterHealth,
  SpeechQueuePhase,
  SpeechQueueUpdatedEvent,
  SpeechStateSnapshot,
  SpeechStatus,
  SpeechStatusEvent,
  TwitchActiveConnection,
  TwitchAuthPollResult,
  TwitchAuthRequiredReason,
  TwitchAuthValidationResult,
  TwitchChatBadge,
  TwitchChatCheermote,
  TwitchChatEmote,
  TwitchChatMessageEvent,
  TwitchConnectionStatus,
  TwitchMessageFragment,
  TwitchStatusDomain,
  TwitchStatusEvent,
  TwitchUserProfile,
} from "../types";

type BridgeRecord = Record<string, unknown>;
export type TwitchChatMessageWireEvent = Omit<TwitchChatMessageEvent, "receivedAt"> & {
  receivedAt?: unknown;
};

function invalid(contract: string, detail: string): Error {
  return new Error(`Tauri bridge の ${contract} payload が不正です: ${detail}`);
}

function record(value: unknown, contract: string): BridgeRecord {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw invalid(contract, "object ではありません。");
  }
  return value as BridgeRecord;
}

function stringField(payload: BridgeRecord, field: string, contract: string): string {
  const value = payload[field];
  if (typeof value !== "string") {
    throw invalid(contract, `${field} は string ではありません。`);
  }
  return value;
}

function numberField(payload: BridgeRecord, field: string, contract: string): number {
  const value = payload[field];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw invalid(contract, `${field} は有限 number ではありません。`);
  }
  return value;
}

function arrayField(payload: BridgeRecord, field: string, contract: string): unknown[] {
  const value = payload[field];
  if (!Array.isArray(value)) {
    throw invalid(contract, `${field} は array ではありません。`);
  }
  return value;
}

function enumField<T extends string>(
  payload: BridgeRecord,
  field: string,
  values: readonly T[],
  contract: string,
): T {
  const value = payload[field];
  if (typeof value !== "string" || !values.includes(value as T)) {
    throw invalid(contract, `${field} の値が不正です。`);
  }
  return value as T;
}

function optionalField<T>(
  payload: BridgeRecord,
  field: string,
  contract: string,
  parse: (value: unknown) => T,
): T | undefined {
  if (!(field in payload)) {
    return undefined;
  }
  if (payload[field] === null) {
    throw invalid(contract, `${field} は省略するか有効な値にしてください。`);
  }
  return parse(payload[field]);
}

/**
 * Struct field の Rust `Option` はすべて JSON field omission で表す。
 * これは nullability だけを検査し、payload shape の schema validation は行わない。
 */
export function rejectUnexpectedNulls(payload: unknown, contract: string): void {
  const visit = (value: unknown, path: string): void => {
    if (value === null) {
      throw invalid(contract, `${path} は null です。`);
    }
    if (Array.isArray(value)) {
      value.forEach((item, index) => visit(item, `${path}[${index}]`));
    } else if (value && typeof value === "object") {
      Object.entries(value).forEach(([key, item]) => visit(item, `${path}.${key}`));
    }
  };
  visit(payload, "payload");
}

const appLogLevels = ["info", "warning", "error"] as const;
const twitchStatuses = ["disconnected", "connecting", "connected", "validating", "reconnecting", "authRequired", "error"] as const;
const twitchDomains = ["auth", "chat"] as const;
const twitchReasons = ["missingRequiredScope"] as const;
const speechStatuses = ["idle", "speaking", "paused", "disconnected", "error"] as const;
const adapterHealths = ["unknown", "connected", "disconnected", "error"] as const;
const queuePhases = ["idle", "speaking", "paused", "error"] as const;
const queueStatuses = ["queued", "speaking", "spoken", "skipped", "blocked", "error"] as const;

export function parseTwitchUserProfile(value: unknown): TwitchUserProfile {
  const payload = record(value, "TwitchUserProfile");
  return {
    userId: stringField(payload, "userId", "TwitchUserProfile"),
    login: stringField(payload, "login", "TwitchUserProfile"),
    scopes: arrayField(payload, "scopes", "TwitchUserProfile").map((scope) => {
      if (typeof scope !== "string") throw invalid("TwitchUserProfile", "scopes の要素は string ではありません。");
      return scope;
    }),
    expiresIn: numberField(payload, "expiresIn", "TwitchUserProfile"),
  };
}

export function parseTwitchAuthPollResult(value: unknown): TwitchAuthPollResult {
  const payload = record(value, "TwitchAuthPollResult");
  if ("storage_warning" in payload) {
    throw invalid("TwitchAuthPollResult", "storage_warning ではなく storageWarning を使用してください。");
  }
  const status = enumField(payload, "status", ["pending", "slowDown", "authorized", "denied", "expired"] as const, "TwitchAuthPollResult");
  if (status === "authorized") {
    const storageWarning = optionalField(payload, "storageWarning", "TwitchAuthPollResult", (warning) => {
      if (typeof warning !== "string") throw invalid("TwitchAuthPollResult", "storageWarning は string ではありません。");
      return warning;
    });
    return { status, profile: parseTwitchUserProfile(payload.profile), ...(storageWarning === undefined ? {} : { storageWarning }) };
  }
  const message = stringField(payload, "message", "TwitchAuthPollResult");
  return status === "pending" || status === "slowDown"
    ? { status, message, interval: numberField(payload, "interval", "TwitchAuthPollResult") }
    : { status, message };
}

export function parseTwitchAuthValidationResult(value: unknown): TwitchAuthValidationResult {
  const payload = record(value, "TwitchAuthValidationResult");
  const storageWarning = optionalField(payload, "storageWarning", "TwitchAuthValidationResult", (warning) => {
    if (typeof warning !== "string") throw invalid("TwitchAuthValidationResult", "storageWarning は string ではありません。");
    return warning;
  });
  return { profile: parseTwitchUserProfile(payload.profile), ...(storageWarning === undefined ? {} : { storageWarning }) };
}

function parseTwitchMessageFragment(value: unknown): TwitchMessageFragment {
  const payload = record(value, "TwitchMessageFragment");
  const emote = optionalField(payload, "emote", "TwitchMessageFragment", (emoteValue): TwitchChatEmote => {
    const emotePayload = record(emoteValue, "TwitchChatEmote");
    const ownerId = optionalField(emotePayload, "ownerId", "TwitchChatEmote", (owner) => {
      if (typeof owner !== "string") throw invalid("TwitchChatEmote", "ownerId は string ではありません。");
      return owner;
    });
    return { id: stringField(emotePayload, "id", "TwitchChatEmote"), emoteSetId: stringField(emotePayload, "emoteSetId", "TwitchChatEmote"), ...(ownerId === undefined ? {} : { ownerId }) };
  });
  const cheermote = optionalField(payload, "cheermote", "TwitchMessageFragment", (cheermoteValue): TwitchChatCheermote => {
    const cheermotePayload = record(cheermoteValue, "TwitchChatCheermote");
    return { prefix: stringField(cheermotePayload, "prefix", "TwitchChatCheermote"), bits: numberField(cheermotePayload, "bits", "TwitchChatCheermote"), tier: numberField(cheermotePayload, "tier", "TwitchChatCheermote") };
  });
  return { type: stringField(payload, "type", "TwitchMessageFragment"), text: stringField(payload, "text", "TwitchMessageFragment"), ...(emote === undefined ? {} : { emote }), ...(cheermote === undefined ? {} : { cheermote }) };
}

export function parseTwitchChatMessageWireEvent(value: unknown): TwitchChatMessageWireEvent {
  const payload = record(value, "TwitchChatMessageEvent");
  const connectionGeneration = optionalField(payload, "connectionGeneration", "TwitchChatMessageEvent", (generation) => {
    if (typeof generation !== "number" || !Number.isFinite(generation)) throw invalid("TwitchChatMessageEvent", "connectionGeneration は有限 number ではありません。");
    return generation;
  });
  return {
    id: stringField(payload, "id", "TwitchChatMessageEvent"), platform: enumField(payload, "platform", ["twitch"] as const, "TwitchChatMessageEvent"), channelId: stringField(payload, "channelId", "TwitchChatMessageEvent"), channelLogin: stringField(payload, "channelLogin", "TwitchChatMessageEvent"), userId: stringField(payload, "userId", "TwitchChatMessageEvent"), userLogin: stringField(payload, "userLogin", "TwitchChatMessageEvent"), userDisplayName: stringField(payload, "userDisplayName", "TwitchChatMessageEvent"), text: stringField(payload, "text", "TwitchChatMessageEvent"), fragments: arrayField(payload, "fragments", "TwitchChatMessageEvent").map(parseTwitchMessageFragment), badges: arrayField(payload, "badges", "TwitchChatMessageEvent").map((badge): TwitchChatBadge => {
      const badgePayload = record(badge, "TwitchChatBadge");
      return { setId: stringField(badgePayload, "setId", "TwitchChatBadge"), id: stringField(badgePayload, "id", "TwitchChatBadge"), info: stringField(badgePayload, "info", "TwitchChatBadge") };
    }), receivedAt: payload.receivedAt, ...(connectionGeneration === undefined ? {} : { connectionGeneration }),
  };
}

export function parseTwitchStatusEvent(value: unknown): TwitchStatusEvent {
  const payload = record(value, "TwitchStatusEvent");
  const reason = optionalField(payload, "reason", "TwitchStatusEvent", (reasonValue) => enumField({ reason: reasonValue }, "reason", twitchReasons, "TwitchStatusEvent") as TwitchAuthRequiredReason);
  const connectionGeneration = optionalField(payload, "connectionGeneration", "TwitchStatusEvent", (generation) => numberField({ connectionGeneration: generation }, "connectionGeneration", "TwitchStatusEvent"));
  const activeConnection = optionalField(payload, "activeConnection", "TwitchStatusEvent", (connection): TwitchActiveConnection => {
    const item = record(connection, "TwitchActiveConnection");
    return { generation: numberField(item, "generation", "TwitchActiveConnection"), broadcasterUserId: stringField(item, "broadcasterUserId", "TwitchActiveConnection"), broadcasterLogin: stringField(item, "broadcasterLogin", "TwitchActiveConnection") };
  });
  const message = optionalField(payload, "message", "TwitchStatusEvent", (messageValue) => {
    if (typeof messageValue !== "string") throw invalid("TwitchStatusEvent", "message は string ではありません。");
    return messageValue;
  });
  const revision = optionalField(payload, "revision", "TwitchStatusEvent", (revisionValue) => numberField({ revision: revisionValue }, "revision", "TwitchStatusEvent"));
  return { domain: enumField(payload, "domain", twitchDomains, "TwitchStatusEvent") as TwitchStatusDomain, status: enumField(payload, "status", twitchStatuses, "TwitchStatusEvent") as TwitchConnectionStatus, occurredAtMs: numberField(payload, "occurredAtMs", "TwitchStatusEvent"), ...(revision === undefined ? {} : { revision }), ...(reason === undefined ? {} : { reason }), ...(connectionGeneration === undefined ? {} : { connectionGeneration }), ...(activeConnection === undefined ? {} : { activeConnection }), ...(message === undefined ? {} : { message }) };
}

export function parseSpeechStatusEvent(value: unknown): SpeechStatusEvent {
  const payload = record(value, "SpeechStatusEvent");
  const revision = optionalField(payload, "revision", "SpeechStatusEvent", (revisionValue) => numberField({ revision: revisionValue }, "revision", "SpeechStatusEvent"));
  const message = optionalField(payload, "message", "SpeechStatusEvent", (messageValue) => {
    if (typeof messageValue !== "string") throw invalid("SpeechStatusEvent", "message は string ではありません。");
    return messageValue;
  });
  return { status: enumField(payload, "status", speechStatuses, "SpeechStatusEvent") as SpeechStatus, adapterHealth: enumField(payload, "adapterHealth", adapterHealths, "SpeechStatusEvent") as SpeechAdapterHealth, occurredAtMs: numberField(payload, "occurredAtMs", "SpeechStatusEvent"), ...(revision === undefined ? {} : { revision }), ...(message === undefined ? {} : { message }) };
}

function parseQueueItem(value: unknown): QueueItem {
  const payload = record(value, "SpeechQueueItemEvent");
  const sourceMessageId = optionalField(payload, "sourceMessageId", "SpeechQueueItemEvent", (id) => {
    if (typeof id !== "string") throw invalid("SpeechQueueItemEvent", "sourceMessageId は string ではありません。");
    return id;
  });
  return { id: stringField(payload, "id", "SpeechQueueItemEvent"), userDisplayName: stringField(payload, "userDisplayName", "SpeechQueueItemEvent"), text: stringField(payload, "text", "SpeechQueueItemEvent"), status: enumField(payload, "status", queueStatuses, "SpeechQueueItemEvent"), ...(sourceMessageId === undefined ? {} : { sourceMessageId }) };
}

export function parseSpeechQueueUpdatedEvent(value: unknown): SpeechQueueUpdatedEvent {
  const payload = record(value, "SpeechQueueUpdatedEvent");
  const revision = optionalField(payload, "revision", "SpeechQueueUpdatedEvent", (revisionValue) => numberField({ revision: revisionValue }, "revision", "SpeechQueueUpdatedEvent"));
  const phase = optionalField(payload, "phase", "SpeechQueueUpdatedEvent", (phaseValue) => enumField({ phase: phaseValue }, "phase", queuePhases, "SpeechQueueUpdatedEvent") as SpeechQueuePhase);
  const warning = optionalField(payload, "warning", "SpeechQueueUpdatedEvent", (warningValue) => {
    if (typeof warningValue !== "string") throw invalid("SpeechQueueUpdatedEvent", "warning は string ではありません。");
    return warningValue;
  });
  return { queuedCount: numberField(payload, "queuedCount", "SpeechQueueUpdatedEvent"), items: arrayField(payload, "items", "SpeechQueueUpdatedEvent").map(parseQueueItem), occurredAtMs: numberField(payload, "occurredAtMs", "SpeechQueueUpdatedEvent"), ...(revision === undefined ? {} : { revision }), ...(phase === undefined ? {} : { phase }), ...(warning === undefined ? {} : { warning }) };
}

function parseAppLogEvent(value: unknown): AppLogEvent {
  const payload = record(value, "AppLogEvent");
  const id = optionalField(payload, "id", "AppLogEvent", (idValue) => {
    if (typeof idValue !== "string") throw invalid("AppLogEvent", "id は string ではありません。");
    return idValue;
  });
  return { level: enumField(payload, "level", appLogLevels, "AppLogEvent") as AppLogLevel, message: stringField(payload, "message", "AppLogEvent"), occurredAtMs: numberField(payload, "occurredAtMs", "AppLogEvent"), ...(id === undefined ? {} : { id }) };
}

function parseAppEventEmitError(value: unknown): AppEventEmitError {
  const payload = record(value, "AppEventEmitError");
  return { id: stringField(payload, "id", "AppEventEmitError"), event: stringField(payload, "event", "AppEventEmitError"), error: stringField(payload, "error", "AppEventEmitError"), occurredAtMs: numberField(payload, "occurredAtMs", "AppEventEmitError") };
}

export function parseAppEventsSnapshot(value: unknown): AppEventsSnapshot {
  const payload = record(value, "AppEventsSnapshot");
  const speechStatus = optionalField(payload, "speechStatus", "AppEventsSnapshot", parseSpeechStatusEvent);
  return { revision: numberField(payload, "revision", "AppEventsSnapshot"), logs: arrayField(payload, "logs", "AppEventsSnapshot").map(parseAppLogEvent), twitchStatuses: arrayField(payload, "twitchStatuses", "AppEventsSnapshot").map(parseTwitchStatusEvent), emitErrors: arrayField(payload, "emitErrors", "AppEventsSnapshot").map(parseAppEventEmitError), ...(speechStatus === undefined ? {} : { speechStatus }) };
}

export function parseSpeechStateSnapshot(value: unknown): SpeechStateSnapshot {
  const payload = record(value, "SpeechStateSnapshot");
  return { revision: numberField(payload, "revision", "SpeechStateSnapshot"), status: parseSpeechStatusEvent(payload.status), queue: parseSpeechQueueUpdatedEvent(payload.queue) };
}
