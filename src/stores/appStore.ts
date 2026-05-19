import type { AppSettings, AuthStatus, ChatMessage, QueueItem, SpeechStatus, ViewId } from "../types";

export interface AppState {
  activeView: ViewId;
  twitchAuthStatus: AuthStatus;
  speechStatus: SpeechStatus;
  settings?: AppSettings;
  chatMessages: ChatMessage[];
  queueItems: QueueItem[];
  warnings: string[];
}

export type AppAction =
  | { type: "view.changed"; view: ViewId }
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "twitch.authStatus"; status: AuthStatus }
  | { type: "speech.status"; status: SpeechStatus }
  | { type: "chat.message"; message: ChatMessage }
  | { type: "queue.changed"; items: QueueItem[] }
  | { type: "warning.added"; warning: string };

export const initialAppState: AppState = {
  activeView: "chat",
  twitchAuthStatus: "unauthenticated",
  speechStatus: "disconnected",
  chatMessages: [],
  queueItems: [],
  warnings: [],
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "view.changed":
      return { ...state, activeView: action.view };
    case "settings.loaded":
      return { ...state, settings: action.settings };
    case "twitch.authStatus":
      return { ...state, twitchAuthStatus: action.status };
    case "speech.status":
      return { ...state, speechStatus: action.status };
    case "chat.message":
      return {
        ...state,
        chatMessages: [action.message, ...state.chatMessages].slice(0, 200),
      };
    case "queue.changed":
      return { ...state, queueItems: action.items };
    case "warning.added":
      return { ...state, warnings: [action.warning, ...state.warnings].slice(0, 5) };
    default:
      return state;
  }
}
