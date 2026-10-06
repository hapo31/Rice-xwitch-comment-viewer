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

/** Uses the browser's modal dialog behavior for focus containment and background inertness. */
function ModalDialog({
  children,
  onCancel,
  cancelDisabled = false,
  labelledBy,
  describedBy,
}: {
  children: ReactNode;
  onCancel: () => void;
  cancelDisabled?: boolean;
  labelledBy: string;
  describedBy: string;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useLayoutEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialog.showModal();
    dialog.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
    return () => {
      if (dialog.open) dialog.close();
      window.setTimeout(() => {
        if (
          (!previousFocus || previousFocus === document.body || !previousFocus.isConnected) &&
          document.activeElement === document.body
        )
          document.querySelector<HTMLElement>("[data-modal-focus-fallback]")?.focus();
      }, 0);
    };
  }, []);

  return (
    <dialog
      ref={dialogRef}
      tabIndex={-1}
      aria-modal="true"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      onCancel={(event) => {
        // A modal confirmation must never be dismissed by a browser default path.
        event.preventDefault();
        if (!cancelDisabled) onCancel();
      }}
      className="fixed inset-0 m-0 flex h-full w-full max-h-none max-w-none items-center justify-center border-0 bg-transparent p-4 text-zinc-100 backdrop:bg-zinc-950/70"
    >
      {children}
    </dialog>
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
  return (
    <ModalDialog
      onCancel={onCancel}
      labelledBy="unsaved-changes-title"
      describedBy="unsaved-changes-description"
    >
      <section className="w-full max-w-md border border-zinc-700 bg-zinc-900 p-5 shadow-xl">
        <h2 id="unsaved-changes-title" className="text-base font-semibold text-zinc-100">
          {hasUnsavedChanges ? "未保存の変更があります" : "移動または終了しますか？"}
        </h2>
        <p id="unsaved-changes-description" className="mt-2 text-sm text-zinc-400">
          {hasUnsavedChanges
            ? "保存してから移動または終了しますか？"
            : "変更は保存されました。操作を続けるか確認してください。"}
        </p>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button
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
      </section>
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
    <ModalDialog
      onCancel={onCancel}
      cancelDisabled={isClosing}
      labelledBy="active-operations-exit-title"
      describedBy="active-operations-exit-description"
    >
      <section className="w-full max-w-md border border-zinc-700 bg-zinc-900 p-5 shadow-xl">
        <h2 id="active-operations-exit-title" className="text-base font-semibold text-zinc-100">
          配信支援を停止して終了しますか？
        </h2>
        <p id="active-operations-exit-description" className="mt-2 text-sm text-zinc-400">
          Twitch
          チャット受信または読み上げキューが動作中です。終了するとチャット受信を停止し、待機中の読み上げをクリアします。
        </p>
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
      </section>
    </ModalDialog>
  );
}
