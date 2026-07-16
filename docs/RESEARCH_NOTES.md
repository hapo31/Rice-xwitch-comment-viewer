# 調査メモ

## 2026-07-16

- 既存リリースは `vX.Y.Z` タグを起点に、Linux Docker + cargo-xwin で Windows x86_64 の NSIS installer と portable zip を生成し、build/release の2ジョブで GitHub Release を公開していた。これを、タグ annotation message を初期 Release 本文に使い、タグ push 後はエージェントが待機しない方式へ変更した。Release は draft 作成、Assets upload、公開の順とし、再実行時は既存本文を保持して Assets を `--clobber` 更新する。
- `git tag -F` の既定 cleanup では Markdown 見出しがコメントとして除去されるため、リリースタグ作成時は `--cleanup=verbatim` を指定する。

調査や作業中に分かった補足情報を記録するファイルです。日付が新しいものほど上に追記してください。

## 2026-07-14

- Queue view は読み上げ済み・手動スキップ済みの履歴を除外し、待機中・読み上げ中・エラー・フィルター設定による除外だけを表示する役割に整理した。キュー ID の連番で降順に並べ、Chat view と同じく新しい項目が上になる表示方向へ統一した。
- リリース時のバージョン更新対象は `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` に加え、ステータスバーのアプリ内表示にも存在する。表示が `0.1.0` のまま、マニフェストが `0.1.1` になっていたため現在値を揃え、`release-rice` スキルで4箇所を同じバージョン commit に含めて旧表示の残存を検査する手順にした。

## 2026-07-11

- Login 画面の Twitch 有効性確認は非同期処理中にボタンを無効化し、スピナーと「確認中...」を表示するようにした。確認成功後は従来の通知一覧に加え、認証設定内にも成功メッセージを表示する。
- Twitch 認証の有効性確認で access token の検証と refresh の両方に失敗した場合、EventSub 接続を停止し、メモリ上および保存済みの認証情報を削除するようにした。Login 画面も未認証状態へ戻し、古いプロフィールを残さない。
- Login 画面の認証アクションを状態連動に整理し、未認証時は認証開始、認証済みでは認証解除を同じ位置に表示するようにした。Device Code Flow のポーリング停止は、Twitch が待機状態を OAuth の `error` ではなく `message: "authorization_pending"` で返すのに、実装が `error` だけを判定していたことが原因だった。両形式を判定して待機ポーリングを継続するよう修正し、応答形式の回帰テストを追加した。

## 2026-05-26

- 左ペイン整理として、Side Panel 末尾の「テスト読み上げ」を削除し、操作場所を Settings 画面へ一本化した。Activity Bar から Logs ナビゲーションも削除し、ステータスバーの警告/状態表示と Logs view `/logs` は維持した。
- UI、Rust の通知/ログ、設計文書、TODO の日本語表記を、配信者向けに一般的な「読み上げ」へ統一。内部 API 名の `speech` はコード境界として維持。

## 2026-05-24

- Twitch 公式用語に合わせ、UI のチャット受信/停止/キュー/ログ説明、Rust 側の日本語ログ、設計ドキュメント/TODO の「コメント」表記を「チャット」へ統一した。型名や EventSub の `channel.chat.message` 境界は既存実装のまま維持。
- UI 整理として Settings route を `/auth` / Auth 表示へ改名し、Activity Bar アイコンを認証用に変更。Auth 画面は Twitch 認証、チャンネル、起動時自動接続だけを扱う。読み上げ基本設定の自動読み上げ/ユーザー名読み上げ/emote 読み上げは Voices へ集約し、Rules は NG/URL/長文の規則に絞った。`pnpm test`、`pnpm build`、`CARGO_TARGET_DIR=/workspaces/Rice-xwitch-comment-viewer/src-tauri/target pnpm tauri build --bundles deb` は成功。通常の `pnpm tauri build` は AppImage bundling が読み取り専用 FS で失敗するため、この devcontainer では bundle 対象指定が必要。
- GitHub Actions の Windows リリースビルド失敗を確認。`v0.0.3` は `RICE_TWITCH_CLIENT_ID` 未設定で `test -n` が即失敗、`v0.0.2` は Tauri の Windows リソース生成で `src-tauri/icons/icon.ico` がなく失敗していた。workflow は Client ID 未設定を警告に変更し、ビルド自体は継続するようにした。Twitch ログインは従来どおりビルド時 Client ID がない場合に UI へ設定エラーを出す。`icon.png` から Windows 用 `icon.ico` を追加し、`tauri.conf.json` の bundle icon に明示した。

## 2026-05-23

- devcontainer が重い原因を調査。メモリ/ディスク容量の枯渇はなく、主因候補は `src-tauri/target` が 7.0GB まで肥大化していること、特に `debug/deps` 4.3GB、`debug/incremental` 883MB、`release/deps` 1.2GB。`postCreateCommand` は `npm install -g @openai/codex@latest` と `pnpm install --frozen-lockfile` を毎回走らせるため、Rebuild/作成時の待ち時間要因になり得る。VS Code 拡張は rust-analyzer/Tailwind/Error Lens があり、watcher exclude は設定済みだが Cargo/Rust 側の target I/O とは別。改善候補は Cargo target をワークスペース外の volume/tmpfs へ逃がす、`setup.sh` を冪等化して Codex CLI の再インストールを避ける、不要時は release 成果物を削除する、rust-analyzer の実行条件をさらに絞ること。
- devcontainer 軽量化を実施。`CARGO_TARGET_DIR=/home/vscode/.cargo-target/rice` と named volume `rice-cargo-target` を追加し、Cargo の重い build artifacts をワークスペース外へ移した。rust-analyzer は専用 target dir を使う設定にした。`setup.sh` は `CODEX_NPM_PACKAGE` 未指定かつ `codex` 既存時に npm global install を省略し、`pnpm install` は `--prefer-offline` を付けた。廃止済み desktop-lite の 6080/5901 port forwarding も削除した。既存の `src-tauri/target` は自動削除していない。
- devcontainer rebuild 後に Codex の認証情報と履歴が消える問題を防ぐため、`rice-codex-home` named volume を `/home/vscode/.codex` にマウントするようにした。`setup.sh` で所有者と `700` 権限を整える。
- Codex 状態永続化のために作成する Docker volume 名が分かるよう、`.devcontainer/README.md` に `docker volume create rice-codex-home` を明記した。
- devcontainer rebuild 前後で Codex 状態を手動退避/復元できるよう、書き捨ての `.devcontainer/codex-state-transfer.sh` を追加した。バックアップ zip は git 管理外の `.codex-state-backup/codex-state-backup.zip` に置く。
- Settings 表示時に `TypeError: undefined is not an object (evaluating 's.trim')` が出る問題を修正。Tauri client 層で取得/更新後の設定を既定値とマージし、Settings / Voices のフォーム初期値も部分的な設定オブジェクトで欠けた項目を既定値で補完するようにした。`pnpm build` は成功。
- `ViewId` と `AppState.activeView` による独自ビュー切り替えを廃止し、`react-router-dom` の `HashRouter` / `NavLink` / `Routes` へ移行した。Tauri の file/custom protocol 配信でも直接パス再読込に依存しないよう hash routing を採用。未実装の Queue / Rules / Logs route は専用の仮ページを表示し、画面遷移したことが分かる状態にした。
- アプリ内の日本語が豆腐表示になる問題を調査。devcontainer に日本語フォントが入っておらず、WebKit/WSLg で CJK fallback が成立しない状態だった。Tailwind の `fontFamily.sans` / `fontFamily.mono` に Windows 標準の日本語フォントと Noto CJK 系 fallback を追加し、devcontainer には `fonts-noto-cjk` を追加した。
- GitHub Actions の Windows リリースビルドで `RICE_TWITCH_CLIENT_ID` を repository variable または secret から Docker build arg として渡すようにした。Client ID は Settings UI と `settings_get` の返却値から外し、OAuth 開始時はビルド時に埋め込まれた内部既定値だけを使う。古い `settings.json` に `clientId` が残っていても serde の未知フィールドとして無視される。
- Windows リリース用に Linux Docker + `cargo-xwin` + NSIS の Dockerfile と、タグ `v[0-9]*` push でビルド/リリースする GitHub Actions workflow を追加。Tauri 公式では Windows 上の `tauri build` が本筋で、Linux/macOS からの Windows クロスビルドは NSIS 限定かつ caveat ありのため、workflow は `--bundles nsis` に固定した。Actions は build job と release job を分離し、build job は `contents: read` のみ、release job のみ `contents: write`。キャッシュ poisoning 回避のため `actions/cache` と Docker GHA cache は使わず、`docker build --pull --no-cache` と短期 artifact 受け渡しにした。
- `cargo-xwin 0.22.0` の MSRV が Rust 1.89 だったため、Dockerfile の Rust image を `rust:1.89.0-bookworm` に更新した。

## 2026-05-22

- Phase 1 実装確認として `cargo test` と `pnpm build` を実行し、どちらも成功。棒読みちゃん実機でのテスト読み上げ、未起動、ポート競合、アプリ連携 OFF の手動確認は未実施。
- Phase 3 の初期実装として `tokio-tungstenite` による EventSub WebSocket 接続、Welcome 後の `channel.chat.message` 購読、keepalive 欠落/reconnect/revocation 処理、`event.message_id` fallback の重複排除、`twitch://chat-message` のフロントエンド購読を追加。`cargo test` と `pnpm build` は成功。実 Twitch チャンネルでの受信確認は未実施。
- Side Panel のキュー上へチャット受信の開始/停止ボタンを追加し、`twitch_stop_chat` で認証解除せずに EventSub 接続だけ停止できるようにした。UI store では Twitch 認証状態とチャット受信接続状態を分離。`cargo test` と `pnpm build` は成功。

## 状態メモ

- Git 作業ツリーは調査開始時点で clean。
- `src-tauri/target` と `dist` がローカルに存在するため、ビルド済み成果物はある。
- `src/components/MainView.tsx` の Chat view は EventSub 由来のチャット表示に接続済み。未受信時のみサンプルメッセージを表示する。
- `src/stores/appStore.ts` は `twitch://status` / `twitch://chat-message` / `speech://status` の購読反映を実装済み。実キュー連携は Phase 4 で実装する。
- `src/tauri/client.ts` で `app://log`, `twitch://status`, `twitch://chat-message`, `speech://status`, `speech://queue-updated` を購読できる。
- `src-tauri/src/app_events/mod.rs` にイベント payload と `tauri::Emitter` helper を実装し、設定/認証/棒読みちゃん操作から発火する。
- `src-tauri/src/twitch/mod.rs` は認証、Helix ユーザー解決、EventSub WebSocket 接続、Helix subscription 作成まで実装済み。
