# リリース手順

Rice は、レビュー・検証して `main` に取り込んだ commit を指す `vX.Y.Z` の注釈付きタグを起点に Windows 版を公開する。tag push workflow は read-only で build し、default branch の publish workflow だけが `contents: write` を取得する。エージェントはタグを push した時点で終了し、GitHub Actions の完了を待つ必要はない。

## 単独管理の運用方針

このリポジトリは所有者一人で管理する。2026-09-08 の所有者判断により、Team / GitHub App、別担当者の承認、required reviewer、release environment、ruleset の設定を公開の必須条件にしない。repository の default branch が `main` であることは API で検証する。

タグの作成者と main の管理者は所有者として信頼する。workflow は tag target / event commit / checkout HEAD / main ancestry / manifest version / build provenance を照合し、公開の各段階でも remote tag object の変化を検出する。暗号学的な署名や複数人の承認を保証する仕組みではない。

既存タグは移動・削除・再利用せず、修正は新しい version で公開する。ruleset を必須にしないため、照合と公開の間の短い競合や、所有者が過去の write workflow を持つ commit にタグを作る操作までは禁止できない。この制約を受け入れ、通常の公開には本手順を使う。

```bash
node scripts/verify-release-repository-policy.mjs OWNER/REPO
```

## Twitch 認証の配布前検査

`RICE_TWITCH_CLIENT_ID` は repository variable または secret に空白のない英数字で設定する。ローカル Docker wrapper と CI は `scripts/verify-twitch-client-id.mjs` で同じ検査を行い、未設定・不正形式ならビルド前に停止する。Docker の直接実行でも同じ gate を通す。ビルド後は生成された `rice.exe` に指定 ID が含まれることを検査し、不一致なら installer/ZIP を出力しない。検査ログは値を出力しない。Client ID は公開識別子として実行ファイルへ埋め込まれるが、トークンや client secret は渡さない。この検査は ID の Twitch 登録状態や実際のログイン成功を保証しない。

## 実行方法

Codex では次のようにリポジトリ内スキルを指定する。

```text
$release-rice を使って v1.2.3 をリリースしてください。
```

スキルは作業ツリーと `main` の同期状態、default branch を確認する。最新 main から version bump branch を作り、3 manifest の version を更新して検証・commit・push し、PR をレビューして main に取り込む。単独管理なので別担当者の承認は要求しない。StatusBar は Cargo package version を動的表示するため、version literal の更新対象ではない。main の同期後、前回タグとの差分からリリースノートを作り、本文を annotation message にしたタグを発行する。

version bump の commit 前には `scripts/verify-release-version.sh X.Y.Z --changed-from HEAD` を実行し、変更対象が `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` のみであることを確認する。タグ作成後は `scripts/verify-release-version.sh X.Y.Z --tag vX.Y.Z` で同じ 3 manifest と tag version を照合する。

```bash
git tag -a v1.2.3 --cleanup=verbatim -F release-notes.md
git push origin v1.2.3
```

リリースノートは GitHub Actions が `--notes-from-tag` で Release 本文に使うため、必ず annotated tag を使う。`--cleanup=verbatim` は Markdown 見出しをコメントとして除去させないために必要となる。`git tag v1.2.3` で作る軽量タグには本文を保存できず、workflow の検証で拒否される。

push 前には tag object と commit を固定して検証する。

```bash
tag_object="$(git rev-parse v1.2.3^{tag})"
scripts/verify-release-tag.sh v1.2.3 \
  --expected-tag-object "${tag_object}" \
  --expected-commit HEAD \
  --checkout-ref HEAD \
  --main-ref origin/main
scripts/verify-release-version.sh 1.2.3 --tag v1.2.3
```

## GitHub Actions

`Release Windows` と `Publish Windows release` workflow は次を非同期で行う。

1. tag push workflow は `contents: read` だけで、注釈付きタグ、空でない annotation message、peeled target と event `GITHUB_SHA` / checkout `HEAD` の完全一致、`origin/main` への到達可能性、3 manifest の version 一致を検証する。
2. 検証した tag object と commit を provenance artifact に固定し、TypeScript のテストと build、Rust のテストを行う。
3. Linux Docker と cargo-xwin で Windows x86_64 の NSIS installer と portable ZIP を作り、チェックサムを付ける。
4. build 成功後、default branch 上の publish workflow が読み取り専用 job で default branch を確認し、同じ run ID の provenance と Assets だけを取得する。tag commit 自身の script は実行せず、default branch の検証・公開 script を使う。
5. publish job の trusted script で provenance の tag object、workflow run commit、current remote tag、checkout `HEAD`、`origin/main`、3 manifest とダウンロードした成果物の checksum を再検証する。
6. Release がなければタグ本文を使った draft を作り、全 Assets の upload 後にダウンロードしてファイル名・内容の完全一致を検証し、remote tag object を再確認して公開する。
7. 公開済み Release の再実行は全 Assets のファイル名・内容が完全一致する場合だけ変更なしで成功する。不一致・不足・余分な Assets があれば停止し、新しい patch version を発行する。未完了 draft に限って `--clobber` で Assets を揃え、ダウンロード検証後に公開する。余分な draft Asset は所有者が除去してから再実行する。

公開処理は `scripts/publish-release.sh` に集約し、workflow から1回だけ呼び出す。

テストや build が失敗した場合、publish workflow は動かない。公開前の再検証で tag / provenance の移動・不一致が見つかれば、Release を変更する前に失敗する。upload 中に失敗した場合は公開せず draft のまま残り、workflow の再実行で継続できる。workflow 内の照合と公開処理の間には短い競合の余地が残る。所有者は公開処理中もタグを移動しない。

## 確認と修正

Actions の状態と失敗ログは次で確認できる。

```bash
gh run list --workflow release-windows.yml --branch v1.2.3
gh run view RUN_ID --log-failed
```

同名タグが local または remote にあればリリースを中断する。既存タグは通常、削除・移動・force push しない。内容を直す必要があれば新しい patch version を発行する。

## 緊急時の復旧

credential 侵害、誤った tag、または不正な Asset を疑う場合は、まず publish workflow を止め、影響する Release を非公開化する。ログと run provenance を保全し、所有者の credential を失効・rotation して原因を確認する。通常の修正では既存 tag を移動・再利用せず、新しい patch version を発行する。

公開済み Release の本文だけを直す場合は、タグを書き換えず次を使う。

```bash
gh release edit v1.2.3 --notes-file release-notes.md
```
