# AGENTS.md

このリポジトリで作業するエージェント向けのガイドです。実装や設計判断を行う前に、必要であれば `docs/` 配下の設計メモを確認してください。

## プロジェクト概要

このプロジェクトは、Twitch のリアルタイムコメント取得、コメント表示、読み上げキュー管理、棒読みちゃん/VOICEROID2 系連携をひとつにまとめる Tauri デスクトップアプリです。

MVP の中核は次の 2 点です。

1. Twitch EventSub WebSocket でコメントを受信する。
2. 棒読みちゃんの TCP アプリ連携で読み上げる。

VOICEROID2 直接連携は MVP の主経路にしません。Windows 専用の実験的アダプタとして、棒読みちゃん連携やアプリ本体から分離して追加できる形にしてください。

## 参照すべき設計文書

- `docs/README.md`: 全体方針と結論
- `docs/01-requirements-and-decisions.md`: 要求、採用判断、非目標、重要制約
- `docs/02-architecture.md`: レイヤ構成、データフロー、主要コンポーネント、Tauri commands/events
- `docs/03-twitch-ingestion.md`: Twitch EventSub WebSocket、OAuth、再接続、正規化
- `docs/04-speech-output.md`: 棒読みちゃん TCP、SpeechAdapter、VOICEROID2 隔離方針、読み上げキュー
- `docs/05-ui-ux.md`: VSCode 風 UI、画面構成、Tailwind 指針
- `docs/06-implementation-roadmap.md`: 実装フェーズ、完了条件、テスト方針、リスク
- `docs/TODO.md`: Phase 別の実装進捗、未完了タスク、手動確認項目

設計文書と実装が矛盾する場合は、先に矛盾を明示し、必要なら設計文書も更新してください。

## 技術スタックと構成方針

- ランタイム: Tauri
- フロントエンド: 最新 TypeScript
- バックエンド: Rust
- UI: Tailwind CSS
- 主対象 OS: Windows
- UI 方針: VSCode 風の左縦ナビゲーション、作業ペイン、下部ステータスバー

想定構成:

```text
src/                      TypeScript UI
  app shell               VSCode風レイアウト、設定、ログ、キュー表示
  stores                  コメント/キュー/接続状態
  tauri client            Rust commands/eventsの呼び出し

src-tauri/
  twitch                  OAuth、EventSub WebSocket、Helix API
  speech                  読み上げキュー、アダプタ共通trait
  speech/bouyomi          棒読みちゃんTCPクライアント
  speech/voiceroid        実験的VOICEROID2直接連携
  settings                永続設定、トークン保存
  app_events              フロントエンドへのイベント配信
```

## 実装優先順位

`docs/06-implementation-roadmap.md` の Phase 順に進めます。

1. Tauri + TypeScript + Tailwind の雛形、基本レイアウト、設定 JSON
2. 棒読みちゃん連携
3. Twitch Device Code Flow 認証
4. EventSub コメント受信
5. 読み上げキュー統合
6. 配信運用向け仕上げ
7. VOICEROID2 実験アダプタ

先の Phase のために境界は残してよいですが、MVP 前に VOICEROID2 直接連携や複数配信サイト対応へ広げないでください。

## Twitch 実装ルール

- 新規実装では EventSub WebSocket を主経路にします。
- IRC は将来のフォールバックとして境界を残すだけにし、MVP では `EventSubChatSource` のみを実装します。
- EventSub WebSocket 購読には User Access Token を使います。App Access Token を WebSocket 購読に使う設計にしないでください。
- デスクトップアプリにクライアントシークレットを埋め込まないでください。OAuth Device Code Flow を使います。
- 必要スコープは読み取りのみなら `user:read:chat` です。投稿機能は MVP に含めません。
- refresh token は更新成功時に必ず保存済みの値を差し替えます。
- `/validate` でトークン有効性を確認し、401 や認可取り消しは UI に再ログインとして出してください。
- `session_welcome` 受信後は速やかに `channel.chat.message` を購読してください。
- `session_keepalive` 欠落、`session_reconnect`、`revocation` を状態として扱い、UI とログへ反映してください。
- WebSocket 切断中のイベントは再送されない前提で、速い再接続/再購読と UI 警告で対応します。
- EventSub 通知は少なくとも一回配送です。`metadata.message_id` または `event.message_id` で重複排除してください。
- MultiCommentViewer は挙動や UI 観察の参考に留め、GPL-3.0 のコードを流用しないでください。

## 読み上げ実装ルール

- MVP の正式アダプタは棒読みちゃん TCP です。
- 既定接続先は `127.0.0.1:50001` です。ポートは設定で変更可能にしてください。
- 発話ごとに短い TCP 接続を張る設計から始めます。
- 接続失敗時に読み上げキューを安易に破棄しないでください。状態を `Disconnected` や `Error` として UI に出します。
- 棒読みちゃん未起動、ポート競合、アプリ連携 OFF はユーザーが判断できる日本語エラーにしてください。
- 長文、URL、改行、制御文字、emote の扱いは `SpeechFormatter` に閉じ込めてください。
- コメント由来の棒読みちゃんタグは初期設定では無効化またはエスケープしてください。
- `SpeechAdapter` trait を境界にして、UI やキューが具体的な読み上げ先を知らない構造にしてください。
- VOICEROID2 直接連携は C# sidecar や UI Automation の不安定さを本体へ漏らさず、Windows 専用の実験的アダプタとして隔離してください。

読み上げキューの初期仕様:

- FIFO
- 最大件数: 200
- 1 コメント最大文字数: 120 文字
- ユーザー単位の連投抑制: 2 秒
- キュー溢れ時: 古い未読を落とし、UI に警告
- 読み上げ失敗時: 1 回だけ短い遅延で再試行

## UI/UX ルール

- 配信中に横目で状態確認し、数クリックで復旧できるツールとして設計します。
- 派手なダッシュボードではなく、VSCode に近い静かなツール UI にしてください。
- 基本構成は Activity Bar、Side Panel、Main View、Status Bar です。
- 初期画面は Chat view にします。
- Activity Bar は 48px 程度の縦ナビゲーションで、アイコン中心、hover tooltip ありにしてください。
- 画面は Chat、Queue、Rules、Voices、Settings、Logs を想定します。
- カードを乱用せず、パネルと行ベースで構成してください。
- ボタンはアイコンボタン中心にし、主要操作だけテキスト付きにします。
- 接続状態はステータスバーと小さなドットで表現してください。
- コメント行は高さを安定させ、長文は折りたたみまたは 2 行までにしてください。
- 「クリア」「切断」など配信中に誤操作しやすい操作は確認を挟んでください。

Tailwind の基本配色:

- 背景: `zinc-950`, `zinc-900`
- パネル: `zinc-900`, `zinc-850` 相当のカスタム
- 境界線: `zinc-800`
- テキスト: `zinc-100`, `zinc-400`
- アクセント: `sky-400`, `emerald-400`
- エラー: `rose-400`
- 警告: `amber-400`

## データと永続化

- 一般設定は Tauri の app data 配下に JSON 保存します。
- Twitch refresh token は OS keyring を優先してください。
- refresh token を平文 JSON に保存しないでください。難しい場合も暗号化またはユーザー明示の保存設定を設けます。
- コメントログは初期 MVP ではメモリのみとし、後で SQLite を追加できる境界を残してください。

## Rust 実装指針

- 非同期処理は `tokio` を前提にします。
- HTTP は `reqwest`、WebSocket は `tokio-tungstenite` を基本候補にします。
- JSON は `serde`/`serde_json` を使います。
- エラーは境界に応じて `anyhow` と `thiserror` を使い分けてください。
- Tauri commands/events は `docs/02-architecture.md` の案を起点にします。
- フロントエンドへは `tauri::Emitter` events で接続状態、コメント、キュー、ログを流します。
- 外部サービスや外部アプリ連携はヘルスチェックを実装し、状態を UI へ返してください。

## TypeScript 実装指針

- コメント、読み上げキュー、接続状態は store に分離してください。
- Rust commands/events との境界は薄い client 層にまとめます。
- 表示状態は `queued`, `spoken`, `skipped`, `blocked`, `error` を区別してください。
- 設定フォームにはバリデーションを入れ、危険な操作や無効なポートをその場で検出してください。
- アイコンライブラリが未導入なら、UI 方針に沿って `lucide-react` を候補にしてください。

## テスト方針

Rust では少なくとも次をテストしてください。

- 棒読みちゃんパケット生成
- `channel.chat.message` JSON fixture のパース
- `SpeechFormatter` の NG/URL/長文処理
- WebSocket 再接続状態遷移

TypeScript では少なくとも次をテストしてください。

- store reducer
- コメント行の表示状態
- 設定フォームのバリデーション

手動確認では次を重点的に見てください。

- 棒読みちゃん未起動/起動中/ポート競合
- Twitch トークン期限切れ/認可取り消し
- 配信中コメント連投
- ネットワーク切断と復帰

## 作業時の注意

- 設計の主軸から外れる機能追加は、先に `docs/` の方針との整合性を確認してください。
- 作業開始時に `docs/TODO.md` の該当 Phase と未完了項目を確認し、作業内容に対応する TODO がなければ追加してください。
- 作業中にスコープや完了条件が変わった場合は、実装と同じ変更単位で `docs/TODO.md` を更新してください。
- 作業完了時は `docs/TODO.md` の該当チェックボックス、進捗サマリ、調査メモを現在の状態に合わせて更新してください。
- 今後の実装作業では、`docs/06-implementation-roadmap.md` の Phase 単位、またはそれに準じる明確な作業フェーズごとにコミットを作成してください。
- 修正作業完了後は、関連する変更をステージングし、コミットメッセージは直近コミットの形式（`type: message`）に合わせて作成してください。例: `chore: add codex cli setup`
- Twitch API や Tauri 周辺は仕様変更があり得るため、実装時は公式ドキュメントを確認してください。
- ライセンスが不明な外部実装をコピーしないでください。特に MultiCommentViewer は GPL-3.0 のため、参考に留めます。
- 配信中の運用を意識し、エラーは原因と復旧手順が分かる短い日本語にしてください。
- VOICEROID2 のライセンス制約を迂回する実装は行わないでください。
