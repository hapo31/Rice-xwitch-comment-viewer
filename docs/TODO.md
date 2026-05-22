# 実装 TODO

最終調査日: 2026-05-22

この TODO は `docs/06-implementation-roadmap.md` の Phase に沿って、現在の実装状況と次に進める作業を追跡するためのものです。作業を始める前後に該当項目を更新してください。

## 現在の進捗サマリ

| Phase | 状態 | メモ |
| --- | --- | --- |
| Phase 0: プロジェクト作成 | ほぼ完了 | Tauri + TypeScript + Tailwind、VSCode 風レイアウト、設定 JSON は実装済み。 |
| Phase 1: 棒読みちゃん連携 | 実装済み、手動確認待ち | TCP 発話、制御、接続診断、Voices 画面は実装済み。実機の棒読みちゃんでの確認が必要。 |
| Phase 2: Twitch 認証 | 実装中 | Device Code Flow、`/validate`、refresh、keyring/ Linux fallback、Settings 画面は実装済み。実 Twitch 環境での確認が必要。 |
| Phase 3: EventSub コメント受信 | 未着手 | WebSocket 接続、購読、イベント正規化、再接続、UI 反映が未実装。 |
| Phase 4: 読み上げキュー統合 | 未着手 | store 上のキュー枠はあるが、Rust 側の `SpeechQueue` / `SpeechFormatter` と自動読み上げは未実装。 |
| Phase 5: 配信運用向け仕上げ | 一部のみ | ステータスバーと警告表示はあるが、Logs 画面、自動接続、自動読み上げ、詳細な運用エラー整理は未実装。 |
| Phase 6: VOICEROID2 実験アダプタ | 未着手 | MVP 後に Windows 専用の実験アダプタとして追加する。 |

## Phase 0: プロジェクト作成

- [x] Tauri + TypeScript + Tailwind の雛形を作る。
- [x] `src-tauri/src` に `twitch`, `speech`, `settings`, `app_events` の境界を作る。
- [x] Activity Bar、Side Panel、Main View、Status Bar の基本レイアウトを作る。
- [x] 独自 Title Bar、ウィンドウ操作、リサイズハンドルを作る。
- [x] UI 倍率の自動/手動切替を作る。
- [x] 一般設定を Tauri app data 配下の `settings.json` に保存する。
- [ ] `app_events` からフロントエンドへ流すイベント設計を実装に接続する。
- [ ] Phase 0 完了条件として、Tauri アプリ起動と設定 JSON 読み書きを手動確認する。

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

- [x] Twitch Client ID を `.env` / build env から既定値として読み込む。
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
- [ ] Twitch ユーザー ID と接続チャンネルを EventSub 接続へ渡す command を実装する。

## Phase 3: EventSub コメント受信

- [ ] `tokio-tungstenite` を導入する。
- [ ] `EventSubClient` を作り、`wss://eventsub.wss.twitch.tv/ws` へ接続する。
- [ ] `session_welcome` 受信後に `channel.chat.message` を購読する。
- [ ] EventSub 購読に User Access Token を使う。
- [ ] `session_keepalive` 欠落を検出して状態とログへ出す。
- [ ] `session_reconnect` を処理する。
- [ ] `revocation` を処理し、UI に再ログインまたは再接続が必要な状態を出す。
- [ ] `metadata.message_id` または `event.message_id` で重複排除する。
- [ ] `channel.chat.message` JSON fixture のパーステストを追加する。
- [ ] `ChatMessage` に fragments / badges / received_at を含める。
- [ ] `tauri::Emitter` events で `twitch://status` と `twitch://chat-message` を送る。
- [ ] TypeScript client で Twitch events を購読し、store へ反映する。
- [ ] Chat view にリアルタイムコメントを表示する。

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
- [ ] Rust: `channel.chat.message` JSON fixture のパーステストを追加する。
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

- Git 作業ツリーは調査開始時点で clean。
- `src-tauri/target` と `dist` がローカルに存在するため、ビルド済み成果物はある。
- `src/components/MainView.tsx` の Chat view は現在サンプルメッセージ表示で、EventSub 由来のコメント表示には未接続。
- `src/stores/appStore.ts` に chat / queue state はあるが、Rust events から更新する購読処理は未実装。
- `src-tauri/src/app_events/mod.rs` は型定義のみで、`tauri::Emitter` への接続は未実装。
- `src-tauri/src/twitch/mod.rs` は認証中心で、EventSub WebSocket client と Helix subscription 作成は未実装。
