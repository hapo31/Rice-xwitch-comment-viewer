import { createContext, useContext, useEffect, useRef, type MutableRefObject } from "react";

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
 * consulting the latest registered draft. Tauri registers the listener
 * asynchronously, so replacing it whenever a draft changes leaves a window
 * where an Alt+F4 request can bypass the confirmation dialog.
 */
export function createNativeCloseHandler(
  activeChangeRef: MutableRefObject<UnsavedChange | undefined>,
  requestConfirmation: () => void,
) {
  return (event: CloseRequestedEvent) => {
    if (!activeChangeRef.current) return;
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

export function UnsavedChangesDialog({
  onSave,
  onDiscard,
  onCancel,
  saveDisabled = false,
}: {
  onSave: () => void;
  onDiscard: () => void;
  onCancel: () => void;
  saveDisabled?: boolean;
}) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-zinc-950/70 p-4" role="presentation">
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="unsaved-changes-title"
        aria-describedby="unsaved-changes-description"
        className="w-full max-w-md border border-zinc-700 bg-zinc-900 p-5 shadow-xl"
      >
        <h2 id="unsaved-changes-title" className="text-base font-semibold text-zinc-100">未保存の変更があります</h2>
        <p id="unsaved-changes-description" className="mt-2 text-sm text-zinc-400">
          保存してから移動または終了しますか？
        </p>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button type="button" onClick={onCancel} autoFocus className="border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:border-sky-400">
            キャンセル
          </button>
          <button type="button" onClick={onDiscard} className="border border-rose-500/70 px-3 py-2 text-sm text-rose-300 hover:bg-rose-500/10">
            破棄して続ける
          </button>
          <button type="button" onClick={onSave} disabled={saveDisabled} className="border border-sky-500 bg-sky-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-sky-400 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500">
            保存して続ける
          </button>
        </div>
      </section>
    </div>
  );
}
