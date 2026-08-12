import { Maximize2, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type React from "react";
import type { UiScaleMode } from "../hooks/useDisplayScale";
import { appExit } from "../tauri/client";
import { subscribeWithCleanup } from "../tauri/subscriptions";

type ResizeDirection = "East" | "North" | "NorthEast" | "NorthWest" | "South" | "SouthEast" | "SouthWest" | "West";

interface TitleBarProps {
  scale: number;
  scaleMode: UiScaleMode;
  onScaleModeChange: (mode: UiScaleMode) => void;
  onClose?: () => void;
}

const scaleOptions: { mode: UiScaleMode; label: string }[] = [
  { mode: "auto", label: "Auto" },
  { mode: "1", label: "100%" },
  { mode: "1.25", label: "125%" },
  { mode: "1.5", label: "150%" },
];

export function TitleBar({ scale, scaleMode, onScaleModeChange, onClose }: TitleBarProps) {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let isMounted = true;

    async function syncMaximized() {
      try {
        const maximized = await appWindow.isMaximized();
        if (isMounted) {
          setIsMaximized(maximized);
        }
      } catch {
        if (isMounted) {
          setIsMaximized(false);
        }
      }
    }

    syncMaximized();
    const dispose = subscribeWithCleanup([() => appWindow.onResized(() => {
        void syncMaximized();
      })]);

    return () => {
      isMounted = false;
      dispose();
    };
  }, []);

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow() {
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    setIsMaximized(await appWindow.isMaximized());
  }

  async function closeWindow() {
    if (onClose) {
      onClose();
      return;
    }
    await appExit();
  }

  async function startDrag(event: React.MouseEvent<HTMLElement>) {
    if (event.detail > 1) {
      return;
    }

    await getCurrentWindow().startDragging();
  }

  return (
    <header className="col-span-3 row-start-1 flex h-8 select-none items-center border-b border-zinc-800 bg-zinc-900 text-xs text-zinc-300">
      <div
        data-tauri-drag-region
        onMouseDown={(event) => void startDrag(event)}
        onDoubleClick={() => void toggleMaximizeWindow()}
        className="flex min-w-0 flex-1 items-center gap-3 self-stretch px-3"
      >
        <span className="font-semibold text-zinc-100">Rice</span>
        <span className="truncate text-zinc-400">Twitch Chat TTS</span>
      </div>

      <div className="flex items-center gap-2 px-2">
        <output className="font-mono text-zinc-400" aria-label="現在の表示倍率">
          {Math.round(scale * 100)}%
        </output>
        <fieldset className="flex overflow-hidden border border-zinc-800 bg-zinc-950">
          <legend className="sr-only">UI倍率</legend>
          {scaleOptions.map((option) => (
            <label
              key={option.mode}
              className={[
                "cursor-pointer",
                scaleMode === option.mode ? "bg-sky-500 text-zinc-950" : "text-zinc-400 hover:bg-zinc-850 hover:text-zinc-100",
              ].join(" ")}
            >
              <input
                type="radio"
                name="ui-scale"
                value={option.mode}
                checked={scaleMode === option.mode}
                onChange={() => onScaleModeChange(option.mode)}
                className="peer sr-only"
              />
              <span
                className={[
                  "flex h-6 items-center px-2 text-[0.6875rem] transition-colors",
                  "peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-offset-[-2px] peer-focus-visible:outline-sky-400",
                ].join(" ")}
              >
                {option.label}
              </span>
            </label>
          ))}
        </fieldset>
      </div>

      <div className="flex self-stretch">
        <TitleBarButton label="最小化" onClick={() => void minimizeWindow()}>
          <Minus className="h-4 w-4" />
        </TitleBarButton>
        <TitleBarButton label={isMaximized ? "元に戻す" : "最大化"} onClick={() => void toggleMaximizeWindow()}>
          {isMaximized ? <Square className="h-3.5 w-3.5" /> : <Maximize2 className="h-3.5 w-3.5" />}
        </TitleBarButton>
        <TitleBarButton label="閉じる" tone="danger" onClick={() => void closeWindow()}>
          <X className="h-4 w-4" />
        </TitleBarButton>
      </div>
    </header>
  );
}

export function ResizeHandles() {
  return (
    <div className="pointer-events-none fixed inset-0 z-50">
      <ResizeHandle direction="North" className="left-1 right-1 top-0 h-1 cursor-n-resize" />
      <ResizeHandle direction="South" className="bottom-0 left-1 right-1 h-1 cursor-s-resize" />
      <ResizeHandle direction="West" className="bottom-1 left-0 top-1 w-1 cursor-w-resize" />
      <ResizeHandle direction="East" className="bottom-1 right-0 top-1 w-1 cursor-e-resize" />
      <ResizeHandle direction="NorthWest" className="left-0 top-0 h-2 w-2 cursor-nw-resize" />
      <ResizeHandle direction="NorthEast" className="right-0 top-0 h-2 w-2 cursor-ne-resize" />
      <ResizeHandle direction="SouthWest" className="bottom-0 left-0 h-2 w-2 cursor-sw-resize" />
      <ResizeHandle direction="SouthEast" className="bottom-0 right-0 h-2 w-2 cursor-se-resize" />
    </div>
  );
}

function ResizeHandle({ direction, className }: { direction: ResizeDirection; className: string }) {
  return (
    <div
      aria-hidden="true"
      onMouseDown={(event) => {
        if (event.button !== 0) {
          return;
        }

        event.preventDefault();
        void getCurrentWindow().startResizeDragging(direction);
      }}
      className={`pointer-events-auto absolute ${className}`}
    />
  );
}

function TitleBarButton({
  label,
  tone = "default",
  onClick,
  children,
}: {
  label: string;
  tone?: "default" | "danger";
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={[
        "flex w-11 items-center justify-center text-zinc-300 transition-colors",
        tone === "danger" ? "hover:bg-rose-500 hover:text-white" : "hover:bg-zinc-800 hover:text-zinc-100",
      ].join(" ")}
    >
      {children}
    </button>
  );
}
