# 実装ロードマップ

## Phase 0: プロジェクト作成

- Tauri + TypeScript + Tailwindの雛形を作る。
- Rust側に `twitch`, `speech`, `settings` モジュールを作る。
- UIはActivity Bar、Side Panel、Status Barだけ先に固定する。

完了条件:

- アプリが起動し、空のChat viewを表示できる。
- 設定JSONを読み書きできる。

## Phase 1: 棒読みちゃん連携

- `BouyomiAdapter` を実装する。
- 接続チェック、テスト発話、一時停止/再開/スキップ/クリアを実装する。
- UIのVoices画面からテスト発話できるようにする。

完了条件:

- 棒読みちゃん起動中にテスト発話できる。
- 未起動時にUIへ分かるエラーが出る。

## Phase 2: Twitch認証

- Device Code Flowを実装する。
- `user:read:chat` スコープでUser Access Tokenを取得する。
- refresh token更新と `/validate` を実装する。
- OAuth状態をOS keyringへ保存し、起動時に復元する。
- keyringへ保存できない場合は平文JSONへフォールバックせず、ログインは継続しつつ再起動後に再ログインが必要な状態としてUIに出す。
- LinuxではSecret Service API対応ストアが利用できない場合も認証フローは止めず、保存失敗時に警告する。
- ユーザーIDとログイン名を取得し、設定画面に表示する。

完了条件:

- Twitchアカウントを接続/解除できる。
- アプリ再起動後も安全に再認証またはトークン更新できる。

## Phase 3: EventSubコメント受信

- EventSub WebSocket接続を実装する。
- `channel.chat.message` 購読を作る。
- 通知を `ChatMessage` に正規化する。
- Chat viewにリアルタイム表示する。

完了条件:

- 指定チャンネルのコメントがアプリに表示される。
- 切断/再接続/再購読が最低限動く。

## Phase 4: 読み上げキュー統合

- コメントを `SpeechFormatter` に通す。
- `SpeechQueue` で順番に `BouyomiAdapter` へ送る。
- UIでキュー状態、スキップ、クリアを操作できる。

完了条件:

- Twitchコメントが棒読みちゃんで自動読み上げされる。
- 長文、URL、NGワード、連投の基本ルールが効く。

## Phase 5: 配信運用向け仕上げ

- ログ画面を追加する。
- ステータスバーを充実させる。
- 起動時自動接続、自動読み上げON/OFFを追加する。
- エラー文言を配信中に判断しやすい日本語にする。

完了条件:

- MultiCommentViewer + 棒読みちゃんの代替として日常利用を試せる。

## Phase 6: VOICEROID2実験アダプタ

- C# sidecarのPoCを作る。
- Rustからstdio JSON-RPCで `speak`, `stop`, `health` を呼ぶ。
- VOICEROID2のバージョン/bitness/起動状態を診断する。
- 失敗時は棒読みちゃんアダプタへ戻せるUIにする。

完了条件:

- 一部環境でVOICEROID2に直接発話できる。
- 失敗してもTauri本体や棒読みちゃん連携に影響しない。

## テスト方針

Rust:

- 棒読みちゃんパケット生成のユニットテスト
- `channel.chat.message` JSON fixtureのパーステスト
- `SpeechFormatter` のNG/URL/長文処理テスト
- WebSocket再接続状態遷移のユニットテスト

TypeScript:

- store reducerのテスト
- コメント行の表示状態テスト
- 設定フォームのバリデーションテスト

手動:

- 棒読みちゃん未起動/起動中/ポート競合
- Twitchトークン期限切れ/認可取り消し
- 配信中コメント連投
- ネットワーク切断と復帰

## リスク

| リスク | 対策 |
| --- | --- |
| Twitch API仕様変更 | 公式EventSubに寄せる。購読失敗理由をログに出す。 |
| WebSocket切断中のコメント欠落 | 仕様上完全防止は不可。素早い再接続とUI警告で対応する。 |
| 棒読みちゃんポート競合 | 接続診断とポート設定をUIに出す。 |
| コメント由来の棒読みちゃんタグ悪用 | 初期設定ではタグをエスケープする。 |
| VOICEROID2直接連携の不安定さ | MVPから外し、adapter/sidecar境界で隔離する。 |
