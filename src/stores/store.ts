import { useSyncExternalStoreWithSelector } from "use-sync-external-store/with-selector";

export type SelectionEquality<Selected> = (previous: Selected, next: Selected) => boolean;

export interface ExternalStore<State, Action> {
  getState: () => State;
  dispatch: (action: Action) => void;
  subscribe: (listener: () => void) => () => void;
}

export function createExternalStore<State, Action>(
  reducer: (state: State, action: Action) => State,
  initialState: State,
): ExternalStore<State, Action> {
  let state = initialState;
  const listeners = new Set<() => void>();

  return {
    getState: () => state,
    dispatch(action) {
      const nextState = reducer(state, action);
      if (Object.is(nextState, state)) return;
      state = nextState;
      for (const listener of listeners) listener();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** Select from immutable snapshots; isEqual must mean equal observable output. */
export function useStoreSelector<State, Selected>(
  store: Pick<ExternalStore<State, never>, "getState" | "subscribe">,
  selector: (state: State) => Selected,
  isEqual: SelectionEquality<Selected> = Object.is,
): Selected {
  return useSyncExternalStoreWithSelector(
    store.subscribe,
    store.getState,
    store.getState,
    selector,
    isEqual,
  );
}
