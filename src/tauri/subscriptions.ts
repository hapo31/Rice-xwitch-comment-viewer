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
  const active = new Set<Unlisten>();

  for (const subscribe of subscriptions) {
    void subscribe().then(
      (unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          active.add(unlisten);
        }
      },
      (error) => onError(error),
    );
  }

  return () => {
    disposed = true;
    for (const unlisten of active) {
      unlisten();
    }
    active.clear();
  };
}
