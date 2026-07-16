# Twitch Chat TTS Tool Design

このディレクトリは、Twitchのリアルタイムチャット取得と、棒読みちゃん/VOICEROID2系の読み上げ連携を統合するTauriアプリの設計メモです。

## 目次

- [要求と採用方針](./01-requirements-and-decisions.md)
- [全体アーキテクチャ](./02-architecture.md)
- [Twitchチャット受信](./03-twitch-ingestion.md)
- [読み上げ出力アダプタ](./04-speech-output.md)
- [UI/UX設計](./05-ui-ux.md)
- [実装ロードマップ](./06-implementation-roadmap.md)
- [リリース手順](./releasing.md)
- [調査メモ](./RESEARCH_NOTES.md)

## 結論

MVPでは次の2機能を中核にする。

1. Twitch EventSub WebSocketでチャットを受信する。
2. 棒読みちゃんのTCPアプリ連携で読み上げる。

VOICEROID2直接連携は「正式MVPの主経路」にはせず、Windows専用の実験的アダプタとして後から差し替え可能にする。VOICEROID2には公式に安定した外部操作APIとして扱える資料が乏しく、公開実装例も.NET DLL/バージョン一致/UI Automationに寄るため、最初から主経路にすると保守コストが大きい。

## 参照元

- Twitch Chat & Chatbots: <https://dev.twitch.tv/docs/irc/>
- Twitch EventSub WebSocket: <https://dev.twitch.tv/docs/eventsub/handling-websocket-events>
- Twitch Chat EventSub setup: <https://dev.twitch.tv/docs/chat/authenticating/>
- Twitch EventSub subscription types: <https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/>
- MultiCommentViewer: <https://github.com/CommentViewerCollection/MultiCommentViewer>
- bouyomi4rs source: <https://docs.rs/bouyomi4rs/latest/src/bouyomi4rs/lib.rs.html>
- RemoteControl.Voiceroid: <https://github.com/VOICeVIO/RemoteControl.Voiceroid>
- RemoteControl.Voiceroid API doc mirror: <https://github-wiki-see.page/m/VOICeVIO/RemoteControl.Voiceroid/wiki/API-Doc>
