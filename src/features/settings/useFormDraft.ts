import { useEffect, useRef, useState } from "react";

/** Keeps only fields the user edited; all other values follow the latest saved settings. */
export function useFormDraft<T extends Record<string, unknown>>(savedValues: T) {
  type Entry<K extends keyof T> = { value: T[K]; version: number };
  type Patch = { [K in keyof T]?: Entry<K> };
  type Snapshot = { [K in keyof T]?: Entry<K> & { baseline: T[K] } };
  const [patch, setPatch] = useState<Patch>({});
  const nextVersion = useRef(0);
  const pending = useRef(new Map<keyof T, Set<number>>());
  const awaitingSaved = useRef(new Map<keyof T, { baseline: unknown }>());
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

  function isProtected(key: keyof T) {
    return isPending(key) || awaitingSaved.current.has(key);
  }

  useEffect(() => {
    setPatch((current) => {
      let changed = false;
      const next: Patch = {};
      for (const key of Object.keys(current) as (keyof T)[]) {
        const entry = current[key];
        const awaiting = awaitingSaved.current.get(key);
        if (awaiting && !Object.is(savedValues[key], awaiting.baseline)) {
          awaitingSaved.current.delete(key);
        }
        if (entry && Object.is(entry.value, savedValues[key]) && !isProtected(key)) {
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
      if (Object.is(value, savedValues[key]) && !isProtected(key)) {
        if (!(key in current)) return current;
        const next = { ...current };
        delete next[key];
        return next;
      }
      if (current[key]?.value === value) return current;
      return { ...current, [key]: { value, version } };
    });
  }

  function beginSave(keys: readonly (keyof T)[] = Object.keys(patch) as (keyof T)[]): Snapshot {
    const snapshot: Snapshot = {};
    for (const key of keys) {
      const entry = patch[key];
      if (entry) snapshot[key] = { ...entry, baseline: savedValues[key] };
    }
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
        if (succeeded && wasSubmitted) {
          changed = true;
        } else {
          next[key] = entry;
        }
      }
      return changed ? next : current;
    });

    if (succeeded) {
      for (const key of Object.keys(snapshot) as (keyof T)[]) {
        const submitted = snapshot[key];
        if (submitted) {
          awaitingSaved.current.set(key, {
            baseline: submitted.baseline,
          });
        }
      }
    }
  }

  function discard() {
    setPatch({});
  }

  return { values, setValue, beginSave, finishSave, discard };
}
