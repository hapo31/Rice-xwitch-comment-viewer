import { describe, expect, it } from "vitest";
import { appReducer, initialAppState, warningNotifications } from "./appStore";
import type { AppSettings, ChatMessage, LauncherItem, QueueItem } from "../types";
import { utcTimestamp } from "../time";

const receivedAt = utcTimestamp("2026-05-23T00:00:00Z");
const augustReceivedAt = utcTimestamp("2026-08-08T00:00:00Z");

function chatMessage(id: string): ChatMessage {
  return {
    kind: "user",
    id,
    receivedAt,
    userDisplayName: "viewer",
    text: `comment ${id}`,
    status: "queued",
  };
}

describe("appReducer", () => {
  it("prepends chat messages and keeps the latest 200", () => {
    const state = Array.from({ length: 205 }, (_, index) => index).reduce(
      (current, index) =>
        appReducer(current, {
          type: "chat.message",
          message: chatMessage(String(index)),
        }),
      initialAppState,
    );

    expect(state.chatMessages).toHaveLength(200);
    expect(state.chatMessages[0]?.id).toBe("204");
    expect(state.chatMessages[state.chatMessages.length - 1]?.id).toBe("5");
  });

  it("keeps system messages distinct from viewer chat", () => {
    const state = appReducer(initialAppState, { type: "chat.message", message: { kind: "system", id: "system-1", receivedAt, userDisplayName: "system", text: "Twitch EventSub を再接続しました。" } });
    expect(state.chatMessages[0]).toMatchObject({ kind: "system", userDisplayName: "system" });
  });

  it("keeps authentication and chat connection transitions independent", () => {
    const authenticated = appReducer(initialAppState, { type: "twitch.authStatus", status: "authenticated" });
    const chatConnected = appReducer(authenticated, { type: "twitch.connectionStatus", status: "connected" });
    const authExpired = appReducer(chatConnected, { type: "twitch.authStatus", status: "expired" });

    expect(chatConnected).toMatchObject({ twitchAuthStatus: "authenticated", twitchConnectionStatus: "connected" });
    expect(authExpired).toMatchObject({ twitchAuthStatus: "expired", twitchConnectionStatus: "connected" });
  });

  it("represents an EventSub revocation as both a chat and authentication failure", () => {
    const connected = {
      ...initialAppState,
      twitchAuthStatus: "authenticated" as const,
      twitchConnectionStatus: "connected" as const,
    };
    const chatRevoked = appReducer(connected, { type: "twitch.connectionStatus", status: "authRequired" });
    const authRevoked = appReducer(chatRevoked, { type: "twitch.authStatus", status: "expired" });

    expect(authRevoked).toMatchObject({ twitchAuthStatus: "expired", twitchConnectionStatus: "authRequired" });
  });

  it("replaces queue items from speech queue events", () => {
    const items: QueueItem[] = [
      {
        id: "speech-1",
        sourceMessageId: "message-1",
        userDisplayName: "viewer",
        text: "viewer。こんにちは",
        status: "speaking",
      },
    ];

    const state = appReducer(initialAppState, { type: "queue.changed", items });

    expect(state.queueItems).toEqual(items);
  });

  it("synchronizes queue results to their source chat messages without changing system rows", () => {
    const state = {
      ...initialAppState,
      chatMessages: [
        chatMessage("queued"),
        chatMessage("spoken"),
        chatMessage("skipped"),
        chatMessage("blocked"),
        chatMessage("error"),
        { kind: "system" as const, id: "system", receivedAt, userDisplayName: "system" as const, text: "接続しました" },
      ],
    };
    const items: QueueItem[] = [
      { id: "1", sourceMessageId: "queued", userDisplayName: "viewer", text: "", status: "speaking" },
      { id: "2", sourceMessageId: "spoken", userDisplayName: "viewer", text: "", status: "spoken" },
      { id: "3", sourceMessageId: "skipped", userDisplayName: "viewer", text: "", status: "skipped" },
      { id: "4", sourceMessageId: "blocked", userDisplayName: "viewer", text: "", status: "blocked" },
      { id: "5", sourceMessageId: "error", userDisplayName: "viewer", text: "", status: "error" },
    ];

    const updated = appReducer(state, { type: "queue.changed", items });

    expect(updated.chatMessages.slice(0, 5).map((message) => message.kind === "user" && message.status)).toEqual([
      "queued", "spoken", "skipped", "blocked", "error",
    ]);
    expect(updated.chatMessages[5]).toEqual(state.chatMessages[5]);
  });

  it("uses a previously received queue snapshot when its source chat message arrives later", () => {
    const queued = appReducer(initialAppState, {
      type: "queue.changed",
      items: [{ id: "speech", sourceMessageId: "blocked", userDisplayName: "viewer", text: "", status: "blocked" }],
    });

    const updated = appReducer(queued, { type: "chat.message", message: chatMessage("blocked") });

    expect(updated.chatMessages[0]).toMatchObject({ id: "blocked", status: "blocked" });
  });

  it("keeps skipped queue history synchronized after a queue clear snapshot", () => {
    const messageIds = Array.from({ length: 200 }, (_, index) => `cleared-${index}`);
    const withQueuedMessages = messageIds.reduce(
      (state, id) => appReducer(state, { type: "chat.message", message: chatMessage(id) }),
      initialAppState,
    );

    const updated = appReducer(withQueuedMessages, {
      type: "queue.changed",
      items: messageIds.map((sourceMessageId, index) => ({
        id: `speech-${index}`,
        sourceMessageId,
        userDisplayName: "viewer",
        text: "",
        status: "skipped" as const,
      })),
    });

    expect(updated.chatMessages).toHaveLength(200);
    expect(updated.chatMessages.every((message) => message.kind === "user" && message.status === "skipped")).toBe(true);
  });

  it("replaces launcher items without changing the other settings", () => {
    const settings = {
      twitch: { channelLogin: "rice", autoConnect: false, confirmBeforeStopChat: true },
      speech: {},
      launcher: { items: [] },
    } as unknown as AppSettings;
    const items: LauncherItem[] = [
      {
        id: "launcher-1",
        kind: "application",
        target: "C:\\Apps\\Example.exe",
        displayName: "Example",
        order: 0,
      },
    ];

    const state = appReducer(
      { ...initialAppState, settings },
      { type: "launcher.changed", items },
    );

    expect(state.settings?.launcher.items).toEqual(items);
    expect(state.settings?.twitch.channelLogin).toBe("rice");
  });

  it("keeps only the latest five actionable notifications without discarding info or success", () => {
    const state = Array.from({ length: 8 }, (_, index) => index).reduce(
      (current, index) =>
        appReducer(current, {
          type: "notification.added",
          notification: {
            severity: "warning",
            source: "event",
            message: `warning ${index}`,
            occurredAtMs: index,
          },
        }),
      appReducer(
        appReducer(initialAppState, {
          type: "notification.added",
          notification: { severity: "info", source: "command", message: "接続しました", occurredAtMs: 10 },
        }),
        {
          type: "notification.added",
          notification: { severity: "success", source: "command", message: "認証しました", occurredAtMs: 11 },
        },
      ),
    );

    expect(warningNotifications(state.notifications).map((notification) => notification.message)).toEqual([
      "warning 7", "warning 6", "warning 5", "warning 4", "warning 3",
    ]);
    expect(state.notifications.map((notification) => notification.severity)).toContain("info");
    expect(state.notifications.map((notification) => notification.severity)).toContain("success");
  });

  it("deduplicates one backend failure received through log, status event, and command rejection", () => {
    const failure = {
      message: "棒読みちゃんに接続できませんでした。",
      occurredAtMs: 1,
      correlationId: "bouyomi-connect-42",
    };
    const state = [
      { source: "log" as const, severity: "warning" as const },
      { source: "event" as const, severity: "error" as const },
      { source: "command" as const, severity: "error" as const },
    ].reduce(
      (current, source) =>
        appReducer(current, {
          type: "notification.added",
          notification: { ...failure, ...source },
        }),
      initialAppState,
    );

    expect(warningNotifications(state.notifications)).toHaveLength(1);
    expect(warningNotifications(state.notifications)[0]).toMatchObject({ ...failure, severity: "error" });
  });

  it("stores application logs for the Logs view", () => {
    const state = appReducer(initialAppState, {
      type: "log.added",
      log: {
        level: "warning",
        message: "Twitch EventSub が切断されました。",
        occurredAtMs: 1,
      },
    });

    expect(state.logs).toHaveLength(1);
    expect(state.logs[0]).toMatchObject({
      id: "1-warning-Twitch EventSub が切断されました。",
      level: "warning",
    });
  });

  it("stores OAuth waiting progress in Logs and system Chat", () => {
    const message = "Twitch の認証を待っています。";
    const state = appReducer(
      appReducer(initialAppState, {
        type: "log.added",
        log: { level: "info", message, occurredAtMs: 1 },
      }),
      {
        type: "chat.message",
        message: {
          kind: "system",
          id: "system-auth-waiting",
          receivedAt: augustReceivedAt,
          userDisplayName: "system",
          text: message,
        },
      },
    );

    expect(state.logs[0]).toMatchObject({ level: "info", message });
    expect(state.chatMessages[0]).toMatchObject({ userDisplayName: "system", text: message });
  });

  it("assigns distinct IDs to consecutive identical application logs", () => {
    const log = {
      level: "warning" as const,
      message: "Twitch EventSub が切断されました。",
      occurredAtMs: 1,
    };

    const state = appReducer(
      appReducer(initialAppState, { type: "log.added", log }),
      { type: "log.added", log },
    );

    expect(state.logs).toHaveLength(2);
    expect(state.logs.map((entry) => entry.id)).toEqual([
      "1-warning-Twitch EventSub が切断されました。-1",
      "1-warning-Twitch EventSub が切断されました。",
    ]);
  });
});
