import { describe, expect, it } from "vitest";
import { hasActiveTwitchChat, hasPendingSpeechWork, requiresExitConfirmation } from "./exitSafety";
import type { QueueItem } from "./types";

const queuedItem: QueueItem = {
  id: "queue-1",
  userDisplayName: "viewer",
  text: "こんにちは",
  status: "queued",
};

describe("終了前の安全確認", () => {
  it("接続中、読み上げ中、または待機キューがあると終了確認を要求する", () => {
    expect(hasActiveTwitchChat("connected")).toBe(true);
    expect(hasPendingSpeechWork("speaking", [])).toBe(true);
    expect(hasPendingSpeechWork("idle", [queuedItem])).toBe(true);
    expect(requiresExitConfirmation("disconnected", "idle", [queuedItem], false)).toBe(true);
  });

  it("切断済みで待機キューも未保存変更もなければ確認しない", () => {
    expect(hasActiveTwitchChat("disconnected")).toBe(false);
    expect(hasPendingSpeechWork("idle", [])).toBe(false);
    expect(requiresExitConfirmation("disconnected", "idle", [], false)).toBe(false);
  });

  it("保存していない変更は処理がなくても確認する", () => {
    expect(requiresExitConfirmation("disconnected", "idle", [], true)).toBe(true);
  });
});
