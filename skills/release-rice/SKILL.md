---
name: release-rice
description: Rice の新しいバージョンを非同期にリリースする。main の同期確認、SemVer と重複タグの検証、バージョン更新、ローカル検証、前回タグとの差分からの日本語リリースノート作成、注釈付きタグへの本文保存、main とタグの push までを安全に行うときに使う。GitHub Actions や GitHub Release の完了待機は行わない。
---

# Rice を非同期リリースする

失敗したら先へ進まず、失敗した段階と復旧方法を報告する。既存タグの削除・移動、force push、Release の削除は行わない。

## 1. 状態を確認する

1. `AGENTS.md`、`docs/TODO.md`、`docs/releasing.md`、`.github/workflows/release-windows.yml` を読む。
2. `git status --short --branch` を確認する。意図が確認できない変更があれば、stash、commit、破棄せず停止する。
3. `git fetch origin main --tags --prune` を実行する。
4. 現在のブランチが `main` であることを確認する。別ブランチなら停止する。
5. `git rev-list --left-right --count main...origin/main` が `0 0` であることを確認する。behind の clean な `main` だけ `git pull --ff-only origin main` を提案できる。ahead または diverged なら停止する。
6. `git tag --list 'v*' --sort=-version:refname` と、必要なら `gh release list` を確認する。未公開タグがあっても自動で削除・再利用しない。

## 2. バージョンと差分を確認する

1. 指定値の先頭の `v` を除き、SemVer `MAJOR.MINOR.PATCH` と完全一致することを検証する。タグ名は `vX.Y.Z` とする。
2. `git show-ref --verify --quiet "refs/tags/vX.Y.Z"` と `git ls-remote --exit-code --tags origin "refs/tags/vX.Y.Z" "refs/tags/vX.Y.Z^{}"` のいずれかで同名タグを検出したら停止する。終了コード 2 は未使用、通信エラーは未使用とみなさない。
3. 前回のバージョンタグを特定し、`git log --first-parent PREVIOUS_TAG..HEAD`、`git diff --stat PREVIOUS_TAG..HEAD`、必要な個別 diff を読む。タグがなければ全履歴を対象にする。
4. バージョン指定がなければ、実際の互換性への影響から patch/minor/major を判断し、タグ作成前にユーザーへ提示する。

## 3. バージョンを更新して検証する

1. `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/components/StatusBar.tsx` を同じ `X.Y.Z` に更新する。
2. `cargo check --manifest-path src-tauri/Cargo.toml` で `Cargo.lock` を同期する。
3. `pnpm test`、`pnpm build`、`cargo test --manifest-path src-tauri/Cargo.toml` を実行する。可能なら `scripts/build-windows-docker.sh` も実行する。省略した検証と理由を報告する。
4. 差分、4 箇所のバージョン、旧表示の残存、作業ツリーを確認する。
5. バージョン変更だけを `chore: bump version to X.Y.Z` として commit する。ユーザーの別変更を混ぜない。
6. `git push origin main` の直前に対象 commit を提示し、許可されたリリース作業として push する。push 後、`main...origin/main` が `0 0` であることを確認する。

## 4. リリースノートを作る

1. 手順 2 の diff、commit、PR、既存の文体を根拠に、利用者向け変更を重複なく日本語でまとめる。内部的な CI、リファクタ、バージョン更新は利用者に影響しない限り省く。
2. 次のうち空でないセクションだけを使い、GitHub Flavored Markdown にする。

```markdown
## Changes

- 変更内容

## Fixes

- 修正内容

## Other

- その他の変更
```

3. secret、内部 URL、認証情報を含めない。内容に確信が持てなければ、この時点で本文をユーザーへ提示して確認を得る。タグ作成後に初期本文を生成しない。
4. 一時ファイルを `mktemp` で作り、リリースノート全文を書き込む。リポジトリ内の履歴には追加しない。

## 5. 注釈付きタグを push して終了する

1. 同名タグが local/remote とも未使用で、`HEAD` と `origin/main` が一致することをもう一度確認する。
2. `git tag -a "vX.Y.Z" --cleanup=verbatim -F "$notes_file"` で annotated tag を作る。`--cleanup=verbatim` で Markdown 見出しの `#` を保持する。`git tag vX.Y.Z` のような軽量タグは禁止する。
3. `scripts/verify-release-tag.sh "vX.Y.Z"` を実行し、タグ object であること、対象 commit、annotation message 全文を確認する。
4. 一時ファイルを削除する。削除後も annotation message は Git tag object に保存される。
5. tag push の直前にタグ名と対象 commit を提示し、許可されたリリース作業として `git push origin "vX.Y.Z"` を実行する。既存タグへの force push は行わない。
6. GitHub Actions の完了を待たず終了する。タグ名、commit SHA、Actions の確認 URL または `gh run list --workflow release-windows.yml --branch "vX.Y.Z"`、失敗時は `gh run view RUN_ID --log-failed` を表示する。

GitHub Release の本文だけを後から直す場合は、タグを動かさず `gh release edit vX.Y.Z --notes-file release-notes.md` を案内する。
