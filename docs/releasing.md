# リリース手順

Rice は、review 済み `main` 上の commit を指す保護された不変の `vX.Y.Z` タグを起点に Windows 版を公開する。tag push workflow は read-only で build し、default branch の publish workflow が `release` environment の承認後にだけ `contents: write` を取得する。エージェントはタグを push した時点で終了し、GitHub Actions の完了を待つ必要はないが、承認担当者へ pending deployment を案内する。

## GitHub repository settings（必須）

workflow だけではタグの作成・移動・削除を不可逆に禁止できない。さらに GitHub は tag commit に存在する tag-push workflow も起動できるため、過去の `main` commit に残る旧 workflow の権限を現在の source 変更だけで取り消すことはできない。最初のリリース前、および管理者・automation の変更後に、repository 管理者が次を確認する。workflow からこれらの設定を自動変更しない。

1. `refs/tags/v*` を対象とし除外なしの独立した tag ruleset を2つ `Active` にする。作成用 ruleset は `Restrict creations` を有効にし、bypass list を指名された release 管理者 team または専用 GitHub App（`Always`）に限定する。不変性用 ruleset は `Restrict updates` と `Restrict deletions` を有効にし、bypass list を空にする。作成許可と更新禁止を同じ ruleset に置くと、作成者が更新・削除も bypass できるため分離する。
2. `refs/heads/main` を対象とし除外なしの active branch ruleset で pull request と1名以上の承認、required status checks、force push / deletion 禁止を有効にする。bypass list は空にする。required checks は GitHub check run 名そのものの `Frontend unit tests`、`Rust unit tests`、`Frontend typecheck`、`Rust format and clippy`、`Dev build` を指定する。タグは `main` の到達可能な commit だけを指せるが、`main` 自体が review 境界でなければ意味がない。既存の classic branch protection だけで運用している場合も、この自動検証で確認できる ruleset を追加する。
3. `release` environment を workflow の初回実行前に作成する。tag 作成者とは別の required reviewer を指定し、prevent self-review を有効にし、管理者による protection rule bypass を無効にする。deployment branch/tag policy は `Selected branches and tags` で **branch `main` のみ**を指定する。公開 workflow は `workflow_run` で起動するため deployment ref はタグではなく default branch の `main` になる。未作成の場合は read-only 検証 job が失敗し、environment を使う公開 job は開始しない。
4. Actions / audit log で tag 作成者、environment 承認者、publish run を追跡できることを確認する。tag push 用 credential と environment 承認者の account / token を共有しない。

読み取り確認の例（`OWNER/REPO` は対象 repository に置き換える）:

```bash
gh api repos/OWNER/REPO/rulesets
gh api repos/OWNER/REPO/branches/main/protection
gh api repos/OWNER/REPO/environments/release
node scripts/verify-release-repository-policy.mjs OWNER/REPO
```

現在の署名ポリシーは、annotated tag と上記 GitHub identity / reviewer approval を必須とし、暗号学的な signed tag は必須にしていない。signed tag を採用する場合は、trusted key または workload identity、失効、rotation、退職・侵害時の削除手順を先に定義し、ruleset と検証 workflow の両方で強制する。単に GitHub UI の `Verified` 表示を確認するだけでは承認主体の分離に代えない。

自動検証は API の取得失敗、未設定、無効化、API に見える bypass、必要な review/check の不足を拒否する。任意の glob や classic branch protection を同等と推測せず、上の明示的な ruleset 構成だけを受け付ける。必要な API 読み取り権限は `contents: read` と `actions: read`。

GitHub の [Get a repository ruleset](https://docs.github.com/en/rest/repos/rules#get-a-repository-ruleset) は ruleset への write 権限がない token から `bypass_actors` を隠す。また [Environment API](https://docs.github.com/en/rest/deployments/environments#get-an-environment) は管理者 bypass 設定を公開しない。このため非公開の bypass 設定、team / App の実際の担当者と reviewer の独立性、credential の共有禁止は管理者が Settings で確認する。読み取り専用 job に管理権限を追加しない。自動検証成功をこれらの手動設定まで含む運用完了とは扱わない。

## 実行方法

Codex では次のようにリポジトリ内スキルを指定する。

```text
$release-rice を使って v1.2.3 をリリースしてください。
```

スキルは作業ツリーと `main` の同期状態に加え、上記 repository settings と実行者が許可された release identity であることを確認する。確認できなければタグを作成しない。続いて 3 manifest の version を更新して検証・commit・push する。StatusBar は Rust の Cargo package version を `app_build_info` command 経由で動的表示するため、version literal の更新対象ではない。その後、前回タグとの差分から Markdown のリリースノートを作り、本文を annotation message にしたタグを発行する。内容が曖昧な場合はタグ作成前に確認を求める。

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
4. build 成功後、default branch 上の publish workflow が読み取り専用 job で repository ruleset と environment 設定を検証する。設定が揃った場合だけ公開 job に進み、同じ run ID の provenance と Assets だけを取得する。tag commit 自身の script は信用せず、`main` から checkout した検証・公開 script を使う。
5. publish job は `release` environment の承認後にだけ開始して `contents: write` を取得する。default branch の trusted script で provenance の tag object、workflow run commit、current remote tag、checkout `HEAD`、`origin/main`、3 manifest を再検証し、成功するまで Release を変更しない。
6. 承認待ちの間の変更も検出するため、公開直前に repository ruleset と environment を再検証する。Release がなければタグ本文を使った draft を作り、全 Assets の upload 成功後に remote tag object を再確認して公開する。
7. 再実行で Release があれば本文は変更せず、Assets だけ `--clobber` で更新する。未完了 draft なら Assets を揃えて公開する。

公開処理は `scripts/publish-release.sh` に集約し、workflow から1回だけ呼び出す。

テストや build が失敗した場合、publish workflow は動かない。承認後の再検証で tag / provenance の移動・不一致が見つかれば、Release を変更する前に失敗する。upload 中に失敗した場合は公開せず draft のまま残り、workflow の再実行で継続できる。workflow 内の照合と publish 直前・直後の remote 照合にも短い TOCTOU window は残るため、tag ruleset の update/delete 禁止を省略してはならない。

## 確認と修正

Actions の状態と失敗ログは次で確認できる。

```bash
gh run list --workflow release-windows.yml --branch v1.2.3
gh run view RUN_ID --log-failed
```

同名タグが local または remote にあればリリースを中断する。既存タグは通常、削除・移動・force push しない。内容を直す必要があれば新しい patch version を発行する。

## 緊急時の復旧

credential 侵害、誤った tag、または不正な Asset を疑う場合は、まず publish workflow / `release` environment の deployment を止め、影響する Release を非公開化して利用者へ検証済み情報を告知する。tag や Release を先に書き換えて監査証跡を失わない。tag 作成 credential と environment reviewer credential を失効・rotation し、ruleset / environment / audit log / run provenance を確認する。

通常の修正では既存 tag を移動・再利用せず、新しい patch version を発行する。侵害された tag の削除が避けられない場合だけ、repository 管理者が時間制限付き break-glass 手順で ruleset bypass を有効にし、証跡保全後に削除する。同じ tag 名は再作成せず、操作理由・承認者・時刻を incident record に残し、作業後すぐ bypass を解除する。

公開済み Release の本文だけを直す場合は、タグを書き換えず次を使う。

```bash
gh release edit v1.2.3 --notes-file release-notes.md
```
