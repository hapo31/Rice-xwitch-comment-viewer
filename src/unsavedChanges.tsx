import * as Dialog from "@radix-ui/react-dialog";
import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  type MutableRefObject,
  type ReactNode,
} from "react";

export interface UnsavedChange {
  isDirty: boolean;
  save: () => Promise<boolean>;
  discard: () => void;
}

interface UnsavedChangesRegistry {
  register: (id: string, change: UnsavedChange) => void;
  unregister: (id: string) => void;
}

export const UnsavedChangesContext = createContext<UnsavedChangesRegistry | undefined>(undefined);

type CloseRequestedEvent = { preventDefault: () => void };

/**
 * Keeps the native close listener stable for the life of the app while still
 * consulting the latest exit risk. Tauri registers the listener asynchronously,
 * so replacing it whenever state changes leaves a window where an Alt+F4
 * request can bypass the confirmation dialog.
 */
export function createNativeCloseHandler(
  confirmationRequiredRef: MutableRefObject<boolean>,
  requestConfirmation: () => void,
) {
  return (event: CloseRequestedEvent) => {
    if (!confirmationRequiredRef.current) return;
    event.preventDefault();
    requestConfirmation();
  };
}

/** Registers a screen-local draft with the app-wide navigation and close guard. */
export function useUnsavedChanges(id: string, change: UnsavedChange) {
  const registry = useContext(UnsavedChangesContext);
  const changeRef = useRef(change);
  changeRef.current = change;

  useEffect(() => {
    if (!registry) {
      return;
    }

    registry.register(id, {
      get isDirty() {
        return changeRef.current.isDirty;
      },
      save: () => changeRef.current.save(),
      discard: () => changeRef.current.discard(),
    });
    return () => registry.unregister(id);
  }, [change.isDirty, id, registry]);
}

/** Shared modal behavior; operation approval remains owned by ExitProtectionProvider. */
function ModalDialog({
  children,
  onCancel,
  cancelDisabled = false,
}: {
  children: ReactNode;
  onCancel: () => void;
  cancelDisabled?: boolean;
}) {
  const previousFocus = useRef<HTMLElement | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (cancelDisabled) contentRef.current?.focus();
  }, [cancelDisabled]);
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open && !cancelDisabled) onCancel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 flex items-center justify-center bg-zinc-950/70 p-4">
          <Dialog.Content
            ref={contentRef}
            aria-modal="true"
            className="w-full max-w-md border border-zinc-700 bg-zinc-900 p-5 text-zinc-100 shadow-xl"
            onOpenAutoFocus={() => {
              previousFocus.current =
                document.activeElement instanceof HTMLElement ? document.activeElement : null;
            }}
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              const target = previousFocus.current;
              if (document.activeElement !== document.body) return;
              if (target?.isConnected && target !== document.body) target.focus();
              else if (document.activeElement === document.body)
                document.querySelector<HTMLElement>("[data-modal-focus-fallback]")?.focus();
            }}
            onEscapeKeyDown={(event) => {
              if (cancelDisabled) event.preventDefault();
            }}
            onInteractOutside={(event) => event.preventDefault()}
          >
            {children}
          </Dialog.Content>
        </Dialog.Overlay>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function UnsavedChangesDialog({
  onSave,
  onDiscard,
  onCancel,
  saveDisabled = false,
  hasUnsavedChanges = true,
  isSaving = false,
}: {
  onSave: () => void;
  onDiscard: () => void;
  onCancel: () => void;
  saveDisabled?: boolean;
  hasUnsavedChanges?: boolean;
  isSaving?: boolean;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  useLayoutEffect(() => {
    // A focused Save button becoming disabled can leave the browser focus on body.
    // Move to the remaining safe action; the primitive continues to own Tab looping.
    if (
      isSaving &&
      (document.activeElement === document.body ||
        (document.activeElement instanceof HTMLButtonElement && document.activeElement.disabled))
    )
      cancelRef.current?.focus();
  }, [isSaving]);
  return (
    <ModalDialog onCancel={onCancel}>
      <>
        <Dialog.Title className="text-base font-semibold text-zinc-100">
          {hasUnsavedChanges ? "未保存の変更があります" : "移動または終了しますか？"}
        </Dialog.Title>
        <Dialog.Description className="mt-2 text-sm text-zinc-400">
          {hasUnsavedChanges
            ? "保存してから移動または終了しますか？"
            : "変更は保存されました。操作を続けるか確認してください。"}
        </Dialog.Description>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            className="border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:border-sky-400"
          >
            キャンセル
          </button>
          {hasUnsavedChanges && (
            <button
              type="button"
              onClick={onDiscard}
              className="border border-rose-500/70 px-3 py-2 text-sm text-rose-300 hover:bg-rose-500/10"
            >
              破棄して続ける
            </button>
          )}
          <button
            type="button"
            onClick={onSave}
            disabled={saveDisabled}
            className="border border-sky-500 bg-sky-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-sky-400 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
          >
            {isSaving ? "保存しています…" : hasUnsavedChanges ? "保存して続ける" : "続ける"}
          </button>
        </div>
      </>
    </ModalDialog>
  );
}

export function ActiveOperationsExitDialog({
  onConfirm,
  onCancel,
  isClosing = false,
}: {
  onConfirm: () => void;
  onCancel: () => void;
  isClosing?: boolean;
}) {
  return (
    <ModalDialog onCancel={onCancel} cancelDisabled={isClosing}>
      <>
        <Dialog.Title className="text-base font-semibold text-zinc-100">
          配信支援を停止して終了しますか？
        </Dialog.Title>
        <Dialog.Description className="mt-2 text-sm text-zinc-400">
          Twitch
          チャット受信または読み上げキューが動作中です。終了するとチャット受信を停止し、待機中の読み上げをクリアします。
        </Dialog.Description>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={isClosing}
            className="border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:border-sky-400 disabled:cursor-not-allowed disabled:text-zinc-500"
          >
            キャンセル
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={isClosing}
            className="border border-rose-500/70 px-3 py-2 text-sm text-rose-300 hover:bg-rose-500/10 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:text-zinc-500"
          >
            {isClosing ? "停止しています…" : "停止して終了"}
          </button>
        </div>
      </>
    </ModalDialog>
  );
}
