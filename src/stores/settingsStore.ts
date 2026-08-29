import type { AppSettings } from "../types";
import { createExternalStore, type ExternalStore } from "./store";

export interface SettingsState { settings?: AppSettings }

export type SettingsAction =
  | { type: "settings.loaded"; settings: AppSettings }
  | { type: "launcher.items.changed"; items: AppSettings["launcher"]["items"] };

export const initialSettingsState: SettingsState = {};

export function settingsReducer(state: SettingsState, action: SettingsAction): SettingsState {
  switch (action.type) {
    case "settings.loaded": return { settings: action.settings };
    case "launcher.items.changed":
      return state.settings
        ? { settings: { ...state.settings, launcher: { ...state.settings.launcher, items: action.items } } }
        : state;
    default: return state;
  }
}

export function createSettingsStore(): ExternalStore<SettingsState, SettingsAction> {
  return createExternalStore(settingsReducer, initialSettingsState);
}
