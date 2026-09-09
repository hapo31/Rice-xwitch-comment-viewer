import { afterEach, expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => { vi.unstubAllGlobals(); vi.resetModules(); invoke.mockReset(); });

it("returns native command snapshots, including paused state, without emitting a reload event", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const events = { revision: 4, logs: [], twitchStatuses: [], emitErrors: [] };
  const speech = { revision: 5, status: { revision: 4, status: "paused", adapterHealth: "connected", occurredAtMs: 1 }, queue: { revision: 5, phase: "paused", items: [], queuedCount: 0, occurredAtMs: 1 } };
  invoke.mockImplementation(async (command: string) => command === "app_events_snapshot" ? events : speech);
  const client = await import("./client");
  expect(await client.getAppEventsSnapshot()).toEqual(events);
  expect(await client.speechQueueReload()).toEqual(speech);
  expect(invoke.mock.calls).toEqual([["app_events_snapshot"], ["speech_queue_reload"]]);
});
it("preview has no native snapshot and needs no Tauri calls", async () => {
  vi.stubGlobal("window", {});
  const client = await import("./client");
  expect(await client.getAppEventsSnapshot()).toBeUndefined();
  expect(await client.speechQueueReload()).toBeUndefined();
  expect(invoke).not.toHaveBeenCalled();
});
