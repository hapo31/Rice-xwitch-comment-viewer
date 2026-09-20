import { describe, expect, it, vi } from "vitest";
import { presentError, reportPresentedError } from "./errors";

describe("command error presentation", () => {
  it.each(["ECONNREFUSED", new Error("connection refused"), { code: "ECONNREFUSED", message: "connect failed" }])("explains a refused connection and its recovery", (error) => {
    const result = presentError(error, "speech");
    expect(result.message).toContain("接続先が応答を受け付けていません");
    expect(result.message).toContain("［診断］");
    expect(result.message).not.toContain("Error:");
    expect(result.details).toMatch(/ECONNREFUSED|connection refused/);
  });

  it.each([undefined, null, "", "   ", {}, { message: {} }, 0])("always gives an actionable fallback for %j", (error) => {
    const result = presentError(error, "settings");
    expect(result.message).toContain("設定");
    expect(result.message).toContain("確認してください");
    expect(result.message).not.toMatch(/\[object Object\]|undefined|null/);
    expect(result.details.length).toBeGreaterThan(0);
  });

  it("keeps the backend's Japanese partial-success explanation", () => {
    const result = presentError("Error: 棒読みちゃん側には届いている可能性があります。", "speech");
    expect(result.message).toContain("棒読みちゃん側には届いている可能性があります。");
    expect(result.message).not.toContain("Error:");
    expect(result.message).toContain("［診断］");
  });

  it("retains stack and structured details in Logs, separate from the notification", () => {
    const error = Object.assign(new Error("request exploded"), { code: "UNEXPECTED", field: "host" });
    const sinks = { notify: vi.fn(), log: vi.fn() };
    const result = reportPresentedError(error, "settings", sinks);
    expect(sinks.notify).toHaveBeenCalledWith(result.message);
    expect(sinks.log).toHaveBeenCalledWith(expect.stringContaining("request exploded"));
    expect(sinks.log).toHaveBeenCalledWith(expect.stringContaining("UNEXPECTED"));
    expect(sinks.log).toHaveBeenCalledWith(expect.stringContaining("stack"));
    expect(result.message).not.toContain("request exploded");
  });

  it("handles circular objects and throwing accessors without breaking error handling", () => {
    const circular: { cause?: unknown } = {};
    circular.cause = circular;
    expect(presentError(circular).details).toContain("[circular]");
    const throwing = Object.defineProperty({}, "message", { enumerable: true, get() { throw new Error("getter failed"); } });
    expect(presentError(throwing).message).toContain("もう一度操作");
  });

  it.each([
    [{ status: 401 }, "auth", "認証が無効", "再認証"],
    [{ code: "ETIMEDOUT" }, "chat", "タイムアウト", "再接続"],
    [new Error("EACCES"), "launcher", "アクセス権限", "登録先"],
    [{ code: "ENOENT" }, "launcher", "見つかりません", "ファイル"],
  ] as const)("maps known failures to causes and operation-specific recovery", (error, operation, cause, action) => {
    const { message } = presentError(error, operation);
    expect(message).toContain(cause);
    expect(message).toContain(action);
  });
});
