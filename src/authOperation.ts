export type AuthOperationName = "restore" | "start" | "poll" | "validate" | "disconnect";

export interface AuthOperationState {
  generation: number;
  activeOperation: AuthOperationName | undefined;
  pollGeneration: number | undefined;
}

export type AuthOperationEvent =
  | { type: "begin"; operation: Exclude<AuthOperationName, "poll"> }
  | { type: "poll.begin" }
  | { type: "poll.finish"; generation: number };

export const initialAuthOperationState: AuthOperationState = {
  generation: 0,
  activeOperation: undefined,
  pollGeneration: undefined,
};

/** Reducer for manual priority, stale operation rejection, and single-flight polling. */
export function authOperationReducer(
  state: AuthOperationState,
  event: AuthOperationEvent,
): AuthOperationState {
  switch (event.type) {
    case "begin":
      return {
        generation: state.generation + 1,
        activeOperation: event.operation,
        pollGeneration: undefined,
      };
    case "poll.begin":
      if (state.pollGeneration === state.generation) return state;
      return {
        ...state,
        activeOperation: "poll",
        pollGeneration: state.generation,
      };
    case "poll.finish":
      if (event.generation !== state.generation || state.pollGeneration !== event.generation)
        return state;
      return { ...state, activeOperation: undefined, pollGeneration: undefined };
  }
}

export class AuthOperationController {
  private state = initialAuthOperationState;

  begin(operation: Exclude<AuthOperationName, "poll"> = "restore"): number {
    this.transition({ type: "begin", operation });
    return this.state.generation;
  }

  isCurrent(generation: number): boolean {
    return generation === this.state.generation;
  }

  tryBeginPoll(): number | undefined {
    const previous = this.state;
    this.transition({ type: "poll.begin" });
    return this.state === previous ? undefined : this.state.generation;
  }

  finishPoll(generation: number) {
    this.transition({ type: "poll.finish", generation });
  }

  getState(): AuthOperationState {
    return this.state;
  }

  private transition(event: AuthOperationEvent) {
    this.state = authOperationReducer(this.state, event);
  }
}
