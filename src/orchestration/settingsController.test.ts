import { expect, it, vi } from "vitest";
import { defaultSpeechSettings, defaultTwitchSettings } from "../features/settings/defaults";
import { createSettingsStore } from "../stores/settingsStore";
import type { AppSettings } from "../types";
import { createSettingsController } from "./settingsController";

const settings = (port: number): AppSettings => ({
  twitch: defaultTwitchSettings(),
  speech: { ...defaultSpeechSettings(), bouyomiPort: port },
  launcher: { items: [] },
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function harness(loadSettings: () => Promise<AppSettings>) {
  const store = createSettingsStore();
  const errors = vi.fn();
  const recovery = vi.fn();
  const takeRecoveryNotice = vi.fn(async () => ({ message: "バックアップから復旧しました。" }));
  const updateSettings = vi.fn(async () => settings(50002));
  const controller = createSettingsController({
    getSettingsRevision: () => store.getState().revision,
    initialGeneration: 0,
    loadSettings,
    updateSettings,
    takeRecoveryNotice,
    onSettingsLoaded: (settings) => store.dispatch({ type: "settings.loaded", settings }),
    onInitializationChanged: (initialization) =>
      store.dispatch({ type: "initialization.changed", initialization }),
    onError: errors,
    onRecoveryNotice: recovery,
    loadErrorMessage: () => "設定を読み込めませんでした。再試行してください。",
  });
  return { controller, store, errors, recovery, takeRecoveryNotice, updateSettings };
}

it.each(["resolve", "reject"] as const)(
  "ignores a stale startup read that completes with %s after a successful save",
  async (completion) => {
    const read = deferred<AppSettings>();
    const h = harness(() => read.promise);
    const loading = h.controller.load();
    await expect(h.controller.mutate({ speech: { bouyomiPort: 50002 } })).resolves.toBe(true);
    if (completion === "resolve") read.resolve(settings(50001));
    else read.reject(new Error("stale read failure"));
    await expect(loading).resolves.toBe(false);
    expect(h.store.getState().settings?.speech.bouyomiPort).toBe(50002);
    expect(h.store.getState().initialization.status).toBe("ready");
    expect(h.errors).not.toHaveBeenCalled();
  },
);

it("publishes only the latest read and retains one-shot recovery across StrictMode lifetimes", async () => {
  const old = deferred<AppSettings>();
  const fresh = deferred<AppSettings>();
  const h = harness(vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise));
  const oldLoad = h.controller.load();
  h.controller.invalidate();
  h.controller.activate();
  const newLoad = h.controller.load();
  fresh.resolve(settings(50003));
  await expect(newLoad).resolves.toBe(true);
  old.resolve(settings(50001));
  await expect(oldLoad).resolves.toBe(false);
  expect(h.store.getState().settings?.speech.bouyomiPort).toBe(50003);
  expect(h.store.getState().initialization).toEqual({ status: "ready", generation: 2 });
  expect(h.takeRecoveryNotice).toHaveBeenCalledOnce();
  expect(h.recovery).toHaveBeenCalledOnce();
});

it("retries a failed read and ignores older initialization actions", async () => {
  const h = harness(
    vi.fn().mockRejectedValueOnce(new Error("unavailable")).mockResolvedValueOnce(settings(50004)),
  );
  await expect(h.controller.load()).resolves.toBe(false);
  expect(h.store.getState().initialization).toMatchObject({ status: "error", generation: 1 });
  await expect(h.controller.load()).resolves.toBe(true);
  h.store.dispatch({
    type: "initialization.changed",
    initialization: { status: "error", generation: 1, message: "old" },
  });
  expect(h.store.getState().initialization).toEqual({ status: "ready", generation: 2 });
  expect(h.errors).toHaveBeenCalledOnce();
});

it("does not publish a disposed read or save, and does not execute queued work from its old lifetime", async () => {
  const read = deferred<AppSettings>();
  const save = deferred<AppSettings>();
  const h = harness(() => read.promise);
  h.updateSettings.mockReturnValue(save.promise);
  const loading = h.controller.load();
  const first = h.controller.mutate({ speech: { bouyomiPort: 50002 } });
  const second = h.controller.mutate({ speech: { bouyomiPort: 50003 } });
  await Promise.resolve();
  h.controller.invalidate();
  read.resolve(settings(50001));
  save.resolve(settings(50002));
  expect(await Promise.all([loading, first, second])).toEqual([false, false, false]);
  expect(h.updateSettings).toHaveBeenCalledOnce();
  expect(h.store.getState().settings).toBeUndefined();
  expect(h.recovery).not.toHaveBeenCalled();
  expect(h.errors).not.toHaveBeenCalled();
});

it("rejects inverse completion of concurrent reads in the same lifetime", async () => {
  const old = deferred<AppSettings>();
  const fresh = deferred<AppSettings>();
  const h = harness(vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise));
  const first = h.controller.load();
  const second = h.controller.load();
  fresh.resolve(settings(50003));
  await expect(second).resolves.toBe(true);
  old.resolve(settings(50001));
  await expect(first).resolves.toBe(false);
  expect(h.store.getState().settings?.speech.bouyomiPort).toBe(50003);
});

it("retains launcher settings published while a reload is pending", async () => {
  const read = deferred<AppSettings>();
  const h = harness(() => read.promise);
  h.store.dispatch({ type: "settings.loaded", settings: settings(50001) });
  const loading = h.controller.load();
  const items = [
    {
      id: "new-app",
      kind: "application" as const,
      target: "C:\\app.exe",
      displayName: "App",
      order: 0,
    },
  ];
  h.store.dispatch({ type: "launcher.items.changed", items });
  read.resolve(settings(50001));
  await expect(loading).resolves.toBe(false);
  expect(h.store.getState().settings?.launcher.items).toEqual(items);
  expect(h.store.getState().initialization.status).toBe("ready");
});
