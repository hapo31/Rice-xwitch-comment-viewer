import { useEffect, useState } from "react";
import {
  FloatingSaveButton,
  NumberRuleRow,
  RuleTextArea,
  SettingsSection,
} from "../../components/SettingsFormControls";
import type { AppSettings } from "../../types";
import { focusIndicatorClass } from "../../presentation/focus";
import { routeHeadingId } from "../../routeAccessibility";
import {
  formatRuleList,
  isValidRepeatSuppressionSeconds,
  parseBlockedUserList,
  parseBlockedWordList,
} from "../../validation";
import { defaultSpeechSettings } from "../settings/defaults";
import { useUnsavedChanges } from "../../unsavedChanges";

export function FilterView({
  settings,
  onSettingsUpdate,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: Partial<AppSettings>) => Promise<boolean>;
}) {
  const speechSettings = {
    ...defaultSpeechSettings,
    ...settings?.speech,
  };
  const [blockedUsers, setBlockedUsers] = useState(formatRuleList(speechSettings.blockedUsers));
  const [blockedWords, setBlockedWords] = useState(formatRuleList(speechSettings.blockedWords));
  const [urlHandling, setUrlHandling] = useState(speechSettings.urlHandling);
  const [maxLength, setMaxLength] = useState(String(speechSettings.maxCommentLength));
  const [repeatSeconds, setRepeatSeconds] = useState(String(speechSettings.repeatSuppressionSeconds));

  useEffect(() => {
    setBlockedUsers(formatRuleList(speechSettings.blockedUsers));
    setBlockedWords(formatRuleList(speechSettings.blockedWords));
    setUrlHandling(speechSettings.urlHandling);
    setMaxLength(String(speechSettings.maxCommentLength));
    setRepeatSeconds(String(speechSettings.repeatSuppressionSeconds));
  }, [
    speechSettings.blockedUsers,
    speechSettings.blockedWords,
    speechSettings.urlHandling,
    speechSettings.maxCommentLength,
    speechSettings.repeatSuppressionSeconds,
  ]);

  const numericMaxLength = Number(maxLength);
  const numericRepeatSeconds = Number(repeatSeconds);
  const isMaxLengthValid = Number.isInteger(numericMaxLength) && numericMaxLength >= 1 && numericMaxLength <= 500;
  const isRepeatSecondsValid = isValidRepeatSuppressionSeconds(repeatSeconds);
  const blockedUserRules = parseBlockedUserList(blockedUsers);
  const blockedWordRules = parseBlockedWordList(blockedWords);
  const areRuleListsValid = blockedUserRules.overflowCount === 0 && blockedWordRules.overflowCount === 0;
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

    return onSettingsUpdate({
      speech: {
        ...speechSettings,
        maxCommentLength: numericMaxLength,
        repeatSuppressionSeconds: numericRepeatSeconds,
        blockedUsers: blockedUserRules.items,
        blockedWords: blockedWordRules.items,
        urlHandling,
      },
    });
  }

  function discardFilter() {
    setBlockedUsers(formatRuleList(speechSettings.blockedUsers));
    setBlockedWords(formatRuleList(speechSettings.blockedWords));
    setUrlHandling(speechSettings.urlHandling);
    setMaxLength(String(speechSettings.maxCommentLength));
    setRepeatSeconds(String(speechSettings.repeatSuppressionSeconds));
  }

  useUnsavedChanges("filter", { isDirty, save: saveFilter, discard: discardFilter });

  return (
    <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 id={routeHeadingId} tabIndex={-1} className="truncate text-sm font-semibold text-zinc-100">Filter</h1>
          <p className="truncate text-xs text-zinc-400">読み上げるチャットの種類と、除外・省略する条件を設定します</p>
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
                onChange={(event) => setUrlHandling(event.target.value as AppSettings["speech"]["urlHandling"])}
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
              onChange={setMaxLength}
              valid={isMaxLengthValid}
              error="1 から 500 の範囲で入力してください。"
            />
            <NumberRuleRow
              id="rule-repeat-seconds"
              label="連投抑制秒（0は無効、1〜30秒は指定間隔）"
              value={repeatSeconds}
              onChange={setRepeatSeconds}
              valid={isRepeatSecondsValid}
              error="0（無効）または 1 から 30 の範囲で入力してください。"
            />
          </SettingsSection>

          <SettingsSection id="blocked-rules" title="除外リスト">
            <RuleTextArea
              id="rule-blocked-users"
              label="NG ユーザー"
              value={blockedUsers}
              onChange={setBlockedUsers}
              itemCount={blockedUserRules.items.length}
              overflowCount={blockedUserRules.overflowCount}
            />
            <RuleTextArea
              id="rule-blocked-words"
              label="NG ワード"
              value={blockedWords}
              onChange={setBlockedWords}
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
