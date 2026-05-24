# 読み上げ出力アダプタ

## 採用方針

MVPでは棒読みちゃんTCPアダプタを正式採用する。VOICEROID2直接連携は実験的アダプタとして設計だけ入れ、安定してから有効化する。

理由:

- 棒読みちゃんは既存の配信ワークフローに近い。
- TCPプロトコルが単純で、Rustから直接実装できる。
- VOICEROID2直接操作は.NET DLL、製品バージョン、32/64bit、UI状態に依存しやすい。
- VOICEROID2連携を棒読みちゃん側プラグイン/SAPI経由に任せられるなら、アプリ側の責務を減らせる。

## 棒読みちゃんTCPプロトコル

接続先の既定値:

```text
127.0.0.1:50001
```

発話コマンドのバイナリ構造:

| 項目 | 型 | endian | 備考 |
| --- | --- | --- | --- |
| command | i16 | little | `1` が発話 |
| speed | i16 | little | `-1` でデフォルト、目安 `50..300` |
| tone | i16 | little | `-1` でデフォルト、目安 `50..200` |
| volume | i16 | little | `-1` でデフォルト、目安 `0..100` |
| voice | i16 | little | `0` default、`1..8` AquesTalk、`10001..` SAPI系 |
| code | u8 | - | 文字コード指定。実装上はUTF-8相当の `0` を基本にする |
| length | u32 | little | 本文バイト長 |
| message | bytes | - | 読み上げ本文 |

制御コマンド:

| コマンド | 値 |
| --- | --- |
| 一時停止 | `0x10` |
| 再開 | `0x20` |
| スキップ | `0x30` |
| クリア | `0x40` |
| 一時停止状態取得 | `0x110` |
| 再生中状態取得 | `0x120` |
| 残りタスク数取得 | `0x130` |

`bouyomi4rs` の実装はこの構造をRustで直接書いており、MVP実装の参考にできる。ただしプロジェクト依存を増やさず、最初は小さな自前実装でよい。

## 棒読みちゃんアダプタ設計

```rust
pub struct BouyomiAdapter {
    addr: std::net::SocketAddr,
    defaults: BouyomiTalkConfig,
    timeout: std::time::Duration,
}

pub struct BouyomiTalkConfig {
    pub speed: i16,
    pub tone: i16,
    pub volume: i16,
    pub voice: i16,
    pub code: u8,
}
```

実装ルール:

- 発話ごとに短いTCP接続を張る設計から始める。棒読みちゃん側の既存連携と相性がよい。
- 接続失敗は読み上げキューを破棄せず、UIに「未接続」と出す。
- ヘルスチェックは空の TCP 接続だけで終えず、設定に応じて短い接続確認メッセージを発話コマンドとして送る。接続成功時の読み上げが OFF の場合は、無音の状態取得コマンドで接続を確認する。
- 長文、URL、改行、制御文字は送信前に整形する。
- 棒読みちゃんタグを許可するかは設定で切り替える。初期値は安全側で「コメント由来タグを無効化/エスケープ」する。

## VOICEROID2直接連携

候補は3つある。

| 方式 | 現実性 | 説明 |
| --- | --- | --- |
| 棒読みちゃん経由 | 高 | 既存環境を活かす。アプリは棒読みちゃんだけ見ればよい。MVP採用。 |
| C# sidecar + RemoteControl.Voiceroid | 中 | VOICEROID2/A.I.VOICE Editor APIを使う実装例がある。Rust/TauriからC#プロセスへJSON-RPCやstdioで依頼する。 |
| UI Automation直接操作 | 低-中 | ウィンドウ、WPF TextBox、再生ボタンを探して操作する。画面状態やバージョン変更に弱い。最終フォールバック。 |

### C# sidecar案

```text
Tauri Rust
  -> stdio JSON-RPC / localhost named pipe
  -> voiceroid-bridge.exe (.NET)
  -> AI.Talk.Editor.Api.dll / RemoteControl.Voiceroid
  -> VOICEROID2
```

利点:

- VOICEROID2 APIに近い世界はC#に閉じ込められる。
- Rust側はプロセス起動とJSON-RPCだけ担当する。
- ビルド/配布をWindows限定featureにできる。

懸念:

- VOICEROID2本体のDLLやバージョンと一致が必要。
- 32bit/64bit差異がある。
- ユーザー環境ごとのセットアップ案内が必要。

### UI Automation案

公開gistには、`VoiceroidEditor` プロセスを探し、WPFのTextBoxと「再生」ボタンを操作する例がある。これは動作イメージの参考にはなるが、MVPの主経路にはしない。

使う場合の制約:

- Windows専用。
- VOICEROID2のUIテキスト、WPF構造、起動状態に依存する。
- フォーカス奪取やモーダルダイアログ処理が必要になる。
- 実装は配信中に失敗しても棒読みちゃんへ戻せるよう、必ずアダプタ分離する。

## 読み上げキュー

初期キュー仕様:

- FIFO
- 最大件数: 200
- 1コメント最大文字数: 120文字
- ユーザー単位の連投抑制: 2秒
- キュー溢れ時: 古い未読を落とし、UIに警告
- 読み上げ失敗時: 1回だけ短い遅延で再試行

状態:

- `Idle`
- `Speaking`
- `Paused`
- `Disconnected`
- `Error`

## 参照元

- bouyomi4rs source: <https://docs.rs/bouyomi4rs/latest/src/bouyomi4rs/lib.rs.html>
- RemoteControl.Voiceroid: <https://github.com/VOICeVIO/RemoteControl.Voiceroid>
- RemoteControl.Voiceroid API doc: <https://github-wiki-see.page/m/VOICeVIO/RemoteControl.Voiceroid/wiki/API-Doc>
- VOICEROID2 UI Automation gist: <https://gist.github.com/sskwwskwww/38d99e2453c31ffc3ed335a6bdd56908>
