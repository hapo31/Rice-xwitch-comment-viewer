# 要求と採用方針

## 目的

Twitch実況配信中に、コメント受信、コメント表示、読み上げキュー管理、棒読みちゃん/VOICEROID2系連携をひとつのTauriアプリにまとめる。

既存構成:

```text
Twitch chat -> MultiCommentViewer -> bouyomichan -> VOICEROID2
```

目標構成:

```text
Twitch chat -> Tauri app -> Speech adapter -> bouyomichan or VOICEROID2
```

## 機能要求

- Twitchのコメントをリアルタイムに受信する。
- コメント一覧、読み上げキュー、接続状態をUIで確認できる。
- 読み上げ対象をフィルタ/整形できる。
- 棒読みちゃんへプロセス間通信またはTCPで発話を送れる。
- 将来的にVOICEROID2直接操作アダプタを追加できる。
- 配信中でも操作しやすい、落ち着いたツールUIにする。

## 技術前提

- ランタイム: Tauri
- フロントエンド: 最新TypeScript
- バックエンド: Rust
- UI: Tailwind CSS
- 対象OS: 主にWindows
- デザイン: VSCode風の左縦ナビゲーションバー

## 採用判断

| 領域 | 採用 | 理由 |
| --- | --- | --- |
| Twitch受信 | EventSub WebSocket | Twitch公式がローカル常駐アプリに推奨している。チャット本文、バッジ、メッセージID、通知系イベントを構造化JSONで受け取れる。 |
| Twitch代替 | IRCはフォールバック | MultiCommentViewer系の既存コメントビューアは歴史的にWebSocket/IRC系実装が中心だが、新規実装では公式EventSubを優先する。 |
| 認証 | OAuth Device Code Flow | Tauriデスクトップアプリにクライアントシークレットを埋め込まなくてよい。ユーザーはTwitchの認可画面で `user:read:chat` を許可する。 |
| 読み上げMVP | 棒読みちゃんTCP | 既存の配信環境に近く、RustからTCPで実装しやすい。棒読みちゃん側の辞書、SAPI連携、VOICEROID2連携資産を活かせる。 |
| VOICEROID2直接連携 | 実験的アダプタ | RemoteControl.VoiceroidやUI Automation実装例はあるが、Windows/.NET/製品バージョン依存が強い。MVPの安定性を優先して分離する。 |

## 非目標

- 配信映像やOBS操作を直接管理する。
- 複数配信サイトを最初からサポートする。
- VOICEROID2のライセンス制約を迂回する。
- Twitchチャットへの投稿機能をMVPに含める。

## 重要な制約

- EventSub WebSocketはUser Access Tokenで購読する。App Access TokenをWebSocket購読に使う設計にはしない。
- WebSocket切断中のEventSubイベントは再送されないため、再接続/再購読を速く行う。
- 棒読みちゃんのTCPポートは通常 `127.0.0.1:50001`。ポート競合やアプリ連携OFFを検出してUIに出す。
- VOICEROID2直接連携は、公式に長期安定を保証されたTauri/Rust向けAPIがある前提で設計しない。

