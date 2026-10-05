import { Controller } from "react-hook-form";
import {
  NumberRuleRow,
  RuleTextArea,
  SettingsSection,
} from "../../components/SettingsFormControls";
import { focusIndicatorClass } from "../../presentation/focus";
import { blockedRulesError, parseBlockedUserList, parseBlockedWordList } from "../../validation";
import { useFormDraftContext } from "../settings/useFormDraft";
import type { FilterDraft } from "../settings/formModels";

export function FilterConditionsSection({
  maxLengthValid,
  repeatSecondsValid,
}: {
  maxLengthValid: boolean;
  repeatSecondsValid: boolean;
}) {
  const { control, getValues } = useFormDraftContext<FilterDraft>();
  return (
    <SettingsSection id="speech-rules" title="読み上げ条件">
      <Controller
        control={control}
        name="urlHandling"
        render={({ field }) => (
          <div className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3">
            <label className="text-sm text-zinc-400" htmlFor="rule-url-handling">
              URL
            </label>
            <select
              id="rule-url-handling"
              value={getValues("urlHandling")}
              onChange={field.onChange}
              className={`h-9 w-52 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
            >
              <option value="replace">URL省略</option>
              <option value="read">そのまま読む</option>
              <option value="block">読み上げない</option>
            </select>
          </div>
        )}
      />
      <Controller
        control={control}
        name="maxLength"
        render={({ field }) => (
          <NumberRuleRow
            id="rule-max-length"
            label="最大文字数"
            value={getValues("maxLength")}
            onChange={field.onChange}
            valid={maxLengthValid}
            error="1 から 500 の範囲で入力してください。"
          />
        )}
      />
      <Controller
        control={control}
        name="repeatSeconds"
        render={({ field }) => (
          <NumberRuleRow
            id="rule-repeat-seconds"
            label="連投抑制秒（0は無効、1〜30秒は指定間隔）"
            value={getValues("repeatSeconds")}
            onChange={field.onChange}
            valid={repeatSecondsValid}
            error="0（無効）または 1 から 30 の範囲で入力してください。"
          />
        )}
      />
    </SettingsSection>
  );
}

export function BlockedRulesSection() {
  const { control, getValues } = useFormDraftContext<FilterDraft>();
  const blockedUsers = getValues("blockedUsers");
  const blockedWords = getValues("blockedWords");
  const blockedUserRules = parseBlockedUserList(blockedUsers);
  const blockedWordRules = parseBlockedWordList(blockedWords);
  const ruleError = blockedRulesError(blockedUserRules.items, blockedWordRules.items);
  return (
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
      <Controller
        control={control}
        name="blockedUsers"
        render={({ field }) => (
          <RuleTextArea
            id="rule-blocked-users"
            label="NG ユーザー"
            value={getValues("blockedUsers")}
            onChange={field.onChange}
            itemCount={blockedUserRules.items.length}
            overflowCount={blockedUserRules.overflowCount}
          />
        )}
      />
      <Controller
        control={control}
        name="blockedWords"
        render={({ field }) => (
          <RuleTextArea
            id="rule-blocked-words"
            label="NG ワード"
            value={getValues("blockedWords")}
            onChange={field.onChange}
            itemCount={blockedWordRules.items.length}
            overflowCount={blockedWordRules.overflowCount}
          />
        )}
      />
    </SettingsSection>
  );
}

export function validateFilterDraft(values: FilterDraft) {
  const maxLength = Number(values.maxLength);
  const repeatSeconds = Number(values.repeatSeconds);
  const blockedUsers = parseBlockedUserList(values.blockedUsers);
  const blockedWords = parseBlockedWordList(values.blockedWords);
  const ruleError = blockedRulesError(blockedUsers.items, blockedWords.items);
  return {
    maxLengthValid: Number.isInteger(maxLength) && maxLength >= 1 && maxLength <= 500,
    repeatSecondsValid:
      Number.isInteger(repeatSeconds) && repeatSeconds >= 0 && repeatSeconds <= 30,
    rulesValid: blockedUsers.overflowCount === 0 && blockedWords.overflowCount === 0 && !ruleError,
    blockedUsers,
    blockedWords,
  };
}
