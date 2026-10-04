import { vi } from "vitest";

type Listener = (event: { event: string; id: number; payload: unknown }) => void;
type Handler = (args?: Record<string, unknown>) => unknown;
const handlers = new Map<string, Handler>();
const listeners = new Map<string, Set<Listener>>();
const pendingSubscriptions: Array<() => void> = [];
const rejectedSubscriptions = new Set<string>();
let holdSubscriptions = false;

export const defaultSettings = {
  twitch: {
    channelLogin: "",
    autoConnect: false,
    confirmBeforeStopChat: true,
    liveChatAnnouncements: true,
  },
  speech: {
    adapter: "bouyomi",
    bouyomiHost: "127.0.0.1",
    bouyomiPort: 50001,
    bouyomiSpeed: -1,
    bouyomiTone: -1,
    bouyomiVolume: -1,
    bouyomiVoice: 0,
    readUserName: true,
    autoSpeak: true,
    maxCommentLength: 120,
    repeatSuppressionSeconds: 2,
    blockedUsers: [],
    blockedWords: [],
    urlHandling: "replace",
    readEmotes: false,
    connectionSuccessSpeechEnabled: true,
    connectionSuccessSpeechText: "",
  },
  launcher: { items: [] },
  window: {},
};

const defaults: Record<string, unknown> = {
  settings_get: defaultSettings,
  settings_take_recovery_notice: null,
  twitch_get_stored_auth: null,
  app_build_info: {
    version: "0.2.3",
    isDev: true,
    launcher: { canRegisterApplications: true, canLaunchApplications: true },
  },
  app_events_snapshot: { revision: 0, logs: [], twitchStatuses: [], emitErrors: [] },
  speech_queue_reload: {
    revision: 0,
    status: { revision: 0, status: "idle", adapterHealth: "connected", occurredAtMs: 1 },
    queue: { revision: 0, phase: "idle", items: [], queuedCount: 0, occurredAtMs: 1 },
  },
};

export const tauriMock = {
  invoke: vi.fn(async (command: string, args?: Record<string, unknown>) => {
    if (handlers.has(command)) return handlers.get(command)!(args);
    if (command in defaults) return structuredClone(defaults[command]);
    throw new Error(`No mock handler configured for ${command}`);
  }),
  open: vi.fn(async (): Promise<string[] | null> => null),
  listen: vi.fn(async (name: string, listener: Listener) => {
    if (rejectedSubscriptions.delete(name)) throw new Error(`Subscription rejected: ${name}`);
    const entries = listeners.get(name) ?? new Set<Listener>();
    entries.add(listener);
    listeners.set(name, entries);
    const unlisten = () => {
      entries.delete(listener);
      if (entries.size === 0) listeners.delete(name);
    };
    if (holdSubscriptions)
      return new Promise<() => void>((resolve) =>
        pendingSubscriptions.push(() => resolve(unlisten)),
      );
    return unlisten;
  }),
  setCommand(name: string, value: unknown | Handler) {
    handlers.set(
      name,
      typeof value === "function" ? (value as Handler) : () => structuredClone(value),
    );
  },
  rejectCommand(name: string, message: string) {
    handlers.set(name, () => {
      throw new Error(message);
    });
  },
  emit(name: string, payload: unknown) {
    for (const listener of [...(listeners.get(name) ?? [])])
      listener({ event: name, id: 1, payload });
  },
  listenerCount(name?: string) {
    return name
      ? (listeners.get(name)?.size ?? 0)
      : [...listeners.values()].reduce((sum, entries) => sum + entries.size, 0);
  },
  delaySubscriptions() {
    holdSubscriptions = true;
  },
  releaseSubscriptions() {
    holdSubscriptions = false;
    for (const release of pendingSubscriptions.splice(0)) release();
  },
  rejectNextSubscription(name: string) {
    rejectedSubscriptions.add(name);
  },
  get pendingCount() {
    return pendingSubscriptions.length;
  },
  reset() {
    if (this.listenerCount() || pendingSubscriptions.length)
      throw new Error("Previous test leaked a native listener");
    handlers.clear();
    rejectedSubscriptions.clear();
    holdSubscriptions = false;
    this.invoke.mockClear();
    this.listen.mockClear();
    this.open.mockReset();
    this.open.mockResolvedValue(null);
  },
};

export const nativeWindow = {
  label: "main",
  isMaximized: vi.fn(async () => false),
  minimize: vi.fn(async () => undefined),
  toggleMaximize: vi.fn(async () => undefined),
  startDragging: vi.fn(async () => undefined),
  startResizeDragging: vi.fn(async () => undefined),
  destroy: vi.fn(async () => undefined),
  onResized: (listener: Listener) => tauriMock.listen("tauri://resize", listener),
  onCloseRequested: (listener: Listener) => tauriMock.listen("tauri://close-requested", listener),
};
