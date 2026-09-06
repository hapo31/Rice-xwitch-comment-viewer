import { Navigate, Route, Routes, useLocation, useNavigationType } from "react-router-dom";
import { useEffect } from "react";
import {
  DomainAuthView,
  DomainChatView,
  DomainFilterView,
  DomainLauncherView,
  DomainLogsView,
  DomainQueueView,
  DomainSettingsView,
} from "../features/domainViews";
import type {
  AppSettingsPatch,
  BouyomiConnectionDiagnostics,
  LauncherItem,
  LauncherLaunchResult,
} from "../types";
import { getRouteDocumentTitle, routeHeadingId, shouldFocusRouteHeading } from "../routeAccessibility";

interface MainViewProps {
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
  onSpeechControl: (command: "pause" | "resume" | "skip" | "clear") => void;
  onQueueReload: () => void;
  onQueueRemove: (itemId: string) => void;
  onQueueDismiss: (itemId: string) => void;
  onQueueDismissHistory: () => void;
  onQueueRetry: (itemId: string) => void;
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
  showStartupGuide,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
  onSpeechControl,
  onQueueReload,
  onQueueRemove,
  onQueueDismiss,
  onQueueDismissHistory,
  onQueueRetry,
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
  const location = useLocation();
  const navigationType = useNavigationType();

  useEffect(() => {
    document.title = getRouteDocumentTitle(location.pathname);

    if (shouldFocusRouteHeading(navigationType)) {
      document.getElementById(routeHeadingId)?.focus();
    }
  }, [location.pathname, navigationType]);

  return (
    <Routes>
      <Route path="/" element={<Navigate to="/chat" replace />} />
      <Route path="/chat" element={<DomainChatView showStartupGuide={showStartupGuide} />} />
      <Route
        path="/queue"
        element={
          <DomainQueueView
            onSpeechControl={onSpeechControl}
            onQueueReload={onQueueReload}
            onQueueRemove={onQueueRemove}
            onQueueDismiss={onQueueDismiss}
            onQueueDismissHistory={onQueueDismissHistory}
            onQueueRetry={onQueueRetry}
          />
        }
      />
      <Route
        path="/launcher"
        element={
          <DomainLauncherView
            onAdd={onLauncherAdd}
            onRemove={onLauncherRemove}
            onLaunch={onLauncherLaunch}
            onLaunchAll={onLauncherLaunchAll}
          />
        }
      />
      <Route
        path="/filter"
        element={<DomainFilterView onSettingsUpdate={onSettingsUpdate} />}
      />
      <Route path="/rules" element={<Navigate to="/filter" replace />} />
      <Route
        path="/settings"
        element={
          <DomainSettingsView
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
          <DomainAuthView
            onSettingsUpdate={onSettingsUpdate}
            onTwitchStartAuth={onTwitchStartAuth}
            onTwitchPollAuth={onTwitchPollAuth}
            onTwitchValidateAuth={onTwitchValidateAuth}
            onTwitchDisconnect={onTwitchDisconnect}
            onOpenExternalUrl={onOpenExternalUrl}
          />
        }
      />
      <Route path="/logs" element={<DomainLogsView />} />
      <Route path="*" element={<Navigate to="/chat" replace />} />
    </Routes>
  );
}
