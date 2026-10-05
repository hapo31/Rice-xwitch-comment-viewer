import { useEffect, useState } from "react";

/** Keeps only fields the user edited; all other values follow the latest saved settings. */
export function useFormDraft<T extends Record<string, unknown>>(savedValues: T) {
  const [patch, setPatch] = useState<Partial<T>>({});
  const values = { ...savedValues, ...patch };

  useEffect(() => {
    setPatch((current) => {
      let changed = false;
      const next: Partial<T> = {};
      for (const key of Object.keys(current) as (keyof T)[]) {
        if (Object.is(current[key], savedValues[key])) {
          changed = true;
        } else {
          next[key] = current[key];
        }
      }
      return changed ? next : current;
    });
  }, [savedValues]);

  function setValue<K extends keyof T>(key: K, value: T[K]) {
    setPatch((current) => {
      if (Object.is(value, savedValues[key])) {
        if (!(key in current)) return current;
        const next = { ...current };
        delete next[key];
        return next;
      }
      return current[key] === value ? current : { ...current, [key]: value };
    });
  }

  function commit(submittedValues: T) {
    setPatch((current) => {
      let changed = false;
      const next: Partial<T> = {};
      for (const key of Object.keys(current) as (keyof T)[]) {
        if (Object.is(current[key], submittedValues[key])) {
          changed = true;
        } else {
          next[key] = current[key];
        }
      }
      return changed ? next : current;
    });
  }

  function discard() {
    setPatch({});
  }

  return { values, setValue, commit, discard };
}
