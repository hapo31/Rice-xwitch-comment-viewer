import { Navigate, Route, Routes } from "react-router-dom";
import { AuthView } from "../features/auth/AuthView";
import { ChatView } from "../features/chat/ChatView";
import { FilterView } from "../features/filter/FilterView";
import { LauncherView } from "../features/launcher/LauncherView";
import { LogsView } from "../features/logs/LogsView";
import { QueueView } from "../features/queue/QueueView";
import { SettingsView } from "../features/settings/SettingsView";
import type { AppState } from "../stores/appStore";
import type {
  AppSettings,
  BouyomiConnectionDiagnostics,
  LauncherItem,
  LauncherLaunchResult,
} from "../types";

interface MainViewProps {
  state: AppState;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onQueueReload: () => void;
  onQueueRemove: (itemId: string) => void;
  onLauncherAdd: (paths: string[]) => Promise<LauncherItem[]>;
  onLauncherRemove: (itemId: string) => Promise<LauncherItem[]>;
  onLauncherLaunch: (itemId: string) => Promise<LauncherLaunchResult>;
  onLauncherLaunchAll: () => Promise<LauncherLaunchResult>;
  onTwitchStartAuth: () => void;
  onTwitchPollAuth: () => void;
  onTwitchValidateAuth: () => Promise<boolean>;
  onTwitchDisconnect: () => void;
  onOpenExternalUrl: (url: string) => void;
  showStartupGuide: boolean;
}

export function MainView({
  state,
  showStartupGuide,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
  onSpeechControl,
  onQueueReload,
  onQueueRemove,
  onLauncherAdd,
  onLauncherRemove,
  onLauncherLaunch,
  onLauncherLaunchAll,
  onTwitchStartAuth,
  onTwitchPollAuth,
  onTwitchValidateAuth,
  onTwitchDisconnect,
  onOpenExternalUrl,
}: MainViewProps) {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/chat" replace />} />
      <Route path="/chat" element={<ChatView state={state} showStartupGuide={showStartupGuide} />} />
      <Route
        path="/queue"
        element={
          <QueueView
            state={state}
            onSpeechControl={onSpeechControl}
            onQueueReload={onQueueReload}
            onQueueRemove={onQueueRemove}
          />
        }
      />
      <Route
        path="/launcher"
        element={
          <LauncherView
            items={state.settings?.launcher.items ?? []}
            isReady={Boolean(state.settings)}
            onAdd={onLauncherAdd}
            onRemove={onLauncherRemove}
            onLaunch={onLauncherLaunch}
            onLaunchAll={onLauncherLaunchAll}
          />
        }
      />
      <Route
        path="/filter"
        element={<FilterView settings={state.settings} onSettingsUpdate={onSettingsUpdate} />}
      />
      <Route path="/rules" element={<Navigate to="/filter" replace />} />
      <Route
        path="/settings"
        element={
          <SettingsView
            settings={state.settings}
            onSettingsUpdate={onSettingsUpdate}
            onSpeechHealthCheck={onSpeechHealthCheck}
            onSpeechDiagnostics={onSpeechDiagnostics}
            onSpeechTest={onSpeechTest}
          />
        }
      />
      <Route path="/voices" element={<Navigate to="/settings" replace />} />
      <Route
        path="/auth"
        element={
          <AuthView
            state={state}
            onSettingsUpdate={onSettingsUpdate}
            onTwitchStartAuth={onTwitchStartAuth}
            onTwitchPollAuth={onTwitchPollAuth}
            onTwitchValidateAuth={onTwitchValidateAuth}
            onTwitchDisconnect={onTwitchDisconnect}
            onOpenExternalUrl={onOpenExternalUrl}
          />
        }
      />
      <Route path="/logs" element={<LogsView state={state} />} />
      <Route path="*" element={<Navigate to="/chat" replace />} />
    </Routes>
  );
}
