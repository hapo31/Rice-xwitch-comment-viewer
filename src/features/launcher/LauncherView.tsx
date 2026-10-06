import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AppWindow, ExternalLink, Layers3, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { presentError } from "../../presentation/errors";
import {
  launcherLaunchSummary,
  launcherTileColor,
  partitionApplicationPaths,
  sortLauncherItems,
} from "../../presentation/launcher";
import { routeHeadingId } from "../../routeAccessibility";
import { useDomainStores } from "../../stores/domainStores";
import { getLauncherCapabilities, isDesktopRuntime } from "../../tauri/client";
import type {
  LauncherAddResult,
  LauncherCapabilities,
  LauncherItem,
  LauncherLaunchFailure,
  LauncherLaunchResult,
} from "../../types";
import { type LauncherDragDropHandlers, subscribeLauncherDragDrop } from "./dragDropListener";

import { LauncherItemMenu } from "./LauncherItemMenu";

interface LauncherViewProps {
  items: LauncherItem[];
  isReady: boolean;
  onAdd: (paths: string[]) => Promise<LauncherAddResult>;
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
  const stores = useDomainStores();
  const [busyAction, setBusyAction] = useState<string>();
  const [isDragActive, setIsDragActive] = useState(false);
  const [notice, setNotice] = useState(defaultLauncherNotice);
  const [launchFailures, setLaunchFailures] = useState<LauncherLaunchFailure[]>([]);
  const [capabilities, setCapabilities] = useState<LauncherCapabilities>({
    canRegisterApplications: false,
    canLaunchApplications: false,
    reason: "ランチャーのOS対応状況を確認しています。",
  });
  const capabilitiesRef = useRef(capabilities);
  capabilitiesRef.current = capabilities;
  const headingRef = useRef<HTMLHeadingElement>(null);
  const orderedItems = useMemo(() => sortLauncherItems(items), [items]);
  const isReadyRef = useRef(isReady);
  const onAddRef = useRef(onAdd);

  isReadyRef.current = isReady;
  onAddRef.current = onAdd;

  useEffect(() => {
    let active = true;
    void getLauncherCapabilities().then(
      (value) => {
        if (active) setCapabilities(value);
      },
      () => {
        if (active)
          setCapabilities({
            canRegisterApplications: false,
            canLaunchApplications: false,
            reason: "ランチャーのOS対応状況を確認できません。アプリを再起動してください。",
          });
      },
    );
    return () => {
      active = false;
    };
  }, []);

  const addPaths = useCallback(async (paths: string[]) => {
    if (!capabilitiesRef.current.canRegisterApplications) {
      setNotice(capabilitiesRef.current.reason ?? "このOSではアプリを登録できません。");
      return;
    }
    if (!isReadyRef.current) {
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
      const { addedCount } = await onAddRef.current(accepted);
      const rejectedNote = rejected.length > 0 ? `（未対応の ${rejected.length} 件は除外）` : "";
      if (addedCount === 0) {
        setNotice(`選択したアプリはすでに登録されています。${rejectedNote}`);
      } else {
        const duplicateCount = accepted.length - addedCount;
        const duplicateNote = duplicateCount > 0 ? ` ${duplicateCount} 件は登録済みです。` : "";
        setNotice(`${addedCount} 件を登録しました。${duplicateNote}${rejectedNote}`);
      }
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }, []);

  const dragDropHandlersRef = useRef<LauncherDragDropHandlers>({
    onEnter: () => undefined,
    onOver: () => undefined,
    onLeave: () => undefined,
    onDrop: () => undefined,
  });
  dragDropHandlersRef.current = {
    onEnter: () => {
      setNotice("アプリをここにドロップして追加します。");
      setIsDragActive(true);
    },
    onOver: () => setIsDragActive(true),
    onLeave: () => {
      setIsDragActive(false);
      setNotice(defaultLauncherNotice);
    },
    onDrop: (paths) => {
      setIsDragActive(false);
      void addPaths(paths);
    },
  };

  useEffect(() => {
    if (!isDesktopRuntime() || !capabilities.canRegisterApplications) {
      return;
    }

    return subscribeLauncherDragDrop(
      (listener) => getCurrentWebview().onDragDropEvent(listener),
      dragDropHandlersRef,
      () => setNotice("ドラッグ＆ドロップの監視に失敗しました。画面を再読み込みしてください。"),
    );
  }, [capabilities.canRegisterApplications]);

  async function selectApplications() {
    if (!capabilities.canRegisterApplications) {
      setNotice(capabilities.reason ?? "このOSではアプリを登録できません。");
      return;
    }
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
      const presented = presentError(error, "launcher");
      stores.logs.dispatch({
        type: "log.added",
        log: { level: "error", message: presented.details, occurredAtMs: Date.now() },
      });
      setNotice(presented.message);
    }
  }

  async function launchItem(item: LauncherItem) {
    if (!capabilities.canLaunchApplications) {
      setNotice(capabilities.reason ?? "このOSではアプリを起動できません。");
      return;
    }
    setBusyAction(`launch:${item.id}`);
    setLaunchFailures([]);
    try {
      const result = await onLaunch(item.id);
      setLaunchFailures(result.failures);
      setNotice(
        result.launchedCount > 0 && result.failures.length === 0
          ? `${item.displayName} の起動プロセスを開始しました。アプリの準備完了は未確認です。`
          : `${item.displayName} を起動できませんでした: ${result.failures[0]?.message ?? "起動エラー"}`,
      );
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function launchAll() {
    if (!capabilities.canLaunchApplications) {
      setNotice(capabilities.reason ?? "このOSではアプリを起動できません。");
      return;
    }
    setBusyAction("launch-all");
    setLaunchFailures([]);
    try {
      const result = await onLaunchAll();
      setLaunchFailures(result.failures);
      setNotice(launcherLaunchSummary(result));
    } catch (error) {
      setNotice(readableError(error));
    } finally {
      setBusyAction(undefined);
    }
  }

  async function removeItem(item: LauncherItem) {
    setBusyAction(`remove:${item.id}`);
    try {
      await onRemove(item.id);
      headingRef.current?.focus();
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
          <h1
            ref={headingRef}
            id={routeHeadingId}
            tabIndex={-1}
            className="truncate text-sm font-semibold text-zinc-100"
          >
            Launcher
          </h1>
          <p className="truncate text-xs text-zinc-400">
            よく使うアプリを登録して、ここからすばやく起動します
          </p>
        </div>
        <button
          type="button"
          disabled={
            !isReady ||
            !capabilities.canLaunchApplications ||
            items.length === 0 ||
            Boolean(busyAction)
          }
          onClick={() => void launchAll()}
          className="flex h-8 shrink-0 items-center gap-2 border border-sky-600 bg-sky-700 px-3 text-xs font-medium text-white transition-colors hover:bg-sky-600 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-400"
        >
          <Layers3 className="h-4 w-4" />
          {busyAction === "launch-all" ? "起動中…" : "一斉に起動"}
        </button>
      </header>

      <section className="relative flex h-[calc(100%-3.5rem)] min-h-0 flex-col">
        {capabilities.reason && (
          <p role="note" className="border-b border-zinc-800 px-5 py-3 text-xs text-amber-300">
            {capabilities.reason}
          </p>
        )}
        <div className="min-h-0 flex-1 overflow-auto p-5">
          {launchFailures.length > 0 && (
            <section
              aria-label="起動できなかったアプリ"
              className="mb-4 border border-amber-700 bg-amber-950/30 p-3 text-xs text-amber-200"
            >
              <h2 className="mb-2 font-semibold">起動できなかったアプリ</h2>
              <ul className="space-y-2">
                {launchFailures.map((failure) => (
                  <li key={failure.itemId} className="break-words">
                    <span className="font-semibold">{failure.displayName}</span>: {failure.message}
                  </li>
                ))}
              </ul>
              <p className="mt-3">
                ショートカットのプロパティでリンク先・作業フォルダーを修正するか、正しいアプリを再登録してください。権限が必要なアプリはWindowsから手動で起動してください。
              </p>
            </section>
          )}
          <div className="grid grid-cols-[repeat(auto-fill,minmax(132px,156px))] auto-rows-[156px] gap-3">
            {orderedItems.map((item) => {
              const isBusy = busyAction?.endsWith(item.id) ?? false;
              return (
                <article
                  key={item.id}
                  className="group relative isolate overflow-visible border border-white/10 text-white shadow-xs focus-within:ring-2 focus-within:ring-sky-300"
                  style={{ backgroundColor: launcherTileColor(item) }}
                >
                  <button
                    type="button"
                    disabled={!capabilities.canLaunchApplications || Boolean(busyAction)}
                    onClick={() => void launchItem(item)}
                    aria-label={`${item.displayName} を起動`}
                    className="flex h-full w-full flex-col items-center justify-center px-3 pb-9 pt-3 text-center transition-[filter,transform] hover:brightness-110 active:scale-[0.98] disabled:cursor-wait disabled:opacity-70"
                  >
                    {item.iconDataUrl ? (
                      <img
                        src={item.iconDataUrl}
                        alt=""
                        className="h-16 w-16 object-contain drop-shadow"
                      />
                    ) : (
                      <AppWindow
                        className="h-14 w-14 stroke-[1.25] drop-shadow"
                        aria-hidden="true"
                      />
                    )}
                    {isBusy && <span className="mt-2 text-[11px] text-white/80">処理中…</span>}
                  </button>

                  <div className="pointer-events-none absolute inset-x-0 bottom-0 flex h-9 items-center bg-black/20 pl-3 pr-9">
                    <span
                      className="truncate text-left text-xs font-medium"
                      title={item.displayName}
                    >
                      {item.displayName}
                    </span>
                  </div>
                  <LauncherItemMenu
                    name={item.displayName}
                    disabled={Boolean(busyAction)}
                    onRemove={() => void removeItem(item)}
                  />
                </article>
              );
            })}

            <button
              type="button"
              disabled={!isReady || !capabilities.canRegisterApplications || Boolean(busyAction)}
              onClick={() => void selectApplications()}
              aria-label="アプリをランチャーに追加"
              className="group flex h-[156px] w-full flex-col items-center justify-center border border-dashed border-zinc-700 bg-zinc-900/60 text-zinc-400 transition-colors hover:border-sky-500 hover:bg-zinc-900 hover:text-sky-300 disabled:cursor-wait disabled:opacity-60"
            >
              <span className="flex h-14 w-14 items-center justify-center border border-zinc-700 bg-zinc-850 group-hover:border-sky-600">
                <Plus className="h-7 w-7" />
              </span>
              <span className="mt-3 text-xs">アプリを追加</span>
            </button>
          </div>
        </div>

        <div className="flex min-h-10 shrink-0 items-center justify-between gap-4 border-t border-zinc-800 bg-zinc-900/95 px-5 py-2 text-[11px] text-zinc-400">
          <p className="break-words" aria-live="polite">
            {notice}
          </p>
          <p className="hidden shrink-0 items-center gap-1.5 text-zinc-400 lg:flex">
            <ExternalLink className="h-3 w-3" />
            タイルを押すと起動・右下の … から削除
          </p>
        </div>

        {isDragActive && (
          <div className="pointer-events-none absolute inset-3 z-40 flex items-center justify-center border-2 border-dashed border-sky-400 bg-sky-950/85 text-sky-100 backdrop-blur-xs">
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
  return presentError(error, "launcher").message;
}
