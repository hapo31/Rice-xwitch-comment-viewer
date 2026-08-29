import type {
  AuthStatus,
  SpeechStatus,
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
}

export type ConnectionAction =
  | { type: "auth.status.changed"; status: AuthStatus }
  | { type: "chat.status.changed"; status: TwitchChatConnectionStatus }
  | { type: "auth.prompt.changed"; prompt?: TwitchDeviceAuthStart }
  | { type: "auth.profile.changed"; profile?: TwitchUserProfile }
  | { type: "speech.status.changed"; status: SpeechStatus };

export const initialConnectionState: ConnectionState = {
  twitchAuthStatus: "unauthenticated",
  twitchConnectionStatus: "disconnected",
  speechStatus: "disconnected",
};

export function connectionReducer(state: ConnectionState, action: ConnectionAction): ConnectionState {
  switch (action.type) {
    case "auth.status.changed": return { ...state, twitchAuthStatus: action.status };
    case "chat.status.changed": return { ...state, twitchConnectionStatus: action.status };
    case "auth.prompt.changed": return { ...state, twitchAuthPrompt: action.prompt };
    case "auth.profile.changed": return { ...state, twitchProfile: action.profile };
    case "speech.status.changed": return { ...state, speechStatus: action.status };
    default: return state;
  }
}

export function createConnectionStore(): ExternalStore<ConnectionState, ConnectionAction> {
  return createExternalStore(connectionReducer, initialConnectionState);
}
