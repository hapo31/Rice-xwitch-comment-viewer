export type Unlisten = () => void;
export type AsyncSubscription = () => Promise<Unlisten>;

/**
 * Registers independent native listeners safely across React unmounts.
 * A listener that resolves after cleanup is immediately removed; one failed
 * registration never prevents already registered listeners from being removed.
 */
export function subscribeWithCleanup(
  subscriptions: AsyncSubscription[],
  onError: (error: unknown) => void = () => undefined,
): Unlisten {
  let disposed = false;
  const active: Unlisten[] = [];

  const reportError = (error: unknown) => {
    try {
      onError(error);
    } catch {
      // A notification handler must not prevent cleanup of native listeners.
    }
  };

  const safelyUnlisten = (unlisten: Unlisten) => {
    try {
      unlisten();
    } catch (error) {
      reportError(error);
    }
  };

  for (const subscribe of subscriptions) {
    void Promise.resolve().then(subscribe).then(
      (unlisten) => {
        if (disposed) {
          safelyUnlisten(unlisten);
        } else {
          active.push(unlisten);
        }
      },
      reportError,
    );
  }

  return () => {
    disposed = true;
    for (const unlisten of active) {
      safelyUnlisten(unlisten);
    }
    active.length = 0;
  };
}
