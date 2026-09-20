import { afterEach, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  invoke.mockReset();
});

it("converts the two root nullable command results to undefined", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  invoke.mockResolvedValue(null);

  const client = await import("./client");

  await expect(client.takeSettingsRecoveryNotice()).resolves.toBeUndefined();
  await expect(client.twitchGetStoredAuth()).resolves.toBeUndefined();
  expect(invoke.mock.calls).toEqual([
    ["settings_take_recovery_notice"],
    ["twitch_get_stored_auth"],
  ]);
});
