import type { AppSettings } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export type SettingsInitialization =
  | { status: "loading"; generation: number }
  | { status: "ready"; generation: number }
  | { status: "error"; generation: number; message: string };

export interface SettingsState {
  revision: number;
  initialization: SettingsInitialization;
  settings?: AppSettings;
}

export type SettingsAction =
  | { type: "initialization.changed"; initialization: SettingsInitialization }
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "launcher.items.changed"; items: AppSettings["launcher"]["items"] };

export const initialSettingsState: SettingsState = {
  revision: 0,
  initialization: { status: "loading", generation: 0 },
};

export function settingsReducer(state: SettingsState, action: SettingsAction): SettingsState {
  switch (action.type) {
    case "initialization.changed":
      return action.initialization.generation < state.initialization.generation
        ? state
        : { ...state, initialization: action.initialization };
    case "settings.loaded":
      return {
        ...state,
        settings: action.settings,
        revision: state.revision + 1,
        initialization: { status: "ready", generation: state.initialization.generation },
      };
    case "launcher.items.changed":
      return state.settings
        ? {
            ...state,
            revision: state.revision + 1,
            initialization: { status: "ready", generation: state.initialization.generation },
            settings: {
              ...state.settings,
              launcher: { ...state.settings.launcher, items: action.items },
            },
          }
        : state;
    default:
      return state;
  }
}

export function createSettingsStore(): ExternalStore<SettingsState, SettingsAction> {
  return createExternalStore(settingsReducer, initialSettingsState);
}
