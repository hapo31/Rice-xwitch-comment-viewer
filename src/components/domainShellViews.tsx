import { LiveStatusAnnouncer } from "./LiveStatusAnnouncer";
import { SidePanel } from "./SidePanel";
import { StatusBar } from "./StatusBar";
import {
  useConnectionSelector,
  useLogsSelector,
  useQueueSelector,
  useSettingsSelector,
} from "../stores/domainStores";

function useSidePanelState() {
  const auth = useConnectionSelector((state) => state.twitchAuthStatus);
  const connectionStatus = useConnectionSelector((state) => state.twitchConnectionStatus);
  const activeConnection = useConnectionSelector((state) => state.twitchActiveConnection);
  const speechHealth = useConnectionSelector((state) => state.speechAdapterHealth);
  const settings = useSettingsSelector((state) => state.settings);
  const queue = useQueueSelector((state) => state);
  const notifications = useLogsSelector((state) => state.notifications);
  return {
    settings,
    twitchAuthStatus: auth,
    twitchConnectionStatus: connectionStatus,
    twitchActiveConnection: activeConnection,
    speechAdapterHealth: speechHealth,
    speechQueuePhase: queue.phase,
    queueItems: queue.items,
    notifications,
  };
}

function useStatusBarState() {
  const auth = useConnectionSelector((state) => state.twitchAuthStatus);
  const connectionStatus = useConnectionSelector((state) => state.twitchConnectionStatus);
  const speechHealth = useConnectionSelector((state) => state.speechAdapterHealth);
  const settings = useSettingsSelector((state) => state.settings);
  const queue = useQueueSelector((state) => state);
  const notifications = useLogsSelector((state) => state.notifications);
  return {
    settings,
    twitchAuthStatus: auth,
    twitchConnectionStatus: connectionStatus,
    speechAdapterHealth: speechHealth,
    speechQueuePhase: queue.phase,
    queueItems: queue.items,
    notifications,
  };
}

function useLiveStatusState() {
  const twitchAuthStatus = useConnectionSelector((state) => state.twitchAuthStatus);
  const twitchConnectionStatus = useConnectionSelector((state) => state.twitchConnectionStatus);
  const speechAdapterHealth = useConnectionSelector((state) => state.speechAdapterHealth);
  const speechQueuePhase = useQueueSelector((state) => state.phase);
  const notifications = useLogsSelector((state) => state.notifications);
  return {
    twitchAuthStatus,
    twitchConnectionStatus,
    speechAdapterHealth,
    speechQueuePhase,
    notifications,
  };
}

export const DomainSidePanel = memo(function DomainSidePanel() {
  return <SidePanel state={useSidePanelState()} />;
});

export const DomainStatusBar = memo(function DomainStatusBar() {
  return <StatusBar state={useStatusBarState()} />;
});

export const DomainLiveStatusAnnouncer = memo(function DomainLiveStatusAnnouncer() {
  return <LiveStatusAnnouncer state={useLiveStatusState()} />;
});
import { memo } from "react";
