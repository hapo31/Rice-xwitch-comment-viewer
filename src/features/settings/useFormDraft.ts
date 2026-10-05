import { useEffect, useRef, useState } from "react";
import {
  type FieldPath,
  type FieldPathValue,
  type FieldValues,
  type DefaultValues,
  type UseFormReturn,
  useForm,
} from "react-hook-form";

export type FormDraft<T extends FieldValues> = UseFormReturn<T, unknown, T> & {
  discard: () => void;
  beginSave: () => DraftSnapshot<T>;
  finishSave: (snapshot: DraftSnapshot<T>, succeeded: boolean) => void;
  isSaving: boolean;
};

export type DraftSnapshot<T extends FieldValues> = {
  values: T;
  savedValues: T;
  fields: FieldPath<T>[];
};

/** Keeps edited fields locally while refreshing pristine fields from saved settings. */
export function useFormDraft<T extends FieldValues>(savedValues: T): FormDraft<T> {
  const form = useForm<T, unknown, T>({
    defaultValues: savedValues as DefaultValues<T>,
    mode: "onChange",
  });
  const previousSaved = useRef(savedValues);
  const pendingFields = useRef(new Map<FieldPath<T>, number>());
  const awaitingSaved = useRef(new Map<FieldPath<T>, T[keyof T]>());
  const [pendingCount, setPendingCount] = useState(0);
  const { dirtyFields } = form.formState;

  useEffect(() => {
    const previous = previousSaved.current;
    for (const name of Object.keys(savedValues) as FieldPath<T>[]) {
      const nextValue = savedValues[name as keyof T];
      const previousValue = previous[name as keyof T];
      if (sameValue(previousValue, nextValue)) continue;

      if (pendingFields.current.has(name)) continue;
      const awaitingBaseline = awaitingSaved.current.get(name);
      if (awaitingBaseline !== undefined || awaitingSaved.current.has(name)) {
        if (sameValue(awaitingBaseline, nextValue)) continue;
        awaitingSaved.current.delete(name);
        rebaseField(name, nextValue);
        if (awaitingSaved.current.size === 0) {
          setPendingCount((count) => Math.max(0, count - 1));
        }
        continue;
      }

      const currentValue = form.getValues(name);
      const wasDirty = form.getFieldState(name).isDirty;
      if (wasDirty) rebaseField(name, nextValue, currentValue);
      else rebaseField(name, nextValue);
    }
    previousSaved.current = savedValues;
    // `dirtyFields` is read to subscribe this synchronization boundary to RHF's field state.
    void dirtyFields;
  }, [dirtyFields, form, savedValues]);

  function discard() {
    form.reset(previousSaved.current);
    pendingFields.current.clear();
    awaitingSaved.current.clear();
    setPendingCount(0);
  }

  function beginSave(): DraftSnapshot<T> {
    const fields = Object.keys(savedValues) as FieldPath<T>[];
    const savedSnapshot = { ...savedValues };
    for (const field of fields) {
      pendingFields.current.set(field, (pendingFields.current.get(field) ?? 0) + 1);
    }
    setPendingCount((count) => count + 1);
    return { values: form.getValues(), savedValues: savedSnapshot, fields };
  }

  function finishSave(snapshot: DraftSnapshot<T>, succeeded: boolean) {
    for (const field of snapshot.fields) {
      const count = (pendingFields.current.get(field) ?? 1) - 1;
      if (count === 0) pendingFields.current.delete(field);
      else pendingFields.current.set(field, count);
    }
    let isAwaiting = false;
    for (const field of snapshot.fields) {
      const saved = previousSaved.current[field as keyof T];
      const baseline = snapshot.savedValues[field as keyof T];
      const current = form.getValues(field);
      if (succeeded && sameValue(saved, baseline)) {
        awaitingSaved.current.set(field, baseline);
        isAwaiting = true;
        continue;
      }
      if (succeeded && sameValue(saved, snapshot.values[field as keyof T])) {
        awaitingSaved.current.delete(field);
      }
      rebaseField(field, saved, current);
    }
    if (!isAwaiting) setPendingCount((count) => Math.max(0, count - 1));
  }

  function rebaseField(name: FieldPath<T>, defaultValue: T[keyof T], value = form.getValues(name)) {
    form.resetField(name, { defaultValue: defaultValue as FieldPathValue<T, typeof name> });
    if (!sameValue(value, defaultValue)) {
      form.setValue(name, value, { shouldDirty: true, shouldValidate: true });
    }
  }

  return Object.assign(form, { discard, beginSave, finishSave, isSaving: pendingCount > 0 });
}

function sameValue(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  return (
    Array.isArray(left) &&
    Array.isArray(right) &&
    left.length === right.length &&
    left.every((value, index) => Object.is(value, right[index]))
  );
}
