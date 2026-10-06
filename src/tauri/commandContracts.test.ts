import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createDefaultAppSettings } from "../settings/model";
import fixture from "./fixtures/bridge-contract.json";
import { parsePayload, twitchDeviceAuthStartSchema } from "./schemas";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
type Client = typeof import("./client");
const profile = { userId: "1", login: "streamer", scopes: ["user:read:chat"], expiresIn: 3600 };
const device = {
  userCode: "CODE",
  verificationUri: "https://www.twitch.tv/activate",
  expiresIn: 1800,
  expiresAtMs: 1_900_000_000_000,
  interval: 5,
};
const status = { revision: 1, status: "idle", adapterHealth: "connected", occurredAtMs: 1 };
const queue = { revision: 1, queuedCount: 0, items: [], phase: "idle", occurredAtMs: 1 };
const cases: Array<{
  command: string;
  call: (client: Client) => Promise<unknown>;
  valid: unknown;
  invalid: unknown;
}> = [
  {
    command: "app_build_info",
    call: (c) => c.getAppBuildInfo(),
    valid: {
      version: "0.2.3",
      isDev: true,
      launcher: { canRegisterApplications: true, canLaunchApplications: true },
    },
    invalid: { version: "0.2.3", isDev: "true" },
  },
  {
    command: "settings_get",
    call: (c) => c.getSettings(),
    valid: createDefaultAppSettings(),
    invalid: {
      ...createDefaultAppSettings(),
      speech: { ...createDefaultAppSettings().speech, bouyomiPort: 65536 },
    },
  },
  {
    command: "settings_update",
    call: (c) => c.updateSettings({}),
    valid: createDefaultAppSettings(),
    invalid: { ...createDefaultAppSettings(), twitch: { channelLogin: 1 } },
  },
  {
    command: "settings_take_recovery_notice",
    call: (c) => c.takeSettingsRecoveryNotice(),
    valid: { message: "設定を復旧しました" },
    invalid: { message: false },
  },
  {
    command: "app_events_snapshot",
    call: (c) => c.getAppEventsSnapshot(),
    valid: { revision: 1, logs: [], twitchStatuses: [], emitErrors: [] },
    invalid: {
      revision: Number.MAX_SAFE_INTEGER + 1,
      logs: [],
      twitchStatuses: [],
      emitErrors: [],
    },
  },
  {
    command: "launcher_add",
    call: (c) => c.launcherAdd([]),
    valid: { items: [], addedCount: 0 },
    invalid: { items: [], addedCount: -1 },
  },
  {
    command: "launcher_remove",
    call: (c) => c.launcherRemove("1"),
    valid: [],
    invalid: [{ id: "1", kind: "script", target: "x", displayName: "x", order: 0 }],
  },
  {
    command: "launcher_launch",
    call: (c) => c.launcherLaunch("1"),
    valid: { launchedCount: 0, failures: [] },
    invalid: { launchedCount: 1.5, failures: [] },
  },
  {
    command: "launcher_launch_all",
    call: (c) => c.launcherLaunchAll(),
    valid: { launchedCount: 0, failures: [] },
    invalid: { launchedCount: 0, failures: ["error"] },
  },
  {
    command: "speech_health_check",
    call: (c) => c.speechHealthCheck(),
    valid: "接続しました",
    invalid: 1,
  },
  {
    command: "speech_health_probe",
    call: (c) => c.speechHealthProbe(),
    valid: "接続しました",
    invalid: false,
  },
  {
    command: "speech_connection_diagnostics",
    call: (c) => c.speechConnectionDiagnostics(),
    valid: {
      configuredAddr: "127.0.0.1:50001",
      attempted: [{ addr: "127.0.0.1:50001", status: "connected", message: "ok", elapsedMs: 1 }],
      recommendation: "",
    },
    invalid: {
      configuredAddr: "x",
      attempted: [{ addr: "x", status: "unknown", message: "", elapsedMs: -1 }],
      recommendation: "",
    },
  },
  {
    command: "speech_queue_reload",
    call: (c) => c.speechQueueReload(),
    valid: { revision: 1, status, queue },
    invalid: { revision: 1, status, queue: { ...queue, queuedCount: -1 } },
  },
  {
    command: "twitch_start_auth",
    call: (c) => c.twitchStartAuth(),
    valid: device,
    invalid: { ...device, interval: 0 },
  },
  {
    command: "twitch_poll_auth",
    call: (c) => c.twitchPollAuth(),
    valid: fixture.authorizedPoll,
    invalid: { status: "pending", message: "", interval: -1 },
  },
  {
    command: "twitch_validate_auth",
    call: (c) => c.twitchValidateAuth(),
    valid: { profile },
    invalid: { profile: { ...profile, expiresIn: -1 } },
  },
  {
    command: "twitch_get_stored_auth",
    call: (c) => c.twitchGetStoredAuth(),
    valid: profile,
    invalid: { ...profile, scopes: [null] },
  },
];
const unitCases: Array<{ command: string; call: (client: Client) => Promise<void> }> = [
  { command: "speech_authorize_endpoint", call: (c) => c.authorizeSpeechEndpoint() },
  { command: "speech_test", call: (c) => c.speechTest("hello") },
  ...(["pause", "resume", "skip", "clear"] as const).map((command) => ({
    command: `speech_${command}`,
    call: (c: Client) => c.speechControl(command),
  })),
  { command: "speech_queue_remove", call: (c) => c.speechQueueRemove("1") },
  { command: "speech_queue_dismiss", call: (c) => c.speechQueueDismiss("1") },
  { command: "speech_queue_dismiss_history", call: (c) => c.speechQueueDismissHistory() },
  { command: "speech_queue_retry", call: (c) => c.speechQueueRetry("1") },
  { command: "twitch_connect", call: (c) => c.twitchConnect("streamer") },
  { command: "twitch_stop_chat", call: (c) => c.twitchStopChat() },
  { command: "twitch_disconnect", call: (c) => c.twitchDisconnect() },
  { command: "app_exit", call: (c) => c.appExit() },
  {
    command: "app_open_external_url",
    call: (c) => c.appOpenExternalUrl("https://www.twitch.tv/activate"),
  },
];
beforeEach(() => vi.stubGlobal("window", { __TAURI_INTERNALS__: {} }));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  invoke.mockReset();
});

it.each(cases)(
  "validates $command before returning a response",
  async ({ command, call, valid, invalid }) => {
    const client = await import("./client");
    invoke.mockResolvedValue(valid);
    await expect(call(client)).resolves.toBeDefined();
    expect(invoke.mock.calls[invoke.mock.calls.length - 1]?.[0]).toBe(command);
    for (const payload of [undefined, {}, invalid]) {
      invoke.mockResolvedValue(payload);
      await expect(call(client)).rejects.toThrow("Tauri bridge");
    }
  },
);

it.each(unitCases)("requires serialized unit null for $command", async ({ command, call }) => {
  const client = await import("./client");
  invoke.mockResolvedValue(null);
  await expect(call(client)).resolves.toBeUndefined();
  expect(invoke.mock.calls[invoke.mock.calls.length - 1]?.[0]).toBe(command);
  for (const payload of [undefined, {}, "ok", false]) {
    invoke.mockResolvedValue(payload);
    await expect(call(client)).rejects.toThrow(command);
  }
});

it("rejects every missing Device Code field and unsafe numeric values", () => {
  for (const key of Object.keys(device)) {
    const incomplete: Record<string, unknown> = { ...device };
    delete incomplete[key];
    expect(() =>
      parsePayload(twitchDeviceAuthStartSchema, incomplete, "twitch_start_auth"),
    ).toThrow(key);
  }
  for (const key of ["interval", "expiresIn", "expiresAtMs"] as const) {
    for (const value of [
      0,
      -1,
      1.5,
      Number.NaN,
      Infinity,
      Number.MAX_SAFE_INTEGER + 1,
      "5",
      null,
    ]) {
      expect(() =>
        parsePayload(twitchDeviceAuthStartSchema, { ...device, [key]: value }, "twitch_start_auth"),
      ).toThrow(key);
    }
  }
  expect(() =>
    parsePayload(
      twitchDeviceAuthStartSchema,
      { ...device, verificationUri: "javascript:alert(1)" },
      "twitch_start_auth",
    ),
  ).toThrow("verificationUri");
});

it("keeps nullable command roots distinct from missing responses and nested nulls", async () => {
  const client = await import("./client");
  for (const call of [client.takeSettingsRecoveryNotice, client.twitchGetStoredAuth]) {
    invoke.mockResolvedValue(null);
    await expect(call()).resolves.toBeUndefined();
    invoke.mockResolvedValue(undefined);
    await expect(call()).rejects.toThrow("Tauri bridge");
  }
  invoke.mockResolvedValue({ profile, storageWarning: null });
  await expect(client.twitchValidateAuth()).rejects.toThrow("storageWarning");
  invoke.mockResolvedValue({ ...createDefaultAppSettings(), window: { position: null } });
  await expect(client.getSettings()).rejects.toThrow("window.position");
});
