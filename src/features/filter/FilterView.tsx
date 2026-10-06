import { useWatch } from "react-hook-form";
import { FloatingSaveButton } from "../../components/SettingsFormControls";
import type { AppSettings, AppSettingsPatch } from "../../types";
import { routeHeadingId } from "../../routeAccessibility";
import { useUnsavedChanges } from "../../unsavedChanges";
import { isValidRepeatSuppressionSeconds } from "../../validation";
import { defaultSpeechSettings } from "../settings/defaults";
import {
  createFilterDraft,
  createFilterSpeechPatch,
  filterFieldsForPatch,
  hasSpeechPatch,
} from "../settings/formModels";
import { FormDraftProvider, useFormDraft } from "../settings/useFormDraft";
import {
  BlockedRulesSection,
  FilterConditionsSection,
  validateFilterDraft,
} from "./FilterFormSections";

export function FilterView({
  settings,
  onSettingsUpdate,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
}) {
  const speechSettings = { ...defaultSpeechSettings(), ...settings?.speech };
  const form = useFormDraft(createFilterDraft(speechSettings));
  useWatch({ control: form.control });
  const draft = form.getValues();
  const patch = createFilterSpeechPatch(draft, speechSettings);
  const isDirty = hasSpeechPatch(patch) || form.isSaving;
  const validation = validateFilterDraft(draft);
  const isValid =
    validation.maxLengthValid && validation.repeatSecondsValid && validation.rulesValid;

  async function saveFilter(): Promise<boolean> {
    const currentDraft = form.getValues();
    const currentValidation = validateFilterDraft(currentDraft);
    const currentPatch = createFilterSpeechPatch(currentDraft, speechSettings);
    const currentIsValid =
      currentValidation.maxLengthValid &&
      currentValidation.repeatSecondsValid &&
      currentValidation.rulesValid;
    if (!currentIsValid || !hasSpeechPatch(currentPatch)) return false;
    const snapshot = form.beginSave(filterFieldsForPatch(currentPatch));
    let succeeded = false;
    try {
      succeeded = await onSettingsUpdate({ speech: currentPatch });
      return succeeded;
    } finally {
      form.finishSave(snapshot, succeeded);
    }
  }

  useUnsavedChanges("filter", { isDirty, save: saveFilter, discard: form.discard });

  return (
    <FormDraftProvider form={form}>
      <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
        <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
          <div className="min-w-0">
            <h1
              id={routeHeadingId}
              tabIndex={-1}
              className="truncate text-sm font-semibold text-zinc-100"
            >
              Filter
            </h1>
            <p className="truncate text-xs text-zinc-400">
              読み上げるチャットの種類と、除外・省略する条件を設定します
            </p>
          </div>
        </header>

        <div className="h-[calc(100%-3rem)] overflow-auto p-4 pb-20">
          <div className="max-w-3xl space-y-6">
            <FilterConditionsSection
              maxLengthValid={validation.maxLengthValid}
              repeatSecondsValid={validation.repeatSecondsValid}
            />
            <BlockedRulesSection />
          </div>
        </div>
        <FloatingSaveButton
          visible={isDirty}
          disabled={!isValid}
          onClick={() => void saveFilter()}
        />
      </main>
    </FormDraftProvider>
  );
}
