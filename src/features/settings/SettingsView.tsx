import { useWatch } from "react-hook-form";
import type { AppSettings, AppSettingsPatch, BouyomiConnectionDiagnostics } from "../../types";
import { FloatingSaveButton } from "../../components/SettingsFormControls";
import { routeHeadingId } from "../../routeAccessibility";
import { useUnsavedChanges } from "../../unsavedChanges";
import {
  isValidBouyomiHost,
  isValidBouyomiVoice,
  isValidConfirmationText,
  isValidPort,
} from "../../validation";
import { defaultSpeechSettings, defaultTwitchSettings } from "./defaults";
import {
  createSettingsDraft,
  createSettingsSpeechPatch,
  hasSpeechPatch,
  settingsFieldsForPatch,
} from "./formModels";
import {
  AutomaticSpeechSection,
  ChatReceptionSection,
  ConnectionDiagnosticsSection,
  ConnectionSuccessSpeechSection,
  SettingsActionsProvider,
  SpeechConnectionSection,
  SpeechHealthCheckButton,
  SpeechTestSection,
  VoiceSettingsSection,
} from "./SettingsFormSections";
import { FormDraftProvider, useFormDraft } from "./useFormDraft";

export function SettingsView({
  settings,
  onSettingsUpdate,
  onSpeechHealthCheck,
  onSpeechDiagnostics,
  onSpeechTest,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
  onSpeechHealthCheck: () => void;
  onSpeechDiagnostics: () => Promise<BouyomiConnectionDiagnostics>;
  onSpeechTest: (text?: string) => void;
}) {
  const twitchSettings = { ...defaultTwitchSettings(), ...settings?.twitch };
  const speechSettings = { ...defaultSpeechSettings(), ...settings?.speech };
  const form = useFormDraft(createSettingsDraft(speechSettings));
  useWatch({ control: form.control });
  const draft = form.getValues();
  const patch = createSettingsSpeechPatch(draft, speechSettings);
  const isDirty = hasSpeechPatch(patch) || form.isSaving;
  const isPortValid = isValidPort(draft.port);
  const isVoiceValid = isValidBouyomiVoice(draft.voice);
  const isHostValid = isValidBouyomiHost(draft.host);
  const isConfirmationValid = isValidConfirmationText(draft.connectionSuccessSpeechText);
  const saveDisabledReason = [
    !isHostValid
      ? draft.host.trim().length === 0
        ? "棒読みちゃんのホストを入力してください。"
        : "IPv4、DNS名、または角括弧なしのIPv6アドレスを入力してください。"
      : undefined,
    !isPortValid ? "棒読みちゃんのポートは 1 から 65535 の範囲で入力してください。" : undefined,
    !isVoiceValid ? "棒読みちゃんの声質は 0 から 30000 の範囲で入力してください。" : undefined,
    !isConfirmationValid
      ? "接続時メッセージは制御文字を含まない120文字・480 UTF-8バイト以内にしてください。"
      : undefined,
  ]
    .filter((message): message is string => Boolean(message))
    .join(" ");

  async function saveSettings(): Promise<boolean> {
    const currentDraft = form.getValues();
    const currentPatch = createSettingsSpeechPatch(currentDraft, speechSettings);
    const currentValuesValid =
      isValidBouyomiHost(currentDraft.host) &&
      isValidPort(currentDraft.port) &&
      isValidBouyomiVoice(currentDraft.voice) &&
      isValidConfirmationText(currentDraft.connectionSuccessSpeechText);
    if (!currentValuesValid || !hasSpeechPatch(currentPatch)) {
      return false;
    }
    const snapshot = form.beginSave(settingsFieldsForPatch(currentPatch));
    let succeeded = false;
    try {
      succeeded = await onSettingsUpdate({ speech: currentPatch });
      return succeeded;
    } finally {
      form.finishSave(snapshot, succeeded);
    }
  }

  function discardSettings() {
    form.discard();
  }

  useUnsavedChanges("settings", { isDirty, save: saveSettings, discard: discardSettings });

  return (
    <FormDraftProvider form={form}>
      <SettingsActionsProvider
        value={{
          twitch: twitchSettings,
          savedSpeech: speechSettings,
          isDirty,
          onSettingsUpdate,
          onSpeechHealthCheck,
          onSpeechDiagnostics,
          onSpeechTest,
        }}
      >
        <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
          <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
            <div className="min-w-0">
              <h1
                id={routeHeadingId}
                tabIndex={-1}
                className="truncate text-sm font-semibold text-zinc-100"
              >
                Settings
              </h1>
              <p className="truncate text-xs text-zinc-400">
                起動時接続、棒読みちゃん接続、声質、自動読み上げの設定を調整します
              </p>
            </div>
            <div className="flex items-center gap-2">
              <SpeechHealthCheckButton />
            </div>
          </header>

          <div className="h-[calc(100%-3rem)] overflow-auto p-4 pb-20">
            <div className="max-w-3xl space-y-6">
              <ChatReceptionSection />
              <ConnectionDiagnosticsSection />
              <AutomaticSpeechSection />
              <SpeechConnectionSection />
              <VoiceSettingsSection />
              <ConnectionSuccessSpeechSection />
              <SpeechTestSection />
            </div>
          </div>
          <FloatingSaveButton
            visible={isDirty}
            disabled={!isHostValid || !isPortValid || !isVoiceValid || !isConfirmationValid}
            disabledReason={saveDisabledReason}
            onClick={() => void saveSettings()}
          />
        </main>
      </SettingsActionsProvider>
    </FormDraftProvider>
  );
}
