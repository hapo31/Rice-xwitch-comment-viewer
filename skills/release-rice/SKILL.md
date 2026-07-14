---
name: release-rice
description: Rice のリリースを安全に実施する。main の同期確認、ローカル検証、SemVer 判断、マニフェストとアプリ内表示のバージョン更新、Git タグの作成と push、GitHub Actions のリリース完了確認、前回リリースとの差分から日本語パッチノートを作成・反映するときに使う。
---

# Rice をリリースする

途中で失敗したら先へ進まず、失敗した段階と復旧方法を報告する。タグの push 後は履歴を書き換えず、タグを削除・移動する必要がある場合は必ずユーザーの明示承認を得る。

## 1. リリース前状態を確認する

1. `AGENTS.md`、`docs/TODO.md`、`.github/workflows/release-windows.yml`、各バージョンファイルとアプリ内のバージョン表示箇所を読む。現在の表示箇所は `src/components/StatusBar.tsx` とする。
2. `git status --short --branch` でブランチと作業ツリーを確認する。ユーザーの未コミット変更があれば勝手に stash、commit、破棄せず停止する。
3. `git fetch origin main --tags --prune` を実行する。
4. 現在のブランチが `main` であることを確認する。違う場合は、作業ツリーが clean なときだけ `main` への切替を提案する。
5. `git rev-list --left-right --count main...origin/main` が `0 0` であることを確認する。behind の場合は clean な `main` を `git pull --ff-only origin main` で更新し、ahead/diverged の場合は停止して状況を報告する。
6. `git tag --list 'v*' --sort=-version:refname` と `gh release list` で最新タグ・最新リリースを確認する。タグとリリースの不整合があれば解消せず報告する。

## 2. バージョンを決める

ユーザー指定があれば、先頭の `v` を除いた値が SemVer `MAJOR.MINOR.PATCH` であり、既存タグと重複しないことを検証する。

指定がなければ、前回タグから `main` までの commit と diff を読み、最も大きい変更に合わせて決める。

- patch: バグ修正、文言修正、依存更新、内部整理など、互換性と機能を維持する軽微な変更
- minor: 後方互換な新機能、既存利用者が移行可能な変更
- major: 後方互換性のない API・設定・データ形式・操作仕様の変更

Conventional Commit の接頭辞だけで機械的に決めず、実際の diff を確認する。判断した場合は、比較範囲、該当変更、patch/minor/major を選んだ理由をタグ作成前に簡潔にユーザーへ示す。判断不能な互換性変更がある場合は質問する。

## 3. バージョンを揃えてローカル検証する

1. `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/components/StatusBar.tsx` の表示バージョンを同じ `X.Y.Z` へ更新する。将来表示箇所が移動・追加されていたら、`rg` で旧バージョンとバージョン表示を検索して更新対象を特定する。
2. `cargo check --manifest-path src-tauri/Cargo.toml` など正規の Cargo コマンドで `Cargo.lock` を同期する。
3. 変更が生じたら差分を確認し、マニフェストとアプリ内表示を同じ `chore: bump version to X.Y.Z` commit に含める。ユーザーの変更を混ぜない。
4. 次をすべて実行する。
   - `pnpm test`
   - `pnpm build`
   - `cargo test --manifest-path src-tauri/Cargo.toml`
   - `scripts/build-windows-docker.sh`（Docker と必要なローカル環境が利用可能な場合）
5. Windows Docker ビルドを実行できない場合は理由を明示する。少なくとも前の3コマンドが成功しない限りリリースしない。Windows 成果物のローカル検証を省略してタグを進めるには、ユーザーの明示承認を得る。
6. `git status --short`、4箇所のバージョン、旧バージョンがアプリ内表示に残っていないこと、`release-artifacts` の `.exe` / `.zip`（実行時）を確認する。`pnpm build` の成功をアプリ内表示のコンパイル検証とする。
7. バージョン commit ができた場合は `git push origin main` を行い、再度 `main...origin/main` が `0 0` であることを確認する。

## 4. タグを発行して CI を待つ

1. `vX.Y.Z` がローカル・remote とも未使用で、HEAD が `origin/main` と一致することを再確認する。
2. annotated tag を `git tag -a vX.Y.Z -m "Release vX.Y.Z"` で作成する。
3. tag の対象 commit と内容を表示してから `git push origin vX.Y.Z` を実行する。
4. `.github/workflows/release-windows.yml` の run を `gh run list` / `gh run watch` で追跡する。待機をこのセッションで継続できない場合は、タグ、commit SHA、workflow 名をチェックポイントとして示し、「リリースが出来たら続行を指示してください」とユーザーへ伝えて終了する。
5. 続行時は `gh release view vX.Y.Z` と workflow 結果を必ず再確認する。CI が失敗中、未完了、成果物不足ならパッチノート更新へ進まない。

## 5. パッチノートを作成して反映する

1. 現在のリリースより前に公開された直近の non-draft release を取得する。
2. `git log --first-parent PREVIOUS_TAG..vX.Y.Z` と `git diff --stat PREVIOUS_TAG..vX.Y.Z` を確認し、必要なら個別 diff も読む。merge commit の題名だけに依存しない。
3. ユーザー向け変更だけを、重複なく簡潔な日本語1行にまとめる。内部的な CI・リファクタ・バージョン更新は、利用者に影響しない限り省く。
4. 次の形式を使い、該当項目がなければ `- 該当なし` とする。

```markdown
## 機能追加

- 1変更を完結な1行で記述する。

## バグ修正

- 1修正を完結な1行で記述する。
```

5. 比較元・比較先と draft をユーザーへ提示して内容を確認する。明示承認後、`gh release edit vX.Y.Z --notes-file ...` で既存の GitHub Release 本文を更新する。
6. 更新後に `gh release view vX.Y.Z` でタイトル、タグ、公開状態、本文、成果物を検証し、URL と要約を報告する。

## ガードレール

- `git reset --hard`、既存タグの上書き、force push、公開リリースの削除を行わない。
- release workflow がタグ `v[0-9]*` で起動することを変更前に確認する。
- Client ID など secret の値をコマンド出力、ログ、リリースノートへ載せない。
- GitHub への push と Release 本文更新は外部状態の変更として扱い、実行直前に対象を明示する。
