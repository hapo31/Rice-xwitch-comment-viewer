import { createContext, useContext, useMemo, useRef, type Context, type ReactNode } from "react";
import type {
  AppSettingsPatch,
  BouyomiConnectionDiagnostics,
  LauncherItem,
  LauncherLaunchResult,
} from "../types";

export interface DomainControllerActions {
  updateSettings: (patch: AppSettingsPatch) => Promise<boolean>;
  speechHealthCheck: () => void;
  speechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  speechTest: (text?: string) => void;
  speechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  queueReload: () => void;
  queueRemove: (itemId: string) => void;
  queueDismiss: (itemId: string) => void;
  queueDismissHistory: () => void;
  queueRetry: (itemId: string) => void;
  launcherAdd: (paths: string[]) => Promise<LauncherItem[]>;
  launcherRemove: (itemId: string) => Promise<LauncherItem[]>;
  launcherLaunch: (itemId: string) => Promise<LauncherLaunchResult>;
  launcherLaunchAll: () => Promise<LauncherLaunchResult>;
  twitchStartAuth: () => void;
  twitchPollAuth: () => void;
  twitchValidateAuth: () => Promise<boolean>;
  twitchDisconnect: () => void;
  twitchConnect: () => void;
  twitchStopChat: () => void;
  openExternalUrl: (url: string) => void;
  clearWarnings: () => void;
}

type StableActions = {
  [Key in keyof DomainControllerActions]: (
    ...args: Parameters<DomainControllerActions[Key]>
  ) => ReturnType<DomainControllerActions[Key]>;
};
type SettingsActions = Pick<
  StableActions,
  "updateSettings" | "speechHealthCheck" | "speechDiagnostics" | "speechTest"
>;
type AuthActions = Pick<
  StableActions,
  | "twitchStartAuth"
  | "twitchPollAuth"
  | "twitchValidateAuth"
  | "twitchDisconnect"
  | "openExternalUrl"
>;
type SpeechActions = Pick<StableActions, "speechControl">;
type QueueActions = Pick<
  StableActions,
  "queueReload" | "queueRemove" | "queueDismiss" | "queueDismissHistory" | "queueRetry"
>;
type LauncherActions = Pick<
  StableActions,
  "launcherAdd" | "launcherRemove" | "launcherLaunch" | "launcherLaunchAll"
>;
type ConnectionActions = Pick<StableActions, "twitchConnect" | "twitchStopChat">;
type NotificationActions = Pick<StableActions, "clearWarnings">;
const SettingsControllerContext = createContext<SettingsActions | undefined>(undefined);
const AuthControllerContext = createContext<AuthActions | undefined>(undefined);
const SpeechControllerContext = createContext<SpeechActions | undefined>(undefined);
const QueueControllerContext = createContext<QueueActions | undefined>(undefined);
const LauncherControllerContext = createContext<LauncherActions | undefined>(undefined);
const ConnectionControllerContext = createContext<ConnectionActions | undefined>(undefined);
const NotificationControllerContext = createContext<NotificationActions | undefined>(undefined);

/** Keep command actions stable while a controller refreshes its implementation. */
export function DomainControllerActionsProvider({
  actions,
  children,
}: {
  actions: DomainControllerActions;
  children: ReactNode;
}) {
  const current = useRef(actions);
  current.current = actions;
  const stable = useMemo(
    () =>
      Object.fromEntries(
        (Object.keys(actions) as Array<keyof DomainControllerActions>).map((key) => [
          key,
          (...args: never[]) => {
            const action = current.current[key] as (...values: never[]) => unknown;
            return action(...args);
          },
        ]),
      ) as StableActions,
    [],
  );
  return (
    <SettingsControllerContext.Provider value={stable}>
      <AuthControllerContext.Provider value={stable}>
        <SpeechControllerContext.Provider value={stable}>
          <QueueControllerContext.Provider value={stable}>
            <LauncherControllerContext.Provider value={stable}>
              <ConnectionControllerContext.Provider value={stable}>
                <NotificationControllerContext.Provider value={stable}>
                  {children}
                </NotificationControllerContext.Provider>
              </ConnectionControllerContext.Provider>
            </LauncherControllerContext.Provider>
          </QueueControllerContext.Provider>
        </SpeechControllerContext.Provider>
      </AuthControllerContext.Provider>
    </SettingsControllerContext.Provider>
  );
}

function useControllerContext<Value>(context: Context<Value | undefined>, name: string): Value {
  const actions = useContext(context);
  if (!actions) throw new Error(`${name} is required`);
  return actions;
}

export const useSettingsController = () =>
  useControllerContext(SettingsControllerContext, "SettingsControllerProvider");
export const useAuthController = () =>
  useControllerContext(AuthControllerContext, "AuthControllerProvider");
export const useSpeechController = () =>
  useControllerContext(SpeechControllerContext, "SpeechControllerProvider");
export const useQueueController = () =>
  useControllerContext(QueueControllerContext, "QueueControllerProvider");
export const useLauncherController = () =>
  useControllerContext(LauncherControllerContext, "LauncherControllerProvider");
export const useConnectionController = () =>
  useControllerContext(ConnectionControllerContext, "ConnectionControllerProvider");
export const useNotificationController = () =>
  useControllerContext(NotificationControllerContext, "NotificationControllerProvider");
