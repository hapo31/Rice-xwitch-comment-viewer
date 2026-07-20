import { useEffect, useState } from "react";
import {
  FloatingSaveButton,
  NumberRuleRow,
  RuleTextArea,
} from "../../components/SettingsFormControls";
import type { AppSettings } from "../../types";
import { formatRuleList, parseRuleList } from "../../validation";
import { defaultSpeechSettings } from "../settings/defaults";

export function FilterView({
  settings,
  onSettingsUpdate,
}: {
  settings?: AppSettings;
  onSettingsUpdate: (patch: Partial<AppSettings>) => void;
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
  const isRepeatSecondsValid = Number.isInteger(numericRepeatSeconds) && numericRepeatSeconds >= 0 && numericRepeatSeconds <= 30;
  const isDirty =
    numericMaxLength !== speechSettings.maxCommentLength ||
    numericRepeatSeconds !== speechSettings.repeatSuppressionSeconds ||
    urlHandling !== speechSettings.urlHandling ||
    !stringArrayEqual(parseRuleList(blockedUsers), speechSettings.blockedUsers) ||
    !stringArrayEqual(parseRuleList(blockedWords), speechSettings.blockedWords);

  function saveFilter() {
    if (!isMaxLengthValid || !isRepeatSecondsValid) {
      return;
    }

    onSettingsUpdate({
      speech: {
        ...speechSettings,
        maxCommentLength: numericMaxLength,
        repeatSuppressionSeconds: numericRepeatSeconds,
        blockedUsers: parseRuleList(blockedUsers),
        blockedWords: parseRuleList(blockedWords),
        urlHandling,
      },
    });
  }

  return (
    <main className="relative col-start-3 row-start-2 min-w-0 overflow-hidden bg-zinc-950">
      <header className="flex h-12 items-center justify-between border-b border-zinc-800 bg-zinc-900 px-4">
        <div className="min-w-0">
          <h1 className="truncate text-sm font-semibold text-zinc-100">Filter</h1>
          <p className="truncate text-xs text-zinc-500">読み上げるチャットの種類と、除外・省略する条件を設定します</p>
        </div>
      </header>

      <div className="h-[calc(100%-3rem)] overflow-auto p-4 pb-20">
        <div className="max-w-3xl space-y-6">
          <section className="border-y border-zinc-800">
            <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
              <label className="text-sm text-zinc-400" htmlFor="rule-url-handling">
                URL
              </label>
              <select
                id="rule-url-handling"
                value={urlHandling}
                onChange={(event) => setUrlHandling(event.target.value as AppSettings["speech"]["urlHandling"])}
                className="h-9 w-52 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 outline-none focus:border-sky-400"
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
              label="連投抑制秒"
              value={repeatSeconds}
              onChange={setRepeatSeconds}
              valid={isRepeatSecondsValid}
              error="0 から 30 の範囲で入力してください。"
            />
          </section>

          <section className="border-y border-zinc-800">
            <RuleTextArea label="NG ユーザー" value={blockedUsers} onChange={setBlockedUsers} />
            <RuleTextArea label="NG ワード" value={blockedWords} onChange={setBlockedWords} />
          </section>
        </div>
      </div>
      <FloatingSaveButton
        visible={isDirty}
        disabled={!isMaxLengthValid || !isRepeatSecondsValid}
        onClick={saveFilter}
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
