import type {
  AuthStatus,
  SpeechStatus,
  SpeechAdapterHealth,
  TwitchChatConnectionStatus,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export interface ConnectionState {
  twitchAuthStatus: AuthStatus;
  twitchConnectionStatus: TwitchChatConnectionStatus;
  twitchAuthPrompt?: TwitchDeviceAuthStart;
  twitchProfile?: TwitchUserProfile;
  speechStatus: SpeechStatus;
  speechAdapterHealth: SpeechAdapterHealth;
  authRevision: number;
  chatRevision: number;
  speechRevision: number;
}

export type ConnectionAction =
  | { type: "auth.status.changed"; status: AuthStatus; revision?: number }
  | { type: "chat.status.changed"; status: TwitchChatConnectionStatus; revision?: number }
  | { type: "auth.prompt.changed"; prompt?: TwitchDeviceAuthStart }
  | { type: "auth.profile.changed"; profile?: TwitchUserProfile }
  | { type: "speech.status.changed"; status: SpeechStatus; revision?: number; adapterHealth?: SpeechAdapterHealth };

export const initialConnectionState: ConnectionState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "disconnected",
  speechAdapterHealth: "unknown",
  authRevision: 0,
  chatRevision: 0,
  speechRevision: 0,
};

export function connectionReducer(state: ConnectionState, action: ConnectionAction): ConnectionState {
  switch (action.type) {
    case "auth.status.changed":
      if (action.revision !== undefined && action.revision <= state.authRevision) return state;
      return { ...state, twitchAuthStatus: action.status, authRevision: action.revision ?? state.authRevision };
    case "chat.status.changed":
      if (action.revision !== undefined && action.revision <= state.chatRevision) return state;
      return { ...state, twitchConnectionStatus: action.status, chatRevision: action.revision ?? state.chatRevision };
    case "auth.prompt.changed": return { ...state, twitchAuthPrompt: action.prompt };
    case "auth.profile.changed": return { ...state, twitchProfile: action.profile };
    case "speech.status.changed":
      if (action.revision !== undefined && action.revision <= state.speechRevision) return state;
      return { ...state, speechStatus: action.status, speechRevision: action.revision ?? state.speechRevision, speechAdapterHealth: action.adapterHealth ?? state.speechAdapterHealth };
    default: return state;
  }
}

export function createConnectionStore(): ExternalStore<ConnectionState, ConnectionAction> {
  return createExternalStore(connectionReducer, initialConnectionState);
}
