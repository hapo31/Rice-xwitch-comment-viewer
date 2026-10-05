import type { AppSettings, AppSettingsPatch } from "../../types";
import { formatRuleList, parseBlockedUserList, parseBlockedWordList } from "../../validation";
import { defaultSpeechSettings } from "./defaults";

export type SettingsDraft = {
  host: string;
  port: string;
  remoteMode: boolean;
  speed: number;
  tone: number;
  volume: number;
  voice: string;
  autoSpeak: boolean;
  readUserName: boolean;
  readEmotes: boolean;
  connectionSuccessSpeechEnabled: boolean;
  connectionSuccessSpeechText: string;
};

export type FilterDraft = {
  blockedUsers: string;
  blockedWords: string;
  urlHandling: AppSettings["speech"]["urlHandling"];
  maxLength: string;
  repeatSeconds: string;
};

export function createSettingsDraft(speech: AppSettings["speech"]): SettingsDraft {
  return {
    host: speech.bouyomiHost,
    port: String(speech.bouyomiPort),
    remoteMode: speech.bouyomiRemoteMode ?? false,
    speed: speech.bouyomiSpeed,
    tone: speech.bouyomiTone,
    volume: speech.bouyomiVolume,
    voice: String(speech.bouyomiVoice),
    autoSpeak: speech.autoSpeak,
    readUserName: speech.readUserName,
    readEmotes: speech.readEmotes,
    connectionSuccessSpeechEnabled: speech.connectionSuccessSpeechEnabled,
    connectionSuccessSpeechText: speech.connectionSuccessSpeechText,
  };
}

export function createSettingsSpeechPatch(
  draft: SettingsDraft,
  saved: AppSettings["speech"],
): NonNullable<AppSettingsPatch["speech"]> {
  const patch: NonNullable<AppSettingsPatch["speech"]> = {};
  const port = Number(draft.port);
  const voice = Number(draft.voice);
  if (draft.host.trim() !== saved.bouyomiHost) patch.bouyomiHost = draft.host.trim();
  if (port !== saved.bouyomiPort) patch.bouyomiPort = port;
  if (draft.remoteMode !== (saved.bouyomiRemoteMode ?? false)) {
    patch.bouyomiRemoteMode = draft.remoteMode;
  }
  if (draft.speed !== saved.bouyomiSpeed) patch.bouyomiSpeed = draft.speed;
  if (draft.tone !== saved.bouyomiTone) patch.bouyomiTone = draft.tone;
  if (draft.volume !== saved.bouyomiVolume) patch.bouyomiVolume = draft.volume;
  if (voice !== saved.bouyomiVoice) patch.bouyomiVoice = voice;
  if (draft.autoSpeak !== saved.autoSpeak) patch.autoSpeak = draft.autoSpeak;
  if (draft.readUserName !== saved.readUserName) patch.readUserName = draft.readUserName;
  if (draft.readEmotes !== saved.readEmotes) patch.readEmotes = draft.readEmotes;
  if (draft.connectionSuccessSpeechEnabled !== saved.connectionSuccessSpeechEnabled) {
    patch.connectionSuccessSpeechEnabled = draft.connectionSuccessSpeechEnabled;
  }
  if (draft.connectionSuccessSpeechText !== saved.connectionSuccessSpeechText) {
    patch.connectionSuccessSpeechText = draft.connectionSuccessSpeechText;
  }
  return patch;
}

export function createFilterDraft(speech: AppSettings["speech"]): FilterDraft {
  return {
    blockedUsers: formatRuleList(speech.blockedUsers),
    blockedWords: formatRuleList(speech.blockedWords),
    urlHandling: speech.urlHandling,
    maxLength: String(speech.maxCommentLength),
    repeatSeconds: String(speech.repeatSuppressionSeconds),
  };
}

export function createFilterSpeechPatch(
  draft: FilterDraft,
  saved: AppSettings["speech"],
): NonNullable<AppSettingsPatch["speech"]> {
  const patch: NonNullable<AppSettingsPatch["speech"]> = {};
  const maxLength = Number(draft.maxLength);
  const repeatSeconds = Number(draft.repeatSeconds);
  const blockedUsers = parseBlockedUserList(draft.blockedUsers).items;
  const blockedWords = parseBlockedWordList(draft.blockedWords).items;
  if (maxLength !== saved.maxCommentLength) patch.maxCommentLength = maxLength;
  if (repeatSeconds !== saved.repeatSuppressionSeconds) {
    patch.repeatSuppressionSeconds = repeatSeconds;
  }
  if (!stringArrayEqual(blockedUsers, saved.blockedUsers)) patch.blockedUsers = blockedUsers;
  if (!stringArrayEqual(blockedWords, saved.blockedWords)) patch.blockedWords = blockedWords;
  if (draft.urlHandling !== saved.urlHandling) patch.urlHandling = draft.urlHandling;
  return patch;
}

export function hasSpeechPatch(patch: NonNullable<AppSettingsPatch["speech"]>): boolean {
  return Object.keys(patch).length > 0;
}

function stringArrayEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function resolvedSpeechSettings(settings?: AppSettings) {
  return { ...defaultSpeechSettings(), ...settings?.speech };
}
