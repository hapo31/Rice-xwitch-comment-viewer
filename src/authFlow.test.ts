import { describe, expect, it } from "vitest";
import { authFlowTransition, type AuthFlowState } from "./authFlow";

const profile = { userId: "1", login: "viewer", scopes: ["user:read:chat"], expiresIn: 3600 };
const prompt = {
  userCode: "ABCD-EFGH",
  verificationUri: "https://www.twitch.tv/activate",
  expiresIn: 600,
  expiresAtMs: 10_000,
  interval: 5,
};
const initial: AuthFlowState = { status: "unauthenticated", prompt };

describe("authFlowTransition", () => {
  it("keeps poll outcomes, prompt/profile state, and user-facing effects in one model", () => {
    const waiting = authFlowTransition(initial, {
      type: "poll.waiting",
      interval: 9,
      message: "Twitch is still waiting.",
    });
    expect(waiting.state).toMatchObject({ status: "unauthenticated", prompt: { interval: 9 } });
    expect(waiting.effects).toEqual([{ type: "info", message: "Twitch is still waiting." }]);

    const loggedIn = authFlowTransition(waiting.state, { type: "poll.authorized", profile });
    expect(loggedIn.state).toEqual({ status: "authenticated", profile });
    expect(loggedIn.effects[0]).toMatchObject({
      type: "info",
      message: expect.stringContaining("viewer"),
    });

    const expired = authFlowTransition(waiting.state, {
      type: "poll.denied",
      status: "expired",
      message: "The code expired.",
    });
    expect(expired.state).toEqual({ status: "unauthenticated" });
    expect(expired.effects).toEqual([
      { type: "warning", message: "The code expired.", severity: "warning" },
    ]);
  });

  it("models operation failures, validation, and expiry without imperative result branches", () => {
    const failed = authFlowTransition(initial, {
      type: "prompt.failed",
      error: new Error("offline"),
    });
    expect(failed.state.status).toBe("error");
    expect(failed.effects).toEqual([{ type: "failure", error: expect.any(Error) }]);

    const invalid = authFlowTransition(
      { status: "authenticated", profile },
      {
        type: "validate.invalid",
        error: new Error("unauthorized"),
      },
    );
    expect(invalid.state).toEqual({ status: "unauthenticated" });
    expect(invalid.effects[0].type).toBe("failure");

    const expired = authFlowTransition(initial, {
      type: "prompt.expired",
      message: "Code expired.",
    });
    expect(expired.state).toEqual({ status: "expired" });
    expect(expired.effects[0]).toMatchObject({ type: "warning", message: "Code expired." });
  });
});
