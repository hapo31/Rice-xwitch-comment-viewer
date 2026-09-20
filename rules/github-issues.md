# GitHub Issue 対応

GitHub Issue の選定、修正、または PR 作成を行う前に、このルールを読みます。修正を伴う場合は [`fixing.md`](fixing.md)、サブエージェントを使う場合は [`subagent.md`](subagent.md) も読みます。

## 事前確認

1. `AGENTS.md`、`docs/TODO.md`、Issue に関係する設計文書を読む。
2. Git remote、既定ブランチ、現在の worktree と変更状態を確認する。既存のユーザー変更は移動、破棄、stash しない。
3. 利用可能なら GitHub connector を優先し、Issue 本文、ラベル、コメント、完了条件と、同じ Issue を扱う既存 PR を一次情報から確認する。
4. ユーザーが指定した Issue、件数、並列数、重要度、ラベル、対象範囲を優先する。

## Issue の選定

ユーザーが対象を指定していない場合は、次の順で候補を評価します。

1. `severity:critical`、`severity:high`、`severity:medium` の順で重要度が高いもの。
2. bug、security、correctness、reliability、privacy に関係するもの。
3. 完了条件が明確で、変更範囲と検証方法が限定されるもの。
4. 外部サービス、実機、秘密情報、破壊的操作を必要とせず、自動テストで検証できるもの。
5. 同時に扱う他の Issue と編集ファイルや設計責務が重なりにくいもの。

既存 PR で対応中、別 Issue に依存、要件判断が未確定、手動確認だけで完了する Issue は除外または後順位にします。選定理由をユーザーへ短く報告してから修正を始めます。

## 修正単位

- 一つの Issue だけを、一つの独立 worktree、ブランチ、commit、PR で扱う。複数 Issue の変更を混ぜない。
- branch は `agent/issue-<NUMBER>-<SHORT-SLUG>`、worktree は `/tmp/<REPOSITORY_SLUG>-issue-<NUMBER>` または衝突しない安全な一時パスを基本とする。
- 同じ Issue の branch、worktree、PR が存在する場合は新規作成せず、状態を確認して再開または除外する。
- `docs/TODO.md` に対応項目がなければ、実装前に未完了項目を追加する。完了時はチェック、進捗サマリ、必要な調査メモを実際の状態へ更新する。
- Issue の範囲を広げず、完了条件を満たす最小で保守可能な修正と自動テストを実装する。
- secret、ライセンス不明コード、生成物、依存キャッシュを追加しない。

## 並列対応

- 並列数が未指定の場合は最大 3 とする。実際の同時実行数は、指定値、選定 Issue 数、利用可能なサブエージェント枠の最小値にし、親エージェント用の枠を一つ残す。
- 上限を超える Issue は同じ並列数で次の wave に送る。
- 各サブエージェントへ渡すタスク固有情報は repository、Issue 番号、base branch、worktree、branch だけとし、Issue の内容、親の診断、原因推測、修正案を追加しない。Issue と完了条件は担当自身に取得させる。
- 実装担当のモデルにユーザー指定がなければ `gpt-5.6-terra` を使い、これより上位のモデルは使わない。思考強度はユーザー指定を優先し、未指定なら局所的な文書／設定／UI 修正を `low`、単一レイヤのコード修正を `medium`、複数レイヤまたは concurrency／security 修正を `high` の目安とする。
- task name は `luna_issue_<NUMBER>` とする。
- サブエージェントは `fork_turns: "none"` で起動する。同じ wave の担当をすべて起動してから親エージェントの作業を続ける。
- サブエージェントは commit までで停止し、push、PR 作成、Issue close、merge、worktree や branch の削除は親エージェントが行う。
- サブエージェントの報告形式は [`subagent.md`](subagent.md) に従う。親エージェントは差分、commit、検証結果を自分で確認する。

## レビュー

各 Issue について次を確認します。

1. worktree とブランチが Issue 専用で、commit 後に clean である。
2. Issue の完了条件を差分とテストが満たす。
3. `AGENTS.md`、設計文書、`docs/TODO.md`、調査メモが実装と一致する。
4. unrelated change、生成物、依存キャッシュ、secret が commit に含まれない。
5. commit message がリポジトリの `type: message` 形式である。
6. `git diff --check` とリスクに応じた関連テストが成功する。

不足があれば同じ担当へ具体的なレビュー指摘を返し、同じ worktree と branch で修正・commit させます。Issue 間で差分を cherry-pick、merge、squash しません。

## PR

PR を求められた場合は、レビュー済みブランチを Issue ごとに push し、既定ブランチを base とする Draft PR を一件ずつ作ります。本文には次を含めます。

- `Closes #<NUMBER>`
- 利用者または開発者に見える修正の要約
- 主要な修正内容
- 根本原因と修正前の影響
- 実行した検証コマンドと結果

PR の head commit がレビュー済み commit と一致することを確認します。Issue を勝手に close、PR を ready 化、merge しません。worktree と branch の後片付け時期は [`fixing.md`](fixing.md) に従います。

親エージェントの最終報告には、Issue 番号、PR URL、branch、commit、検証結果、未実施の手動確認を含めます。

## 停止条件

次の場合は該当 Issue だけを停止し、他の独立 Issue は続行します。

- 完了に新しい製品判断、秘密情報、外部アカウント、実機操作が必要。
- base branch の更新で要件または差分が変わった。
- ユーザー変更と重なり、安全に分離できない。
- テスト失敗の原因が Issue の範囲外で、修正するとスコープが広がる。
- push または PR 作成に必要な認証・権限がない。

停止した Issue は、再開に必要な判断または操作だけを具体的に報告します。
