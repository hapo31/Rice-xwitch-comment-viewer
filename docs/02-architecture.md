# 全体アーキテクチャ

## レイヤ構成

```text
src/                      TypeScript UI
  app shell               VSCode風レイアウトとルーティング
  features                Chat/Queue/Launcher等の画面単位UI
  presentation            表示用の純粋関数
  stores                  チャット/キュー/接続状態
  tauri client            Rust commands/eventsの呼び出し

src-tauri/
  twitch                  OAuth、EventSub WebSocket、Helix API
  speech                  読み上げキュー、アダプタ共通trait
  speech/bouyomi          棒読みちゃんTCPクライアント
  speech/voiceroid        実験的VOICEROID2直接連携
  launcher                アプリ登録、検証、起動
  settings                永続設定、トークン保存
  app_events              フロントエンドへのイベント配信
```

## データフロー

```text
Twitch EventSub WebSocket
  -> TwitchEvent
  -> ChatMessage正規化
  -> Filter/Formatter
  -> SpeechQueue
  -> SpeechAdapter
  -> BouyomiChan TCP / VOICEROID2 adapter

Rust backend
  -> tauri::Emitter events
  -> TypeScript stores
  -> UI
```

## Rust側の主要コンポーネント

| コンポーネント | 責務 |
| --- | --- |
| `TwitchAuthService` | Device Code Flow、トークン更新、`/validate`、ユーザーID取得 |
| `EventSubClient` | WebSocket接続、welcome/keepalive/reconnect/revocation処理 |
| `TwitchChatService` | `channel.chat.message`購読、イベント重複排除、チャット正規化 |
| `SpeechQueue` | 優先度、停止/再開/スキップ、連投抑制、バックプレッシャ |
| `SpeechFormatter` | 読み上げ文生成、ユーザー名付与、絵文字/URL/長文処理 |
| `SpeechAdapter` | 読み上げ先を抽象化するtrait |
| `BouyomiAdapter` | 棒読みちゃんTCPプロトコル実装 |
| `VoiceroidAdapter` | Windows専用の実験的アダプタ。C# sidecarまたはUI Automationを隠蔽する |
| `SettingsStore` | 一般設定JSONの保存。OAuthトークンは扱わない |
| `TwitchAuthStore` | Twitch OAuth状態をOS keyringへ保存/復元/削除する |
| `LauncherService` | 登録アプリのパス検証、重複排除、単体/一斉起動を扱う |

## SpeechAdapter trait案

```rust
#[async_trait::async_trait]
pub trait SpeechAdapter: Send + Sync {
    async fn health_check(&self) -> anyhow::Result<SpeechHealth>;
    async fn speak(&self, request: SpeechRequest) -> anyhow::Result<SpeechResult>;
    async fn pause(&self) -> anyhow::Result<()>;
    async fn resume(&self) -> anyhow::Result<()>;
    async fn skip(&self) -> anyhow::Result<()>;
    async fn clear(&self) -> anyhow::Result<()>;
}
```

`BouyomiAdapter` と `VoiceroidAdapter` はこのtraitだけを実装する。UIやキューは具体的な読み上げ先を知らない。

## ドメインモデル案

```rust
pub struct ChatMessage {
    pub id: String,
    pub platform: Platform,
    pub channel_id: String,
    pub channel_login: String,
    pub user_id: String,
    pub user_login: String,
    pub user_display_name: String,
    pub text: String,
    pub fragments: Vec<MessageFragment>,
    pub badges: Vec<Badge>,
    pub received_at: chrono::DateTime<chrono::Utc>,
}

pub struct SpeechRequest {
    pub id: uuid::Uuid,
    pub source_message_id: Option<String>,
    pub text: String,
    pub voice: Option<String>,
    pub speed: Option<i16>,
    pub tone: Option<i16>,
    pub volume: Option<i16>,
}
```

## Tauri command/event案

Commands:

- `twitch_start_auth()`
- `twitch_connect(channel_login: String)`
- `twitch_disconnect()`
- `speech_set_adapter(adapter: SpeechAdapterKind)`
- `speech_test(text: String)`
- `speech_pause()`
- `speech_resume()`
- `speech_skip()`
- `speech_clear()`
- `settings_get()`
- `settings_update(patch: SettingsPatch)`
- `launcher_add(paths: Vec<String>)`
- `launcher_remove(item_id: String)`
- `launcher_launch(item_id: String)`
- `launcher_launch_all()`
- `app_open_external_url(url: String)`: Twitch認証URLなど、許可した外部URLをOS既定ブラウザで開く。

Events:

- `twitch://status`
- `twitch://chat-message`
- `speech://queue-updated`
- `speech://status`
- `app://log`

## 永続化

- 一般設定: Tauriのapp data配下にJSON保存。
- ランチャー項目: 一般設定の `launcher.items` に保存する。`kind`, `target`, `displayName`, `order` と、将来用の `backgroundColor`, `groupId`, `iconDataUrl` を境界として持つ。
- Twitch OAuth状態: access token、refresh token、スコープ、有効期限、検証済みプロフィールをOS keyringへ保存する。設定JSONへは保存しない。
- refresh token: 更新成功時に保存済みの値を新しい値へ差し替える。keyring保存に失敗した場合もログイン状態はメモリ上で継続する。Linuxでは `~/.rice/twitch-auth.json` にローカル保存し、それ以外ではUIへ永続化不可の警告を出す。設定JSONへはフォールバックしない。
- LinuxではWindows Credential ManagerやmacOS Keychainに相当する単一の標準ストアがないため、Secret Service API対応ストア（GNOME Keyring、KWallet、KeePassXC Secret Serviceなど）を優先する。利用できない場合は `~/.rice` を `0700`、認証ファイルを `0600` で作成して保存する。kernel keyutilsやmock backendは永続OAuth保存には使わない。
- チャットログ: 初期MVPではメモリのみ。後でSQLiteを追加できる境界を残す。

## 推奨crate

- Tauri: `tauri`
- async runtime: `tokio`
- HTTP: `reqwest`
- WebSocket: `tokio-tungstenite`
- JSON: `serde`, `serde_json`
- error: `anyhow`, `thiserror`
- config path: `directories` またはTauri API
- keyring: `keyring`
- Windows拡張: `windows` crate
