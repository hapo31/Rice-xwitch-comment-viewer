import { memo, Profiler, type ProfilerOnRenderCallback } from "react";
import { act, render, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { DomainSidePanel, DomainStatusBar } from "../components/domainShellViews";
import { DomainLauncherView, DomainLogsView, DomainSettingsView } from "../features/domainViews";
import {
  DomainControllerActionsProvider,
  type DomainControllerActions,
} from "../orchestration/domainControllerContext";
import {
  createDomainStores,
  DomainProvider,
  useConnectionSelector,
  useQueueSelector,
  useSettingsSelector,
} from "../stores/domainStores";

const actions: DomainControllerActions = {
  updateSettings: async () => true,
  speechHealthCheck: () => undefined,
  speechDiagnostics: async () => ({}) as never,
  speechTest: () => undefined,
  speechControl: () => undefined,
  queueReload: () => undefined,
  queueRemove: () => undefined,
  queueDismiss: () => undefined,
  queueDismissHistory: () => undefined,
  queueRetry: () => undefined,
  launcherAdd: async () => [],
  launcherRemove: async () => [],
  launcherLaunch: async () => ({ launchedCount: 0, successes: [], failures: [] }),
  launcherLaunchAll: async () => ({ launchedCount: 0, successes: [], failures: [] }),
  twitchStartAuth: () => undefined,
  twitchPollAuth: () => undefined,
  twitchValidateAuth: async () => true,
  twitchDisconnect: () => undefined,
  twitchConnect: () => undefined,
  twitchStopChat: () => undefined,
  openExternalUrl: () => undefined,
  clearWarnings: () => undefined,
};

let commits: Record<"settings" | "logs" | "launcher", number>;
let renderBody: () => React.JSX.Element;
const MemoizedDomainBody = memo(function MemoizedDomainBody() {
  return renderBody();
});

function ControllerHostProbe() {
  useConnectionSelector((state) => state);
  useSettingsSelector((state) => state.settings);
  useQueueSelector((state) => state);
  return <MemoizedDomainBody />;
}

describe("domain view render boundaries", () => {
  it("does not rerender Settings, Logs, or Launcher when only the queue changes", async () => {
    const stores = createDomainStores();
    commits = { settings: 0, logs: 0, launcher: 0 };
    const count =
      (key: keyof typeof commits): ProfilerOnRenderCallback =>
      () => {
        commits[key] += 1;
      };

    renderBody = () => (
      <MemoryRouter initialEntries={["/settings"]}>
        <Profiler id="settings" onRender={count("settings")}>
          <DomainSettingsView />
        </Profiler>
        <Profiler id="logs" onRender={count("logs")}>
          <DomainLogsView />
        </Profiler>
        <Profiler id="launcher" onRender={count("launcher")}>
          <DomainLauncherView />
        </Profiler>
      </MemoryRouter>
    );

    render(
      <DomainProvider stores={stores}>
        <DomainControllerActionsProvider actions={actions}>
          <ControllerHostProbe />
        </DomainControllerActionsProvider>
      </DomainProvider>,
    );

    await waitFor(() => expect(commits.launcher).toBeGreaterThan(0));
    const baseline = { ...commits };
    act(() => {
      stores.queue.dispatch({
        type: "items.replaced",
        phase: "speaking",
        items: [
          {
            id: "queue-1",
            userDisplayName: "viewer",
            text: "hello",
            status: "queued",
          },
        ],
      });
    });

    expect(commits).toEqual(baseline);
  });
});
