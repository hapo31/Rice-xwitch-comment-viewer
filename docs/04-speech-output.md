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

読み上げコマンドのバイナリ構造:

| 項目 | 型 | endian | 備考 |
| --- | --- | --- | --- |
| command | i16 | little | `1` が読み上げ |
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

MVPでは項目単位の音声overrideを提供しない。`SpeechRequest`に`voice/speed/tone/volume`を指定すると未知の項目として拒否する。すべてのtalk packetは設定単位の`BouyomiTalkConfig`を使い、requestとのmergeや暗黙の変換は行わない。voiceはこの設定内の`i16`数値ID（0が既定、1〜8がAquesTalk、10001以上がSAPI系）であり、汎用文字列voice IDは公開しない。

```rust
pub struct BouyomiAdapter {
    address: BouyomiAddress,
    defaults: BouyomiTalkConfig,
    timeout: std::time::Duration,
}

pub struct BouyomiAddress {
    host: String,
    port: u16,
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

- 読み上げごとに短いTCP接続を張る設計から始める。棒読みちゃん側の既存連携と相性がよい。
- アプリ内の talk、テスト読み上げ、接続確認、無音プローブ、pause/resume/skip/clear は共有 async dispatcher を通す。短命TCP接続は維持するが、一つの送信が物理的に完了するまで次の接続を開始しない。キューワーカーは dispatcher を取得してから pending を in-flight へ予約し、同じ guard のまま talk packet を書き込む。control は queue の control-in-progress を先に記録し、同じ dispatcher guard の中で packet 送信、ローカル queue 反映、成功 status/log の通知を行う。これにより、control が先に開始された場合に予約済みの talk が control 成功後に送られること、pause/resume の wire 順とローカル適用・通知順が入れ替わることを防ぐ。制御送信の失敗時はローカル queue が未変更で、棒読みちゃん側は到達不明と明示する。失敗した control が最後の barrier なら、保留中の自動読み上げ worker を再開する。送信後のローカル反映に失敗した場合は、棒読みちゃん側は送信済みでローカル状態だけが未反映と明示する。
- 接続先はhost/portを構造化して保持し、共通destination policyでIPv4・DNS・IPv6を2秒/最大16address以内へ解決する。全addressを検証してから`SocketAddr`集合へ直接接続し、connect内部でhostnameを再解決しない。通常は127/8・::1（IPv4-mapped loopbackも含む）だけへ接続する。private LAN/VPN宛先も明示remote mode＋native consentがなければ送信しない。
- remote modeだけでは許可されない。Settingsでhost/port/modeを保存し、［保存済みの接続先をネイティブ確認で許可］を操作すると、宛先/DNS結果とTwitch user/chat/test/controlの平文送信・TLS/相手認証なしをnative UIで確認する。public/link-local/multicast/未指定宛先は未対応。信頼するprivate宛先を最小allowlistとしてこの起動中だけ保持し、暗号化トンネル/VPNを推奨する（トンネルの接続/暗号化をRiceが保証するわけではない）。同意は設定JSONに保存せず、再起動/endpoint変更/DNS集合変更後は手動で再確認する。自動読み上げ・diagnostics・health/test・全controlに例外はない。許可拒否/変更時は自動再送せず、既に開始済みの送信を取り消す保証はない。
- host欄はIPv4、DNS名、または角括弧なしのIPv6アドレスを受け付ける。portをhost欄へ含めず、IPv6 zone identifierは初期実装では受け付けない。表示・diagnosticsではIPv6を `[::1]:50001` のように角括弧付きで表記する。
- hostの妥当性検証とaddress構築はアダプタの一箇所に集約し、設定保存、queue、health、test、control、diagnosticsから共通して利用する。
- 接続失敗は読み上げキューを破棄せず、UIに「未接続」と出す。
- 接続拒否・timeout の backend エラーは、画面名や route を含めず［診断］という復旧操作だけを案内する。frontend は `appRoutes` の Settings 定義を使い、読み上げが `Disconnected` / `Error` のとき Side Panel から Settings の［診断］へ1操作で移動できるようにする。
- ユーザーが実行する接続確認は空の TCP 接続だけで終えず、設定に応じて短い接続確認メッセージを読み上げコマンドとして送る。確認読み上げの ON/OFF にかかわらず、先に再生中状態取得 `0x120` を送り、1 byte の boolean 応答（0 または 1）を受信した場合だけ接続成功として扱う。確認読み上げはその後に送る。診断・無音プローブも同じ応答検証を使い、応答待ちは2秒で打ち切る。EOF・不正値・無応答はポート競合や非互換サービスの可能性を案内する。これは互換の状態応答の確認であり、相手の認証や音声出力完了を保証するものではない。
- 起動後および障害後の自動復旧に使う周期ヘルスプローブは、接続成功時の読み上げ設定にかかわらず無音の状態取得コマンドだけを送る。下流の音声合成アプリが未起動の間に読み上げ要求を蓄積しない。
- 長文、URL、改行、制御文字は送信前に整形する。URL の検出・置換・遮断は `SpeechFormatter` に閉じ込め、空白区切りには依存しない。
- 最大文字数は URL・NG・制御文字・emote の処理とユーザー名 prefix の付与を終えた最終読み上げ文へ Unicode 文字単位で適用する。切り詰め時の省略記号 `…` も上限に含めるため、上限 1 で切り詰めが必要な場合の出力は `…` とする。表示名だけで上限を超える場合も、最終読み上げ文の先頭から同じ規則で切り詰める。正規化後の本文が空の場合は、この切り詰めを行わず `Blocked` とする。
- URL は ASCII の `http://`、`https://`、または scheme なしの `www.` で始まる形式を大文字小文字を区別せず検出する。日本語文中、括弧、引用符内でも対象にするが、ASCII の識別子やメールアドレスに連結した部分文字列は URL とみなさない。URL の直後に続く日本語本文は URL に含めず、末尾の句読点、対応しない閉じ括弧、対になる引用符、後続本文を囲む開き括弧・引用符は本文として残す。URL path 内で対になった括弧は URL の一部として扱う。角括弧付き IPv6 を含む authority は URL parser で検証し、不完全な authority は対象外とする。国際化ドメインは punycode 表記を対象にする。

Speech の実装は責務ごとに分ける。`speech/mod.rs` は共通trait・公開型・module/export と既存 adapter 群の組立を担い、queue model/遷移は `queue.rs`、整形と URL 判定は `formatter.rs`、queue Tauri commands は `queue_commands.rs`、event payload/snapshot 変換は `events.rs` に置く。各回帰テストは `tests.rs` にまとめ、公開 command と worker が同じ queue/formatter 経路を使うことを確認する。

URL 候補の範囲抽出には `linkify` の `LinkFinder`（`LinkKind::Url`、scheme 任意）を使えるが、その結果は有効 URL と保証されないため、候補をそのまま受理しない。アプリ側で http/https/www の許可、ASCII 識別子・メール境界、日本語隣接、host/port の厳格な URL parser 検証、末尾記号処理を維持する。linkify が候補を返さない場合に既存 parser で互換確認する経路を残し、候補が他にもある場合に落ちる角括弧付き IPv6 authority も同 parser で補う。依存の一般的な候補抽出と、製品固有の受理・置換規則を分離する。
- 正規化（制御文字・空白・emote 除外など）の後に本文が空なら、ユーザー名読み上げの ON/OFF にかかわらず理由 `読み上げる本文がありません。` で `Blocked` とする。空の talk packet やユーザー名だけの読み上げは送信しない。
- 棒読みちゃんタグを許可するかは設定で切り替える。初期値は安全側で「チャット由来タグを無効化/エスケープ」する。

## 通信失敗の分類

### 項目ごとの理由と復旧（Issue #85）

blocked理由はrepeatSuppressed/blockedUser/blockedWord/blockedUrl/emptyAfterFormatting、skipped理由はoverflow/userSkip/removed/cleared、error理由はadapter共通の12 FailureCodeを使う。formatterも表示文字列でなくBlockedReasonを返す。理由の説明は固定日本語とし、NG一致語、ユーザー名、host/設定、token、adapterのdetailをoutcomeへ複製しない。元のチャット本文を既存のitemへ保持することと、理由へ機微な情報を追加することは区別する。

自動再試行中は直前のerror outcomeを保持するが、明示再試行では古い理由を消し、次の失敗でcode/timeを更新する。正常完了では理由を消す。受付後の未確認と送信到達不明はretryable=false/recoveryAction=confirmDeliveryを記録し、重複の可能性を利用者へ示す。retryableは失敗分類上の安全な再試行可否であり、残りbudgetを表さない。terminal errorをhealth復旧だけで再送しない。

queue/snapshotの履歴上限200件、pending+in-flight上限200件は維持する。取消後の遅延成功/失敗は理由も上書きしない。理由に関するwarning/logは`[itemId]`を付け、項目詳細から対応するログを識別できる。全理由の共通Rust/TS fixture、formatter/連投、4取消理由、再試行/未確認、reload/latest snapshot、200件上限、本番fake-workerと既存6,144操作列で検証する。

### 接続healthとqueue phaseの独立性

`SpeechAdapterHealth`（unknown/connected/disconnected/error）と`SpeechQueuePhase`（idle/speaking/paused/error）は別々に保存し、それぞれのrevision付きevent/snapshotから復元する。queue活動の通知は最後のhealthを保持し、Idle/Speaking/Pausedを接続確認と解釈しない。無音probe・接続確認の成功はhealthだけを更新し、pausedや失敗項目の手動再試行待ちを解除しない。

自動probeは接続中・paused中も5秒周期で継続し、未解決probeがあれば重ねない。終了後の遅延応答はfrontend通知を発生させない。定期probeは無音であり、失敗項目の再送やworkerの起動を行わない。

復旧policyは既存の「失敗した項目は手動再試行」を維持する。queueのerror phaseは、処理可能なpending/in-flightがなく、失敗履歴が手動対応を待っている状態を指す。新着/後続pendingは失敗履歴に妨げられず処理できる。履歴があるだけで実行中のqueueをerrorとしない。利用者の再試行/履歴削除だけがその待ち状態を変える。

Status Bar・Side Panel・live announcementは接続とqueue状態を別表示する。起動ガイドの準備完了はhealth=connected、queue=idle/speaking、自動読み上げONの組合せから判定し、接続復旧だけで準備完了としない。終了保護とpause/resume hotkeyもlegacy statusではなくqueue phase/itemsを使う。`SpeechStatusEvent.status`は互換用の活動/エラー投影として残すが、接続/準備完了/操作可否の根拠にはしない。

共通の`src/tauri/fixtures/speech-independent-states.json`をRustの保存/serializationとfrontendのbridge parse/reducerで再生し、health・pause・失敗・復旧の到着順とsnapshot復元の不変条件を検証する。

connect/write/responseのtimeoutとI/O、設定不正、非互換応答を`BouyomiError`で区別する。OSの表示文やerror番号の部分一致では判定しない。`io::ErrorKind`から共通のfailure code、status、再試行可否、日本語短文を導出し、queue・接続確認・無音probe・test・control・diagnosticsで同じ原因に同じ分類を使う。接続拒否、接続timeout、切断は`Disconnected`、設定不正や非互換応答は`Error`。元のcause chainはLogsへ残し、UIへ低レベルの英語文を混ぜない。

talkの自動再試行は、packetを書き始める前の一時的な接続失敗だけに限る。write失敗/timeoutや受付後のresponse失敗は届いた可能性があるため自動再送しない。履歴へ保持し、利用者が状態を確認してから明示的に再試行する。制御失敗はローカルqueue未変更と相手側の到達不明を付記するが、根本原因のstatusと復旧案内は同じ分類を使う。

`SpeechAdapter::health_check`は無音probeを行う。transportの未接続は型付きfailureを保持した`Ok(SpeechHealth::Disconnected { failure })`、設定/protocol/unknownは`Err(SpeechFailure)`とする。共通command境界が同じfailureをUI/Logsへ報告し、分類を失わない。Windows/Linuxのnative error→ErrorKind→分類、表示localeに依存しないmapping、fake transport、write timeoutを`bouyomi-errors.yml`で継続検証する。healthとqueue phaseの独立保持は#69で導入した。

Issue #70のfactory/session境界は`docs/02-architecture.md`を参照。queue workerと共通commandsは具体adapter・host/port・声質を参照しない。設定snapshotの解釈はfactory、protocol/diagnostics/完了queryはbouyomi、再試行・履歴・FIFOはqueueという責務を保つ。fake adapter/clock/sinkによる本番workerの成功・遅延・失敗・再試行・受付後未確認と共通制御のテストを追加した。キュー操作列と並行enqueueの網羅性は別のIssue #72で確認する。

### 決定的なキュー検証（Issue #72）

`speech/worker/tests.rs`と`tests/scenarios.rs`は、本番のenqueue/control/workerへfake adapter・clock・event sinkを注入する。sleepで順序を推測せず、oneshot/Notify/barrierで送信・完了・制御の境界を固定する。実時間のtimeoutは停止したテストを検出するwatchdogに限り、再試行や連投の時刻判定には使わない。

| 検証対象 | 自動検証する不変条件 |
| --- | --- |
| FIFO・200件上限 | 送信中項目を保持し、最古の未送信だけをoverflowで落とす。満杯の手動再試行は履歴を移動せず拒否する。 |
| 連投・整形 | 0/1/2/30秒の直前と境界、ユーザー別時刻、NG/URL/空本文、自動読み上げOFFを確認する。 |
| 失敗・復旧 | 初回成功、1回再試行、上限、受付後未確認の非再送、699ms/700ms、接続復旧後の手動再試行とbudget復元を確認する。 |
| 取消・制御 | clear/skip/removeと成功・送信失敗・完了未確認の9順序、完了後の操作、pause前後の予約、複数control barrier、retry待ち中clearを確認する。 |
| 並行enqueue | async barrierの2世代と20 native threadでworkerの単一所有権と空キュー終了前後の処理継続を確認する。 |
| 操作列・snapshot | 固定3seedの計6,144操作でIDの所属一意性、pending/in-flight/history上限、retry budget、worker所有権、件数/status/source IDの整合とwarningを検査する。 |

既存の純粋state/formatterテストに加え、上記fake-worker検証はWindows/Linux CIでも実行する。実棒読みちゃんの音声出力やWindowsの実配布WebViewは代替せず、実機/配布smokeを別に扱う。

参考: [Rust ErrorKind](https://doc.rust-lang.org/std/io/enum.ErrorKind.html)、[Tokio write_allのキャンセル安全性](https://docs.rs/tokio/1.52.3/tokio/io/trait.AsyncWriteExt.html#method.write_all)。write_allは途中まで書いた状態で中断され得るため、timeoutを「未送信」と解釈しない。

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

コメント受信時の設定 snapshot が自動読み上げ OFF の場合は、pending に入れず `skipped` / `autoSpeakDisabled` の結果を履歴に保持し、通常の queue event と snapshot で通知する。履歴上限200件、待機数に含めないこと、連投抑制時刻を更新しないことを維持する。ON/OFF の切替が後から起きても既存結果を変更・再 enqueue しない。frontend は現在の設定から受付を推測せず、結果受信前を「受信済み」、受付後だけ「待機」と表示する。

初期キュー仕様:

- FIFO
- 最大件数: 200
- 1件のチャット最終読み上げ文（ユーザー名 prefix・省略記号を含む）の最大文字数: 120文字
- ユーザー単位の連投抑制: 既定は2秒。設定値 `0` は抑制なし、`1` は1秒、`2` は既定と同じ2秒、`3`〜`30` も指定した秒数を厳密に適用する。空欄は `0` とみなさず入力エラーにし、保存済み設定も起動時に同じ範囲を検証する。抑制時刻は channel ID と EventSub 接続 generation を scope とし、いずれかが切り替わった場合または抑制なしへ変更された場合は、次の連投判定時に破棄する。同一 scope 内では期限30秒の cache として保持する。background cleanup は1秒間隔で FIFO の最大64件を処理し、期限切れが残る場合は mutex を解放して次の batch を直ちに処理する。解放時刻は runtime のスケジューリングと mutex 待ちにも依存する。プロセスや runtime の停止中は復帰後に解放される。enqueue 側も同じ bounded cleanup を行うため、全ユーザー map を走査しない。cache と期限 FIFO はともに4096件の受理記録に上限を設ける。保持する受理記録が4096件を超える場合、古い記録が現行時刻を指していればそのユーザーの抑制が早く解除され、window 内でも次のコメントを読み上げる可能性がある。
- キュー溢れ時: 古い未読を落とし、UIに警告
- 読み上げ失敗時: 1回だけ短い遅延で再試行

状態:

- `Idle`
- `Speaking`
- `Paused`
- `Disconnected`
- `Error`

送信項目の状態遷移:

- 新規項目は `Ready` として pending に入り、初回送信する。talk packet の `write_all` 成功は受付済みであり完了ではないため、項目は in-flight / `Speaking` に残す。
- talk 受付後は共有 dispatcher を短時間ずつ解放しながら、残りタスク数 `0x130` と再生中状態 `0x120` をポーリングする。両方が 0 になった場合だけ `Spoken` とし、それまでは次の項目を送らない。これにより送信済みの未再生項目も200件上限と待機表示に含まれる。
- 受付後の状態照会失敗または5分の追跡期限超過は、重複読み上げを避けるため自動再送せず `Error` 履歴へ移す。棒読みちゃん側へは既に登録済みの可能性があることをUI/Logsへ明示し、再送は利用者の明示的な「再試行」に限る。
- 初回送信に失敗した項目は `RetryScheduled` となり、短い遅延後に 1 回だけ自動再試行する。この間は FIFO を保つため後続を送信しない。
- 再試行にも失敗した項目は `RetryExhausted` / 表示状態 `Error` として pending からエラー履歴へ隔離する。自動 worker は履歴を送信対象にせず、後続 pending 項目の処理を継続する。
- 新しいチャットの enqueue と通常の再開は `RetryExhausted` を `Ready` へ戻さない。Queue 画面の「再試行」は明示的な復旧操作であり、対象を pending の末尾へ戻して自動再試行枠を新たに 1 回だけ与える。
- エラー履歴は Queue 画面で確認・再試行・削除できる。接続復旧後に読み上げ直すか、破棄するかを配信者が判断する。

## 参照元

- bouyomi4rs source: <https://docs.rs/bouyomi4rs/latest/src/bouyomi4rs/lib.rs.html>
- RemoteControl.Voiceroid: <https://github.com/VOICeVIO/RemoteControl.Voiceroid>
- RemoteControl.Voiceroid API doc: <https://github-wiki-see.page/m/VOICeVIO/RemoteControl.Voiceroid/wiki/API-Doc>
- VOICEROID2 UI Automation gist: <https://gist.github.com/sskwwskwww/38d99e2453c31ffc3ed335a6bdd56908>

## 送信中のキュー操作（Issue #55）

送信開始時は、共有 dispatcher を取得して control-in-progress がないことを確認してから pending から取り出し、同じ dispatcher guard のまま talk packet を書き込む。最大200件は pending と in-flight の合計とし、overflow は未送信の pending だけを落とす。スナップショットの待機件数には in-flight を含む。棒読みちゃん宛ての物理送信と control のローカル反映・成功通知は同じ dispatcher 順序に入るため、clear/pause/skip より先に開始された talk が制御成功後に到着すること、pause/resume の反映順が wire 順と入れ替わることはない。control の失敗解除で processable な pending が残る場合は、その解除時に worker を再取得して読み上げを再開する。

clear は in-flight と pending を取消、skip は in-flight を優先して1件取消、個別削除は指定IDを取消にする。取消項目は Skipped として履歴へ移し、ID が一致しない遅延完了・失敗は無効にする。送信済みの TCP byte を撤回する保証はなく、下流の制御順序は Issue #58、受付と発声完了の区別は Issue #56 で扱う。

取消後も物理送信が完了するまでは同じ worker が所有権を保持し、新しい enqueue で二重起動しない。古い送信の成功・失敗後は同じ loop が次の pending を処理する。停止・空判定と所有権解放は queue lock 内で行う。自動再試行時だけ元の項目を pending の先頭へ戻す。
