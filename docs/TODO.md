# 実装 TODO

最終調査日: 2026-05-26

この TODO は `docs/06-implementation-roadmap.md` の Phase に沿って、現在の実装状況と次に進める作業を追跡するためのものです。作業を始める前後に該当項目を更新してください。

調査メモは [`docs/RESEARCH_NOTES.md`](./RESEARCH_NOTES.md) に分離し、日付が新しいものほど上に追記してください。

## 現在の進捗サマリ

| Phase | 状態 | メモ |
| --- | --- | --- |
| Phase 0: プロジェクト作成 | 完了 | `app_events` の配信基盤と frontend 購読を接続し、`settings.json` の生成/読込を確認した。 |
| Phase 1: 棒読みちゃん連携 | 実装済み、自動検証済み、手動確認待ち | TCP 読み上げ、制御、接続診断、Settings 画面は実装済み。接続確認は設定に応じて確認読み上げまたは無音の状態取得を行う。`cargo test` と `pnpm build` は成功。実機の棒読みちゃんでの確認が必要。 |
| Phase 2: Twitch 認証 | 実装中 | Device Code Flow、`/validate`、refresh、keyring/ Linux fallback、Login 画面は実装済み。Client ID は UI/設定JSONに出さずビルド時既定値を使う。実 Twitch 環境での確認が必要。 |
| Phase 3: EventSub チャット受信 | 実装中 | WebSocket 接続、`channel.chat.message` 購読、正規化、重複排除、開始/停止 UI、フロントエンド反映を実装。実 Twitch 環境での手動確認が必要。 |
| Phase 4: 読み上げキュー統合 | 実装済み、自動検証済み、手動確認待ち | `SpeechFormatter`、FIFO `SpeechQueue`、EventSub チャットから棒読みちゃんへの自動読み上げ、Queue 画面を実装。`cargo test`、`pnpm test`、`pnpm build` は成功。実 Twitch + 棒読みちゃん環境での統合確認が必要。 |
| Phase 5: 配信運用向け仕上げ | 実装中 | Logs/Rules 画面、`app://log` 接続、ステータスバー集約、Login/Settings の設定整理、起動時自動接続、自動読み上げ ON/OFF、棒読みちゃんエラー後の復帰ポーリング、SidePanel の未完了キュー件数表示、各画面ヘッダー説明の日本語化、Chat view の仮想スクロール、Windows installer/portable zip のリリース生成、Client ID を渡す devcontainer/Docker ビルド経路、関連 TS テストを実装。詳細な運用エラー整理は継続。 |
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
- [x] 棒読みちゃん読み上げパケットを生成する。
- [x] 一時停止、再開、スキップ、クリアの制御コマンドを実装する。
- [x] 接続確認と接続診断を実装する。
- [x] 棒読みちゃん未起動時の日本語エラーを返す。
- [x] Settings 画面から接続確認、診断、テスト読み上げ、ホスト/ポート/声質設定を操作できるようにする。
- [x] 接続確認で空接続を送らず、「棒読みちゃんと接続しました」の確認読み上げを送る。
- [x] 接続成功時の読み上げ ON/OFF と読み上げ文カスタマイズを設定できるようにする。
- [ ] 実機の棒読みちゃんでテスト読み上げできることを確認する。
- [ ] 棒読みちゃん未起動、ポート競合、アプリ連携 OFF の手動確認を行う。
- [x] 接続失敗時に読み上げキューを破棄しない挙動を Phase 4 で統合確認する。

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
- [x] Login 画面に認証開始、確認、有効性確認、解除を実装する。
- [ ] 実 Twitch Client ID で Device Code Flow を手動確認する。
- [ ] 認可取り消し、401、期限切れ時の UI 表示を手動確認する。
- [ ] アプリ起動時の保存済み認証復元と refresh 更新を手動確認する。
- [x] Twitch ユーザー ID と接続チャンネルを EventSub 接続へ渡す command を実装する。

## Phase 3: EventSub チャット受信

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
- [x] Chat view にリアルタイムチャットを表示する。
- [x] Side Panel のキュー上にチャット受信の開始/停止ボタンを追加する。
- [x] Twitch 認証状態とチャット受信接続状態を UI store 上で分離する。
- [ ] 実 Twitch 環境で `channel.chat.message` 購読と Chat view 表示を手動確認する。

## Phase 4: 読み上げキュー統合

- [x] `SpeechFormatter` を実装する。
- [x] URL、改行、制御文字、長文、emote の扱いを `SpeechFormatter` に閉じ込める。
- [x] チャット由来の棒読みちゃんタグを初期設定で無効化またはエスケープする。
- [x] FIFO の `SpeechQueue` を実装する。
- [x] 最大件数 200、1 件のチャット最大 120 文字、ユーザー単位 2 秒の連投抑制を実装する。
- [x] キュー溢れ時に古い未読を落とし、UI に警告を出す。
- [x] 読み上げ失敗時に 1 回だけ短い遅延で再試行する。
- [x] チャット受信から `SpeechFormatter`、`SpeechQueue`、`BouyomiAdapter` への流れを接続する。
- [x] `speech://queue-updated` と `speech://status` events を実装する。
- [x] Queue view を実装し、スキップ、クリア、再読込、削除を操作できるようにする。
- [x] `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] TypeScript の store reducer テストを追加する。

## Phase 5: 配信運用向け仕上げ

- [x] Settings 画面から Login 画面を分離し、認証専用の画面として整理する。
- [x] Settings 画面へ読み上げ基本設定を集約し、Login/Rules 側に重複した読み上げ設定を残さない。
- [x] `v[0-9]*` タグ push で Windows NSIS ビルドと GitHub Release 作成を行う Actions workflow を追加する。
- [x] Windows リリースビルド用 Dockerfile と `.dockerignore` を追加する。
- [x] Windows リリースでインストーラーに加えて portable zip を作成する。
- [x] devcontainer に Docker outside Docker feature を追加し、手元でも Dockerfile 経由で Windows 成果物を作れるようにする。
- [x] 手元 Docker ビルドでは `.env` の `RICE_TWITCH_CLIENT_ID` を build arg として渡すラッパースクリプトを使う。
- [x] リリース workflow では build/release job を分離し、release job のみ `contents: write`、build cache は未使用にする。
- [x] Windows リリースビルドが Client ID 未設定で即失敗しないようにし、Windows 用 `icon.ico` を追加する。
- [x] Logs view を実装する。
- [x] `app://log` event をフロントエンドへ接続する。
- [x] EventSub、認証、読み上げアダプタのログを Logs view に表示する。
- [x] ステータスバーに Twitch 接続状態、棒読みちゃん状態、キュー件数、警告状態を集約する。
- [x] 起動時自動接続を実装する。
- [x] 自動読み上げ ON/OFF を実装する。
- [x] 棒読みちゃん接続エラー後、成功するまで接続確認をポーリングして状態を復帰する。
- [x] SidePanel の「待機中」件数を読み上げ未完了の項目数に揃える。
- [x] Rules view を実装する。
- [x] NG ユーザー、NG ワード、URL 処理、長文処理の設定を実装する。
- [x] 各画面のタイトル下説明を、画面機能が分かる日本語の概要文へ整理する。
- [x] Rules/Settings は設定変更時だけ保存ボタンを右下からスライドイン表示し、Login は認証操作ボタンと自動保存に分離する。
- [x] Auth の表示名を Login に変更する。
- [x] 「起動時にチャット受信を開始」を Settings 画面の先頭に移動する。
- [x] Voices 画面を Settings へ改名し、`/voices` から `/settings` へリダイレクトする。
- [x] 左ペインのチャンネル行から Login 画面へ移動できるようにする。
- [x] Twitch 文脈の「コメント」表記を「チャット」へ統一する。
- [x] 配信者向け文言を「読み上げ」へ統一する。
- [x] Chat view のチャットリストを仮想スクロール化する。
- [ ] 配信中に判断しやすい日本語エラー文言を整理する。
- [x] キュー行の状態表示テストを追加する。
- [x] 設定フォームのバリデーションテストを追加する。

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
- [x] Rust: `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] Rust: WebSocket 再接続状態遷移テストを追加する。
- [x] TypeScript: store reducer テストを追加する。
- [x] TypeScript: キュー行の状態表示テストを追加する。
- [x] TypeScript: 設定フォームのバリデーションテストを追加する。
- [ ] 手動: 棒読みちゃん未起動/起動中/ポート競合を確認する。
- [ ] 手動: Twitch トークン期限切れ/認可取り消しを確認する。
- [ ] 手動: 配信中チャット連投を確認する。
- [ ] 手動: ネットワーク切断と復帰を確認する。
