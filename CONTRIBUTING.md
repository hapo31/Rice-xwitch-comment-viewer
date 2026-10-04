# 貢献について

Rice自身のソースコードと同梱するオリジナルassetは、既存の [MIT License](./LICENSE) を正本として提供します。変更を送る場合、あなたがその変更を提供する権利を持ち、RiceのMIT Licenseで公開・改変・再配布できることを確認してください。貢献の許諾はinbound=outbound（提供した変更にも同じMIT）です。別途CLAやDCO署名は現在要求していません。既存copyright表示を削除しないでください。

他者のコード・画像・フォント等を追加する場合は出典と適用licenseを明記し、必要なnotice/textを保持してください。license不明、custom、copyleft、exception付きの素材は自動承認せず、maintainerへ事前に確認します。MultiCommentViewerはGPL-3.0の参考実装であり、そのコードをRiceへコピーする方針ではありません。Twitchや棒読みちゃん、音声合成製品の権利・利用規約はRiceのMITとは別です。

Issue/PRには問題、変更範囲、検証結果を書いてください。package/Cargo/bundleのlicense表示を変更する場合は、正本・README・配布物・policyを同じreviewで揃えます。

## ローカルの品質検査

`pnpm test`はNode unitとjsdom componentの両projectを実行します。個別実行、Tauri mockの使い方、DOM検証の境界は[component testガイド](./docs/component-tests.md)を参照してください。

`pnpm install --frozen-lockfile`後、`pnpm run format:check`、`pnpm run lint`、`pnpm run typecheck`、`pnpm test`、`pnpm build`を実行します。format修正は`pnpm run format`です。Biomeはlockfileで2.5.15へ固定し、TS/TSXの空白・改行・引用符を統一します。lintは型検査の別名ではなく、到達不能コード、未宣言変数、hookの条件呼出し、debugger、重複key/parameter、非厳密比較、危険なHTML/evalをエラーとして検出します。スタイル変更による機能修正を混ぜないため、選択したcorrectness/securityルールを明示管理します。

Rustは`build/release-inputs.json`のcompiler（現在1.90.0）とrustfmt/clippyを使います。`cargo fmt --check --manifest-path src-tauri/Cargo.toml`、`cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`、`cargo test --locked --manifest-path src-tauri/Cargo.toml --all-features`が必須です。

CIと同じ入口は`bash scripts/quality-gate.sh <gate>`です。gate名は`frontend-format/lint/typecheck/test/build`、`rust-format/clippy/test`。PR・main push・release tagが`.github/workflows/quality-checks.yml`を再利用し、各gateは独立したrequired checkに指定できます。Linuxのfeature matrixとWindows runtime検証は別workflowであり、このquality gateだけで実機動作を保証しません。
