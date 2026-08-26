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
| `SettingsStore` | 一般設定JSONを原子的に保存し、JSON構文または検証対象の設定値が不正な場合はbackupまたは既定値へ復旧する。更新は候補を保存できた場合だけ共有メモリへ反映する。OAuthトークンは扱わない |
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

`ChatMessage.received_at` はアプリ内部で常に `DateTime<Utc>` とする。Tauri event では serde の camelCase 規約により `receivedAt` として、UTC の RFC 3339（末尾 `Z`、小数秒は nanosecond 精度まで保持）を送る。frontend は bridge 受信時にこの契約を検証し、`UtcTimestamp` として store へ渡す。欠落・空文字・タイムゾーンなし・非文字列を含む不正値、および JavaScript の `Date` / `Intl` が表現できない leap second は backend で WebSocket frame を取り出した時刻へフォールバックして warning log を残し、frontend の境界でも受信時刻を使って防御する。Chat view は保存値を変えず利用者のローカルタイムゾーンで表示し、表示不能な値では `--:--:--` を表示する。

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
- `app://log`: `id` は Logs view の React key に使う表示用 ID として一意にする。受信時に ID が欠ける、または既存 ID と重複する場合は、frontend store が連番 suffix を付ける。ログ本文の重複排除は行わない。

## Renderer のセキュリティ境界

production の bundled window は `default-src 'self'` を起点とする CSP を使う。script は bundled asset と Tauri が build 時に付与する hash / nonce、通信は Tauri IPC の `ipc:` / `http://ipc.localhost`、画像は bundled asset と検証済みの PNG data URL だけを許可する。frame、object、worker、media、base、form は使用しないため拒否する。Twitch HTTP / WebSocket と棒読みちゃん TCP は Rust 側で処理し、renderer の `connect-src` へ外部 origin を追加しない。

React の仮想スクロール、ウィンドウ倍率、Launcher tile は動的な style 属性を使うため、`style-src-attr 'unsafe-inline'` だけを例外とする。script の inline handler は `script-src-attr 'none'` で拒否する。Vite dev server / HMR は production の許可元へ含めず、development policy だけに `ws://localhost:1420` と Vite の style injection 用 `style-src 'unsafe-inline'` を明示する。Tauri は `devCsp` が `null` または未指定だと production `csp` へ fallback するため、開発時 policy を省略しない。

capability は `main` window の `default` だけを設定から明示的に有効化する。core API は event の listen/unlisten、現在の window の状態確認・移動・resize・native close 完了、Dialog の open に限定する。custom command は `tauri_build::AppManifest` へ列挙し、同じ main capability に明示した command だけを許可する。新しい window / capability / command を追加するときは、既存の default set を広げず、その利用箇所と permission を同じ変更で追加する。CSP や capability は backend の入力検証を代替しないため、外部 URL、Launcher path、設定値の Rust 側検証は維持する。

Launcher の `iconDataUrl` は backend で `data:image/png;base64,`、encoded/decoded payload 上限、base64 decode、PNG の chunk 構造・checksum・終端、単一 frame、最大 512 x 512 px を検証し、不正値は表示モデルへ渡さない。`assetProtocol` は有効化せず、任意ファイル path や remote image を renderer から読める境界は設けない。

判断根拠は Tauri v2 公式の [Content Security Policy](https://v2.tauri.app/security/csp/)、[Capabilities](https://v2.tauri.app/security/capabilities/)、[configuration schema](https://v2.tauri.app/reference/config/#securityconfig) に従う。

### フロントエンド通知

対処が必要な通知は `{ id, severity, source, message, occurredAtMs, correlationId? }` として保持する。`severity` は `info` / `success` / `warning` / `error`、`source` は command / event / log / system を区別する。Side Panel と Status Bar の Warnings は warning / error のみを最新 5 件まで表示するため、成功通知で実警告を押し出さない。`correlationId` がある通知はその値で重複排除し、ID がない既存イベントは本文と 5 秒の受信時間で重複排除する。重複経路で severity が異なるときは、より重大な値を残す。info / success は Logs と system Chat に残す。

## 永続化

- 一般設定: Tauriのapp data配下にJSON保存。同一ディレクトリの一時ファイルへ書き込み・`sync_all` した後、OSごとの atomic replace で `settings.json` を更新する。直前の正常版は `settings.json.bak` 1世代だけ保持する。
- ウィンドウ位置: `settings.json` の `window.position` に物理ピクセル座標を保存する。終了要求時とアプリ内の終了操作で保存し、次回起動時は現在のいずれかのモニター作業領域にタイトルバー相当（64 x 32px）以上が残る位置だけを復元する。モニター構成の変更で画面外になる位置は復元せず、初期の中央配置を使う。
- 設定復旧: 起動時に本体のJSON構文または検証対象の設定値が不正なら backup を同じ契約で検証して復旧する。backup も不正または不在なら、無効なファイルを `settings.json.corrupt-<timestamp>-<suffix>` として退避して既定値で起動する。復旧理由・内容・退避先は Logs、system Chat、警告通知に日本語で表示する。
- ランチャー項目: 一般設定の `launcher.items` に保存する。`kind`, `target`, `displayName`, `order` と、将来用の `backgroundColor`, `groupId`, `iconDataUrl` を境界として持つ。
- Twitch OAuth状態: access token、refresh token、スコープ、有効期限、検証済みプロフィールをOS keyringへ保存する。設定JSONへは保存しない。
- refresh token: 更新成功時に保存済みの値を新しい値へ差し替える。keyring保存に失敗した場合もログイン状態はメモリ上で継続するが、token はディスクへ保存しない。UIには session-only であることと、再起動後に再ログインが必要なことを表示する。
- 旧版が Linux に作成した `~/.rice/twitch-auth.json` は互換性のため検出する。OS keyringへ移行できた場合だけ削除し、移行できない場合は安全のため読み込まず、削除・Twitch 側のアクセス取り消し・再ログインを案内する。新たな平文ファイルは作成しない。Linuxでは Secret Service API対応ストア（GNOME Keyring、KWallet、KeePassXC Secret Serviceなど）を優先する。kernel keyutilsやmock backendは永続OAuth保存には使わない。
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
