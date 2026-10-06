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
  const closingRef = useRef(false);
  const saveContinuation = useRef<symbol | undefined>(undefined);
  const [isSavingContinuation, setIsSavingContinuation] = useState(false);
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
  const confirmationChange = useRef<UnsavedChange | undefined>(undefined);
  if (!closeRequested && blocker.state !== "blocked") confirmationChange.current = undefined;
  else if (activeUnsavedChange) confirmationChange.current = activeUnsavedChange;
  const requestedChange = confirmationChange.current;

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
    if (closingRef.current) return;
    closingRef.current = true;
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
      closingRef.current = false;
      setIsClosing(false);
      reportError(error, "exit");
    }
  }, [hasActiveChat, hasPendingSpeech, reportError, stores]);

  const currentRequest = useRef({ closeRequested, blocker, completeWindowClose });
  currentRequest.current = { closeRequested, blocker, completeWindowClose };
  const cancelSaveContinuation = useCallback(() => {
    saveContinuation.current = undefined;
    setIsSavingContinuation(false);
  }, []);
  const requestCloseConfirmation = useCallback(() => {
    if (closingRef.current || currentRequest.current.closeRequested) return;
    cancelSaveContinuation();
    setCloseRequested(true);
  }, [cancelSaveContinuation]);

  useEffect(
    () => () => {
      // Saving can finish after this provider unmounts; it must not navigate/exit.
      saveContinuation.current = undefined;
    },
    [],
  );

  async function saveAndContinue() {
    if (!requestedChange || saveContinuation.current) return;
    const operation = Symbol("save continuation");
    const requestedClose = closeRequested;
    const requestedLocation = blocker.location?.key;
    saveContinuation.current = operation;
    setIsSavingContinuation(true);
    try {
      const saved = !requestedChange.isDirty || (await requestedChange.save());
      if (saveContinuation.current !== operation || !saved) return;
      const current = currentRequest.current;
      if (
        current.closeRequested !== requestedClose ||
        current.blocker.location?.key !== requestedLocation
      )
        return;
      if (requestedClose) void current.completeWindowClose();
      else if (current.blocker.state === "blocked") current.blocker.proceed();
    } catch (error) {
      if (saveContinuation.current === operation) reportError(error, "exit");
    } finally {
      if (saveContinuation.current === operation) cancelSaveContinuation();
    }
  }

  const requestWindowClose = useCallback(() => {
    if (exitConfirmationRequired) {
      requestCloseConfirmation();
      return;
    }
    void completeWindowClose();
  }, [completeWindowClose, exitConfirmationRequired, requestCloseConfirmation]);
  const controller = useMemo(() => ({ requestWindowClose }), [requestWindowClose]);

  useEffect(() => {
    if (!isDesktopRuntime()) return;
    return subscribeWithCleanup(
      [
        () =>
          getCurrentWindow().onCloseRequested(
            createNativeCloseHandler(closeConfirmationRequiredRef, requestCloseConfirmation),
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
  }, [stores, requestCloseConfirmation]);

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
        {(blocker.state === "blocked" || closeRequested) && requestedChange && (
          <UnsavedChangesDialog
            hasUnsavedChanges={Boolean(activeUnsavedChange)}
            saveDisabled={isSavingContinuation}
            isSaving={isSavingContinuation}
            onCancel={() => {
              cancelSaveContinuation();
              if (blocker.state === "blocked") blocker.reset();
              setCloseRequested(false);
            }}
            onDiscard={() => {
              cancelSaveContinuation();
              requestedChange.discard();
              if (closeRequested) void completeWindowClose();
              else if (blocker.state === "blocked") blocker.proceed();
            }}
            onSave={() => void saveAndContinue()}
          />
        )}
        {closeRequested && !requestedChange && (
          <ActiveOperationsExitDialog
            isClosing={isClosing}
            onCancel={() => {
              cancelSaveContinuation();
              setCloseRequested(false);
            }}
            onConfirm={() => void completeWindowClose()}
          />
        )}
      </ExitControllerContext.Provider>
    </UnsavedChangesContext.Provider>
  );
}
