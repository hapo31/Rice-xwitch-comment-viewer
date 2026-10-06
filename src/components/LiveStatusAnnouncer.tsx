import { useEffect, useRef, useState } from "react";
import {
  type LiveAnnouncement,
  LiveAnnouncementQueue,
  type LiveStatusSnapshot,
} from "../models/liveAnnouncements";

export const LIVE_ANNOUNCEMENT_GAP_MS = 100;
export const LIVE_ANNOUNCEMENT_HOLD_MS = 2500;

export function LiveStatusAnnouncer({ state }: { state: LiveStatusSnapshot }) {
  const delivery = useRef<LiveAnnouncementQueue>(undefined);
  if (!delivery.current) delivery.current = new LiveAnnouncementQueue(state);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const active = useRef<LiveAnnouncement>(undefined);
  const [announcement, setAnnouncement] = useState<LiveAnnouncement>();

  useEffect(() => {
    const queue = delivery.current;
    if (!queue) return;
    queue.update(state);
    const clear = () => {
      active.current = undefined;
      setAnnouncement(undefined);
    };
    const schedule = () => {
      if (timer.current !== undefined || !queue.size) return;
      clear();
      // Stable, initially empty regions get a distinct empty commit between messages,
      // including different occurrences with exactly the same wording.
      timer.current = setTimeout(() => {
        const next = queue.take();
        active.current = next;
        setAnnouncement(next);
        timer.current = setTimeout(
          () => {
            timer.current = undefined;
            clear();
            schedule();
          },
          Math.max(LIVE_ANNOUNCEMENT_HOLD_MS, (next?.message.length ?? 0) * 80),
        );
      }, LIVE_ANNOUNCEMENT_GAP_MS);
    };
    if (
      (active.current?.notificationId &&
        !state.notifications.some((item) => item.id === active.current?.notificationId)) ||
      (active.current?.priority === "status" && queue.hasAlert)
    ) {
      clearTimeout(timer.current);
      timer.current = undefined;
      clear();
    }
    schedule();
  }, [
    state.twitchAuthStatus,
    state.twitchConnectionStatus,
    state.speechAdapterHealth,
    state.speechQueuePhase,
    state.notifications,
  ]);

  useEffect(
    () => () => {
      clearTimeout(timer.current);
      timer.current = undefined;
    },
    [],
  );
  return (
    <>
      <p className="sr-only" role="status" aria-atomic="true">
        {announcement?.priority === "status" ? announcement.message : ""}
      </p>
      <p className="sr-only" role="alert" aria-atomic="true">
        {announcement?.priority === "alert" ? announcement.message : ""}
      </p>
    </>
  );
}
