import { describe, expect, it, vi } from "vitest";
import { restoreAndValidateStartupAuth } from "./startupAuth";

const storedProfile = {
  userId: "1234",
  login: "rice_channel",
  scopes: ["user:read:chat"],
  expiresIn: 3600,
};

describe("restoreAndValidateStartupAuth", () => {
  it("validates stored authentication before reporting it as authenticated", async () => {
    const calls: string[] = [];
    const result = await restoreAndValidateStartupAuth({
      getStoredAuth: async () => {
        calls.push("restore");
        return storedProfile;
      },
      validateAuth: async () => {
        calls.push("validate");
        return { profile: storedProfile };
      },
      reportSystemMessage: (message) => calls.push(message),
    });

    expect(result).toEqual({ status: "authenticated", result: { profile: storedProfile } });
    expect(calls).toEqual([
      "restore",
      "保存済みの Twitch 認証情報を確認しています…",
      "validate",
      "Twitch 認証の有効性を確認しました（rice_channel）。",
    ]);
  });

  it("does not validate when stored authentication is absent", async () => {
    const validateAuth = vi.fn();
    const reportSystemMessage = vi.fn();

    const result = await restoreAndValidateStartupAuth({
      getStoredAuth: async () => undefined,
      validateAuth,
      reportSystemMessage,
    });

    expect(result).toEqual({ status: "missing" });
    expect(validateAuth).not.toHaveBeenCalled();
    expect(reportSystemMessage).toHaveBeenCalledWith("保存済みの Twitch 認証情報はありません。");
  });

  it("reports validation failures as system chat", async () => {
    const reportSystemMessage = vi.fn();

    const result = await restoreAndValidateStartupAuth({
      getStoredAuth: async () => storedProfile,
      validateAuth: async () => {
        throw new Error("token expired");
      },
      reportSystemMessage,
    });

    expect(result).toEqual({ status: "error", error: "Error: token expired" });
    expect(reportSystemMessage).toHaveBeenLastCalledWith(
      "Twitch 認証の確認に失敗しました。Login から再認証してください: Error: token expired",
    );
  });
});
