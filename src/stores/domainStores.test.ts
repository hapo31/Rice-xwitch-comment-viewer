import { describe, expect, it } from "vitest";
import { createDomainStores } from "./domainStores";
import { utcTimestamp } from "../time";

describe("domain store subscription boundaries", () => {
  it("keeps the newest active Twitch connection when an older generation arrives late", () => {
    const stores = createDomainStores();
    stores.connection.dispatch({
      type: "chat.status.changed",
      status: "connected",
      revision: 10,
      connectionGeneration: 2,
      activeConnection: { generation: 2, broadcasterUserId: "b", broadcasterLogin: "channel_b" },
    });
    stores.connection.dispatch({
      type: "chat.status.changed",
      status: "connected",
      revision: 11,
      connectionGeneration: 1,
      activeConnection: { generation: 1, broadcasterUserId: "a", broadcasterLogin: "channel_a" },
    });

    expect(stores.connection.getState()).toMatchObject({
      twitchConnectionStatus: "connected",
      twitchConnectionGeneration: 2,
      twitchActiveConnection: { generation: 2, broadcasterLogin: "channel_b" },
    });
  });

  it("notifies only the chat subscribers for a chat event", () => {
    const stores = createDomainStores();
    const renders = { chat: 0, queue: 0, connection: 0, settings: 0, logs: 0 };
    stores.chat.subscribe(() => renders.chat++);
    stores.queue.subscribe(() => renders.queue++);
    stores.connection.subscribe(() => renders.connection++);
    stores.settings.subscribe(() => renders.settings++);
    stores.logs.subscribe(() => renders.logs++);

    stores.chat.dispatch({
      type: "message.added",
      message: {
        kind: "system",
        id: "system-1",
        receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
        userDisplayName: "system",
        text: "chat event",
      },
    });

    expect(renders).toEqual({ chat: 1, queue: 0, connection: 0, settings: 0, logs: 0 });
  });

  it("keeps queue status synchronization inside the chat/queue boundary", () => {
    const stores = createDomainStores();
    stores.chat.dispatch({
      type: "message.added",
      message: {
        kind: "user",
        id: "message-1",
        receivedAt: utcTimestamp("2026-08-01T00:00:00Z"),
        userDisplayName: "viewer",
        text: "hello",
        status: "queued",
      },
    });
    stores.queue.dispatch({
      type: "items.replaced",
      items: [{ id: "queue-1", sourceMessageId: "message-1", userDisplayName: "viewer", text: "hello", status: "spoken" }],
    });
    stores.chat.dispatch({ type: "queue.statuses.changed", items: stores.queue.getState().items });

    expect(stores.chat.getState().messages[0]).toMatchObject({ id: "message-1", status: "spoken" });
  });
});
