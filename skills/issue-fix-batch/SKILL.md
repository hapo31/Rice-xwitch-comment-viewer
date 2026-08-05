---
name: issue-fix-batch
description: GitHub Issues を重要度と修正規模で選定し、Issue ごとの独立 git worktree とブランチで Luna サブエージェントへ並列実装させ、レビュー・検証後に個別の Draft PR として提出する。ユーザーが複数 Issue の一括修正、重要 Issue の並列対応、並列数を指定した Issue 修正、または Issue ごとの個別 PR 作成を求めたときに使う。
---

# GitHub Issues を並列修正する

親エージェントが選定、割当、レビュー、公開を管理する。各 Luna は一つの Issue だけを独立 worktree で実装する。複数 Issue の変更を同じブランチ、commit、PR に混ぜない。

## 1. 実行条件を決める

1. リポジトリの `AGENTS.md` と関連設計文書、`docs/TODO.md` を読む。
2. Git remote、既定ブランチ、現在の worktree と変更状態を確認する。既存のユーザー変更は移動、破棄、stash しない。
3. ユーザー指定の Issue 件数、並列数、重要度、ラベル、対象範囲を優先する。
4. 並列数が未指定なら 3 とする。実際の同時実行数は、指定値、選定 Issue 数、利用可能なサブエージェント枠の最小値にする。親エージェント用の枠を一つ残す。超過分は同じ並列数で次の wave に送る。
5. 実装担当には `gpt-5.6-terra` を使う。Sol または Terra より上位のモデルを使わない。思考強度はユーザー指定を優先し、未指定なら局所的な docs/config/UI 修正は `low`、単一レイヤのコード修正は `medium`、複数レイヤまたは concurrency/security 修正は `high` を基本にする。必要なら `xhigh`、`max`、`ultra` も選べる。

## 2. Issue を選定する

GitHub connector を優先して open Issue と既存 PR を取得する。Issue 本文やラベルにある severity、影響、再現条件、完了条件を根拠に順位付けする。

次を優先する。

1. ユーザー指定の重要度。指定がなければ `severity:critical`、`severity:high`、`severity:medium` の順。
2. bug、security、correctness、reliability、privacy。
3. 完了条件が明確で、変更範囲と検証方法が限定されるもの。
4. 外部サービス、実機、秘密情報、破壊的操作を必要とせず、自動テストで検証できるもの。
5. 他の選定 Issue と編集ファイルや設計責務が重なりにくいもの。

既存 PR で対応中、別 Issue に依存、要件判断が未確定、手動確認だけで完了する Issue は除外または後順位にする。選定理由をユーザーへ短く報告してから開始する。

## 3. Luna を起動する

起動前に [subagent-prompt.md](references/subagent-prompt.md) を完全に読み、プレースホルダーだけを置換する。Issue の内容、親の診断、修正案を追加しない。Luna 自身に GitHub から Issue を取得させる。

Issue ごとに次を一意にする。

- task name: `luna_issue_<NUMBER>`
- worktree: `/tmp/<REPOSITORY_SLUG>-issue-<NUMBER>` または安全な一時ディレクトリ。`REPOSITORY_SLUG` は owner を含まない repository 名を英数字・`-`・`_` だけへ正規化する
- branch: `agent/issue-<NUMBER>-<SHORT-SLUG>`

同じ Issue の branch、worktree、PR が既に存在する場合は新規作成せず、状態を確認して再開または除外する。`spawn_agent` は `fork_turns: "none"`、`model: "gpt-5.6-terra"`、選んだ `reasoning_effort` で呼ぶ。各 wave の全 Luna を起動してから親の作業を続ける。

## 4. 成果をレビューする

Luna は commit までで停止し、push と PR 作成は親が行う。各完了報告について次を確認する。

1. worktree とブランチが Issue 専用で clean か。
2. Issue の完了条件を差分とテストが満たすか。
3. `AGENTS.md`、設計文書、`docs/TODO.md`、調査メモが実装と一致するか。
4. unrelated change、生成物、依存キャッシュ、secret が commit に含まれないか。
5. commit message がリポジトリの `type: message` 形式か。
6. `git diff --check` とリスクに応じた関連テストが成功するか。

不足があれば同じ Luna に具体的なレビュー指摘を送り、同じ worktree と branch で修正・commit させる。Issue 間で差分を cherry-pick、merge、squash しない。

## 5. Issue ごとに Draft PR を提出する

GitHub の公開ワークフローに従い、レビュー済みブランチを個別に push する。PR は既定ブランチを base とし、必ず Draft で一件ずつ作る。本文は次の構成にする。

```markdown
## 概要

Closes #<NUMBER>

<利用者または開発者に見える修正の要約>

## 修正内容

- <主要変更>

## 原因と影響

<根本原因と修正前の影響>

## 検証

- `<実行したコマンド>`（<結果>）
```

PR の head commit がレビュー済み commit と一致することを確認する。PR URL、Issue 番号、branch、commit、検証結果、未実施の手動確認を最終報告する。Issue を勝手に close、PR を ready 化、merge、worktree 削除しない。

## 停止条件

次の場合は該当 Issue だけを停止し、他の独立 Issue は続行する。

- Issue の完了に新しい製品判断、秘密情報、外部アカウント、実機操作が必要。
- base branch の更新で要件または差分が変わった。
- ユーザー変更と重なり、安全に分離できない。
- テスト失敗の原因が Issue の範囲外で、修正するとスコープが広がる。
- push または PR 作成に必要な認証・権限がない。

停止した Issue は、再開に必要な判断または操作を具体的に報告する。
