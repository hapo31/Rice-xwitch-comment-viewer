import { speechHealthLabels, speechQueuePhaseLabels } from "../presentation/speech";
import { getTwitchAuthLabel, getTwitchConnectionLabel } from "../presentation/twitch";
import type { AppState } from "../stores/appState";
import type { AppNotification, NotificationStatusDomain } from "../types";

export type LiveStatusSnapshot = Pick<
  AppState,
  | "twitchAuthStatus"
  | "twitchConnectionStatus"
  | "speechAdapterHealth"
  | "speechQueuePhase"
  | "notifications"
>;
export interface LiveAnnouncement {
  id: string;
  message: string;
  priority: "status" | "alert";
  keys: string[];
  notificationId?: string;
}
const domains = ["auth", "chat", "speech", "queue"] as const;
const rank = (item: LiveAnnouncement) => (item.priority === "alert" ? 2 : 1);
function value(snapshot: LiveStatusSnapshot, domain: NotificationStatusDomain) {
  switch (domain) {
    case "auth":
      return snapshot.twitchAuthStatus;
    case "chat":
      return snapshot.twitchConnectionStatus;
    case "speech":
      return snapshot.speechAdapterHealth;
    case "queue":
      return snapshot.speechQueuePhase;
  }
}
function statusAnnouncement(snapshot: LiveStatusSnapshot, domain: NotificationStatusDomain) {
  switch (domain) {
    case "auth":
      return {
        message: `Twitch 認証: ${getTwitchAuthLabel(snapshot.twitchAuthStatus, "announcement")}`,
        error: ["expired", "error"].includes(snapshot.twitchAuthStatus),
      };
    // Auth owns the recovery instruction for a revoked chat subscription.
    case "chat":
      return snapshot.twitchConnectionStatus === "authRequired"
        ? undefined
        : {
            message: `Twitch 接続: ${getTwitchConnectionLabel(snapshot.twitchConnectionStatus)}`,
            error: snapshot.twitchConnectionStatus === "error",
          };
    case "speech":
      return {
        message: `棒読みちゃん: ${speechHealthLabels[snapshot.speechAdapterHealth]}`,
        error: ["disconnected", "error"].includes(snapshot.speechAdapterHealth),
      };
    case "queue":
      return {
        message: `読み上げキュー: ${speechQueuePhaseLabels[snapshot.speechQueuePhase]}`,
        error: snapshot.speechQueuePhase === "error",
      };
  }
}

/** Delivery state is separate from the visual snapshot. Reading one item never consumes its peers. */
export class LiveAnnouncementQueue {
  private previous: LiveStatusSnapshot;
  private sequence = 0;
  private episodes = new Map<NotificationStatusDomain, string>();
  private delivered = new Map<string, number>();
  private pending: LiveAnnouncement[] = [];
  private noticeEpisodes = new Map<string, Map<NotificationStatusDomain, string>>();
  private episodeOwners = new Map<string, { id: string; correlationId?: string }>();

  constructor(initial: LiveStatusSnapshot) {
    this.previous = { ...initial, notifications: [] };
    for (const domain of domains) this.episodes.set(domain, `${domain}:0`);
  }

  update(snapshot: LiveStatusSnapshot) {
    const ids = new Set(snapshot.notifications.map((notice) => notice.id));
    // Explicitly cleared notices must not stay queued or reappear on a later replay.
    this.pending = this.pending.filter((item) => {
      if (!item.notificationId || ids.has(item.notificationId)) return true;
      this.remember(item);
      return false;
    });
    for (const domain of domains) {
      if (value(this.previous, domain) === value(snapshot, domain)) continue;
      const key = `state:${domain}:${++this.sequence}`;
      this.episodes.set(domain, key);
      const status = statusAnnouncement(snapshot, domain);
      if (status)
        this.accept({
          id: key,
          keys: [key],
          message: status.message,
          priority: status.error ? "alert" : "status",
        });
    }
    // The store is newest first. Within a priority, deliver oldest first.
    const previousNotices = new Map(
      this.previous.notifications.map((notice) => [notice.id, notice]),
    );
    for (const notification of [...snapshot.notifications].reverse()) {
      const previous = previousNotices.get(notification.id);
      if (
        previous &&
        previous.severity === notification.severity &&
        previous.correlationId === notification.correlationId &&
        (previous.announcementDomains ?? []).join() ===
          (notification.announcementDomains ?? []).join()
      )
        continue;
      this.acceptNotification(notification);
    }
    this.previous = snapshot;
  }

  private acceptNotification(notification: AppNotification) {
    if (notification.severity !== "error" && notification.severity !== "warning") return;
    const keys = [`notification:${notification.id}`];
    if (notification.correlationId) keys.push(`correlation:${notification.correlationId}`);
    let episodes = this.noticeEpisodes.get(notification.id);
    if (!episodes) {
      episodes = new Map();
      this.noticeEpisodes.set(notification.id, episodes);
    }
    for (const domain of notification.announcementDomains ?? []) {
      const episode = episodes.get(domain) ?? this.episodes.get(domain);
      if (!episode) continue;
      const owner = this.episodeOwners.get(episode);
      if (
        owner &&
        owner.id !== notification.id &&
        !(owner.correlationId && owner.correlationId === notification.correlationId)
      )
        continue;
      this.episodeOwners.set(episode, {
        id: notification.id,
        correlationId: notification.correlationId,
      });
      episodes.set(domain, episode);
      keys.push(episode);
    }
    while (this.noticeEpisodes.size > 200) {
      const oldest = this.noticeEpisodes.keys().next().value;
      if (oldest === undefined) break;
      this.noticeEpisodes.delete(oldest);
    }
    while (this.episodeOwners.size > 1000) {
      const oldest = this.episodeOwners.keys().next().value;
      if (oldest === undefined) break;
      this.episodeOwners.delete(oldest);
    }
    this.accept({
      id: `notification:${notification.id}`,
      notificationId: notification.id,
      keys,
      message: `${notification.severity === "error" ? "エラー" : "警告"}: ${notification.message}`,
      priority: notification.severity === "error" ? "alert" : "status",
    });
  }

  private accept(incoming: LiveAnnouncement) {
    const matching = this.pending.filter((item) =>
      item.keys.some((key) => incoming.keys.includes(key)),
    );
    const keys = [...new Set([...incoming.keys, ...matching.flatMap((item) => item.keys)])];
    const rememberedRank = Math.max(0, ...keys.map((key) => this.delivered.get(key) ?? 0));
    if (rememberedRank >= Math.max(rank(incoming), ...matching.map(rank))) {
      // A later source can reveal the status correlation after the detailed notice
      // was spoken. Consume only that summary; retain unrelated pending failures.
      this.pending = this.pending.filter((item) => !matching.includes(item));
      this.remember({ ...incoming, keys });
      return;
    }
    if (matching.length) {
      const first = matching[0];
      const best = [incoming, ...matching].sort((a, b) => rank(b) - rank(a))[0];
      const merged = {
        ...best,
        keys,
      };
      this.pending = this.pending.flatMap((item) =>
        item === first ? [merged] : matching.includes(item) ? [] : [item],
      );
      return;
    }
    this.pending.push(incoming);
  }

  private remember(item: LiveAnnouncement) {
    for (const key of item.keys)
      this.delivered.set(key, Math.max(this.delivered.get(key) ?? 0, rank(item)));
    while (this.delivered.size > 1000) {
      const oldest = this.delivered.keys().next().value;
      if (oldest === undefined) break;
      this.delivered.delete(oldest);
    }
  }

  take(): LiveAnnouncement | undefined {
    const firstAlert = this.pending.findIndex((item) => item.priority === "alert");
    const [next] = this.pending.splice(firstAlert < 0 ? 0 : firstAlert, 1);
    if (next) this.remember(next);
    return next;
  }
  get hasAlert() {
    return this.pending.some((item) => item.priority === "alert");
  }
  get size() {
    return this.pending.length;
  }
}
