# 実装 TODO

最終調査日: 2026-05-23

この TODO は `docs/06-implementation-roadmap.md` の Phase に沿って、現在の実装状況と次に進める作業を追跡するためのものです。作業を始める前後に該当項目を更新してください。

## 現在の進捗サマリ

| Phase | 状態 | メモ |
| --- | --- | --- |
| Phase 0: プロジェクト作成 | 完了 | `app_events` の配信基盤と frontend 購読を接続し、`settings.json` の生成/読込を確認した。 |
| Phase 1: 棒読みちゃん連携 | 実装済み、自動検証済み、手動確認待ち | TCP 発話、制御、接続診断、Voices 画面は実装済み。`cargo test` と `pnpm build` は成功。実機の棒読みちゃんでの確認が必要。 |
| Phase 2: Twitch 認証 | 実装中 | Device Code Flow、`/validate`、refresh、keyring/ Linux fallback、Settings 画面は実装済み。Client ID は UI/設定JSONに出さずビルド時既定値を使う。実 Twitch 環境での確認が必要。 |
| Phase 3: EventSub コメント受信 | 実装中 | WebSocket 接続、`channel.chat.message` 購読、正規化、重複排除、開始/停止 UI、フロントエンド反映を実装。実 Twitch 環境での手動確認が必要。 |
| Phase 4: 読み上げキュー統合 | 未着手 | store 上のキュー枠はあるが、Rust 側の `SpeechQueue` / `SpeechFormatter` と自動読み上げは未実装。 |
| Phase 5: 配信運用向け仕上げ | 一部のみ | ステータスバーと警告表示、タグリリース用 Windows ビルド CI はあるが、Logs 画面、自動接続、自動読み上げ、詳細な運用エラー整理は未実装。 |
| Phase 6: VOICEROID2 実験アダプタ | 未着手 | MVP 後に Windows 専用の実験アダプタとして追加する。 |

## Phase 0: プロジェクト作成

- [x] Tauri + TypeScript + Tailwind の雛形を作る。
- [x] `src-tauri/src` に `twitch`, `speech`, `settings`, `app_events` の境界を作る。
- [x] Activity Bar、Side Panel、Main View、Status Bar の基本レイアウトを作る。
- [x] Activity Bar のビュー切り替えを `react-router-dom` ベースのルーティングへ移行する。
- [x] 未実装 route に Chat view ではなく仮ページを表示する。
- [x] 独自 Title Bar、ウィンドウ操作、リサイズハンドルを作る。
- [x] UI 倍率の自動/手動切替を作る。
- [x] 一般設定を Tauri app data 配下の `settings.json` に保存する。
- [x] `app_events` からフロントエンドへ流すイベント設計を実装に接続する。
- [x] Phase 0 完了条件として、Tauri アプリ起動と設定 JSON 読み書きを手動確認する。

## Phase 1: 棒読みちゃん連携

- [x] `SpeechAdapter` trait の境界を作る。
- [x] `BouyomiAdapter` の短命 TCP 接続を実装する。
- [x] 棒読みちゃん発話パケットを生成する。
- [x] 一時停止、再開、スキップ、クリアの制御コマンドを実装する。
- [x] 接続確認と接続診断を実装する。
- [x] 棒読みちゃん未起動時の日本語エラーを返す。
- [x] Voices 画面から接続確認、診断、テスト発話、ホスト/ポート/声質設定を操作できるようにする。
- [ ] 実機の棒読みちゃんでテスト発話できることを確認する。
- [ ] 棒読みちゃん未起動、ポート競合、アプリ連携 OFF の手動確認を行う。
- [ ] 接続失敗時に読み上げキューを破棄しない挙動を Phase 4 で統合確認する。

## Phase 2: Twitch 認証

- [x] Twitch Client ID を `.env` / build env から内部既定値として読み込む。
- [x] Twitch Client ID を Settings UI と設定 JSON の公開項目から外す。
- [x] OAuth Device Code Flow の開始とポーリングを実装する。
- [x] `user:read:chat` スコープでトークンを取得する。
- [x] `/validate` でトークン有効性を確認する。
- [x] access token 検証失敗時に refresh token で更新する。
- [x] refresh 成功時に保存済み refresh token を差し替える。
- [x] OS keyring 優先の OAuth 保存/復元/削除を実装する。
- [x] Linux の Secret Service 不可時に `~/.rice/twitch-auth.json` へ `0600` で保存する fallback を実装する。
- [x] Settings 画面に認証開始、確認、有効性確認、解除を実装する。
- [ ] 実 Twitch Client ID で Device Code Flow を手動確認する。
- [ ] 認可取り消し、401、期限切れ時の UI 表示を手動確認する。
- [ ] アプリ起動時の保存済み認証復元と refresh 更新を手動確認する。
- [x] Twitch ユーザー ID と接続チャンネルを EventSub 接続へ渡す command を実装する。

## Phase 3: EventSub コメント受信

- [x] `tokio-tungstenite` を導入する。
- [x] `EventSubClient` 相当の接続ループを作り、`wss://eventsub.wss.twitch.tv/ws` へ接続する。
- [x] `session_welcome` 受信後に `channel.chat.message` を購読する。
- [x] EventSub 購読に User Access Token を使う。
- [x] `session_keepalive` 欠落を検出して状態とログへ出す。
- [x] `session_reconnect` を処理する。
- [x] `revocation` を処理し、UI に再ログインまたは再接続が必要な状態を出す。
- [x] `metadata.message_id` または `event.message_id` で重複排除する。
- [x] `channel.chat.message` JSON fixture のパーステストを追加する。
- [x] `ChatMessage` に fragments / badges / received_at を含める。
- [x] `tauri::Emitter` events で `twitch://status` と `twitch://chat-message` を送る。
- [x] TypeScript client で Twitch events を購読し、store へ反映する。
- [x] Chat view にリアルタイムコメントを表示する。
- [x] Side Panel のキュー上にコメント受信の開始/停止ボタンを追加する。
- [x] Twitch 認証状態とコメント受信接続状態を UI store 上で分離する。
- [ ] 実 Twitch 環境で `channel.chat.message` 購読と Chat view 表示を手動確認する。

## Phase 4: 読み上げキュー統合

- [ ] `SpeechFormatter` を実装する。
- [ ] URL、改行、制御文字、長文、emote の扱いを `SpeechFormatter` に閉じ込める。
- [ ] コメント由来の棒読みちゃんタグを初期設定で無効化またはエスケープする。
- [ ] FIFO の `SpeechQueue` を実装する。
- [ ] 最大件数 200、1 コメント最大 120 文字、ユーザー単位 2 秒の連投抑制を実装する。
- [ ] キュー溢れ時に古い未読を落とし、UI に警告を出す。
- [ ] 読み上げ失敗時に 1 回だけ短い遅延で再試行する。
- [ ] コメント受信から `SpeechFormatter`、`SpeechQueue`、`BouyomiAdapter` への流れを接続する。
- [ ] `speech://queue-updated` と `speech://status` events を実装する。
- [ ] Queue view を実装し、スキップ、クリア、再読込、削除を操作できるようにする。
- [ ] `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [ ] TypeScript の store reducer テストを追加する。

## Phase 5: 配信運用向け仕上げ

- [x] `v[0-9]*` タグ push で Windows NSIS ビルドと GitHub Release 作成を行う Actions workflow を追加する。
- [x] Windows リリースビルド用 Dockerfile と `.dockerignore` を追加する。
- [x] リリース workflow では build/release job を分離し、release job のみ `contents: write`、build cache は未使用にする。
- [ ] Logs view を実装する。
- [ ] `app://log` event をフロントエンドへ接続する。
- [ ] EventSub、認証、読み上げアダプタのログを Logs view に表示する。
- [ ] ステータスバーに Twitch 接続状態、棒読みちゃん状態、キュー件数、警告状態を集約する。
- [ ] 起動時自動接続を実装する。
- [ ] 自動読み上げ ON/OFF を実装する。
- [ ] Rules view を実装する。
- [ ] NG ユーザー、NG ワード、URL 処理、長文処理、emote 処理の設定を実装する。
- [ ] 配信中に判断しやすい日本語エラー文言を整理する。
- [ ] コメント行の表示状態テストを追加する。
- [ ] 設定フォームのバリデーションテストを追加する。

## Phase 6: VOICEROID2 実験アダプタ

- [ ] MVP 完了後に着手可否を再判断する。
- [ ] Windows 専用 feature として隔離する方針を維持する。
- [ ] C# sidecar の PoC を作る。
- [ ] Rust から stdio JSON-RPC で `speak`, `stop`, `health` を呼ぶ。
- [ ] VOICEROID2 のバージョン、bitness、起動状態を診断する。
- [ ] 失敗時に棒読みちゃんアダプタへ戻せる UI を作る。

## テストと確認

- [x] Rust: 棒読みちゃんパケット生成テストを追加する。
- [x] Rust: 棒読みちゃん制御パケットテストを追加する。
- [x] Rust: 外部 URL 許可リストのテストを追加する。
- [x] Rust: `channel.chat.message` JSON fixture のパーステストを追加する。
- [ ] Rust: `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [ ] Rust: WebSocket 再接続状態遷移テストを追加する。
- [ ] TypeScript: store reducer テストを追加する。
- [ ] TypeScript: コメント行の表示状態テストを追加する。
- [ ] TypeScript: 設定フォームのバリデーションテストを追加する。
- [ ] 手動: 棒読みちゃん未起動/起動中/ポート競合を確認する。
- [ ] 手動: Twitch トークン期限切れ/認可取り消しを確認する。
- [ ] 手動: 配信中コメント連投を確認する。
- [ ] 手動: ネットワーク切断と復帰を確認する。

## 調査メモ

- 2026-05-23: devcontainer が重い原因を調査。メモリ/ディスク容量の枯渇はなく、主因候補は `src-tauri/target` が 7.0GB まで肥大化していること、特に `debug/deps` 4.3GB、`debug/incremental` 883MB、`release/deps` 1.2GB。`postCreateCommand` は `npm install -g @openai/codex@latest` と `pnpm install --frozen-lockfile` を毎回走らせるため、Rebuild/作成時の待ち時間要因になり得る。VS Code 拡張は rust-analyzer/Tailwind/Error Lens があり、watcher exclude は設定済みだが Cargo/Rust 側の target I/O とは別。改善候補は Cargo target をワークスペース外の volume/tmpfs へ逃がす、`setup.sh` を冪等化して Codex CLI の再インストールを避ける、不要時は release 成果物を削除する、rust-analyzer の実行条件をさらに絞ること。
- 2026-05-23: devcontainer 軽量化を実施。`CARGO_TARGET_DIR=/home/vscode/.cargo-target/rice` と named volume `rice-cargo-target` を追加し、Cargo の重い build artifacts をワークスペース外へ移した。rust-analyzer は専用 target dir を使う設定にした。`setup.sh` は `CODEX_NPM_PACKAGE` 未指定かつ `codex` 既存時に npm global install を省略し、`pnpm install` は `--prefer-offline` を付けた。廃止済み desktop-lite の 6080/5901 port forwarding も削除した。既存の `src-tauri/target` は自動削除していない。
- 2026-05-23: Settings 表示時に `TypeError: undefined is not an object (evaluating 's.trim')` が出る問題を修正。Tauri client 層で取得/更新後の設定を既定値とマージし、Settings / Voices のフォーム初期値も部分的な設定オブジェクトで欠けた項目を既定値で補完するようにした。`pnpm build` は成功。
- 2026-05-23: `ViewId` と `AppState.activeView` による独自ビュー切り替えを廃止し、`react-router-dom` の `HashRouter` / `NavLink` / `Routes` へ移行した。Tauri の file/custom protocol 配信でも直接パス再読込に依存しないよう hash routing を採用。未実装の Queue / Rules / Logs route は専用の仮ページを表示し、画面遷移したことが分かる状態にした。
- 2026-05-23: GitHub Actions の Windows リリースビルドで `RICE_TWITCH_CLIENT_ID` を repository variable または secret から Docker build arg として渡すようにした。Client ID は Settings UI と `settings_get` の返却値から外し、OAuth 開始時はビルド時に埋め込まれた内部既定値だけを使う。古い `settings.json` に `clientId` が残っていても serde の未知フィールドとして無視される。
- 2026-05-23: Windows リリース用に Linux Docker + `cargo-xwin` + NSIS の Dockerfile と、タグ `v[0-9]*` push でビルド/リリースする GitHub Actions workflow を追加。Tauri 公式では Windows 上の `tauri build` が本筋で、Linux/macOS からの Windows クロスビルドは NSIS 限定かつ caveat ありのため、workflow は `--bundles nsis` に固定した。Actions は build job と release job を分離し、build job は `contents: read` のみ、release job のみ `contents: write`。キャッシュ poisoning 回避のため `actions/cache` と Docker GHA cache は使わず、`docker build --pull --no-cache` と短期 artifact 受け渡しにした。
- 2026-05-23: `cargo-xwin 0.22.0` の MSRV が Rust 1.89 だったため、Dockerfile の Rust image を `rust:1.89.0-bookworm` に更新した。
- 2026-05-22: Phase 1 実装確認として `cargo test` と `pnpm build` を実行し、どちらも成功。棒読みちゃん実機でのテスト発話、未起動、ポート競合、アプリ連携 OFF の手動確認は未実施。
- 2026-05-22: Phase 3 の初期実装として `tokio-tungstenite` による EventSub WebSocket 接続、Welcome 後の `channel.chat.message` 購読、keepalive 欠落/reconnect/revocation 処理、`event.message_id` fallback の重複排除、`twitch://chat-message` のフロントエンド購読を追加。`cargo test` と `pnpm build` は成功。実 Twitch チャンネルでの受信確認は未実施。
- 2026-05-22: Side Panel のキュー上へコメント受信の開始/停止ボタンを追加し、`twitch_stop_chat` で認証解除せずに EventSub 接続だけ停止できるようにした。UI store では Twitch 認証状態とコメント受信接続状態を分離。`cargo test` と `pnpm build` は成功。
- Git 作業ツリーは調査開始時点で clean。
- `src-tauri/target` と `dist` がローカルに存在するため、ビルド済み成果物はある。
- `src/components/MainView.tsx` の Chat view は EventSub 由来のコメント表示に接続済み。未受信時のみサンプルメッセージを表示する。
- `src/stores/appStore.ts` は `twitch://status` / `twitch://chat-message` / `speech://status` の購読反映を実装済み。実キュー連携は Phase 4 で実装する。
- `src/tauri/client.ts` で `app://log`, `twitch://status`, `twitch://chat-message`, `speech://status`, `speech://queue-updated` を購読できる。
- `src-tauri/src/app_events/mod.rs` にイベント payload と `tauri::Emitter` helper を実装し、設定/認証/棒読みちゃん操作から発火する。
- `src-tauri/src/twitch/mod.rs` は認証、Helix ユーザー解決、EventSub WebSocket 接続、Helix subscription 作成まで実装済み。
