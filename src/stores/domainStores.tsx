import { createContext, useContext, useRef, type ReactNode } from "react";
import { createChatStore, type ChatAction, type ChatState } from "./chatStore";
import { createConnectionStore, type ConnectionAction, type ConnectionState } from "./connectionStore";
import { createLogsStore, type LogsAction, type LogsState } from "./logsStore";
import { createQueueStore, type QueueAction, type QueueState } from "./queueStore";
import { createSettingsStore, type SettingsAction, type SettingsState } from "./settingsStore";
import type { ExternalStore } from "./store";
import { useStoreSelector } from "./store";

export interface DomainStores {
  chat: ExternalStore<ChatState, ChatAction>;
  queue: ExternalStore<QueueState, QueueAction>;
  connection: ExternalStore<ConnectionState, ConnectionAction>;
  settings: ExternalStore<SettingsState, SettingsAction>;
  logs: ExternalStore<LogsState, LogsAction>;
}

export function createDomainStores(): DomainStores {
  return {
    chat: createChatStore(),
    queue: createQueueStore(),
    connection: createConnectionStore(),
    settings: createSettingsStore(),
    logs: createLogsStore(),
  };
}

const DomainStoresContext = createContext<DomainStores | undefined>(undefined);

export function DomainProvider({ children, stores }: { children: ReactNode; stores?: DomainStores }) {
  const storesRef = useRef<DomainStores>();
  if (!storesRef.current) storesRef.current = stores ?? createDomainStores();
  return <DomainStoresContext.Provider value={storesRef.current}>{children}</DomainStoresContext.Provider>;
}

export function useDomainStores(): DomainStores {
  const stores = useContext(DomainStoresContext);
  if (!stores) throw new Error("DomainProvider is required");
  return stores;
}

export function useChatSelector<Selected>(selector: (state: ChatState) => Selected): Selected {
  return useStoreSelector(useDomainStores().chat, selector);
}
export function useQueueSelector<Selected>(selector: (state: QueueState) => Selected): Selected {
  return useStoreSelector(useDomainStores().queue, selector);
}
export function useConnectionSelector<Selected>(selector: (state: ConnectionState) => Selected): Selected {
  return useStoreSelector(useDomainStores().connection, selector);
}
export function useSettingsSelector<Selected>(selector: (state: SettingsState) => Selected): Selected {
  return useStoreSelector(useDomainStores().settings, selector);
}
export function useLogsSelector<Selected>(selector: (state: LogsState) => Selected): Selected {
  return useStoreSelector(useDomainStores().logs, selector);
}
