import { useEffect, useRef, useState } from "react";

/** Keeps only fields the user edited; all other values follow the latest saved settings. */
export function useFormDraft<T extends Record<string, unknown>>(savedValues: T) {
  type Entry<K extends keyof T> = { value: T[K]; version: number };
  type Patch = { [K in keyof T]?: Entry<K> };
  type Snapshot = Patch;
  const [patch, setPatch] = useState<Patch>({});
  const nextVersion = useRef(0);
  const pending = useRef(new Map<keyof T, Set<number>>());
  const latestSavedValues = useRef(savedValues);
  latestSavedValues.current = savedValues;
  const values = Object.assign(
    {},
    savedValues,
    Object.fromEntries(
      Object.entries(patch).map(([key, entry]) => [key, (entry as { value: unknown }).value]),
    ),
  ) as T;

  function isPending(key: keyof T) {
    return (pending.current.get(key)?.size ?? 0) > 0;
  }

  useEffect(() => {
    setPatch((current) => {
      let changed = false;
      const next: Patch = {};
      for (const key of Object.keys(current) as (keyof T)[]) {
        const entry = current[key];
        if (entry && Object.is(entry.value, savedValues[key]) && !isPending(key)) {
          changed = true;
        } else {
          next[key] = entry;
        }
      }
      return changed ? next : current;
    });
  }, [savedValues]);

  function setValue<K extends keyof T>(key: K, value: T[K]) {
    const version = ++nextVersion.current;
    setPatch((current) => {
      if (Object.is(value, savedValues[key]) && !isPending(key)) {
        if (!(key in current)) return current;
        const next = { ...current };
        delete next[key];
        return next;
      }
      if (current[key]?.value === value) return current;
      return { ...current, [key]: { value, version } };
    });
  }

  function beginSave(): Snapshot {
    const snapshot = patch;
    for (const key of Object.keys(snapshot) as (keyof T)[]) {
      const version = snapshot[key]?.version;
      if (version === undefined) continue;
      const versions = pending.current.get(key) ?? new Set<number>();
      versions.add(version);
      pending.current.set(key, versions);
    }
    return snapshot;
  }

  function finishSave(snapshot: Snapshot, succeeded: boolean) {
    for (const key of Object.keys(snapshot) as (keyof T)[]) {
      const version = snapshot[key]?.version;
      if (version === undefined) continue;
      const versions = pending.current.get(key);
      versions?.delete(version);
      if (versions?.size === 0) pending.current.delete(key);
    }

    setPatch((current) => {
      let changed = false;
      const next: Patch = {};
      for (const key of Object.keys(current) as (keyof T)[]) {
        const entry = current[key];
        const wasSubmitted = entry?.version === snapshot[key]?.version;
        const matchesSaved = entry && Object.is(entry.value, latestSavedValues.current[key]);
        if ((succeeded && wasSubmitted) || (matchesSaved && !isPending(key))) {
          changed = true;
        } else {
          next[key] = entry;
        }
      }
      return changed ? next : current;
    });
  }

  function discard() {
    setPatch({});
  }

  return { values, setValue, beginSave, finishSave, discard };
}
