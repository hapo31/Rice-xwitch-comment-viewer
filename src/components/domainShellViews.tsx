import { useMemo } from "react";
import { LiveStatusAnnouncer } from "./LiveStatusAnnouncer";
import { SidePanel } from "./SidePanel";
import { StatusBar } from "./StatusBar";
import { initialAppState } from "../stores/appStore";
import { useConnectionSelector, useLogsSelector, useQueueSelector, useSettingsSelector } from "../stores/domainStores";

function useShellState() {
  const connection = useConnectionSelector((state) => state);
  const settings = useSettingsSelector((state) => state.settings);
  const queueItems = useQueueSelector((state) => state.items);
  const notifications = useLogsSelector((state) => state.notifications);
  return useMemo(() => ({
    ...initialAppState,
    ...connection,
    settings,
    queueItems,
    notifications,
  }), [connection, settings, queueItems, notifications]);
}

export function DomainSidePanel(props: Omit<React.ComponentProps<typeof SidePanel>, "state">) {
  return <SidePanel {...props} state={useShellState()} />;
}

export function DomainStatusBar() {
  return <StatusBar state={useShellState()} />;
}

export function DomainLiveStatusAnnouncer() {
  return <LiveStatusAnnouncer state={useShellState()} />;
}
