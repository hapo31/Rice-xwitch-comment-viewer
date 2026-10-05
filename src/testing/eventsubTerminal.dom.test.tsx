import { act, render, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { expect, it } from "vitest";
import { AppShell } from "../AppShell";
import { createDomainStores, DomainProvider } from "../stores/domainStores";
import { tauriMock } from "./tauriMock";

it.each(["event", "snapshot"])(
  "retains a terminal subscription failure in connection state, Logs and system Chat via %s",
  async (delivery) => {
    const message =
      "Twitch EventSub 購読の条件が無効です。接続チャンネルとアプリ設定を確認してください。";
    const status = {
      revision: 2,
      domain: "chat",
      status: "error",
      message,
      connectionGeneration: 7,
      occurredAtMs: 1,
    };
    const log = { id: "terminal-7", level: "error", message, occurredAtMs: 1 };
    if (delivery === "snapshot") {
      tauriMock.setCommand("app_events_snapshot", {
        revision: 3,
        logs: [log],
        twitchStatuses: [status],
        emitErrors: [],
      });
    }
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
    await waitFor(() => expect(tauriMock.invoke).toHaveBeenCalledWith("app_events_snapshot"));
    await act(async () => undefined);
    if (delivery === "event") {
      act(() => {
        tauriMock.emit("twitch://status", {
          ...status,
          revision: 1,
          status: "connecting",
          message: "接続中",
        });
        tauriMock.emit("twitch://status", status);
        tauriMock.emit("app://log", log);
      });
    }
    await waitFor(() => expect(stores.connection.getState().twitchConnectionStatus).toBe("error"));
    expect(stores.connection.getState().twitchConnectionGeneration).toBe(7);
    expect(stores.logs.getState().logs.some((entry) => entry.message === message)).toBe(true);
    expect(
      stores.chat
        .getState()
        .messages.some((entry) => entry.kind === "system" && entry.text === message),
    ).toBe(true);
  },
);
