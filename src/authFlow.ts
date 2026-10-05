import type {
  AuthStatus,
  NotificationSeverity,
  TwitchDeviceAuthStart,
  TwitchUserProfile,
} from "./types";

export interface AuthFlowState {
  status: AuthStatus;
  prompt?: TwitchDeviceAuthStart;
  profile?: TwitchUserProfile;
}

export type AuthFlowEvent =
  | { type: "restore.started" }
  | { type: "restore.authenticated"; profile: TwitchUserProfile }
  | { type: "restore.missing" }
  | { type: "restore.failed"; message: string }
  | { type: "prompt.requested" }
  | { type: "validate.started" }
  | { type: "validate.valid"; profile: TwitchUserProfile }
  | { type: "validate.invalid"; error: unknown }
  | { type: "prompt.failed"; error: unknown }
  | { type: "disconnect.started" }
  | { type: "disconnect.succeeded" }
  | { type: "disconnect.failed"; error: unknown }
  | { type: "poll.authorized"; profile: TwitchUserProfile }
  | { type: "poll.started" }
  | { type: "poll.waiting"; interval: number; message: string }
  | { type: "poll.denied"; status: "denied" | "expired"; message: string }
  | { type: "poll.failed"; error: unknown }
  | { type: "prompt.started"; prompt: TwitchDeviceAuthStart }
  | { type: "prompt.expired"; message: string };

export type AuthFlowEffect =
  | { type: "info"; message?: string }
  | { type: "failure"; error: unknown }
  | { type: "notification"; message: string; severity: NotificationSeverity }
  | { type: "warning"; message: string; severity: NotificationSeverity };

export interface AuthFlowTransition {
  state: AuthFlowState;
  effects: AuthFlowEffect[];
}

/** Pure transition model for Device Code outcomes; timers remain owned by the controller. */
export function authFlowTransition(state: AuthFlowState, event: AuthFlowEvent): AuthFlowTransition {
  switch (event.type) {
    case "restore.started":
      return { state: { ...state, status: "checking" }, effects: [] };
    case "restore.authenticated":
      return { state: { status: "authenticated", profile: event.profile }, effects: [] };
    case "restore.missing":
      return { state: { status: "unauthenticated" }, effects: [] };
    case "restore.failed":
      return {
        state: { status: "unauthenticated" },
        effects: [{ type: "notification", message: event.message, severity: "error" }],
      };
    case "prompt.requested":
      return { state: { ...state, status: "authorizing" }, effects: [] };
    case "validate.started":
      return { state: { ...state, status: "checking" }, effects: [] };
    case "validate.valid":
      return {
        state: { status: "authenticated", profile: event.profile },
        effects: [{ type: "info", message: "Twitch 認証は有効です。" }],
      };
    case "validate.invalid":
      return {
        state: { status: "unauthenticated" },
        effects: [{ type: "failure", error: event.error }],
      };
    case "prompt.failed":
      return {
        state: { status: "error", prompt: state.prompt, profile: state.profile },
        effects: [{ type: "failure", error: event.error }],
      };
    case "disconnect.started":
      return { state: { ...state, status: "disconnecting" }, effects: [] };
    case "disconnect.succeeded":
      return { state: { status: "unauthenticated" }, effects: [] };
    case "disconnect.failed":
      return {
        state: { ...state, status: "error" },
        effects: [{ type: "failure", error: event.error }],
      };
    case "prompt.started":
      return { state: { status: "unauthenticated", prompt: event.prompt }, effects: [] };
    case "poll.authorized":
      return {
        state: { status: "authenticated", profile: event.profile },
        effects: [
          { type: "info", message: `Twitch に ${event.profile.login} としてログインしました。` },
        ],
      };
    case "poll.started":
      return { state: { ...state, status: "polling" }, effects: [] };
    case "poll.waiting":
      return {
        state: {
          ...state,
          status: "unauthenticated",
          prompt: state.prompt ? { ...state.prompt, interval: event.interval } : undefined,
        },
        effects: [{ type: "info", message: event.message }],
      };
    case "poll.denied":
      return {
        state: { status: "unauthenticated" },
        effects: [{ type: "warning", message: event.message, severity: "warning" }],
      };
    case "poll.failed":
      return {
        state: { ...state, status: "error" },
        effects: [{ type: "failure", error: event.error }],
      };
    case "prompt.expired":
      return {
        state: { status: "expired" },
        effects: [{ type: "warning", message: event.message, severity: "warning" }],
      };
  }
}
