# Twitchチャット受信

## 採用方式

Twitch公式のEventSub WebSocketを採用する。TwitchのChat & Chatbotsドキュメントでは、IRCは歴史的インターフェースとして残っているが、現在の推奨はEventSubとTwitch API側に寄っている。

MultiCommentViewerはTwitch用ディレクトリとして `TwitchIF` と `TwitchSitePlugin` を持つ既存チャットビューア実装で、参考対象としては有用。ただしGPL-3.0のため、コードを流用せず、挙動やUI観察に留める。

## EventSub WebSocketの流れ

```text
1. OAuthでUser Access Tokenを取得
2. wss://eventsub.wss.twitch.tv/ws に接続
3. session_welcomeを受信し、session.idを取得
4. Helix Create EventSub Subscriptionで channel.chat.message を購読
5. session_keepalive / notification / reconnect / revocation を処理
6. notification.payload.event を ChatMessage に正規化
```

購読対象:

- `channel.chat.message`: 通常チャット本文
- `channel.chat.notification`: サブスク/ギフトなどのチャット上通知。MVP後に追加
- `channel.chat.message_delete`: 削除反映。MVP後に追加
- `channel.chat.clear`: 全消去反映。MVP後に追加

## 認証

Tauriのデスクトップアプリでは、クライアントシークレットを配布物に含めるべきではない。MVPではOAuth Device Code Flowを採用する。

必要スコープ:

- 読み取りのみ: `user:read:chat`
- 将来チャット投稿も入れる場合: `user:write:chat`

Device Code Flowの利点:

- public clientとして使える。
- クライアントシークレット不要。
- refresh tokenを取得できる。
- ユーザーはTwitchの認可ページでコード入力するだけでよい。

注意点:

- refresh tokenは使い回し不可の前提で、更新に成功したら保存済みrefresh tokenを必ず差し替える。
- `/validate` でトークン有効性を確認する。
- `/validate` の `scopes` に必須の `user:read:chat` が含まれることを、初回認証・保存済み認証の復元・refresh 後のすべてで確認する。不足時は `authRequired` の machine-readable な理由を `missingRequiredScope` とし、不足 scope 名と Login から再ログインして許可する手順を UI に表示する。scope 不足の認証状態では EventSub 接続 task を開始しない。
- 認可取り消しや401時はUIに再ログインを促す。
- access tokenとrefresh tokenはOS keyringに保存し、設定JSONには含めない。
- keyring保存に失敗した場合は OS を問わずログイン状態をメモリ上で継続する。ただし access token と refresh token の平文ファイルや設定JSONは作成せず、UIへ「今回の起動中だけ有効」「再起動後は再ログインが必要」と警告する。
- 旧版の Linux fallback `~/.rice/twitch-auth.json` を検出した場合は、keyringが利用できる時だけ移行して削除する。移行できない場合は安全のため token を読み込まず、ファイル削除、Twitch の「設定と接続」でのアクセス取り消し、再ログインを案内する。
- 起動時はkeyringを優先してOAuth状態を復元する。保存済み認証を復元しただけでは認証済みとして扱わず、Login画面の有効性確認と同じく `/validate` を実行する。access tokenの検証に失敗した場合はrefresh tokenで更新を試み、成功時は保存済みrefresh tokenを即時差し替えてから認証済み状態へ遷移する。確認の開始・成功・失敗は system チャットへ表示する。
- LinuxではSecret Service API対応ストアを優先する。Secret Serviceが利用できない環境でも認証フローは許可するが、永続化はしない。kernel keyutils、平文ローカルファイル、設定JSONへは退避しない。

Client ID:

- Client IDは秘匿情報ではないため、配布ビルドに既定値として含めてもよい。
- 既定のClient IDはビルド時に `RICE_TWITCH_CLIENT_ID` で指定する。互換用に `TWITCH_CLIENT_ID` も受け付ける。
- リポジトリ直下の `.env` に `RICE_TWITCH_CLIENT_ID=...` を置いた場合も、通常の Tauri ビルド時に同じ既定値として読み込む。テンプレートは `.env.example` を使う。
- Dockerfile 経由の Windows リリースビルドでは `.env` は Docker build context に含めない。手元では `scripts/build-windows-docker.sh` が `.env` を読み込み、`RICE_TWITCH_CLIENT_ID` を build arg として渡す。
- Client IDはユーザー設定JSONやUIへは出さず、OAuth開始時にアプリ内部のビルド時既定値を使う。既存設定JSONに古い `clientId` が残っていても無視する。
- Client Secretはデスクトップアプリへ含めない。

## WebSocket接続管理

Twitch EventSub WebSocketでは、最初に `session_welcome` が届き、そのsession IDをEventSub購読リクエストの `transport.session_id` に使う。

実装ポイント:

- 接続直後の購読は素早く行う。Twitchドキュメントではwelcome後の購読猶予が短い。
- `session_keepalive` が一定時間来ない場合は切断扱いにして再接続/再購読する。
- `session_reconnect` を受けたら、指定された `reconnect_url` に接続し、新しいwelcomeを受けるまでは旧接続を維持する。新しい接続またはwelcomeに失敗した場合も旧接続を25秒間処理し続け、その期限後に通常再接続へ戻す。
- 通常再接続で再購読する際は、接続開始時の token を保持せず、認証状態からその時点の access token を取得する。購読が 401 の場合だけ refresh token を用いて一度更新・安全な保存を行い、新しい access token で一度だけ再試行する。更新や再試行が失敗した場合は認証状態を解除し、UI に再ログインを案内する。
- EventSub API エラーは HTTP status と OAuth/API code を保持する型として扱う。timeout と 5xx は backoff 再接続の対象、401 は一度だけ refresh 後に再購読し、400/403/410 などの永続障害は再接続せず Error または AuthRequired を UI へ出す。revocation は authorization_revoked を再ログイン、user_removed をチャンネル確認、version_removed をアプリ更新として扱い、日本語メッセージの部分一致で制御フローを決めない。
- 通知は少なくとも一回配送のため、`metadata.message_id` または `event.message_id` で重複排除する。
- WebSocket切断中の通知は再送されないため、再接続は指数バックオフしつつ最初の数回は短い間隔にする。
- backoff は通常接続では `session_welcome` と購読成功後、Twitch 指定の handover では新しい `session_welcome` 後に「確立済み」と記録する。handover 開始時には旧 session の確立時刻を失効させ、新しい welcome 前の接続失敗で旧 session の安定実績を使って reset しない。ただし確立直後の切断で待機時間が毎回最短に戻る retry storm を避けるため、30 秒以上安定していた session の次の障害時にだけ失敗回数を reset する。確立前・30 秒未満の失敗は従来の 2 / 5 / 10 / 30 秒バックオフを継続する。

## 正規化

EventSubの `channel.chat.message` は本文とfragmentsを持つ。アプリ内部ではまず読み上げ向けの素朴な文字列に寄せる。

```text
event.message.text -> ChatMessage.text
event.message.fragments -> ChatMessage.fragments
event.chatter_user_name -> user_display_name
event.chatter_user_login -> user_login
event.badges -> badges
event.message_id -> id
metadata.message_timestamp -> received_at (DateTime<Utc>)
```

`metadata.message_timestamp` は RFC 3339 として解析し、明示された offset を同じ instant の UTC へ正規化する。小数秒は nanosecond 精度まで保持し、Tauri event の `receivedAt` は末尾 `Z` の UTC RFC 3339 とする。timestamp が欠落、空文字、タイムゾーンなし、非文字列を含む不正値の場合も chat notification 自体は破棄せず、その WebSocket frame を取り出した時刻を使って warning log を残す。RFC 3339 の leap second は Chrono が受理しても JavaScript の `Date` / `Intl` が表現できないため、backend で同じ fallback と warning を適用する。frontend は bridge で同じ形式を検証してから store へ渡し、Chat view でのみ利用者のローカルタイムゾーンへ変換して表示する。

読み上げ本文の初期フォーマット:

```text
{display_name}。{message_text}
```

設定で以下を切り替える:

- ユーザー名を読む/読まない
- 初回チャットだけユーザー名を読む
- URLを「URL」と読む
- emoteだけのチャットを読む/読まない
- 長文の最大文字数
- 連投抑制秒数
- NGユーザー/NGワード

## IRCフォールバック

EventSubが使えない状況に備えて、後でIRCフォールバックを追加できるよう `TwitchChatSource` traitを切る。

```rust
#[async_trait::async_trait]
pub trait TwitchChatSource {
    async fn connect(&self, channel: &str) -> anyhow::Result<()>;
    async fn next_message(&mut self) -> anyhow::Result<ChatMessage>;
    async fn disconnect(&self) -> anyhow::Result<()>;
}
```

MVPでは `EventSubChatSource` のみ実装する。IRCは匿名読み取りや旧実装互換の魅力はあるが、公式の構造化イベント、認可、将来性の面ではEventSubを優先する。

## 参照元

- EventSub WebSocket: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events>
- Chat認証とEventSub設定: <https://dev.twitch.tv/docs/chat/authenticating/>
- `channel.chat.message`: <https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/#channelchatmessage>
- OAuth: <https://dev.twitch.tv/docs/authentication/getting-tokens-oauth>
- MultiCommentViewer: <https://github.com/CommentViewerCollection/MultiCommentViewer>
