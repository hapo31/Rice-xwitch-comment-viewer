import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AppWindow, Ellipsis, ExternalLink, Layers3, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  launcherLaunchSummary,
  launcherTileColor,
  partitionApplicationPaths,
  sortLauncherItems,
} from "../../presentation/launcher";
import { isDesktopRuntime } from "../../tauri/client";
import type { LauncherItem, LauncherLaunchResult } from "../../types";

interface LauncherViewProps {
  items: LauncherItem[];
  isReady: boolean;
  onAdd: (paths: string[]) => Promise<LauncherItem[]>;
  onRemove: (itemId: string) => Promise<LauncherItem[]>;
  onLaunch: (itemId: string) => Promise<LauncherLaunchResult>;
  onLaunchAll: () => Promise<LauncherLaunchResult>;
}

const defaultLauncherNotice = "四角い ＋ ボタン、またはドラッグ＆ドロップでアプリを登録できます。";

export function LauncherView({
  items,
  isReady,
  onAdd,
  onRemove,
  onLaunch,
  onLaunchAll,
}: LauncherViewProps) {
  const [openMenuId, setOpenMenuId] = useState<string>();
  const [busyAction, setBusyAction] = useState<string>();
  const [isDragActive, setIsDragActive] = useState(false);
  const [notice, setNotice] = useState(defaultLauncherNotice);
  const orderedItems = useMemo(() => sortLauncherItems(items), [items]);

  const addPaths = useCallback(async (paths: string[]) => {
    if (!isReady) {
      setNotice("設定を読み込んでいます。少し待ってからもう一度お試しください。");
      return;
    }

    const { accepted, rejected } = partitionApplicationPaths(paths);
    if (accepted.length === 0) {
      setNotice("追加できるのは Windows アプリ（.exe）またはショートカット（.lnk）です。");
      return;
    }

    setBusyAction("add");
    try {
      const nextItems = await onAdd(accepted);
      const addedCount = Math.max(0, nextItems.length - items.length);
      const rejectedNote = rejected.length > 0 ? `（未対応の ${rejected.length} 件は除外）` : "";
      setNotice(`${addedCount} 件を登録しました。${rejectedNote}`);
      if (addedCount === 0) {
        setNotice(`選択したアプリはすでに登録されています。${rejectedNote}`);
      }
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }, [isReady, items.length, onAdd]);

  useEffect(() => {
    if (!isDesktopRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter") {
        setNotice("アプリをここにドロップして追加します。");
        setIsDragActive(true);
        return;
      }
      if (event.payload.type === "over") {
        setIsDragActive(true);
        return;
      }
      setIsDragActive(false);
      if (event.payload.type === "leave") {
        setNotice(defaultLauncherNotice);
      }
      if (event.payload.type === "drop") {
        void addPaths(event.payload.paths);
      }
    }).then((dispose) => {
      if (disposed) {
        dispose();
      } else {
        unlisten = dispose;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [addPaths]);

  useEffect(() => {
    if (!openMenuId) {
      return;
    }
    const closeMenu = () => setOpenMenuId(undefined);
    const closeMenuWithEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMenu();
      }
    };
    document.addEventListener("pointerdown", closeMenu);
    document.addEventListener("keydown", closeMenuWithEscape);
    return () => {
      document.removeEventListener("pointerdown", closeMenu);
      document.removeEventListener("keydown", closeMenuWithEscape);
    };
  }, [openMenuId]);

  async function selectApplications() {
    if (!isDesktopRuntime()) {
      setNotice("アプリの選択は Tauri デスクトップ版で利用できます。");
      return;
    }

    try {
      const selected = await open({
        title: "ランチャーに追加するアプリを選択",
        multiple: true,
        directory: false,
        filters: [{ name: "Windows アプリ", extensions: ["exe", "lnk"] }],
      });
      if (selected) {
        await addPaths(selected);
      }
    } catch (error) {
      setNotice(`ファイル選択画面を開けませんでした: ${readableError(error)}`);
    }
  }

  async function launchItem(item: LauncherItem) {
    setOpenMenuId(undefined);
    setBusyAction(`launch:${item.id}`);
    try {
      const result = await onLaunch(item.id);
      setNotice(result.failures.length === 0
        ? `${item.displayName} を起動しました。`
        : `${item.displayName} を起動できませんでした: ${result.failures[0]?.message ?? "起動エラー"}`);
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function launchAll() {
    setBusyAction("launch-all");
    try {
      setNotice(launcherLaunchSummary(await onLaunchAll()));
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function removeItem(item: LauncherItem) {
    setOpenMenuId(undefined);
    setBusyAction(`remove:${item.id}`);
    try {
      await onRemove(item.id);
      setNotice(`${item.displayName} をランチャーから削除しました。`);
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  return (
    <main className="relative col-span-2 col-start-2 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-14 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-5">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Launcher</h1>
          <p className="truncate text-xs text-zinc-500">よく使うアプリを登録して、ここからすばやく起動します</p>
        </div>
        <button
          type="button"
          disabled={!isReady || items.length === 0 || Boolean(busyAction)}
          onClick={() => void launchAll()}
          className="flex h-8 shrink-0 items-center gap-2 border border-sky-600 bg-sky-700 px-3 text-xs font-medium text-white transition-colors hover:bg-sky-600 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-500"
        >
          <Layers3 className="h-4 w-4" />
          {busyAction === "launch-all" ? "起動中…" : "一斉に起動"}
        </button>
      </header>

      <section className="relative flex h-[calc(100%-3.5rem)] min-h-0 flex-col">
        <div className="min-h-0 flex-1 overflow-auto p-5 pb-16">
          <div className="grid grid-cols-[repeat(auto-fill,minmax(132px,156px))] auto-rows-[156px] gap-3">
            {orderedItems.map((item) => {
              const isBusy = busyAction?.endsWith(item.id) ?? false;
              const isMenuOpen = openMenuId === item.id;
              return (
                <article
                  key={item.id}
                  className="group relative isolate overflow-visible border border-white/10 text-white shadow-sm focus-within:ring-2 focus-within:ring-sky-300"
                  style={{ backgroundColor: launcherTileColor(item) }}
                >
                  <button
                    type="button"
                    disabled={Boolean(busyAction)}
                    onClick={() => void launchItem(item)}
                    aria-label={`${item.displayName} を起動`}
                    className="flex h-full w-full flex-col items-center justify-center px-3 pb-9 pt-3 text-center transition-[filter,transform] hover:brightness-110 active:scale-[0.98] disabled:cursor-wait disabled:opacity-70"
                  >
                    {item.iconDataUrl ? (
                      <img src={item.iconDataUrl} alt="" className="h-16 w-16 object-contain drop-shadow" />
                    ) : (
                      <AppWindow className="h-14 w-14 stroke-[1.25] drop-shadow" aria-hidden="true" />
                    )}
                    {isBusy && <span className="mt-2 text-[11px] text-white/80">処理中…</span>}
                  </button>

                  <div className="pointer-events-none absolute inset-x-0 bottom-0 flex h-9 items-center bg-black/20 pl-3 pr-9">
                    <span className="truncate text-left text-xs font-medium" title={item.displayName}>{item.displayName}</span>
                  </div>
                  <button
                    type="button"
                    aria-label={`${item.displayName} のメニュー`}
                    aria-haspopup="menu"
                    aria-expanded={isMenuOpen}
                    aria-controls={`launcher-menu-${item.id}`}
                    disabled={Boolean(busyAction)}
                    onPointerDown={(event) => event.stopPropagation()}
                    onClick={(event) => {
                      event.stopPropagation();
                      setOpenMenuId((current) => current === item.id ? undefined : item.id);
                    }}
                    className="absolute bottom-0 right-0 z-10 flex h-9 w-9 items-center justify-center text-white/80 hover:bg-black/25 hover:text-white disabled:opacity-50"
                  >
                    <Ellipsis className="h-4 w-4" />
                  </button>

                  {isMenuOpen && (
                    <div
                      id={`launcher-menu-${item.id}`}
                      role="menu"
                      aria-label={`${item.displayName} の操作`}
                      onPointerDown={(event) => event.stopPropagation()}
                      className="absolute bottom-8 right-1 z-30 min-w-32 border border-zinc-700 bg-zinc-850 py-1 text-zinc-100 shadow-xl"
                    >
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => void removeItem(item)}
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs hover:bg-zinc-700 hover:text-rose-200"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                        削除
                      </button>
                    </div>
                  )}
                </article>
              );
            })}

            <button
              type="button"
              disabled={!isReady || Boolean(busyAction)}
              onClick={() => void selectApplications()}
              aria-label="アプリをランチャーに追加"
              className="group flex h-[156px] w-full flex-col items-center justify-center border border-dashed border-zinc-700 bg-zinc-900/60 text-zinc-500 transition-colors hover:border-sky-500 hover:bg-zinc-900 hover:text-sky-300 disabled:cursor-wait disabled:opacity-60"
            >
              <span className="flex h-14 w-14 items-center justify-center border border-zinc-700 bg-zinc-850 group-hover:border-sky-600">
                <Plus className="h-7 w-7" />
              </span>
              <span className="mt-3 text-xs">アプリを追加</span>
            </button>
          </div>
        </div>

        <div className="pointer-events-none absolute inset-x-0 bottom-0 flex min-h-10 items-center justify-between gap-4 border-t border-zinc-800 bg-zinc-900/95 px-5 py-2 text-[11px] text-zinc-500">
          <p className="truncate" aria-live="polite">{notice}</p>
          <p className="hidden shrink-0 items-center gap-1.5 text-zinc-600 lg:flex">
            <ExternalLink className="h-3 w-3" />
            タイルを押すと起動・右下の … から削除
          </p>
        </div>

        {isDragActive && (
          <div className="pointer-events-none absolute inset-3 z-40 flex items-center justify-center border-2 border-dashed border-sky-400 bg-sky-950/85 text-sky-100 backdrop-blur-sm">
            <div className="flex flex-col items-center gap-3 text-sm font-medium">
              <Plus className="h-10 w-10" />
              ここにドロップして追加
            </div>
          </div>
        )}
      </section>
    </main>
  );
}

function readableError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.replace(/^Error:\s*/, "");
}
