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
- 認可取り消しや401時はUIに再ログインを促す。
- access tokenとrefresh tokenはOS keyringに保存し、設定JSONには含めない。
- Linuxでkeyring保存に失敗した場合は `~/.rice/twitch-auth.json` へローカル保存する。ディレクトリは `0700`、ファイルは `0600` で作成し、読み込み時にも権限を補正する。ただし暗号化はしないため、OS keyringより安全性は下がる。
- Linux以外でkeyring保存に失敗した場合もログイン状態はメモリ上で継続し、UIへ「再起動後は再ログインが必要」と警告する。設定JSONへはフォールバックしない。
- 起動時はkeyringを優先してOAuth状態を復元し、Linuxではローカル保存ファイルも復元元にする。access tokenの検証に失敗した場合はrefresh tokenで更新を試み、成功時は保存済みrefresh tokenを即時差し替える。
- LinuxではSecret Service API対応ストアを優先する。Secret Serviceが利用できない環境では認証フロー自体は許可し、保存失敗時にローカル保存へ退避する。kernel keyutilsや設定JSONへは退避しない。

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
- `session_reconnect` を受けたら、指定された `reconnect_url` に接続し、新しいwelcomeを受けるまでは旧接続を維持する。
- 通知は少なくとも一回配送のため、`metadata.message_id` または `event.message_id` で重複排除する。
- WebSocket切断中の通知は再送されないため、再接続は指数バックオフしつつ最初の数回は短い間隔にする。

## 正規化

EventSubの `channel.chat.message` は本文とfragmentsを持つ。アプリ内部ではまず読み上げ向けの素朴な文字列に寄せる。

```text
event.message.text -> ChatMessage.text
event.message.fragments -> ChatMessage.fragments
event.chatter_user_name -> user_display_name
event.chatter_user_login -> user_login
event.badges -> badges
event.message_id -> id
```

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
