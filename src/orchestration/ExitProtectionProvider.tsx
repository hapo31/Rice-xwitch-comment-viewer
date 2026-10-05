import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useBlocker } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appExit, isDesktopRuntime, speechControl, twitchStopChat } from "../tauri/client";
import { useConnectionSelector, useDomainStores, useQueueSelector } from "../stores/domainStores";
import { dispatchDomainAction } from "./domainOrchestration";
import { subscribeWithCleanup } from "../tauri/subscriptions";
import { hasActiveTwitchChat, hasPendingSpeechWork, requiresExitConfirmation } from "../exitSafety";
import {
  ActiveOperationsExitDialog,
  createNativeCloseHandler,
  UnsavedChangesContext,
  UnsavedChangesDialog,
  type UnsavedChange,
} from "../unsavedChanges";

interface ExitController {
  requestWindowClose: () => void;
}

const ExitControllerContext = createContext<ExitController | undefined>(undefined);

export function useExitController(): ExitController {
  const controller = useContext(ExitControllerContext);
  if (!controller) throw new Error("ExitProtectionProvider is required");
  return controller;
}

export function ExitProtectionProvider({
  children,
  reportError,
}: {
  children: ReactNode;
  reportError: (error: unknown, operation: "exit") => unknown;
}) {
  const stores = useDomainStores();
  const connectionStatus = useConnectionSelector((state) => state.twitchConnectionStatus);
  const queue = useQueueSelector((state) => state);
  const unsavedChanges = useRef(new Map<string, UnsavedChange>());
  const closeConfirmationRequiredRef = useRef(false);
  const [, setUnsavedChangesVersion] = useState(0);
  const [closeRequested, setCloseRequested] = useState(false);
  const [isClosing, setIsClosing] = useState(false);
  const hasActiveChat = hasActiveTwitchChat(connectionStatus);
  const hasPendingSpeech = hasPendingSpeechWork(queue.phase, queue.items);
  const activeUnsavedChange = [...unsavedChanges.current.values()].find((change) => change.isDirty);
  const exitConfirmationRequired = requiresExitConfirmation(
    connectionStatus,
    queue.phase,
    queue.items,
    Boolean(activeUnsavedChange),
  );
  closeConfirmationRequiredRef.current = exitConfirmationRequired;
  const blocker = useBlocker(Boolean(activeUnsavedChange));

  const registry = useMemo(
    () => ({
      register(id: string, change: UnsavedChange) {
        unsavedChanges.current.set(id, change);
        setUnsavedChangesVersion((version) => version + 1);
      },
      unregister(id: string) {
        unsavedChanges.current.delete(id);
        setUnsavedChangesVersion((version) => version + 1);
      },
    }),
    [],
  );

  const completeWindowClose = useCallback(async () => {
    setCloseRequested(false);
    setIsClosing(true);
    const results = await Promise.allSettled([
      hasActiveChat ? twitchStopChat() : Promise.resolve(),
      hasPendingSpeech ? speechControl("clear") : Promise.resolve(),
    ]);
    if (hasPendingSpeech && results[1].status === "fulfilled" && !isDesktopRuntime()) {
      dispatchDomainAction(stores, { type: "speech.status", status: "idle" });
    }
    for (const result of results) {
      if (result.status === "rejected") reportError(result.reason, "exit");
    }
    try {
      await appExit();
    } catch (error) {
      setIsClosing(false);
      reportError(error, "exit");
    }
  }, [hasActiveChat, hasPendingSpeech, reportError, stores]);

  const requestWindowClose = useCallback(() => {
    if (exitConfirmationRequired) {
      setCloseRequested(true);
      return;
    }
    void completeWindowClose();
  }, [completeWindowClose, exitConfirmationRequired]);
  const controller = useMemo(() => ({ requestWindowClose }), [requestWindowClose]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    return subscribeWithCleanup(
      [
        () =>
          getCurrentWindow().onCloseRequested(
            createNativeCloseHandler(closeConfirmationRequiredRef, () => setCloseRequested(true)),
          ),
      ],
      () =>
        dispatchDomainAction(stores, {
          type: "notification.added",
          notification: {
            severity: "warning",
            source: "event",
            message: "終了確認の監視に失敗しました。未保存の変更を確認してから終了してください。",
            occurredAtMs: Date.now(),
            correlationId: "app-close-subscription",
          },
        }),
    );
  }, [stores]);

  useEffect(() => {
    const preventUnload = (event: BeforeUnloadEvent) => {
      if (!activeUnsavedChange) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", preventUnload);
    return () => window.removeEventListener("beforeunload", preventUnload);
  }, [activeUnsavedChange]);

  return (
    <UnsavedChangesContext.Provider value={registry}>
      <ExitControllerContext.Provider value={controller}>
        {children}
        {(blocker.state === "blocked" || closeRequested) && activeUnsavedChange && (
          <UnsavedChangesDialog
            onCancel={() => {
              if (blocker.state === "blocked") blocker.reset();
              setCloseRequested(false);
            }}
            onDiscard={() => {
              activeUnsavedChange.discard();
              if (closeRequested) void completeWindowClose();
              else if (blocker.state === "blocked") blocker.proceed();
            }}
            onSave={() => {
              void activeUnsavedChange.save().then((saved) => {
                if (!saved) return;
                if (closeRequested) void completeWindowClose();
                else if (blocker.state === "blocked") blocker.proceed();
              });
            }}
          />
        )}
        {closeRequested && !activeUnsavedChange && (
          <ActiveOperationsExitDialog
            isClosing={isClosing}
            onCancel={() => setCloseRequested(false)}
            onConfirm={() => void completeWindowClose()}
          />
        )}
      </ExitControllerContext.Provider>
    </UnsavedChangesContext.Provider>
  );
}
