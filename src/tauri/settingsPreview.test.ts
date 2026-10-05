import { afterEach, expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  invoke.mockReset();
});

it("accumulates preview patches across sections and fields and returns reload state", async () => {
  vi.stubGlobal("window", {});
  const { getSettings, updateSettings } = await import("./client");

  const afterPort = await updateSettings({ speech: { bouyomiPort: 50002 } });
  expect(afterPort.speech.bouyomiPort).toBe(50002);

  const afterChannel = await updateSettings({ twitch: { channelLogin: "rice" } });
  expect(afterChannel.speech.bouyomiPort).toBe(50002);
  expect(afterChannel.twitch.channelLogin).toBe("rice");

  const afterHost = await updateSettings({ speech: { bouyomiHost: "localhost" } });
  expect(afterHost.speech.bouyomiPort).toBe(50002);
  expect(afterHost.speech.bouyomiHost).toBe("localhost");

  expect(await getSettings()).toEqual(afterHost);
  expect(invoke).not.toHaveBeenCalled();
});

it("returns copies so callers cannot mutate the preview store", async () => {
  vi.stubGlobal("window", {});
  const { getSettings } = await import("./client");
  const result = await getSettings();
  result.speech.blockedUsers.push("viewer");
  expect((await getSettings()).speech.blockedUsers).toEqual([]);
});

it("snapshots input arrays and uses saved preview speech settings for diagnostics", async () => {
  vi.stubGlobal("window", {});
  const { getSettings, speechConnectionDiagnostics, updateSettings } = await import("./client");
  const blockedUsers = ["viewer"];
  const blockedWords = ["spoiler"];

  await updateSettings({
    speech: { blockedUsers, blockedWords, bouyomiHost: "localhost", bouyomiPort: 50002 },
  });
  blockedUsers.push("later");
  blockedWords[0] = "changed";

  expect((await getSettings()).speech).toMatchObject({
    blockedUsers: ["viewer"],
    blockedWords: ["spoiler"],
  });
  const diagnostics = await speechConnectionDiagnostics();
  expect(diagnostics.configuredAddr).toBe("localhost:50002");
  expect(diagnostics.attempted[0]?.addr).toBe("localhost:50002");
});
