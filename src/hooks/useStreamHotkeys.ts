import { useEffect } from "react";

export type StreamHotkey = "toggleSpeech" | "skipSpeech" | "openSettings";

type EventTargetLike = {
  closest?: (selectors: string) => EventTargetLike | null;
  getAttribute?: (qualifiedName: string) => string | null;
  isContentEditable?: boolean;
  tagName?: string;
};

function isTextEditingTarget(target: EventTarget | null) {
  if (!target || typeof target !== "object") {
    return false;
  }

  const element = target as EventTargetLike;
  if (element.isContentEditable) {
    return true;
  }

  const editableAncestor = element.closest?.("[contenteditable]");
  if (editableAncestor) {
    return editableAncestor.getAttribute?.("contenteditable") !== "false";
  }

  return ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName ?? "");
}

function isButtonTarget(target: EventTarget | null) {
  return (target as EventTargetLike | null)?.tagName === "BUTTON";
}

export function getStreamHotkey(event: KeyboardEvent): StreamHotkey | undefined {
  if (
    event.defaultPrevented ||
    event.repeat ||
    event.isComposing ||
    isTextEditingTarget(event.target) ||
    isButtonTarget(event.target)
  ) {
    return undefined;
  }

  if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key === ",") {
    return "openSettings";
  }

  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) {
    return undefined;
  }

  if (event.code === "Space") {
    return "toggleSpeech";
  }

  return event.key.toLowerCase() === "s" ? "skipSpeech" : undefined;
}

export function useStreamHotkeys({
  onToggleSpeech,
  onSkipSpeech,
  onOpenSettings,
}: {
  onToggleSpeech: () => void;
  onSkipSpeech: () => void;
  onOpenSettings: () => void;
}) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const hotkey = getStreamHotkey(event);
      if (!hotkey) {
        return;
      }

      event.preventDefault();
      if (hotkey === "toggleSpeech") {
        onToggleSpeech();
      } else if (hotkey === "skipSpeech") {
        onSkipSpeech();
      } else {
        onOpenSettings();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onOpenSettings, onSkipSpeech, onToggleSpeech]);
}
