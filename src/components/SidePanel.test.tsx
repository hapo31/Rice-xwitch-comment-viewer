import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { defaultSpeechSettings, defaultTwitchSettings } from "../features/settings/defaults";
import {
  type DomainControllerActions,
  DomainControllerActionsProvider,
} from "../orchestration/domainControllerContext";
import { initialAppState } from "../stores/appState";
import { SidePanel } from "./SidePanel";

const actions: DomainControllerActions = {
  updateSettings: async () => true,
  speechHealthCheck: () => undefined,
  speechDiagnostics: async () => ({
    configuredAddr: "127.0.0.1:50001",
    attempted: [],
    recommendation: "",
  }),
  speechTest: () => undefined,
  speechControl: () => undefined,
  queueReload: () => undefined,
  queueRemove: () => undefined,
  queueDismiss: () => undefined,
  queueDismissHistory: () => undefined,
  queueRetry: () => undefined,
  launcherAdd: async () => ({ items: [], addedCount: 0 }),
  launcherRemove: async () => [],
  launcherLaunch: async () => ({ launchedCount: 0, failures: [] }),
  launcherLaunchAll: async () => ({ launchedCount: 0, failures: [] }),
  twitchStartAuth: () => undefined,
  twitchPollAuth: () => undefined,
  twitchValidateAuth: async () => true,
  twitchDisconnect: () => undefined,
  twitchConnect: () => undefined,
  twitchStopChat: () => undefined,
  openExternalUrl: () => undefined,
  clearWarnings: () => undefined,
};

describe("SidePanel speech recovery", () => {
  it("shows the active channel separately from a changed configured channel", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/chat"]}>
        <DomainControllerActionsProvider actions={actions}>
          <SidePanel
            state={{
              ...initialAppState,
              twitchConnectionStatus: "connected",
              twitchActiveConnection: {
                generation: 1,
                broadcasterUserId: "a",
                broadcasterLogin: "channel_a",
              },
              settings: {
                twitch: { ...defaultTwitchSettings(), channelLogin: "channel_b" },
                speech: defaultSpeechSettings(),
                launcher: { items: [] },
              },
            }}
          />
        </DomainControllerActionsProvider>
      </MemoryRouter>,
    );

    expect(markup).toContain("channel_a");
    expect(markup).toContain("次回接続先");
    expect(markup).toContain("channel_b");
  });

  it("links a disconnected speech status to the Settings diagnostic", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/chat"]}>
        <DomainControllerActionsProvider actions={actions}>
          <SidePanel state={initialAppState} />
        </DomainControllerActionsProvider>
      </MemoryRouter>,
    );

    expect(markup).toContain('href="/settings"');
    expect(markup).toContain("Settings 画面の［診断］を開く");
  });
});
