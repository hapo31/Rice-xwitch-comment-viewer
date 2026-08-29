import { useMemo } from "react";
import { AuthView } from "./auth/AuthView";
import { ChatView } from "./chat/ChatView";
import { FilterView } from "./filter/FilterView";
import { LauncherView } from "./launcher/LauncherView";
import { LogsView } from "./logs/LogsView";
import { QueueView } from "./queue/QueueView";
import { SettingsView } from "./settings/SettingsView";
import { initialAppState, type AppState } from "../stores/appStore";
import {
  useChatSelector,
  useConnectionSelector,
  useLogsSelector,
  useQueueSelector,
  useSettingsSelector,
} from "../stores/domainStores";
import type {
  AppSettingsPatch,
  BouyomiConnectionDiagnostics,
  LauncherItem,
  LauncherLaunchResult,
} from "../types";

function useDomainAppState(): AppState {
  const messages = useChatSelector((state) => state.messages);
  const queueItems = useQueueSelector((state) => state.items);
  const connection = useConnectionSelector((state) => state);
  const settings = useSettingsSelector((state) => state.settings);
  const logs = useLogsSelector((state) => state.logs);
  const notifications = useLogsSelector((state) => state.notifications);
  return useMemo(() => ({
    ...initialAppState,
    twitchAuthStatus: connection.twitchAuthStatus,
    twitchConnectionStatus: connection.twitchConnectionStatus,
    twitchAuthPrompt: connection.twitchAuthPrompt,
    twitchProfile: connection.twitchProfile,
    speechStatus: connection.speechStatus,
    settings,
    chatMessages: messages,
    queueItems,
    logs,
    notifications,
  }), [connection, messages, queueItems, settings, logs, notifications]);
}

export function DomainChatView({ showStartupGuide }: { showStartupGuide: boolean }) {
  const state = useDomainAppState();
  return <ChatView state={state} showStartupGuide={showStartupGuide} />;
}

export function DomainQueueView(props: Omit<React.ComponentProps<typeof QueueView>, "state">) {
  const state = useDomainAppState();
  return <QueueView {...props} state={state} />;
}

export function DomainLauncherView(props: Omit<React.ComponentProps<typeof LauncherView>, "items" | "isReady">) {
  const settings = useSettingsSelector((state) => state.settings);
  return <LauncherView {...props} items={settings?.launcher.items ?? []} isReady={Boolean(settings)} />;
}

export function DomainFilterView({ onSettingsUpdate }: { onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean> }) {
  const settings = useSettingsSelector((state) => state.settings);
  return <FilterView settings={settings} onSettingsUpdate={onSettingsUpdate} />;
}

export function DomainSettingsView({
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
}: {
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
}) {
  const settings = useSettingsSelector((state) => state.settings);
  return <SettingsView settings={settings} onSettingsUpdate={onSettingsUpdate} onSpeechHealthCheck={onSpeechHealthCheck} onSpeechDiagnostics={onSpeechDiagnostics} onSpeechTest={onSpeechTest} />;
}

export function DomainAuthView(props: Omit<React.ComponentProps<typeof AuthView>, "state">) {
  const settings = useSettingsSelector((state) => state.settings);
  const connection = useConnectionSelector((state) => state);
  const state = useMemo(() => ({
    ...initialAppState,
    settings,
    twitchAuthStatus: connection.twitchAuthStatus,
    twitchConnectionStatus: connection.twitchConnectionStatus,
    twitchAuthPrompt: connection.twitchAuthPrompt,
    twitchProfile: connection.twitchProfile,
    speechStatus: connection.speechStatus,
  }), [settings, connection]);
  return <AuthView {...props} state={state} />;
}

export function DomainLogsView() {
  const logs = useLogsSelector((state) => state.logs);
  return <LogsView state={{ ...initialAppState, logs }} />;
}

export type DomainLauncherActions = {
  onAdd: (paths: string[]) => Promise<LauncherItem[]>;
  onRemove: (itemId: string) => Promise<LauncherItem[]>;
  onLaunch: (itemId: string) => Promise<LauncherLaunchResult>;
  onLaunchAll: () => Promise<LauncherLaunchResult>;
};
