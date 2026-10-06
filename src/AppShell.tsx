import {
  memo,
  Profiler,
  type ProfilerOnRenderCallback,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useNavigate } from "react-router-dom";
import { AuthOperationController } from "./authOperation";
import { ActivityBar } from "./components/ActivityBar";
import {
  DomainLiveStatusAnnouncer,
  DomainSidePanel,
  DomainStatusBar,
} from "./components/domainShellViews";
import { MainView } from "./components/MainView";
import { ResizeHandles, TitleBar } from "./components/TitleBar";
import { useDisplayScale } from "./hooks/useDisplayScale";
import { useStreamHotkeys } from "./hooks/useStreamHotkeys";
import { APP_SHELL_CLASS_NAME } from "./layout/appShell";
import type { SystemTimelineEvent } from "./models/systemTimeline";
import {
  createLauncherController,
  createQueueController,
  createSpeechController,
} from "./orchestration/domainCommandControllers";
import { DomainControllerActionsProvider } from "./orchestration/domainControllerContext";
import {
  createSettingsMutationOrchestrator,
  dispatchDomainAction,
  subscribeDomainEvents,
} from "./orchestration/domainOrchestration";
import { ExitProtectionProvider, useExitController } from "./orchestration/ExitProtectionProvider";
import { startSpeechHealthMonitor } from "./orchestration/speechHealthMonitor";
import { createTwitchController } from "./orchestration/twitchController";
import { type ErrorOperation, presentError, reportPresentedError } from "./presentation/errors";
import { claimStartupGuideForSession } from "./presentation/startupGuide";
import {
  autoConnectTimelineEvent,
  SystemTimelineRouter,
  speechRecoveryTimelineEvent,
  timelineEventFromTwitchStatus,
} from "./presentation/systemTimeline";
import { type AppAction } from "./stores/appState";
import {
  useConnectionSelector,
  useDomainStores,
  useQueueSelector,
  useSettingsSelector,
} from "./stores/domainStores";
import {
  appExit,
  getAppEventsSnapshot,
  getSettings,
  isDesktopRuntime,
  speechControl,
  speechHealthProbe,
  speechQueueReload,
  subscribeAppLogEvents,
  subscribeSpeechQueueUpdatedEvents,
  subscribeSpeechStatusEvents,
  subscribeTwitchChatMessageEvents,
  subscribeTwitchStatusEvents,
  takeSettingsRecoveryNotice,
  twitchStopChat,
  updateSettings,
} from "./tauri/client";
import { subscribeWithCleanup } from "./tauri/subscriptions";
import { utcNow } from "./time";
import type {
  AppNotification,
  AppSettings,
  AppSettingsPatch,
  NotificationSeverity,
  NotificationSource,
} from "./types";

const showStartupGuideForSession = claimStartupGuideForSession(window.sessionStorage);

export function AppShell({ onRouteCommit }: { onRouteCommit?: ProfilerOnRenderCallback } = {}) {
  return (
    <ApplicationControllerProvider>
      <AppShellLayout onRouteCommit={onRouteCommit} />
    </ApplicationControllerProvider>
  );
}

function ApplicationControllerProvider({ children }: { children: ReactNode }) {
  const stores = useDomainStores();
  const [eventsRestored, setEventsRestored] = useState(false);
  const connection = useConnectionSelector((value) => value);
  const settings = useSettingsSelector((value) => value.settings);
  const queue = useQueueSelector((value) => value);
  const dispatch = useCallback(
    (action: AppAction) => dispatchDomainAction(stores, action),
    [stores],
  );
  const navigate = useNavigate();
  const autoConnectAttempted = useRef(false);
  const settingsMutation = useRef(
    createSettingsMutationOrchestrator({
      updateSettings,
      onSettingsLoaded: (nextSettings) => {
        settingsSnapshot.current = nextSettings;
        dispatch({ type: "settings.loaded", settings: nextSettings });
      },
      onError: (error) => reportError(error, "settings"),
    }),
  );
  const settingsSnapshot = useRef<AppSettings | undefined>(undefined);
  const startupAuthAttempted = useRef(false);
  const authOperations = useRef(new AuthOperationController());
  const systemTimelineRouter = useRef(new SystemTimelineRouter());

  useEffect(() => () => authOperations.current.invalidate(), []);

  useEffect(() => {
    Promise.all([getSettings(), takeSettingsRecoveryNotice()])
      .then(([settings, recoveryNotice]) => {
        settingsSnapshot.current = settings;
        dispatch({ type: "settings.loaded", settings });
        if (recoveryNotice) {
          addSystemChatMessage(recoveryNotice.message);
          dispatch({
            type: "log.added",
            log: {
              level: "warning",
              message: recoveryNotice.message,
              occurredAtMs: Date.now(),
            },
          });
          reportNotification("warning", "system", recoveryNotice.message);
        }
      })
      .catch((error) => reportError(error, "settings"));
  }, []);

  useEffect(() => {
    if (!eventsRestored || startupAuthAttempted.current) {
      return;
    }
    startupAuthAttempted.current = true;

    void twitchController.restore();
  }, [eventsRestored]);

  function addSystemChatMessage(text: string) {
    dispatch({
      type: "chat.message",
      message: {
        kind: "system",
        id: `system-${Date.now()}-${crypto.randomUUID()}`,
        receivedAt: utcNow(),
        userDisplayName: "system",
        text,
      },
    });
  }

  function reportNotification(
    severity: NotificationSeverity,
    source: NotificationSource,
    message: string,
    correlationId?: string,
    announcementDomains?: AppNotification["announcementDomains"],
  ) {
    dispatch({
      type: "notification.added",
      notification: {
        severity,
        source,
        message,
        occurredAtMs: Date.now(),
        correlationId,
        announcementDomains,
      },
    });
  }

  function reportError(
    error: unknown,
    operation: ErrorOperation = "general",
    announcementDomains?: AppNotification["announcementDomains"],
  ) {
    return reportPresentedError(error, operation, {
      notify: (message) =>
        reportNotification(
          "error",
          "command",
          message,
          `command:${crypto.randomUUID()}`,
          announcementDomains,
        ),
      log: (message) =>
        dispatch({ type: "log.added", log: { level: "error", message, occurredAtMs: Date.now() } }),
    });
  }

  function reportInfo(message: string, source: NotificationSource = "command") {
    reportNotification("success", source, message);
    dispatch({ type: "log.added", log: { level: "info", message, occurredAtMs: Date.now() } });
    addSystemChatMessage(message);
  }

  function routeSystemTimelineEvent(event: SystemTimelineEvent) {
    if (systemTimelineRouter.current.shouldRecord(event)) addSystemChatMessage(event.message);
  }

  const twitchController = useMemo(
    () =>
      createTwitchController({
        operations: authOperations.current,
        dispatch,
        getAuthPrompt: () => stores.connection.getState().twitchAuthPrompt,
        getAuthStatus: () => stores.connection.getState().twitchAuthStatus,
        getAuthRevision: () => stores.connection.getState().authRevision,
        getAuthProfile: () => stores.connection.getState().twitchProfile,
        getChannelLogin: () =>
          settingsSnapshot.current?.twitch.channelLogin ??
          stores.settings.getState().settings?.twitch.channelLogin,
        getConfirmBeforeStopChat: () =>
          stores.settings.getState().settings?.twitch.confirmBeforeStopChat ?? true,
        waitForSettings: () => settingsMutation.current.waitForIdle(),
        reportSystemMessage: addSystemChatMessage,
        reportInfo,
        reportNotification,
        reportError,
        reportTechnicalError: (message) =>
          dispatch({
            type: "log.added",
            log: { level: "error", message, occurredAtMs: Date.now() },
          }),
        routeAutoConnectTimeline: routeSystemTimelineEvent,
      }),
    [dispatch, stores],
  );

  useEffect(
    () =>
      connection.twitchAuthStatus === "unauthenticated"
        ? twitchController.schedulePoll(connection.twitchAuthPrompt)
        : undefined,
    [connection.twitchAuthPrompt, connection.twitchAuthStatus, twitchController],
  );

  const commandControllers = useMemo(
    () => ({
      speech: createSpeechController({
        reportError,
        reportInfo,
        dispatchSpeechStatus: (status) => dispatch({ type: "speech.status", status }),
      }),
      queue: createQueueController({
        reportError,
        reportInfo,
        dispatchQueueSnapshot: (snapshot) => {
          if (snapshot) dispatch({ type: "speech.snapshot", snapshot });
        },
      }),
      launcher: createLauncherController({
        reportError,
        reportInfo,
        dispatchLauncherItems: (items) => dispatch({ type: "launcher.changed", items }),
      }),
    }),
    [dispatch],
  );

  useEffect(
    () =>
      subscribeDomainEvents({
        stores,
        bridge: {
          getAppEventsSnapshot,
          speechQueueReload,
          subscribeAppLogEvents,
          subscribeTwitchStatusEvents,
          subscribeTwitchChatMessageEvents,
          subscribeSpeechStatusEvents,
          subscribeSpeechQueueUpdatedEvents,
        },
        reportNotification,
        replaySystemLog: addSystemChatMessage,
        onRestored: () => setEventsRestored(true),
        routeSystemTimelineEvent,
        speechRecoveryMessage: speechRecoveryTimelineEvent,
        twitchTimelineEvent: timelineEventFromTwitchStatus,
      }),
    [],
  );
  useEffect(() => {
    if (
      !eventsRestored ||
      autoConnectAttempted.current ||
      !settings?.twitch.autoConnect ||
      connection.twitchAuthStatus !== "authenticated" ||
      connection.twitchConnectionStatus !== "disconnected"
    )
      return;
    autoConnectAttempted.current = true;
    void twitchController.connect({ automatic: true });
  }, [
    eventsRestored,
    settings?.twitch.autoConnect,
    connection.twitchAuthStatus,
    connection.twitchConnectionStatus,
    twitchController,
  ]);

  useEffect(() => {
    if (!eventsRestored || !settings || !isDesktopRuntime()) return;
    return startSpeechHealthMonitor({
      probe: speechHealthProbe,
      getHealth: () => stores.connection.getState().speechAdapterHealth,
      onRecovered: (message) => {
        reportInfo(message, "event");
        const phase = stores.queue.getState().phase;
        routeSystemTimelineEvent(speechRecoveryTimelineEvent(message, phase));
      },
    });
  }, [eventsRestored, settings?.speech.bouyomiHost, settings?.speech.bouyomiPort]);

  const handleSpeechTest = commandControllers.speech.test;
  const handleSpeechHealthCheck = commandControllers.speech.healthCheck;
  const handleSpeechDiagnostics = commandControllers.speech.diagnostics;

  function handleSettingsUpdate(patch: AppSettingsPatch): Promise<boolean> {
    return settingsMutation.current.mutate(patch);
  }

  const handleTwitchStartAuth = twitchController.startAuth;
  const handleTwitchPollAuth = () => {
    void twitchController.pollAuth();
  };
  const handleTwitchValidateAuth = twitchController.validateAuth;
  const handleTwitchConnect = () => {
    void twitchController.connect();
  };
  const handleTwitchStopChat = () => {
    void twitchController.stopChat();
  };
  const handleTwitchDisconnect = twitchController.disconnect;
  const handleOpenExternalUrl = twitchController.openExternalUrl;

  const handleSpeechControl = commandControllers.speech.control;

  useStreamHotkeys({
    onToggleSpeech: () => {
      void handleSpeechControl(queue.phase === "paused" ? "resume" : "pause");
    },
    onSkipSpeech: () => {
      void handleSpeechControl("skip");
    },
    onOpenSettings: () => navigate("/settings"),
  });

  const handleQueueReload = commandControllers.queue.reload;
  const handleQueueRemove = commandControllers.queue.remove;
  const handleQueueDismiss = commandControllers.queue.dismiss;
  const handleQueueDismissHistory = commandControllers.queue.dismissHistory;
  const handleQueueRetry = commandControllers.queue.retry;
  const handleLauncherAdd = commandControllers.launcher.add;
  const handleLauncherRemove = commandControllers.launcher.remove;
  const handleLauncherLaunch = commandControllers.launcher.launch;
  const handleLauncherLaunchAll = commandControllers.launcher.launchAll;

  return (
    <ExitProtectionProvider reportError={reportError}>
      <DomainControllerActionsProvider
        actions={{
          updateSettings: handleSettingsUpdate,
          speechHealthCheck: handleSpeechHealthCheck,
          speechDiagnostics: handleSpeechDiagnostics,
          speechTest: handleSpeechTest,
          speechControl: handleSpeechControl,
          queueReload: handleQueueReload,
          queueRemove: handleQueueRemove,
          queueDismiss: handleQueueDismiss,
          queueDismissHistory: handleQueueDismissHistory,
          queueRetry: handleQueueRetry,
          launcherAdd: handleLauncherAdd,
          launcherRemove: handleLauncherRemove,
          launcherLaunch: handleLauncherLaunch,
          launcherLaunchAll: handleLauncherLaunchAll,
          twitchStartAuth: handleTwitchStartAuth,
          twitchPollAuth: () => {
            void handleTwitchPollAuth();
          },
          twitchValidateAuth: handleTwitchValidateAuth,
          twitchDisconnect: handleTwitchDisconnect,
          twitchConnect: () => {
            void handleTwitchConnect();
          },
          twitchStopChat: () => {
            void handleTwitchStopChat();
          },
          openExternalUrl: handleOpenExternalUrl,
          clearWarnings: () => dispatch({ type: "warnings.cleared" }),
        }}
      >
        {children}
      </DomainControllerActionsProvider>
    </ExitProtectionProvider>
  );
}

const AppShellLayout = memo(function AppShellLayout({
  onRouteCommit,
}: {
  onRouteCommit?: ProfilerOnRenderCallback;
}) {
  const displayScale = useDisplayScale();
  return (
    <div className={APP_SHELL_CLASS_NAME}>
      <CloseAwareTitleBar
        scale={displayScale.scale}
        scaleMode={displayScale.mode}
        onScaleModeChange={displayScale.setMode}
      />
      <ActivityBar />
      <DomainSidePanel />
      <Profiler id="app-route-body" onRender={onRouteCommit ?? (() => undefined)}>
        <MainView showStartupGuide={showStartupGuideForSession} />
      </Profiler>
      <DomainStatusBar />
      <DomainLiveStatusAnnouncer />
      <ResizeHandles />
    </div>
  );
});

function CloseAwareTitleBar({
  scale,
  scaleMode,
  onScaleModeChange,
}: {
  scale: number;
  scaleMode: Parameters<typeof TitleBar>[0]["scaleMode"];
  onScaleModeChange: Parameters<typeof TitleBar>[0]["onScaleModeChange"];
}) {
  const { requestWindowClose } = useExitController();
  return (
    <TitleBar
      scale={scale}
      scaleMode={scaleMode}
      onScaleModeChange={onScaleModeChange}
      onClose={requestWindowClose}
    />
  );
}
