import { expect, it } from "vitest";
import cases from "../tauri/fixtures/speech-independent-states.json";
import { createDomainStores } from "../stores/domainStores";
import { dispatchDomainAction } from "./domainOrchestration";
import { parseSpeechStateSnapshot } from "../tauri/bridge";

it.each(cases)(
  "replays backend/frontend independent speech contract: $name",
  ({ steps, expected }) => {
    const stores = createDomainStores();
    let status = { revision: 1, status: "disconnected", adapterHealth: "unknown", occurredAtMs: 1 };
    let queue = { revision: 2, phase: "idle", items: [], queuedCount: 0, occurredAtMs: 1 };
    let revision = 2;
    for (const step of steps) {
      revision++;
      if ("health" in step) {
        status = {
          ...status,
          revision,
          adapterHealth: step.health!,
          status:
            step.health === "connected"
              ? "idle"
              : step.health === "error"
                ? "error"
                : "disconnected",
        };
      } else {
        queue = { ...queue, revision, phase: step.phase! };
      }
      const paired = parseSpeechStateSnapshot({ revision, status, queue });
      dispatchDomainAction(stores, { type: "speech.snapshot", snapshot: paired });
      expect(stores.connection.getState().speechAdapterHealth).toBe(status.adapterHealth);
      expect(stores.queue.getState().phase).toBe(queue.phase);
    }
    expect(stores.connection.getState().speechAdapterHealth).toBe(expected.adapterHealth);
    expect(stores.queue.getState().phase).toBe(expected.phase);
    expect(stores.queue.getState().items).toEqual([]); // Probe never retries failed items.
  },
);
