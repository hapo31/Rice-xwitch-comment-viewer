import { act, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { defaultSettings, tauriMock } from "./tauriMock";

const outcome = {
  kind: "skipped",
  reasonCode: "autoSpeakDisabled",
  message: "自動読み上げが OFF のため読み上げ対象外です。",
  retryable: false,
  recoveryAction: "none",
  occurredAtMs: 1,
};
const item = {
  id: "speech-off",
  sourceMessageId: "received-off",
  userDisplayName: "Viewer",
  text: "対象外のコメント",
  status: "skipped",
  outcome,
};
const queue = { revision: 10, phase: "idle", items: [item], queuedCount: 0, occurredAtMs: 1 };
const chat = {
  id: "received-off",
  platform: "twitch",
  channelId: "channel",
  channelLogin: "streamer",
  userId: "viewer",
  userLogin: "viewer",
  userDisplayName: "Viewer",
  text: "対象外のコメント",
  fragments: [],
  badges: [],
  receivedAt: "2026-10-05T00:00:00Z",
  connectionGeneration: 3,
};

it.each(["message-first", "queue-first", "snapshot"] as const)(
  "backend OFF outcome survives %s ordering and current ON settings",
  async (order) => {
    tauriMock.setCommand("settings_get", {
      ...defaultSettings,
      speech: { ...defaultSettings.speech, autoSpeak: true },
    });
    if (order === "snapshot")
      tauriMock.setCommand("speech_queue_reload", {
        revision: 10,
        status: { revision: 10, status: "idle", adapterHealth: "connected", occurredAtMs: 1 },
        queue,
      });
    const stores = createDomainStores();
    const router = createMemoryRouter(
      [
        {
          path: "*",
          element: (
            <DomainProvider stores={stores}>
              <AppShell />
            </DomainProvider>
          ),
        },
      ],
      { initialEntries: ["/chat"] },
    );
    render(<RouterProvider router={router} />);
    await waitFor(() => expect(stores.settings.getState().settings).toBeDefined());
    await waitFor(() => expect(tauriMock.listenerCount("twitch://chat-message")).toBe(1));
    if (order === "snapshot")
      await waitFor(() => expect(stores.queue.getState().revision).toBe(10));
    await act(async () => {
      tauriMock.emit("twitch://status", {
        revision: 20,
        domain: "chat",
        status: "connected",
        connectionGeneration: 3,
        activeConnection: {
          generation: 3,
          broadcasterUserId: "channel",
          broadcasterLogin: "streamer",
        },
        occurredAtMs: 1,
      });
    });
    if (order === "queue-first")
      await act(async () => tauriMock.emit("speech://queue-updated", queue));
    await act(async () => tauriMock.emit("twitch://chat-message", chat));
    const row = screen.getByText(chat.text).closest('[role="row"]');
    if (!(row instanceof HTMLElement)) throw new Error("Chat row missing");
    if (order === "message-first") {
      expect(within(row).getByText("受信済み")).toBeInTheDocument();
      expect(within(row).queryByText("待機")).not.toBeInTheDocument();
      await act(async () => tauriMock.emit("speech://queue-updated", queue));
    }
    await waitFor(() => expect(within(row).getByText("スキップ")).toBeInTheDocument());
    expect(stores.chat.getState().messages.find((message) => message.id === chat.id)).toMatchObject(
      {
        status: "skipped",
        speechOutcome: outcome,
      },
    );
    expect(stores.queue.getState().items.filter((entry) => entry.status === "queued")).toHaveLength(
      0,
    );
    // Later preference updates cannot rewrite a past backend admission decision.
    await act(async () =>
      stores.settings.dispatch({
        type: "settings.loaded",
        settings: {
          ...defaultSettings,
          speech: { ...defaultSettings.speech, autoSpeak: false },
        },
      }),
    );
    expect(within(row).getByText("スキップ")).toBeInTheDocument();
    const accepted = { ...chat, id: "received-on", text: "受付済みのコメント" };
    await act(async () => {
      tauriMock.emit("twitch://chat-message", accepted);
      tauriMock.emit("speech://queue-updated", {
        ...queue,
        revision: 11,
        phase: "paused",
        queuedCount: 1,
        items: [
          item,
          {
            id: "speech-on",
            sourceMessageId: accepted.id,
            userDisplayName: "Viewer",
            text: accepted.text,
            status: "queued",
          },
        ],
      });
    });
    const acceptedRow = screen.getByText(accepted.text).closest('[role="row"]');
    if (!(acceptedRow instanceof HTMLElement)) throw new Error("Accepted chat row missing");
    expect(within(acceptedRow).getByText("待機")).toBeInTheDocument();
  },
);
