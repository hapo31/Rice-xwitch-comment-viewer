import { useMemo } from "react";
import { AuthView } from "./auth/AuthView";
import { ChatView } from "./chat/ChatView";
import { FilterView } from "./filter/FilterView";
import { LauncherView } from "./launcher/LauncherView";
import { LogsView } from "./logs/LogsView";
import { QueueView } from "./queue/QueueView";
import { SettingsView } from "./settings/SettingsView";
import {
  useChatSelector,
  useConnectionSelector,
  useLogsSelector,
  useQueueSelector,
  useSettingsSelector,
} from "../stores/domainStores";
import {
  useAuthController,
  useLauncherController,
  useQueueController,
  useSettingsController,
  useSpeechController,
} from "../orchestration/domainControllerContext";

export function DomainChatView({ showStartupGuide }: { showStartupGuide: boolean }) {
  const messages = useChatSelector((state) => state.messages);
  const settings = useSettingsSelector((state) => state.settings);
  const connection = useConnectionSelector((state) => state);
  const queuePhase = useQueueSelector((state) => state.phase);
  const state = useMemo(
    () => ({
      chatMessages: messages,
      settings,
      twitchAuthStatus: connection.twitchAuthStatus,
      twitchProfile: connection.twitchProfile,
      twitchConnectionStatus: connection.twitchConnectionStatus,
      twitchActiveConnection: connection.twitchActiveConnection,
      speechAdapterHealth: connection.speechAdapterHealth,
      speechQueuePhase: queuePhase,
    }),
    [
      connection.twitchActiveConnection,
      connection.twitchConnectionStatus,
      connection.twitchProfile,
      connection.speechAdapterHealth,
      connection.twitchAuthStatus,
      queuePhase,
      messages,
      settings,
    ],
  );
  return <ChatView state={state} showStartupGuide={showStartupGuide} />;
}

export function DomainQueueView() {
  const speech = useSpeechController();
  const queueActions = useQueueController();
  const queue = useQueueSelector((state) => state);
  const state = useMemo(
    () => ({ queueItems: queue.items, speechQueuePhase: queue.phase }),
    [queue],
  );
  return (
    <QueueView
      state={state}
      onSpeechControl={speech.speechControl}
      onQueueReload={queueActions.queueReload}
      onQueueRemove={queueActions.queueRemove}
      onQueueDismiss={queueActions.queueDismiss}
      onQueueDismissHistory={queueActions.queueDismissHistory}
      onQueueRetry={queueActions.queueRetry}
    />
  );
}

export function DomainLauncherView() {
  const actions = useLauncherController();
  const settings = useSettingsSelector((state) => state.settings);
  return (
    <LauncherView
      items={settings?.launcher.items ?? []}
      isReady={Boolean(settings)}
      onAdd={actions.launcherAdd}
      onRemove={actions.launcherRemove}
      onLaunch={actions.launcherLaunch}
      onLaunchAll={actions.launcherLaunchAll}
    />
  );
}

export function DomainFilterView() {
  const { updateSettings } = useSettingsController();
  const settings = useSettingsSelector((state) => state.settings);
  return <FilterView settings={settings} onSettingsUpdate={updateSettings} />;
}

export function DomainSettingsView() {
  const actions = useSettingsController();
  const settings = useSettingsSelector((state) => state.settings);
  return (
    <SettingsView
      settings={settings}
      onSettingsUpdate={actions.updateSettings}
      onSpeechHealthCheck={actions.speechHealthCheck}
      onSpeechDiagnostics={actions.speechDiagnostics}
      onSpeechTest={actions.speechTest}
    />
  );
}

export function DomainAuthView() {
  const settingsActions = useSettingsController();
  const authActions = useAuthController();
  const settings = useSettingsSelector((state) => state.settings);
  const connection = useConnectionSelector((state) => state);
  const state = useMemo(
    () => ({
      settings,
      twitchAuthStatus: connection.twitchAuthStatus,
      twitchDisconnectRequest: connection.twitchDisconnectRequest,
      twitchActiveConnection: connection.twitchActiveConnection,
      twitchAuthPrompt: connection.twitchAuthPrompt,
      twitchProfile: connection.twitchProfile,
    }),
    [
      settings,
      connection.twitchAuthStatus,
      connection.twitchDisconnectRequest,
      connection.twitchActiveConnection,
      connection.twitchAuthPrompt,
      connection.twitchProfile,
    ],
  );
  return (
    <AuthView
      state={state}
      onSettingsUpdate={settingsActions.updateSettings}
      onTwitchStartAuth={authActions.twitchStartAuth}
      onTwitchPollAuth={authActions.twitchPollAuth}
      onTwitchValidateAuth={authActions.twitchValidateAuth}
      onTwitchDisconnect={authActions.twitchDisconnect}
      onOpenExternalUrl={authActions.openExternalUrl}
    />
  );
}

export function DomainLogsView() {
  const logs = useLogsSelector((state) => state.logs);
  return <LogsView state={{ logs }} />;
}
