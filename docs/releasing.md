# リリース手順

Rice は `main` 上の `vX.Y.Z` タグを起点に Windows 版を公開する。エージェントはタグを push した時点で終了し、GitHub Actions の完了を待つ必要はない。

## 実行方法

Codex では次のようにリポジトリ内スキルを指定する。

```text
$release-rice を使って v1.2.3 をリリースしてください。
```

スキルは作業ツリーと `main` の同期状態を確認し、3 manifest の version を更新して検証・commit・push する。StatusBar は Rust の Cargo package version を `app_build_info` command 経由で動的表示するため、version literal の更新対象ではない。その後、前回タグとの差分から Markdown のリリースノートを作り、本文を annotation message にしたタグを発行する。内容が曖昧な場合はタグ作成前に確認を求める。

version bump の commit 前には `scripts/verify-release-version.sh X.Y.Z --changed-from HEAD` を実行し、変更対象が `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` のみであることを確認する。タグ作成後は `scripts/verify-release-version.sh X.Y.Z --tag vX.Y.Z` で同じ 3 manifest と tag version を照合する。

```bash
git tag -a v1.2.3 --cleanup=verbatim -F release-notes.md
git push origin v1.2.3
```

リリースノートは GitHub Actions が `--notes-from-tag` で Release 本文に使うため、必ず annotated tag を使う。`--cleanup=verbatim` は Markdown 見出しをコメントとして除去させないために必要となる。`git tag v1.2.3` で作る軽量タグには本文を保存できず、workflow の検証で拒否される。

## GitHub Actions

`Release Windows` workflow はタグ push を検知すると、次を非同期で行う。

1. 注釈付きタグと空でない annotation message を検証する。
2. TypeScript のテストと build、Rust のテストを行う。
3. Linux Docker と cargo-xwin で Windows x86_64 の NSIS installer と portable ZIP を作り、チェックサムを付ける。
4. build job の Artifact を単一の release job へ渡す。
5. Release がなければタグ本文を使った draft を作り、全 Assets の upload 成功後に公開する。
6. 再実行で Release があれば本文は変更せず、Assets だけ `--clobber` で更新する。未完了 draft なら Assets を揃えて公開する。

公開処理は `scripts/publish-release.sh` に集約し、workflow から1回だけ呼び出す。

テストや build が失敗した場合、release job は動かない。upload 中に失敗した場合は公開せず draft のまま残り、workflow の再実行で継続できる。

## 確認と修正

Actions の状態と失敗ログは次で確認できる。

```bash
gh run list --workflow release-windows.yml --branch v1.2.3
gh run view RUN_ID --log-failed
```

同名タグが local または remote にあればリリースを中断する。既存タグは通常、削除・移動・force push しない。内容を直す必要があれば新しい patch version を発行する。

公開済み Release の本文だけを直す場合は、タグを書き換えず次を使う。

```bash
gh release edit v1.2.3 --notes-file release-notes.md
```
