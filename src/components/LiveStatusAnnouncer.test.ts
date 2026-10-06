import { describe, expect, it } from "vitest";
import { LiveAnnouncementQueue, type LiveStatusSnapshot } from "../models/liveAnnouncements";
import type { AppNotification } from "../types";

const initial: LiveStatusSnapshot = {
  twitchAuthStatus: "authenticated",
  twitchConnectionStatus: "disconnected",
  speechAdapterHealth: "connected",
  speechQueuePhase: "idle",
  notifications: [],
};
const notice = (id: string, overrides: Partial<AppNotification> = {}): AppNotification => ({
  id,
  severity: "error",
  source: "command",
  message: "操作に失敗しました。",
  occurredAtMs: 1,
  ...overrides,
});

describe("production live announcement delivery", () => {
  it("retains every simultaneous state failure and prioritizes alerts over polite status", () => {
    const queue = new LiveAnnouncementQueue(initial);
    queue.update({
      ...initial,
      twitchAuthStatus: "expired",
      speechAdapterHealth: "disconnected",
      notifications: [notice("warning", { severity: "warning" })],
    });
    expect(queue.take()).toMatchObject({
      message: "Twitch 認証: 再ログイン必要",
      priority: "alert",
    });
    expect(queue.take()).toMatchObject({ message: "棒読みちゃん: 未接続", priority: "alert" });
    expect(queue.take()).toMatchObject({ priority: "status" });
    expect(queue.take()).toBeUndefined();
  });
  it("keeps queue errors independent of adapter health and suppresses only the revoked chat summary", () => {
    const queue = new LiveAnnouncementQueue(initial);
    queue.update({ ...initial, twitchConnectionStatus: "authRequired", speechQueuePhase: "error" });
    expect(queue.take()).toMatchObject({
      message: "読み上げキュー: 手動再試行待ち",
      priority: "alert",
    });
    expect(queue.take()).toBeUndefined();
    queue.update({
      ...initial,
      twitchConnectionStatus: "authRequired",
      speechQueuePhase: "error",
      twitchAuthStatus: "expired",
    });
    expect(queue.take()).toMatchObject({ message: "Twitch 認証: 再ログイン必要" });
  });
  it("delivers distinct occurrences with the same wording and suppresses correlated replay", () => {
    const queue = new LiveAnnouncementQueue(initial);
    const first = notice("first", { correlationId: "failure-1" });
    queue.update({ ...initial, notifications: [first] });
    expect(queue.take()?.notificationId).toBe("first");
    queue.update({
      ...initial,
      notifications: [notice("log", { correlationId: "failure-1", source: "log" }), first],
    });
    expect(queue.take()).toBeUndefined();
    queue.update({
      ...initial,
      notifications: [notice("second", { correlationId: "failure-2" }), first],
    });
    expect(queue.take()?.notificationId).toBe("second");
  });
  it("replaces a pending status summary with its actionable notice, without swallowing unrelated failures", () => {
    const queue = new LiveAnnouncementQueue(initial);
    const next = {
      ...initial,
      twitchConnectionStatus: "error" as const,
      speechAdapterHealth: "error" as const,
    };
    queue.update(next);
    const error = notice("connection", {
      announcementDomains: ["chat"],
      correlationId: "chat-failure",
    });
    queue.update({ ...next, notifications: [error] });
    expect(queue.take()).toMatchObject({ notificationId: "connection" });
    expect(queue.take()).toMatchObject({ message: "棒読みちゃん: 接続エラー" });
    expect(queue.take()).toBeUndefined();
    // Keeping an old unresolved notice cannot consume a later recovery and failure.
    queue.update({ ...initial, notifications: [error] });
    while (queue.take()) {
      /* deliver recovery notices */
    }
    queue.update({ ...next, notifications: [error] });
    expect(queue.take()).toMatchObject({ message: "Twitch 接続: 接続エラー" });
  });
  it("removes explicitly cleared pending notices and reannounces a new ID after clear", () => {
    const queue = new LiveAnnouncementQueue(initial);
    queue.update({ ...initial, notifications: [notice("first")] });
    queue.update(initial);
    expect(queue.take()).toBeUndefined();
    queue.update({ ...initial, notifications: [notice("second")] });
    expect(queue.take()?.notificationId).toBe("second");
  });
  it("announces warning escalation once, with error priority", () => {
    const queue = new LiveAnnouncementQueue(initial);
    queue.update({ ...initial, notifications: [notice("first", { severity: "warning" })] });
    expect(queue.take()?.priority).toBe("status");
    queue.update({ ...initial, notifications: [notice("first")] });
    expect(queue.take()?.priority).toBe("alert");
    queue.update({ ...initial, notifications: [notice("first")] });
    expect(queue.take()).toBeUndefined();
  });
});

it("does not equate independent notification IDs merely because their domain status is unchanged", () => {
  const queue = new LiveAnnouncementQueue(initial);
  const one = notice("one", { correlationId: "one", announcementDomains: ["chat"] });
  const two = notice("two", { correlationId: "two", announcementDomains: ["chat"] });
  const snapshot = {
    ...initial,
    twitchConnectionStatus: "error" as const,
    notifications: [two, one],
  };
  queue.update(snapshot);
  expect(queue.take()?.notificationId).toBe("one");
  expect(queue.take()?.notificationId).toBe("two");
  queue.update(snapshot);
  expect(queue.take()).toBeUndefined();
});

it("does not replay retained notices after a long stream of unrelated state changes", () => {
  const queue = new LiveAnnouncementQueue(initial);
  const notifications = [notice("retained")];
  queue.update({ ...initial, notifications });
  expect(queue.take()?.notificationId).toBe("retained");
  for (let index = 0; index < 1100; index += 1) {
    queue.update({
      ...initial,
      notifications,
      twitchConnectionStatus: index % 2 ? "connected" : "connecting",
    });
    expect(queue.take()?.notificationId).toBeUndefined();
    expect(queue.take()).toBeUndefined();
  }
});
