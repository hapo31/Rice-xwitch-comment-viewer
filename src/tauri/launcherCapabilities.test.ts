import { createDefaultAppSettings } from "../settings/model";
import { afterEach, expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  invoke.mockReset();
});

it.each([true, false])(
  "uses backend OS capabilities, not a user-agent guess (%s)",
  async (windows) => {
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    const launcher = {
      canRegisterApplications: windows,
      canLaunchApplications: windows,
      ...(!windows ? { reason: "Windows版で利用してください。" } : {}),
    };
    invoke.mockResolvedValue({ version: "0.2.3", isDev: true, launcher });
    const client = await import("./client");
    expect(await client.getLauncherCapabilities()).toEqual(launcher);
    expect(invoke).toHaveBeenCalledWith("app_build_info");
  },
);

it.each([
  undefined,
  { canRegisterApplications: "true", canLaunchApplications: true },
  { canRegisterApplications: true, canLaunchApplications: null },
])("fails closed on malformed native capabilities", async (launcher) => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  invoke.mockResolvedValue({ version: "0.2.3", isDev: true, launcher });
  const client = await import("./client");
  await expect(client.getLauncherCapabilities()).rejects.toThrow();
});

it("browser preview cannot register or launch and needs no native calls", async () => {
  vi.stubGlobal("window", {});
  const client = await import("./client");
  expect(await client.getLauncherCapabilities()).toMatchObject({
    canRegisterApplications: false,
    canLaunchApplications: false,
  });
  expect(invoke).not.toHaveBeenCalled();
});

it("sends only editable metadata through the settings patch contract", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
  const { updateSettings } = await import("./client");
  const edit = {
    id: "existing",
    displayName: "名前",
    backgroundColor: "#aabbcc",
    groupId: "配信",
    order: 2,
  };
  invoke.mockResolvedValue(createDefaultAppSettings());
  await updateSettings({ launcher: { items: [edit] } });
  expect(invoke).toHaveBeenCalledWith("settings_update", {
    patch: { launcher: { items: [edit] } },
  });
  expect(invoke.mock.calls[0][1].patch.launcher.items[0]).not.toHaveProperty("target");
  expect(invoke.mock.calls[0][1].patch.launcher.items[0]).not.toHaveProperty("iconDataUrl");
});

it("browser preview does not synthesize a canonical model from a new metadata ID", async () => {
  vi.stubGlobal("window", {});
  const { updateSettings } = await import("./client");
  await expect(
    updateSettings({ launcher: { items: [{ id: "new-id", displayName: "new", order: 0 }] } }),
  ).rejects.toThrow("登録済み");
  expect(invoke).not.toHaveBeenCalled();
});
