import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { SidePanel } from "./SidePanel";
import { initialAppState } from "../stores/appStore";
import {
  DomainControllerActionsProvider,
  type DomainControllerActions,
} from "../orchestration/domainControllerContext";

const actions = {} as DomainControllerActions;

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
                twitch: {
                  channelLogin: "channel_b",
                  autoConnect: false,
                  confirmBeforeStopChat: true,
                  liveChatAnnouncements: true,
                },
                speech: {} as any,
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
