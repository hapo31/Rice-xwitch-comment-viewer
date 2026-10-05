import { describe, expect, it } from "vitest";
import {
  authOperationReducer,
  AuthOperationController,
  initialAuthOperationState,
} from "./authOperation";

describe("AuthOperationController", () => {
  it("invalidates delayed operations after a newer operation begins", () => {
    const controller = new AuthOperationController();
    const first = controller.begin();
    const second = controller.begin();

    expect(controller.isCurrent(first)).toBe(false);
    expect(controller.isCurrent(second)).toBe(true);
  });

  it("allows only one poll for the current generation", () => {
    const controller = new AuthOperationController();
    const generation = controller.begin();

    expect(controller.tryBeginPoll()).toBe(generation);
    expect(controller.getState()).toMatchObject({
      activeOperation: "poll",
      pollGeneration: generation,
    });
    expect(controller.tryBeginPoll()).toBeUndefined();
    controller.finishPoll(generation);
    expect(controller.getState()).toMatchObject({
      activeOperation: undefined,
      pollGeneration: undefined,
    });
    expect(controller.tryBeginPoll()).toBe(generation);
  });

  it("reduces manual operations as generation-changing transitions that cancel pending polls", () => {
    const restoring = authOperationReducer(initialAuthOperationState, {
      type: "begin",
      operation: "restore",
    });
    const polling = authOperationReducer(restoring, { type: "poll.begin" });
    const manual = authOperationReducer(polling, { type: "begin", operation: "start" });

    expect(manual).toEqual({
      generation: restoring.generation + 1,
      activeOperation: "start",
      pollGeneration: undefined,
    });
    expect(
      authOperationReducer(manual, { type: "poll.finish", generation: polling.generation }),
    ).toBe(manual);
  });

  it("does not let a stale poll unlock a newer poll", () => {
    const controller = new AuthOperationController();
    const stale = controller.begin();
    expect(controller.tryBeginPoll()).toBe(stale);
    const current = controller.begin();
    expect(controller.tryBeginPoll()).toBe(current);
    controller.finishPoll(stale);

    expect(controller.tryBeginPoll()).toBeUndefined();
  });

  it("rejects a deferred startup validation after a newer authentication starts", async () => {
    const controller = new AuthOperationController();
    const startup = controller.begin();
    let resolveValidation!: () => void;
    const validation = new Promise<void>((resolve) => {
      resolveValidation = resolve;
    });
    const newAuthentication = controller.begin();

    resolveValidation();
    await validation;

    expect(controller.isCurrent(startup)).toBe(false);
    expect(controller.isCurrent(newAuthentication)).toBe(true);
  });
});
