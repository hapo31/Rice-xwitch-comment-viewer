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

### Frontend domain store 境界

`DomainProvider` は chat、queue、connection、settings、logs を独立した `useSyncExternalStore` source として保持する。各画面は `use*Selector` で必要な slice だけを購読し、Chat event は Chat store の subscriber だけを通知する。Launcher は settings の launcher selector、警告は logs store の notifications slice を使う。`AppShell` は配置と controller provider の組み立てを担当し、`ApplicationControllerProvider` が起動時の設定・認証復元と Tauri event 購読を起動する。認証遷移と副作用は `twitchController`、speech/queue/Launcher のcommand処理は用途別 controller、終了確認は `ExitProtectionProvider` に置く。設定更新は既存の直列化 orchestrator に集約する。

domain store が状態の唯一のsourceであり、React Context はdomain単位の安定した操作APIと unsaved/exit の操作だけを渡す。画面は表示に必要なstore selectorとaction contextを直接参照し、`MainView` はroute title・focus通知だけを担当する。controller providerは画面状態を集約した旧 `AppState` を再構成しない。Jotai等の状態管理依存は追加しない。

Device Code認証の結果、status、prompt、profile、通知/error副作用は`authFlowTransition`の小さな純粋モデルで一緒に決める。`AuthOperationController`は世代付きの操作開始/完了、手動操作による古い応答の無効化、単一pollの排他、provider破棄時のinvalidateを担う。AppShell effectはprompt/status lifecycleに沿ってpoll timerを開始・cleanupし、timerはschedule時の世代とprompt情報を照合してからpollまたは期限切れを要求する。世代が変わった後、異なるpromptになった後、手動認証操作中は期限切れtimerも状態や通知を更新しない。期限切れはauth flowへ通知する。XStateの[`invoke`](https://stately.ai/docs/invoke)と[遅延遷移](https://stately.ai/docs/delayed-transitions)はpromise完了による遷移とstate退出時のtimer解除を提供するが、この認証には追加actor/runtime依存と移行費用がある。promise actor退出後の結果破棄も実行中のTauri commandを止める保証ではなく、backend generation保護は別途必要である。そのため、現在の操作競合と短いDevice Code timerは明示reducer/controllerで管理し、XStateは導入しない。

旧 `appReducer` と専用テストは除去し、`appState.ts` は画面用の合成 read model と action 契約だけを保持する。状態更新は本番 domain store が担当し、`dispatchDomainAction` は action を各 store へ振り分ける。ログ表示IDとbackend replay IDの区別・重複排除は `logsStore` に集約し、bridge の副作用も受理されたログに限定する。設定更新は `createSettingsMutationOrchestrator` だけで直列化し、失敗後の待機済み更新と全処理の完了待ちを同じ経路で検証する。queue snapshot は queue store と chat status synchronization action を通じて Chat 行へ反映する。項目のoutcomeも同じsourceMessageIdで同期し、statusが同じでもcode/message/time等の変更を反映する。同期実装はchatStoreで共用し、同値snapshotではmessage参照を維持する。queue履歴の削除/退避後もChatの最後の結果は既存200行の範囲で保持する。

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
| `TwitchAuthService` | Device Code Flow、トークン更新、`/validate`、認証世代による古い応答の拒否 |
| `EventSubClient` | WebSocket接続、welcome/keepalive/reconnect/revocation、購読・重複排除・正規化へのdispatch |
| `TwitchChatService` | チャンネル入力検証とHelixユーザー取得、接続taskの所有・交換、受信停止と連携解除 |
| `SpeechQueue` | 優先度、停止/再開/スキップ、連投抑制、バックプレッシャ |
| `SpeechFormatter` | 読み上げ文生成、ユーザー名付与、絵文字/URL/長文処理 |
| `SpeechAdapter` | 読み上げ先を抽象化するtrait |
| `BouyomiAdapter` | 棒読みちゃんTCPプロトコル実装 |
| `VoiceroidAdapter` | Windows専用の実験的アダプタ。C# sidecarまたはUI Automationを隠蔽する |
| `SettingsStore` | optional/versionedな永続wireを移行・共通検証し、不正な項目だけ既定値へ戻す。JSON構文/容量の破損はbackupまたは既定値へ復旧する。未知の版・項目は読取り専用。原子的保存の成功後だけ候補を共有メモリへ反映する。OAuthトークンは扱わない |
| `TwitchAuthStore` | Twitch OAuth状態をOS keyringへ保存/復元/削除する |
| `LauncherService` | 登録アプリのパス検証、重複排除、単体/一斉起動を扱う |

### Twitch責務分割（Issue #44）

`twitch/model.rs`は公開chat DTOだけを保持し、既存の`crate::twitch::*`で再exportする。camelCase/optional field omissionとcommand/event payloadは変更しない。`error.rs`はHTTP status/OAuth codeの型付き分類と日本語表示を分け、表示文言が認証解除・retry可否を決めない。`normalization.rs`はEventSub wireとchat正規化を担当し、欠損/不正timestampには呼出元が渡した受信時刻を使う。`dedupe.rs`は接続全体で共有するbounded cacheと明示`Instant`によるTTLを保持する。この2つのpure境界はTauri、keyring、network clientに依存しない。

`auth_state.rs`は認証DTO・世代・scopeの規則、`auth_service.rs`は認証操作、`auth_store.rs`はcredential I/Oの直列化とkeyring/旧Linuxファイルの移行、`oauth.rs`はHTTP wireとOAuth transportを担当する。`chat_service.rs`は接続taskのライフサイクル、`eventsub.rs`はsession/handover/backoff、`subscription.rs`は最新credential取得・401時1回refresh・保存後の再購読を担当する。ファイル移動で保存/削除の世代照合やHTTP deadlineを緩めない。

`commands.rs`は既存7 commandの引数/戻り値を維持する薄いadapterで、`runtime.rs`だけがTauriのmanaged state、event送信、speech enqueueと本番transportを接続する。認証serviceには`AuthRuntime`/`DeviceOAuthTransport`、チャットserviceには`ChatRuntime`、EventSubには`EventSubRuntime`、購読には`SubscriptionRuntime`を注入する。`TwitchAuthStore::with_backend`で保存先を差し替えられる。Device Codeのwall clockと通知のreceive/monotonic clockもruntimeから渡し、非同期deadlineはTokio test clockで制御する。serviceはTauri/reqwest/keyringをimportしない。

既存の認証競合・bridge fixture・再接続回帰は`tests.rs`/`test_harness.rs`へ保持し、`service_tests.rs`で同じ本番serviceをscripted transport/store/clockへ接続する。Device Code各応答、並行start/poll、保存後の認証通知、失敗した解除、チャンネルの事前検証、接続交換、停止時の認証保持、解除時の削除、型付き購読失敗とrefresh保存順、clockによるTTLを検証する。各leafの分類・receive clock・TTL/capacity回帰と、serviceへのインフラ依存/command名の退行を検出する境界チェックも維持する。

Launcherのアプリ登録・起動はWindows専用。`app_build_info.launcher`で`canRegisterApplications/canLaunchApplications/reason`を型付きで返す。UIは取得成功まで安全側に無効化し、非対応OSでは選択・DnD購読・単体/一斉起動を提供しない。backendも登録commandと設定patchによる新規登録/target変更を保存前に拒否し、起動をfilesystem操作前に拒否する。既存設定の項目は他OSでも表示・並び替え/表示名変更・削除でき、OS標準ランチャーまたはWindows版を案内する（Issue #79）。

## 設定入力と読み上げ接続先の境界

設定入力は`settings/validation.rs`でwireとdomainを分ける。`settings_update`はframework所有JSONを256KiB/nodes/depth・既知field・文字列/rule量でpreflightしてからDTOをcloneし、leaf patchを最新candidateへ適用、全domainとLauncher資源を検証・保存できた場合だけ公開する。`ValidationError { field, code, message, recovery }`で安全な日本語と修正対象を返す。`TwitchLogin`は設定保存と`twitch_connect`で共用し、空欄は自分のチャンネル、非空は英数字・_の3〜25文字、raw128 UTF-8 bytes以内/controlなしとする。hostはraw253 UTF-8 bytes/DNS label63、NGユーザーはlogin形式、NGワードは500 Unicode文字/2048 UTF-8 bytes、各200件/両list合計64KiB、接続成功文は120文字/480bytesまで。文字数/range違反をclamp/truncateで成功扱いにしない。永続wireのmigration/field fallbackも同じpatch適用・domain validatorとLauncher構造validatorを使う（#64）。

`SpeechRuntime`がprocess-localの`DestinationPolicy`をfactory/diagnosticsと共有する。各TCP接続はhostを2秒以内・最大16addressへ解決し、全addressを検証して検証済み`SocketAddr`集合へ直接接続する（connect時の再DNS解決なし）。通常は127/8・::1・IPv4-mapped loopbackだけを許可する。remote modeはopt-in要求であり許可ではない。private IPv4/IPv6 ULAだけが外部許可の対象で、public/link-local/multicast/未指定宛先は拒否する。明示`speech_authorize_endpoint`がhostname/IP/port・全解決address・ユーザー名/chat/test/controlの平文送信/TLSと相手認証の欠如/VPN注意をnative dialogへ表示する。callbackをawaitし設定lockは保持しない。許可後にDNSを再確認し、設定変更がないことを短いlock下で比較してからopaque approvalをメモリへinstallする。1つのpending prompt/30秒rate limit、拒否時は旧許可も取り消し、endpoint変更/再起動/解決address変更は再同意なしに送信しない。設定fileやrendererへconsent flagは持たせない。既に開始した送信の取消やbyte回収、相手identityの認証は保証しない。

domain/endpointの境界値は同じJSON fixtureをRustとフォームで検証する。fake resolver/native consentでmixed DNS・rebinding・拒否・再起動/endpoint変更・全talk/query/control/diagnostics経路を検証する。実Windows WebViewは不正6入力のstructured rejection・disk/memory保持、rendererからremote flagを送ったprobe/diagnostics2経路の拒否を追加検証する。DNS/consent failureは自動再送しないConfigurationとする。既存Launcher 200件/4MiB iconsのfixtureはvalidな最大NG rule payloadへ変更し、8MiB wire read/backup budgetはJSON whitespace paddingで維持する（multi-MiB NG wordでdomain上限を回避しない）。native result channelもvalidatedな500文字以内のNG wordを使い、Twitch loginを検証除外にしない。

## Launcherの実行境界

`launcher/model.rs`は永続DTO・編集DTO・quota/PNG検証・正規化・path identity/ID/orderのpureな境界とする。Tauri、filesystem確認、PowerShell、process起動をimportしない。`ports.rs`の小さなobject-safe traitを通じて、`service.rs`の登録/削除/単体・一斉起動へresolver、icon extractor、application launcher、repository、event sinkを注入する。

`AppState.launcher_runtime`がアプリ全体で1つのworker poolとadapterを保持する。`commands.rs`はborrowed IPCのpreflight/DTO変換、repository/event sinkのwiring、service呼出しだけを行う。`repository.rs`は共有設定の最新candidateへmutationを1回適用し、既存のsettings transactionで検証・永続化した後だけメモリへ公開する。Launcher以外のsectionも保持する。保存失敗時は追加/削除の成功ログやicon fallback通知を発行しない。filesystem/COM処理中にsettings lockを保持しない。

`launcher_add`は更新後の`items`と、そのtransactionで実際に追加した`addedCount`を返す。UIはPromise解決時の共有state件数から追加数を推測しない。並行追加が同じtargetを含む場合もrepositoryの最新candidateへのcommit内で件数を確定する。

`workers.rs`は最大4つのblocking taskを共通poolで制限する。取得待ち6秒・job待ち7秒を維持し、timeout後も実workerが終了するまでpermitを返さない。`platform/target.rs`だけが実ファイルの存在/種類/canonical pathを確認し、WindowsではDOS/UNCへ変換する。`platform/windows/icon.rs`はPowerShell/COMの5秒timeout、kill/reap、bounded pipe回収を担当する。`platform/windows/launch.rs`はapplication pathだけを受け取り、Launcherのkindを解釈しない。Websiteの予約/拒否と将来のdispatch追加はservice/modelに閉じる。

Windowsのsupported caseは存在する`.exe`と、通常の`.exe`を指す`.lnk`（拡張子の大文字小文字を区別しない）。`.exe`はshellを経由せずpathをCreateProcessへ渡し、parentをworking directoryにする。`.lnk`は起動の都度COMでtarget/arguments/working directory/icon sourceを構造化し、解決結果を設定に保存しない。参照先の存在・regular file/canonical path、絶対パスのworking directoryを検証した後、そのexeを直接CreateProcessする。引数はWindowsのraw argument tailとして渡すため、shellのメタ文字へ再解釈しない（起動先自身の引数解釈は別）。cwd空欄はexeのparentを使う。link修復/移動先探索のResolve、Explorerへの受付、ShellExecuteは使わない。移動したtargetは失敗として修復・再登録を案内する。

shortcutは固定header/CLSIDと1MiB上限をCOM呼出し前に検査し、MSI advertised（HasDarwinID）、管理者/別ユーザー要求（RunAsUser）は自動起動せず手動起動を案内する。URL/仮想folder/入れ子link/非exeも拒否する。読み取りはIShellLinkW/IPersistFileへUTF-16を直接渡し、WSH/PowerShellの文字変換を経由しない。COM apartmentは同threadで初期化/解放し、guardはthread間移動不可とする。GetPathのMAX_PATH制約によりraw targetは258 UTF-16 unitsまで（切り詰めを受理しない）とし、cwd/iconは4096 units、argumentは16Ki unitsまでの固定bufferに余分な1文字を設けてoverflow/不正Unicodeを拒否する。環境変数はtarget/cwd/iconだけで展開し、展開後pathは4096 UTF-8 bytes、argumentは64KiB UTF-8も検証する。control文字は拒否する（argumentのTABのみ許可）。長いtargetは短いパスの通常exeへ再登録する。COMが停止した場合はserviceの7秒job timeout/最大4worker境界で呼出しを返すが、同期COM自体の強制取消は保証しない。requireAdministratorのexeのCreateProcessがOS error740を返した場合も、UAC promptを送らず手動起動を案内する。権限・installer・UAC設定を変更しない。

`launchedCount`の契約は「対象exeのprocess生成を確認した数」であり、shell受付やアプリの準備完了を意味しない。UIはその範囲と全failureの対象名/原因/修復手順を表示する。単体/一斉起動はasync serviceでlock外のblocking workerへ送り、登録と同じ4-permit pool・取得6秒/job7秒を共有する。一斉起動は保存order順のまま1項目ずつ開始し、部分失敗を保持するため、操作全体の7秒上限はない。timeout/cancelされたresolverはOS呼出し直前のcontext検査で遅延起動を中止し、workerの実終了までpermitを保持する。既に実行中のCreateProcess自体は強制取消できないため、期限超過は成功に数えず「既に起動している可能性」を伝え、自動再試行しない。COM読込とexe起動の間のfilesystem競合や同一ユーザーの別processを隔離する保証ではない。

全OSの`launcher::service::tests`は本番と同じserviceへfake resolver/extractor/launcher/repository/sinkを注入し、icon timeout/failureのfallback、job/permit timeout、invalid/resource-limit icon、broken shortcut、spawn部分失敗、保存rollback、同時add/removeと別section保持、Websiteのadapter非呼出し、launch timeout後の遅延process非生成を検証する。blocking adapterの開始/解放はchannelで同期し、時計はTokioのtest clockを明示advanceする（blocking taskはauto-advanceを止める）。Windows本番featureのintegrationは隔離directoryへ自作probeをcompileし、実COM linkの正常/引数・日本語・空白・メタ文字/cwd/再解決/broken/moved/RunAsUser/直接exe/部分成功を検査する。対話的UAC承認や外部アプリの準備完了は証拠としない。Windows native CIの200 tile/IPC拒否・2process focusも維持する。実UNC停止/全COM障害、installer/portable配布物の検証は別境界（#91）として残す。

## SpeechAdapterの実行境界

```rust
pub type SpeechFuture<'a, T> =
    Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait SpeechAdapter: Send + Sync {
    fn health_check(&self) -> SpeechFuture<'_, Result<SpeechHealth, SpeechFailure>>;
    fn speak(&self, request: SpeechRequest)
        -> SpeechFuture<'_, Result<SpeechResult, SpeechFailure>>;
    fn pause(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn resume(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn skip(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn clear(&self) -> SpeechFuture<'_, Result<(), SpeechFailure>>;
    fn wait_for_completion(&self) -> SpeechFuture<'_, SpeechPlaybackCompletion>;
}
```

boxed futureにより`Arc<dyn SpeechAdapter>`として差し替えられる（[Rust Reference: dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)）。MVPの実装は`BouyomiAdapter`のみで、VOICEROID2は未実装のままとする。

app stateの`SpeechRuntime`がfactory・共通dispatch gate・clockを保持する。factoryだけが設定snapshotから具体adapterを構築し、棒読みちゃんのhost/port/声質を解釈する。health、無音probe、test、queue、pause/resume/skip/clearは同じ選択を通る。`SelectedSpeechAdapter::lock`から得るsessionを介して呼び出し、raw traitの送信は既にgateを所有している前提で再lockしない。

workerは共通gateを取得してから項目を予約し、session内で送信する。controlはremote送信からlocal queue反映・成功通知まで同じsessionを保持する。受付後の完了待ちはsessionを解放して共通の`Completed / Unconfirmed(SpeechFailure)`を待つため、完了待ち中にもcontrolを送れる。adapter側の完了確認queryは同じgateで短時間ずつ直列化する。受付済みと再生完了を混同せず、未確認の要求は自動再送しない。

`SpeechFailure`は共通のcode・health・再試行可否・日本語案内・技術詳細を持つ。BouyomiErrorからの写像と固有diagnosticsはbouyomi側に閉じ込め、workerはprotocolや日本語文から分類しない。workerへqueue、選択callback、clock、event sinkを注入でき、Tauri/WebView/実TCPなしで本番schedulerを再生する。snapshot通知は従来どおりqueue mutex内で実行し、revision採取との整合を保つ。

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
}
```

MVPの`SpeechRequest`は本文と追跡IDだけを持つ。項目単位の`voice/speed/tone/volume` overrideは未対応のためモデルに公開せず、JSONで指定された未知の項目もdeserialize時に拒否する。声質はアダプタ設定からのみ取得する（Issue #81）。将来overrideを追加する際は型、許容範囲、優先順位とpacket契約を同時に実装する。

`ChatMessage.received_at` はアプリ内部で常に `DateTime<Utc>` とする。Tauri event では serde の camelCase 規約により `receivedAt` として、UTC の RFC 3339（末尾 `Z`、小数秒は nanosecond 精度まで保持）を送る。frontend は bridge 受信時にこの契約を検証し、`UtcTimestamp` として store へ渡す。欠落・空文字・タイムゾーンなし・非文字列を含む不正値、および JavaScript の `Date` / `Intl` が表現できない leap second は backend で WebSocket frame を取り出した時刻へフォールバックして warning log を残し、frontend の境界でも受信時刻を使って防御する。Chat view は保存値を変えず利用者のローカルタイムゾーンで表示し、表示不能な値では `--:--:--` を表示する。

## Tauri command/event案

### 設定ファイルのプライバシー境界

一般設定も非公開データとして扱う。Twitch channel、NGユーザー/ワード、Launcherの実行ファイルpath/icon、棒読みちゃん接続先はOAuth tokenを含まなくても利用者固有の情報である。Unixではアプリ専用directoryを0700、settings本体・backup・temporary・破損退避fileを0600にする。load/saveの前に既存permissionを補正し、補正できない場合は内容を読まず失敗する。現在のeffective UID以外のowner、symlink、非regular file、複数hardlinkは拒否する。共有するHOME/app-data rootやancestorのpermissionは変更しない。

Windowsでは現在のユーザーのAppData（Roaming、`%APPDATA%`）のACL継承を使用する。通常のユーザーprofileでは本人、SYSTEM、Administratorsが管理する。共有directoryへの移動や独自のACL設定をサポートするという意味ではない。Windows CIとpackaged実機では、保存先にUsers/Everyone等への不要なwrite権限がないことを確認する。同一ユーザーの別processやadministratorの侵害は、このpermission制限だけでは隔離できない。

Commands:

- `twitch_start_auth()`
- `twitch_connect(channel_login: String)`
- `twitch_disconnect()`
- `speech_set_adapter(adapter: SpeechAdapterKind)`
- `speech_test(text: String)`
- `speech_authorize_endpoint()`（保存済みremote接続先のnative consent。接続/読み上げは開始しない）
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

### Tauri bridge の JSON 契約

Rust の struct field にある `Option<T>` は、Tauri command と event のすべてで `None` を field omission として送る。TypeScript は対応する field を `?: T` とし、`null` を許可しない。これには status の `message`、queue の `warning` / `sourceMessageId`、chat fragment の `emote` / `cheermote` / `ownerId`、認証結果の `storageWarning`、snapshot の `speechStatus`、Launcher の任意表示属性、window position、build info の `commitHash` を含む。

struct 全体を `Option<T>` として返す command だけは JSON `null` を使う。現在は `settings_take_recovery_notice` と `twitch_get_stored_auth` が該当し、client 層で `undefined` に変換してから UI へ渡す。frontend は generic の `invoke<T>` / `listen<T>` を信頼せず、認証、chat、status、speech queue、snapshot の主要 payload では required field、enum、camelCase field 名まで検証する。その他の command result は再帰的な null 排除だけを行うため、shape の検証が必要な利用箇所を追加するときは個別 parser も同じ変更で追加する。

## Renderer のセキュリティ境界

production の bundled window は `default-src 'self'` を起点とする CSP を使う。script は bundled asset と Tauri が build 時に付与する hash / nonce、通信は Tauri IPC の `ipc:` / `http://ipc.localhost`、画像は bundled asset と検証済みの PNG data URL だけを許可する。frame、object、worker、media、base、form は使用しないため拒否する。Twitch HTTP / WebSocket と棒読みちゃん TCP は Rust 側で処理し、renderer の `connect-src` へ外部 origin を追加しない。

React の仮想スクロール、ウィンドウ倍率、Launcher tile は動的な style 属性を使うため、`style-src-attr 'unsafe-inline'` だけを例外とする。script の inline handler は `script-src-attr 'none'` で拒否する。Vite dev server / HMR は production の許可元へ含めず、development policy だけに `ws://localhost:1420` と Vite の style injection 用 `style-src 'unsafe-inline'` を明示する。Tauri は `devCsp` が `null` または未指定だと production `csp` へ fallback するため、開発時 policy を省略しない。

capability は `main` window の `default` だけを設定から明示的に有効化する。core API は event の listen/unlisten、現在の window の状態確認・移動・resize・native close 完了、Dialog の open に限定する。custom command は `tauri_build::AppManifest` へ列挙し、同じ main capability に明示した command だけを許可する。新しい window / capability / command を追加するときは、既存の default set を広げず、その利用箇所と permission を同じ変更で追加する。CSP や capability は backend の入力検証を代替しないため、外部 URL、Launcher path、設定値の Rust 側検証は維持する。

Issue #75の最小集合は次の9 core/plugin permissionと既存の明示custom commandだけ。`test-tauri-security.mjs`はcustomも含む全体snapshot、実policyを使ったdefault/emit/image/menu/tray等の拡張・remote/window/webview/platform scope追加拒否を検証する。新しい許可は利用箇所とsnapshotの両方をreviewする。

| permission | 本番frontendで必要な理由 |
| --- | --- |
| `core:event:allow-listen` / `allow-unlisten` | domain event、AppShellのclose、TitleBarのresize、Launcherのnative DnDの購読/解除 |
| `core:window:allow-destroy` | SDKの`Window.onCloseRequested`が確認不要のnative close後に間接呼出しする。通常終了に必要であり、未使用ではない |
| `core:window:allow-is-maximized` | TitleBarの最大化/復元アイコン同期 |
| `core:window:allow-minimize` / `allow-toggle-maximize` | TitleBarの最小化/最大化/復元 |
| `core:window:allow-start-dragging` / `allow-start-resize-dragging` | TitleBarの移動と8方向のresize handle |
| `dialog:allow-open` | Launcherの`.exe`/`.lnk`複数選択。save/message等のrenderer権限は不要 |

配布候補のWindows検査は変更していない実portable/NSIS exeを起動し、WebView2 loopback debuggerから本番IPC/DOMへ接続する。11実core/plugin commandはrelease固有の`not allowed by ACL`を厳密確認し、command-not-found/feature-disabled/引数errorを代用しない。実HWNDの最小化/最大化/復元・native移動/resize、backendが発行した保存logのlisten/unlisten、所有fixture2fileのnative dialog選択とOLE FileDrop1fileによる本番Launcher登録/解除を検査する。portableはnative closeとSDK destroy、installedはtitlebar/app_exitで通常終了させる。検証記録はPID/source/run/exact artifact digestへ結び付け、未検証項目をpublisherで拒否する。debuggerはfresh GitHub-hosted runnerの所有Rice子tree・loopbackだけに限定し、production config/CSP/ACL・global環境は変更しない。hosted runnerがHigh ILのためWebView2が環境/HKCUのoverrideを無視する場合も、RiceのAppID/exe名だけへ一時的なHKLM browser args/user-data-folderを指定する。既存valueは上書きせず、所有valueと新規空leaf keyだけをfinallyで除去し、wildcard policy・sandbox無効化は使わない（[Microsoft: elevated host overrides](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security#for-an-elevated-host-app-use-appropriate-override-flags)）。

Launcher の `iconDataUrl` は backend で `data:image/png;base64,`、base64部分64KiB / PNG file48KiB、PNGのchecksum・終端・単一frame・最大128×128pxを検証する。PNG decoder作業領域は1MiB、pixel出力bufferは128KiB以内。保存済みの不正/旧上限超過iconは読み込み時に汎用iconへfallbackし、新規追加の上限超過は全体を拒否する。合計data URLは4MiB以内。inline PNGをquotaで制限するため、cache用の追加filesystem権限や`assetProtocol`は有効化しない。

Launcherの資源境界（#71）: 最大200件、pathは各4096UTF-8 bytes・合計128KiB、IDは64 ASCII bytes以内の英数字/ハイフン/下線、表示名1〜120 Unicode文字、group1〜64文字（いずれも制御文字なし）、背景色`#RRGGBB`。追加要求のJSONは256KiB、設定patch/保存JSONは8MiB、要求treeは4096nodes/深さ16まで。Tauriのparse済みbodyを`Request`で借用し、アプリDTOをcloneする前に検査する。framework自体の初回transport parseのallocationを制限できたとは扱わない。

設定patchのLauncher itemsは`LauncherItemEdit`（登録済みIDを参照し、displayName/backgroundColor/groupId/orderだけを更新する置換一覧）へ分離する。新規ID、target/kind/iconDataUrl、未知の編集fieldは保存前に拒否する。canonical登録は`launcher_add`だけが行い、並行追加は最新stateへmergeし、件数/合計quota超過で一部だけ保存しない。永続pathの検査はpureな文字列検査で、metadata編集/設定load時にfilesystem/COMへ触れない。実ファイル検査は登録・起動時に行う。

判断根拠は Tauri v2 公式の [Content Security Policy](https://v2.tauri.app/security/csp/)、[Capabilities](https://v2.tauri.app/security/capabilities/)、[configuration schema](https://v2.tauri.app/reference/config/#securityconfig) に従う。

### backend event replay と speech state snapshot

frontend の `SpeechQueueOutcome` は blocked/各 skipped reason/error の retryable と recoveryAction の組合せまで型で表す。再送不能な writeTimeout/writeFailed/connectionLost/unknown は送達確認を要求する。adapter が受付済みなら理由を問わず送達不明になり得るため、全 error reason に retryable=false/confirmDelivery を許す。`TwitchStatusEvent` は Auth/Chat の判別可能 union とし、各状態の共通定数を型と parser の両方へ使う。validating は Auth、reconnecting と接続世代/identity は Chat、missingRequiredScope は Auth の authRequired だけに属する。従来の正常な省略 payload は受理し、不正組合せは型検査と受信時の parser の両方で拒否する。

Issue #85の各queue itemは任意の`outcome`を持つ。Rust/TS共通の`kind`（blocked/skipped/error）でreasonCodeを区別し、固定の日本語message、retryable、recoveryAction、occurredAtMsを送る。Noneはfield omission（旧payload互換）であり、terminal production itemには必ず理由を付ける。auto retry待ち/送信中は直前のerror理由を保持し、手動retryと正常完了で消す。最新snapshot/reload/late subscriberはwarningの有無に依存せず同じoutcomeを復元する。時刻は遷移時のUTC wall clock（表示用）で、並び順/新旧判定は既存ID/revisionを使う。共通fixture`src/tauri/fixtures/queue-outcomes.json`で全21codeを検証する。

backend は bounded な operational log ring と Twitch（auth/chat）/speech の最新 status を managed state に保持する。`app_events_snapshot` command は listener 登録後にこの状態を取得するため、起動時に先行 emit されたログ・status も late subscriber へ復元できる。各 status と speech queue event には単調増加 `revision` を付与し、`speech_queue_reload` は status と queue を同一ロック下で採取した `SpeechStateSnapshot` として返す（各componentは最後の更新revisionを保持する）。frontend は全 listener を登録してから snapshot を取得し、snapshot より新しい並行 event を古い値で上書きしない。

Twitch の Chat 状態・接続世代・実接続 identity は backend の Chat event/snapshot を正本とする。認証確認・Device Code・解除の command 応答は Auth 状態だけを更新し、認証成功や一時的な通信失敗から Chat の切断を推測しない。チャット開始・停止・終了処理も command の完了から revision のない状態を書き込まず、backend の通知を受け取る。command 自体の失敗は既存の通知と Logs に表示する。将来 UI に要求中の表示を追加する場合も、要求状態として分けて実接続の revision/generation 判定を迂回しない。

保存済み Twitch credential の deserialize は認証済みを意味しない。起動時は `Validating` を通知し、`/validate` 成功後だけ `Connected` へ遷移する。event emit の失敗は stderr だけでなく bounded diagnostic として snapshot へ記録する。

### フロントエンド通知

system Chat の状態通知は中立モデル `models/systemTimeline.ts` の `SystemTimelineEvent` を生成・購読・routing の共通契約とする。source が認証なら認証状態、接続なら接続状態または自動接続の開始/失敗、speech なら読み上げ状態を transition とする。認証で reconnecting、接続で validating は型で排除する。購読の callback は message のみに狭めず、型 assertion で復元しない。`SystemTimelineRouter` が source ごとに直前の重複抑制キーを保持する。認証は transition と案内文、他は transition をキーとし、初回・状態変化・認証案内の変化を記録して連続重複を抑える。

対処が必要な通知は `{ id, severity, source, message, occurredAtMs, correlationId? }` として保持する。`severity` は `info` / `success` / `warning` / `error`、`source` は command / event / log / system を区別する。logs store は対処待ちの warning / error を notifications、info / success を notificationHistory に各100件まで独立して保持する。Side Panel と Status Bar の Warnings は対処待ち通知を最新5件まで表示し、成功通知が対処待ち通知の保持枠を消費しない。warnings.cleared は対処待ち通知だけを消す。`correlationId` がある通知はその値で重複排除し、ID がない既存イベントは本文と 5 秒の受信時間で重複排除する。重複経路で severity が異なるときは、より重大な値を残す。情報履歴から warning / error に昇格した通知は同じIDを保って対処待ち領域へ移す。info / success は Logs と system Chat に残す。

## 永続化

- 一般設定: Tauriのapp data配下にJSON保存。同一ディレクトリの一時ファイルへ書き込み・`sync_all` した後、OSごとの atomic replace で `settings.json` を更新する。直前の正常版は `settings.json.bak` 1世代だけ保持する。
- 永続wireは`settings/schema.rs`でdomain/IPC DTOと分離し、`schemaVersion: 1`を保存時だけ付ける。唯一の既存版である番号なし/null/0のv0からv1への明示段階を通し、正常な値を保持して再保存する。型違い・範囲外のleafはその項目だけ既定値へ戻す。各section/fieldはoptional、通常の補正通知はowner方針により不要。NG listの不正値/合計quota超過は該当listを空へ、必須identity/targetのないLauncherは項目を除外し、重複ID/target・一覧quota違反は一覧を空へ戻す。表示metadataは不正なleafだけ既定値へ戻す。純粋な保存path文字列検証は登録/起動時の実ファイル検証とは別で、load中にfilesystem/COMを呼ばない。
- 未対応のversion（型違い/負数を含む）、重複版番号や解釈不能なroot keyにより版番号が曖昧な設定は、自動接続しないdomain既定値で起動し、対応版で開くか、終了後に本体・backupをコピーして移動する復旧案内を出す。未知field/通常fieldの重複wire keyでは既知の正常な項目だけ読めるが読取り専用とする。いずれも元ファイル/backupを保持し、自動migrationしない。全Settings/Launcher/window保存はtemporary作成/backup更新より前に現在のディスク内容を再検査し、起動後に将来版へ交換された場合も拒否する。Settings IPCは`unsupportedSchema`を返し、終了時の位置保存失敗は終了を妨げない。構文破損primaryから未対応backupを復旧する場合も、そのbytesをそのまま戻して読取り専用を維持する。
- file decoderは8MiB全体をowned `Value` treeに展開せず、Serde JSONの借用`RawValue`と上限付きVisitorで既知fieldを参照する。文字列はraw JSON escapeの上限を検査し、rules/Launcher配列は201件目で打ち切ってからdomainへ変換する。PNG decoderのbounded buffer・既存の時間/heap上限は維持する。API根拠は[Serde JSON RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html)、[Serde map Visitor](https://serde.rs/deserialize-map.html)を参照（lockのcrate版は変更しない）。
- JSON読込/serializerは8MiBまで。上限超過の新設定はtemporary/backupを変更する前に拒否し、候補stateも公開しない。巨大な既存primary/backupはmetadataとbounded readで検出して元fileを退避し、既存の復旧方針を適用する。IO/permission失敗を破損と決めつけて上書きしない。
- 多重起動: 正式方針は同一アプリの複数起動禁止。最初にsingle-instance pluginを登録し、2回目は既存main windowをshow/unminimize/focusして終了する。起動setup完了前の通知は保留して完了時に処理し、引数/cwdをcommandとして解釈しない。設定の読込・初期作成・破損復旧より前に、同じapp dataの固定`settings.writer.lock`を非blockingで排他lockし、process lifetimeのmanaged stateが保持する。全Settings/Launcher/window保存で同じ所有権と保存先を確認する。pluginの通知が失敗しても2つ目のwriterは設定に触れる前に失敗する。lock fileは削除/atomic replaceしない（inodeの分裂を防ぐ）；OSが正常終了/異常終了で所有権を解放する。手動lock削除による起動回避は非サポートであり、他ユーザー/同一ユーザーの悪意あるprocessの隔離機構ではない。
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
