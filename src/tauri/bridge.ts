import type { z } from "zod";
import * as schemas from "./schemas";

export type TwitchChatMessageWireEvent = z.infer<typeof schemas.twitchChatMessageInputSchema>;

export const parseTwitchUserProfile = (value: unknown) =>
  schemas.parsePayload(schemas.twitchUserProfileSchema, value, "TwitchUserProfile");

export const parseTwitchAuthPollResult = (value: unknown) =>
  schemas.parsePayload(schemas.twitchAuthPollResultSchema, value, "TwitchAuthPollResult");

export const parseTwitchAuthValidationResult = (value: unknown) =>
  schemas.parsePayload(
    schemas.twitchAuthValidationResultSchema,
    value,
    "TwitchAuthValidationResult",
  );

export const parseTwitchChatMessageWireEvent = (value: unknown) =>
  schemas.parsePayload(schemas.twitchChatMessageInputSchema, value, "TwitchChatMessageWireEvent");

export const parseTwitchStatusEvent = (value: unknown) =>
  schemas.parsePayload(schemas.twitchStatusSchema, value, "TwitchStatusEvent");

export const parseSpeechStatusEvent = (value: unknown) =>
  schemas.parsePayload(schemas.speechStatusEventSchema, value, "SpeechStatusEvent");

export const parseSpeechQueueOutcome = (value: unknown) =>
  schemas.parsePayload(schemas.speechQueueOutcomeSchema, value, "SpeechQueueOutcome");

export const parseSpeechQueueUpdatedEvent = (value: unknown) =>
  schemas.parsePayload(schemas.speechQueueUpdatedSchema, value, "SpeechQueueUpdatedEvent");

export const parseAppLogEvent = (value: unknown) =>
  schemas.parsePayload(schemas.appLogEventSchema, value, "AppLogEvent");

export const parseAppEventsSnapshot = (value: unknown) =>
  schemas.parsePayload(schemas.appEventsSnapshotSchema, value, "AppEventsSnapshot");

export const parseSpeechStateSnapshot = (value: unknown) =>
  schemas.parsePayload(schemas.speechStateSnapshotSchema, value, "SpeechStateSnapshot");
