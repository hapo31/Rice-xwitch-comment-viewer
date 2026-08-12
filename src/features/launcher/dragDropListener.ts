export interface LauncherDragDropEvent {
  payload: {
    type: "enter" | "over" | "leave" | "drop";
    paths?: string[];
  };
}

export interface LauncherDragDropHandlers {
  onEnter: () => void;
  onOver: () => void;
  onLeave: () => void;
  onDrop: (paths: string[]) => void;
}

export interface MutableRef<T> {
  current: T;
}

type DragDropSubscribe = (
  listener: (event: LauncherDragDropEvent) => void,
) => Promise<() => void>;

/**
 * Keeps one native DnD subscription alive while handlers are replaced by React renders.
 */
export function subscribeLauncherDragDrop(
  subscribe: DragDropSubscribe,
  handlersRef: MutableRef<LauncherDragDropHandlers>,
  onError?: (error: unknown) => void,
): () => void {
  return subscribeWithCleanup([() => subscribe((event) => {
      const handlers = handlersRef.current;
      switch (event.payload.type) {
        case "enter":
          handlers.onEnter();
          break;
        case "over":
          handlers.onOver();
          break;
        case "leave":
          handlers.onLeave();
          break;
        case "drop":
          handlers.onDrop(event.payload.paths ?? []);
          break;
      }
    })], onError);
}
import { subscribeWithCleanup } from "../../tauri/subscriptions";
