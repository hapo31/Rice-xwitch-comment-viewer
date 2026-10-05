import { useState } from "react";
import {
  FloatingSaveButton,
  NumberRuleRow,
  RuleTextArea,
  SettingsSection,
} from "../../components/SettingsFormControls";
import type { AppSettings, AppSettingsPatch } from "../../types";
import { focusIndicatorClass } from "../../presentation/focus";
import { routeHeadingId } from "../../routeAccessibility";
import {
  blockedRulesError,
  formatRuleList,
  isValidRepeatSuppressionSeconds,
  parseBlockedUserList,
  parseBlockedWordList,
} from "../../validation";
import { defaultSpeechSettings } from "../settings/defaults";
import { useUnsavedChanges } from "../../unsavedChanges";
import { useFormDraft } from "../settings/useFormDraft";

export function FilterView({
  settings,
  onSettingsUpdate,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: AppSettingsPatch) => Promise<boolean>;
}) {
  const speechSettings = {
    ...defaultSpeechSettings,
    ...settings?.speech,
  };
  const form = useFormDraft({
    blockedUsers: formatRuleList(speechSettings.blockedUsers),
    blockedWords: formatRuleList(speechSettings.blockedWords),
    urlHandling: speechSettings.urlHandling,
    maxLength: String(speechSettings.maxCommentLength),
    repeatSeconds: String(speechSettings.repeatSuppressionSeconds),
  });
  const { blockedUsers, blockedWords, urlHandling, maxLength, repeatSeconds } = form.values;

  const numericMaxLength = Number(maxLength);
  const numericRepeatSeconds = Number(repeatSeconds);
  const isMaxLengthValid =
    Number.isInteger(numericMaxLength) && numericMaxLength >= 1 && numericMaxLength <= 500;
  const isRepeatSecondsValid = isValidRepeatSuppressionSeconds(repeatSeconds);
  const blockedUserRules = parseBlockedUserList(blockedUsers);
  const blockedWordRules = parseBlockedWordList(blockedWords);
  const ruleError = blockedRulesError(blockedUserRules.items, blockedWordRules.items);
  const areRuleListsValid =
    blockedUserRules.overflowCount === 0 && blockedWordRules.overflowCount === 0 && !ruleError;
  const isDirty =
    numericMaxLength !== speechSettings.maxCommentLength ||
    numericRepeatSeconds !== speechSettings.repeatSuppressionSeconds ||
    urlHandling !== speechSettings.urlHandling ||
    !stringArrayEqual(blockedUserRules.items, speechSettings.blockedUsers) ||
    !stringArrayEqual(blockedWordRules.items, speechSettings.blockedWords);

  async function saveFilter(): Promise<boolean> {
    if (!isMaxLengthValid || !isRepeatSecondsValid || !areRuleListsValid) {
      return false;
    }

    const speech: NonNullable<AppSettingsPatch["speech"]> = {};
    const submittedKeys: (keyof typeof form.values)[] = [];
    if (numericMaxLength !== speechSettings.maxCommentLength) {
      speech.maxCommentLength = numericMaxLength;
      submittedKeys.push("maxLength");
    }
    if (numericRepeatSeconds !== speechSettings.repeatSuppressionSeconds) {
      speech.repeatSuppressionSeconds = numericRepeatSeconds;
      submittedKeys.push("repeatSeconds");
    }
    if (!stringArrayEqual(blockedUserRules.items, speechSettings.blockedUsers)) {
      speech.blockedUsers = blockedUserRules.items;
      submittedKeys.push("blockedUsers");
    }
    if (!stringArrayEqual(blockedWordRules.items, speechSettings.blockedWords)) {
      speech.blockedWords = blockedWordRules.items;
      submittedKeys.push("blockedWords");
    }
    if (urlHandling !== speechSettings.urlHandling) {
      speech.urlHandling = urlHandling;
      submittedKeys.push("urlHandling");
    }
    const snapshot = form.beginSave(submittedKeys);
    let succeeded = false;
    try {
      succeeded = await onSettingsUpdate({ speech });
      return succeeded;
    } finally {
      form.finishSave(snapshot, succeeded);
    }
  }

  function discardFilter() {
    form.discard();
  }

  useUnsavedChanges("filter", { isDirty, save: saveFilter, discard: discardFilter });

  return (
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
          <SettingsSection id="speech-rules" title="読み上げ条件">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="rule-url-handling">
                URL
              </label>
              <select
                id="rule-url-handling"
                value={urlHandling}
                onChange={(event) =>
                  form.setValue(
                    "urlHandling",
                    event.target.value as AppSettings["speech"]["urlHandling"],
                  )
                }
                className={`h-9 w-52 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
              >
                <option value="replace">URL省略</option>
                <option value="read">そのまま読む</option>
                <option value="block">読み上げない</option>
              </select>
            </div>
            <NumberRuleRow
              id="rule-max-length"
              label="最大文字数"
              value={maxLength}
              onChange={(value) => form.setValue("maxLength", value)}
              valid={isMaxLengthValid}
              error="1 から 500 の範囲で入力してください。"
            />
            <NumberRuleRow
              id="rule-repeat-seconds"
              label="連投抑制秒（0は無効、1〜30秒は指定間隔）"
              value={repeatSeconds}
              onChange={(value) => form.setValue("repeatSeconds", value)}
              valid={isRepeatSecondsValid}
              error="0（無効）または 1 から 30 の範囲で入力してください。"
            />
          </SettingsSection>

          <SettingsSection id="blocked-rules" title="除外リスト">
            <p className="py-2 text-xs text-zinc-400">
              各200件まで。NGユーザーはTwitch login、NGワードは500文字まで、両リスト合計64KiB
              UTF-8以内です。
            </p>
            {ruleError && (
              <p role="alert" className="py-2 text-xs text-zinc-400">
                {ruleError}
              </p>
            )}
            <RuleTextArea
              id="rule-blocked-users"
              label="NG ユーザー"
              value={blockedUsers}
              onChange={(value) => form.setValue("blockedUsers", value)}
              itemCount={blockedUserRules.items.length}
              overflowCount={blockedUserRules.overflowCount}
            />
            <RuleTextArea
              id="rule-blocked-words"
              label="NG ワード"
              value={blockedWords}
              onChange={(value) => form.setValue("blockedWords", value)}
              itemCount={blockedWordRules.items.length}
              overflowCount={blockedWordRules.overflowCount}
            />
          </SettingsSection>
        </div>
      </div>
      <FloatingSaveButton
        visible={isDirty}
        disabled={!isMaxLengthValid || !isRepeatSecondsValid || !areRuleListsValid}
        onClick={() => void saveFilter()}
      />
    </main>
  );
}

function stringArrayEqual(left: string[], right: string[]): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((value, index) => value === right[index]);
}
