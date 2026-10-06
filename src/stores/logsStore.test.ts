import { describe, expect, it } from "vitest";
import type { AppNotification } from "../types";
import { initialLogsState, type LogsState, logsReducer, warningNotifications } from "./logsStore";

function add(
  state: LogsState,
  severity: AppNotification["severity"],
  message: string,
  occurredAtMs = 1,
  correlationId?: string,
) {
  return logsReducer(state, {
    type: "notification.added",
    notification: { severity, source: "command", message, occurredAtMs, correlationId },
  });
}

describe("production notification retention", () => {
  it("retains unresolved warnings through hundreds of informational and successful operations", () => {
    let state = add(initialLogsState, "error", "接続に失敗しました");
    state = add(state, "warning", "復旧を確認してください");
    const warnings = state.notifications;
    for (let index = 0; index < 300; index += 1) {
      state = add(state, index % 2 ? "info" : "success", `operation ${index}`);
    }
    expect(state.notifications).toBe(warnings);
    expect(warningNotifications(state.notifications).map((entry) => entry.message)).toEqual([
      "復旧を確認してください",
      "接続に失敗しました",
    ]);
    expect(state.notificationHistory).toHaveLength(100);
    expect(state.notificationHistory[0].message).toBe("operation 299");
    expect(state.notificationHistory[state.notificationHistory.length - 1]?.message).toBe(
      "operation 200",
    );
  });

  it("bounds actionable notices separately and keeps only five in the warning presentation", () => {
    let state = add(initialLogsState, "success", "保存しました");
    const history = state.notificationHistory;
    for (let index = 0; index < 130; index += 1) {
      state = add(state, index % 2 ? "warning" : "error", `failure ${index}`);
    }
    expect(state.notificationHistory).toBe(history);
    expect(state.notifications).toHaveLength(100);
    expect(state.notifications[0].message).toBe("failure 129");
    expect(state.notifications[state.notifications.length - 1]?.message).toBe("failure 30");
    expect(warningNotifications(state.notifications)).toHaveLength(5);
  });

  it("moves a correlated notice into the warning bucket on escalation without duplication", () => {
    let state = initialLogsState;
    for (let index = 0; index < 100; index += 1) state = add(state, "warning", `existing ${index}`);
    state = add(state, "info", "接続を確認しています", 1, "connection");
    const original = state.notificationHistory[0];
    state = add(state, "warning", "接続を再確認してください", 60_000, "connection");
    expect(state.notificationHistory).toEqual([]);
    expect(state.notifications).toHaveLength(100);
    expect(state.notifications[0]).toEqual({ ...original, severity: "warning" });
    expect(state.notifications.some((entry) => entry.message === "existing 0")).toBe(false);
    state = add(state, "error", "接続が失敗しました", 70_000, "connection");
    expect(state.notifications[0]).toEqual({ ...original, severity: "error" });
    expect(add(state, "success", "接続しました", 80_000, "connection")).toBe(state);
  });

  it("keeps time-window deduplication and clears only actionable notices explicitly", () => {
    let state = add(initialLogsState, "info", "same message", 100);
    state = add(state, "success", "same message", 200);
    expect(state.notificationHistory).toHaveLength(1);
    expect(state.notificationHistory[0].severity).toBe("success");
    state = add(state, "warning", "same message", 300);
    expect(state.notificationHistory).toHaveLength(0);
    state = add(state, "info", "same message", 10_000);
    expect(state.notifications).toHaveLength(1);
    expect(state.notificationHistory).toHaveLength(1);
    state = logsReducer(state, {
      type: "log.added",
      log: { level: "error", message: "接続記録", occurredAtMs: 1 },
    });
    const cleared = logsReducer(state, { type: "warnings.cleared" });
    expect(cleared.notifications).toEqual([]);
    expect(cleared.notificationHistory).toBe(state.notificationHistory);
    expect(cleared.logs).toBe(state.logs);
    const newWarning = add(cleared, "warning", "same message", 20_000);
    expect(newWarning.notifications).toHaveLength(1);
    expect(logsReducer(newWarning, { type: "logs.cleared" }).notifications).toBe(
      newWarning.notifications,
    );
  });
});

describe("production log identity", () => {
  it("deduplicates replayed backend event IDs while preserving distinct ID collisions", () => {
    const replay = {
      id: "event-1",
      level: "warning" as const,
      message: "接続が切れました",
      occurredAtMs: 1,
    };
    const once = logsReducer(initialLogsState, { type: "log.added", log: replay });
    expect(logsReducer(once, { type: "log.added", log: replay })).toBe(once);

    const generated = {
      level: "warning" as const,
      message: "接続が切れました",
      occurredAtMs: 2,
    };
    const collision = logsReducer(
      logsReducer(initialLogsState, { type: "log.added", log: generated }),
      { type: "log.added", log: { ...generated, id: "2-warning-接続が切れました" } },
    );
    expect(collision.logs).toHaveLength(2);
    expect(collision.logs.map((entry) => entry.id)).toEqual([
      "2-warning-接続が切れました-1",
      "2-warning-接続が切れました",
    ]);
  });

  it("assigns unique display IDs to repeated ID-less logs", () => {
    const log = { level: "warning" as const, message: "同じ記録", occurredAtMs: 10 };
    const once = logsReducer(initialLogsState, { type: "log.added", log });
    const twice = logsReducer(once, { type: "log.added", log });
    expect(twice.logs.map((entry) => entry.id)).toEqual([
      "10-warning-同じ記録-1",
      "10-warning-同じ記録",
    ]);
  });
});
