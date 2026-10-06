import { createElement, useEffect, useRef, useState, type PropsWithChildren } from "react";
import {
  type FieldPath,
  type FieldPathValue,
  type FieldValues,
  type DefaultValues,
  FormProvider,
  type FormProviderProps,
  type UseFormReturn,
  useForm,
  useFormContext,
} from "react-hook-form";

export type FormDraft<T extends FieldValues> = UseFormReturn<T, unknown, T> & {
  discard: () => void;
  beginSave: (fields?: FieldPath<T>[]) => DraftSnapshot<T>;
  finishSave: (snapshot: DraftSnapshot<T>, succeeded: boolean) => void;
  isSaving: boolean;
};

export type DraftSnapshot<T extends FieldValues> = {
  id: number;
  values: T;
  savedValues: T;
  fields: FieldPath<T>[];
};

export function FormDraftProvider<T extends FieldValues>({
  form,
  children,
}: PropsWithChildren<{ form: FormDraft<T> }>) {
  const Provider = FormProvider as unknown as React.ComponentType<FormProviderProps<T, unknown, T>>;
  return createElement(Provider, { ...form, children });
}

export function useFormDraftContext<T extends FieldValues>(): FormDraft<T> {
  return useFormContext<T>() as FormDraft<T>;
}

/** Keeps edited fields locally while refreshing pristine fields from saved settings. */
export function useFormDraft<T extends FieldValues>(savedValues: T): FormDraft<T> {
  const form = useForm<T, unknown, T>({
    defaultValues: savedValues as DefaultValues<T>,
    mode: "onChange",
  });
  const previousSaved = useRef(savedValues);
  const pendingFields = useRef(new Map<FieldPath<T>, number>());
  const awaitingSaved = useRef(new Map<number, Map<FieldPath<T>, T[keyof T]>>());
  const nextSnapshotId = useRef(0);
  const [pendingCount, setPendingCount] = useState(0);
  const [, setSyncRevision] = useState(0);
  const { dirtyFields } = form.formState;

  useEffect(() => {
    const previous = previousSaved.current;
    let completedSaves = 0;
    for (const name of Object.keys(savedValues) as FieldPath<T>[]) {
      const nextValue = savedValues[name as keyof T];
      const previousValue = previous[name as keyof T];
      if (sameValue(previousValue, nextValue)) continue;

      let acknowledgedSave = false;
      for (const [snapshotId, fields] of awaitingSaved.current) {
        const baseline = fields.get(name);
        if (baseline === undefined && !fields.has(name)) continue;
        if (sameValue(baseline, nextValue)) continue;
        fields.delete(name);
        acknowledgedSave = true;
        if (fields.size === 0) {
          awaitingSaved.current.delete(snapshotId);
          completedSaves += 1;
        }
      }

      if (acknowledgedSave) {
        // The draft may equal its old baseline after a post-submit edit, so
        // preserve the value explicitly while rebasing to the acknowledged save.
        rebaseField(name, nextValue, form.getValues(name));
        continue;
      }
      if (pendingFields.current.has(name)) continue;

      const currentValue = form.getValues(name);
      const wasDirty = form.getFieldState(name).isDirty;
      rebaseField(name, nextValue, wasDirty ? currentValue : nextValue);
    }
    previousSaved.current = savedValues;
    if (completedSaves > 0) {
      setPendingCount((count) => Math.max(0, count - completedSaves));
    }
    // `dirtyFields` is read to subscribe this synchronization boundary to RHF's field state.
    void dirtyFields;
  }, [dirtyFields, form, savedValues]);

  function discard() {
    form.reset(previousSaved.current);
    pendingFields.current.clear();
    awaitingSaved.current.clear();
    setPendingCount(0);
  }

  function beginSave(savedFields?: FieldPath<T>[]): DraftSnapshot<T> {
    const fields = savedFields ?? (Object.keys(savedValues) as FieldPath<T>[]);
    const savedSnapshot = { ...savedValues };
    const id = nextSnapshotId.current;
    nextSnapshotId.current += 1;
    for (const field of fields) {
      pendingFields.current.set(field, (pendingFields.current.get(field) ?? 0) + 1);
    }
    setPendingCount((count) => count + 1);
    return { id, values: form.getValues(), savedValues: savedSnapshot, fields };
  }

  function finishSave(snapshot: DraftSnapshot<T>, succeeded: boolean) {
    for (const field of snapshot.fields) {
      const count = (pendingFields.current.get(field) ?? 1) - 1;
      if (count === 0) pendingFields.current.delete(field);
      else pendingFields.current.set(field, count);
    }
    const awaitingFields = new Map<FieldPath<T>, T[keyof T]>();
    for (const field of snapshot.fields) {
      const saved = previousSaved.current[field as keyof T];
      const baseline = snapshot.savedValues[field as keyof T];
      const current = form.getValues(field);
      const submitted = snapshot.values[field as keyof T];
      if (succeeded && sameValue(saved, baseline) && !sameValue(submitted, baseline)) {
        awaitingFields.set(field, baseline);
        continue;
      }
      rebaseField(field, saved, current);
    }
    if (awaitingFields.size > 0) {
      awaitingSaved.current.set(snapshot.id, awaitingFields);
    } else {
      setPendingCount((count) => Math.max(0, count - 1));
    }
  }

  function rebaseField(name: FieldPath<T>, defaultValue: T[keyof T], value = form.getValues(name)) {
    // resetField updates RHF's stored value but its notification only contains
    // field state. Publish through setValue first so useWatch/Controller sees
    // the incoming baseline; resetField then records that value as pristine.
    form.setValue(name, defaultValue as FieldPathValue<T, typeof name>, {
      shouldDirty: false,
      shouldValidate: true,
    });
    form.resetField(name, { defaultValue: defaultValue as FieldPathValue<T, typeof name> });
    if (!sameValue(value, defaultValue)) {
      form.setValue(name, value, { shouldDirty: true, shouldValidate: true });
    }
    setSyncRevision((revision) => revision + 1);
  }

  return Object.assign(form, {
    discard,
    beginSave,
    finishSave,
    isSaving: pendingCount > 0,
  });
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
