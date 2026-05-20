import { useEffect, useState } from "react";

const BASE_FONT_SIZE = 16;
const MAX_UI_SCALE = 1.25;
const STORAGE_KEY = "rice.uiScaleMode";

export type UiScaleMode = "auto" | "1" | "1.25" | "1.5";

function isUiScaleMode(value: string | null): value is UiScaleMode {
  return value === "auto" || value === "1" || value === "1.25" || value === "1.5";
}

function getAutoDisplayScale() {
  const devicePixelRatio = window.devicePixelRatio || 1;
  return Math.min(MAX_UI_SCALE, Math.max(1, Math.sqrt(devicePixelRatio)));
}

function getDisplayScale(mode: UiScaleMode) {
  return mode === "auto" ? getAutoDisplayScale() : Number(mode);
}

function applyDisplayScale(mode: UiScaleMode) {
  const scale = getDisplayScale(mode);
  document.documentElement.style.setProperty("--app-ui-scale", scale.toFixed(3));
  document.documentElement.style.fontSize = `${BASE_FONT_SIZE * scale}px`;

  return scale;
}

export function useDisplayScale() {
  const [mode, setMode] = useState<UiScaleMode>(() => {
    if (typeof window === "undefined") {
      return "auto";
    }

    const storedMode = window.localStorage.getItem(STORAGE_KEY);
    return isUiScaleMode(storedMode) ? storedMode : "auto";
  });
  const [scale, setScale] = useState(1);

  useEffect(() => {
    let mediaQueryList: MediaQueryList | undefined;

    const refresh = () => {
      setScale(applyDisplayScale(mode));

      mediaQueryList?.removeEventListener("change", refresh);
      mediaQueryList = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
      mediaQueryList.addEventListener("change", refresh);
    };

    refresh();
    window.addEventListener("resize", refresh);
    window.addEventListener("focus", refresh);

    return () => {
      window.removeEventListener("resize", refresh);
      window.removeEventListener("focus", refresh);
      mediaQueryList?.removeEventListener("change", refresh);
      document.documentElement.style.removeProperty("--app-ui-scale");
      document.documentElement.style.removeProperty("font-size");
    };
  }, [mode]);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, mode);
  }, [mode]);

  return { mode, scale, setMode };
}
