# 実装 TODO

- [x] Issue #209: Twitch接続taskの開始前にgeneration・connection handle・Connecting状態を確定する。mutex共有のconnection ownerでgeneration予約・登録・stop/cancelを順序付け、古い予約の登録拒否、Connecting公開後のstart gate、即時lookup・登録前stop/新接続・登録失敗をdeterministicに検証する。

- [x] Issue #206: Twitch の validate と EventSub refresh を共通の credential service/revision で管理する。成功・失敗・scope 不足・永続化結果を同一 credential revision と照合し、別generation/client/userの再ログインと古い購読が交差しても新しい認証を流用・解除しない。同一auth session内のrefresh token rotationは安全に再購読し、deferred fake transportで各競合順序を回帰化する。

2026-10-06 実装進捗: 共通 credential revision と credential-update lock を導入し、validate と EventSub の refresh・scope 判定・rotation・保存を共通 helper へ集約した。古い success/error/revocation/scope failure と遅延 save/clear は generation・revision・token identity が一致する場合だけ適用する。deferred fake で validate 対 EventSub refresh、refresh 対 refresh、revision 変更後の invalid_grant・遅延成功、同一 generation 内の古い保存、保存中の revision 変更を固定した。architecture と Twitch ingestion の設計メモも更新した。`cargo fmt --check`・`git diff --check` は成功し、公式 Debian DBus package を `/tmp` の sysroot に置いた環境で `cargo test --lib twitch:: --no-default-features` は20件成功した。no-default featureでは `service_tests` が有効にならないため、deferred regressionの実行結果はCIで確認する。no-default clippy は既存のno-app dead-code warningsを許容して完了したが、strict clippy とGitHub CIは未確認。

2026-10-06 レビュー修正: 送信した subscription token と失敗時に照合する EventSubAuthCredentials をrefresh関数の同じ返却値から記録し、待機中に認証が変わった古い401/403は credential clear/AuthRequired ではなく retryable として扱う。deferred subscription fake で送信後に認証をrotationしてから401が戻る順序を追加し、最新のメモリ/保存credential保持とterminal auth errorなしを検証する。logout は共有更新lockを待つ前にgeneration/revisionを無効化し、遅延 validate と logout の順序を回帰化した。test-onlyでしか使われない保存/clear helper wrapperを削除し、本番未使用のprivate itemを残さない。環境DNS制限でgit fetchは失敗したためGitHub REST compareを使用。現在の `main` (`98bfd81`) は `d966846` から45 commit進み、frontend、Auth controller、Settings、timeline、docsのみの変更で、Twitch Rust factory/serviceの追加差分はない。#205 の status presentation factory も frontend側の変更で認証serviceと競合しない。他Issue branchは取り込んでいない。Rust 1.90 app feature Twitch service tests 83件と `cargo clippy --all-targets --features app -- -D warnings` が成功した。

2026-10-06 第2レビュー対応: 同一auth generation内のtoken rotationと別Login sessionを区別する。EventSub接続paramsに認証generation/client/user identityを固定し、古い接続の遅延401やrefresh完了が新しいログイン資格情報で再購読しないようにする。stale refresh応答もgeneration/client/userが一致する場合だけ最新rotationを採用し、別sessionならobsolete接続として静かに終了する。遅延401中の再ログイン、古いrefresh応答中の再ログイン、再接続開始時の旧paramsを実購読経路のdeferred fakeで検証する。最新 origin/main `6916a44` を通常workspaceのfetch済みobjectから専用cloneのorigin/mainへfetchし、mergeした。#208 のterminal supervisor / AppEventState snapshot回帰と既存factory呼び出しも統合し、auth generation保護との競合がないことをレビューした。統合後のRust 1.90 app-feature Twitch tests 88件とstrict all-target clippyが成功した。

2026-10-06 第3レビュー・最新main統合: refresh後に返したaccess tokenと、その送信失敗を照合するcredential snapshotが一対であることを確認する。deferred実購読テストで初回401待機中の別Login、refresh/validate応答中の別Login、同一auth session内のtoken rotationを区別し、古いgenerationの処理が最新tokenで旧user/client条件を送信せず、新しい認証をclear/AuthRequiredにしないことを検証した。StaleCredentialResponse経路もgeneration/client/userを一致させてから最新tokenを採用する。最新 `origin/main` `0d72925`（#194 Settings/Filterのfrontend・依存変更のみ）を専用branchへ統合し、Rust 1.90 app-feature Twitch tests 88件、strict all-target app Clippy、fmt check、diff checkが成功した。

2026-10-06 追加レビュー対応: auth generation変更により subscription が ObsoleteConnection で終了しても Chat generation が同じ場合、supervisorがConnecting等のsnapshotを残したまま taskを終了する問題を修正した。ObsoleteConnection終了時はparamsのChat generationにDisconnectedを記録するため、同じChat generationのみ終了状態になり、AppEventStateがより新しいChat generationのsnapshotを保護する。production AppEventState recorderを使ったsupervisor回帰で、現行Chat generationと新しいChat generationの双方を確認した。reviewed `origin/main` `1743979` のwire contract generationと `ffc391a` の認証復元型付けを統合し、`d0c58b9` のshared chat delivery境界も保持した。`bindings/wire.ts` と `src-tauri/src/wire_contracts.rs` はmainと完全一致し、wire contract generation回帰が成功した。Rust 1.90 app-feature Twitch tests 95件とstrict all-target app Clippyも成功した。#212はmain統合済み、#207の生成境界も含む親レビューを待つ。

2026-10-06 着手計画: auth_service.rs と subscription.rs の認証更新/失効経路、および auth_state.rs・auth_store.rs の generation と永続化境界を調査する。revision を含む共通 service に refresh/validate/rotation/clear/save の判定を集約し、validate 対 EventSub、refresh 対 refresh、scope 不足、遅延保存を deferred fake で検証する。Rust の Twitch 関連回帰、fmt、clippy を実行し、設計文書と実装の整合を確認する。

- [x] Issue #195: AppShellの認証・接続・speech・Launcher・終了保護をcontroller/providerへ分離し、各画面がdomain selector/actionを直接利用する。巨大な旧AppStateの再構成とMainView経由のcallback転送をなくし、無関係な画面の再renderを計測回帰で保証する。認証の遷移は既存のgeneration/poll排他と手動優先を保ち、XState invoke/delayと小さなreducerを比較して判断する。

- [x] Issue #44: Twitchのmodel/error、認証service/store/OAuth、EventSub transport/state/subscription/dedupe/正規化を責務別moduleへ分割する。Tauri commandを薄いadapterにし、型付き状態制御、command/event payload、generationによる競合制御を維持する。fake transport/storeと明示clockを使う既存・追加回帰を分割後の本番経路へ適用し、両OS/feature matrix/native CIで確認する。

2026-10-05段階1: main2b83b6aから専用worktreeで公開chat model、型付きAPI/認証/購読エラーと表示、EventSub wire/正規化、bounded dedupeを4つのprivate moduleへ抽出した。公開型のroot再export、payload、generation、token保存と接続処理は維持する。既存inline回帰をtests.rsへ移動し、mod.rsは4550行から2723行になった。元productionと既存テストはvisibility/format以外のtoken・文字列が同一であることも照合した。文言非依存の分類、明示receive clock/metadata fallback、TTL/capacity等の5回帰を追加し、Rust1.90のall-targets/all-features267件、no-default216件（いずれも0fail/0ignore）、fmt/strict clippy、frontend build、security/workflow/license guardが成功した。同時compile中の最初の全体実行では既存5秒budgetが5.26秒で失敗したが、閾値や条件を変えず単独再実行で4.67秒、no-defaultでも4.50秒の成功を確認した。認証service/store/OAuth、EventSub transport/state/subscriptionと薄いcommand adapter、分割後の両OS/native CIはまだ必要であり、Issueは未完了、mainへは未反映。

2026-10-05段階2: 認証state/service/store/OAuth、chat service、EventSub state machine、subscription、Tauri command/runtimeを独立moduleへ分割した。mod.rsは96行となり、productionの依存は各moduleの明示importで接続する。TwitchAuthService/ChatService/EventSubClientを本番commandから使用し、AuthRuntime/ChatRuntime/EventSubRuntime/SubscriptionRuntime、OAuth transport、credential backendと明示clockをfakeへ差し替えられる。Device Code各応答、同時start/pollと古い応答、refresh保存前後、解除失敗、入力検証、接続交換/停止/解除、購読の型付き失敗/1回refresh、receive/TTL clock、インフラ依存と7 command名の退行検出に17回帰を追加した。3つの認証操作のbodyは依存先への置換を除いて元のtoken/文字列と一致し、HTTP・保存競合kernelも維持する。local Rust1.90 all-targets/all-features284件/no-default216件（0fail/0ignore）、fmt/strict all-features clippy/no-default unused-import検査、frontend322件/format/lint/typecheck/build、権限/Windows proofのNode71件とsecurity/workflow/license policyが成功した。既存5秒performance budgetもall-features4.49秒、no-default4.39秒で成功し、閾値は変更していない。この段階ではmainは変更せず、分割後の両OS/nativeを含むGitHub CI成功を確認してから統合・完了扱いとする。実Twitchアカウントの認可/通信を新たに実施した記録ではない。

2026-10-05 CI調整: source5e28e73では品質全9jobs・依存監査・両OS契約・設定権限・feature matrixが成功したが、Windows nativeのtest harnessが分割前のroot経由でcredential test portを利用していたため、test-only再exportのvisibility不足でcompileに失敗した。本番の権限や挙動を変えず、AuthCredentialStore/AuthLoadResultのcrate内公開をapp有効のtest buildに限定して復元した。twitch_test_ports.rsに同じsibling-moduleからbackendを注入する全OSの回帰を追加し、local285件/0fail/0ignore、fmt/strict clippyが成功した。mod.rsは98行、段階2の追加回帰は合計18件となる。最終sourceで通常6workflowを再検証する。

2026-10-05最終検証: source77d5488b9672a0199bc0818855b8fd92680c641dの[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301154291)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301153814)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301153718)、[Windows実動作](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301154054)、[設定権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301153737)、[機能構成](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37301153794)がすべて成功した。Windows通常286件・実keyring・Launcher41件に加え、通常runではignoreするheadful3件を別stepで明示実行して全件成功した。local285件/no-default216件とfrontend322件も成功している。確認中にmainへ入ったPR179/180のActions更新と別作業のTODO記録を保持し、Rust/frontend/command/eventの入力は検証済みsourceから変えずに統合する。更新されたworkflowのpolicyを再確認し、統合後のexact mainの通常6workflow成功を確認してからIssueを閉じる。Twitch実アカウントでの新たな認可/通信やRelease公開は実施していない。

- [x] Issue #75: main rendererのcore/plugin権限を実際のfrontend利用へ限定し、不要なemit/emit-to/image/menu/trayを拒否する。SDKのonCloseRequestedが間接利用するdestroyは通常終了のため保持する。明示allowlistのsnapshotと拡張拒否回帰を追加し、実Windows配布版でtitlebar・resize・native DnD・複数file dialog・backend event購読を検証する。配布版の成功前には完了扱いにしない。

2026-10-05進捗: core/pluginは既に9権限へ縮小されており、SDKのnative close経路を確認して必要なdestroyを保持した。全customを含むallowlist snapshot、scope/default set/不要commandの拡張拒否、実release ACL拒否判定とloopback debugger対象検査、必須UI証跡の欠落拒否を追加した。関連local133件/0skipとsecurity/workflow/context policyが成功。source a79a9caの通常6workflowは成功し、[初期配布候補](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37280438448)ではWindows全体263件/実keyring/明示headful3件とDocker製の実配布物生成が成功したが、High ILでWebView2の環境overrideが無視されてsmokeが停止した。fresh runnerのRice AppID/exeだけに一時HKLM overrideを指定し、既存値非変更と所有値のfinally cleanupを維持している。

2026-10-05追加進捗: 成功済みsource74ed02eのexact配布物を再利用するread-only診断を用意し、app/config/deps/build入力の差分があれば拒否する。これは最終sourceの同run配布smokeやRelease公開gateの代用にはしない。[source663c195の診断](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37288332050)ではportableの11 ACL拒否、実最大化/復元/最小化・titlebar移動/resize、backend listen/unlisten、native file dialogの2件登録、実OLE FileDropの1件登録、Launcher cleanup、native close後exit0が成功した。native dialogは所有EditへのフォーカスとUnicodeキー入力の実文字列を照合してEnterで確定する。一方、installedではACL/イベント/最大化等は確認できたが、物理入力点のOS hit-testが別ConsoleWindowClassへ向くため移動以降を完了できていない。表示/有効状態・座標・前面HWNDの診断を記録し、検査用windowの一時topmost状態もfinallyで元へ戻す。両配布形式と最終候補の成功前にはIssueを閉じない。

2026-10-05追加検証: [source1720818のread-only診断](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37290826614)で両配布形式の全実操作が成功した。通常の同exe再起動によるsingle-instanceの前面化を準備操作へ使い、元の所有PID/HWNDの前面化と第二processの正常exitを確認する。これをUI操作の証拠として代用せず、その後の物理titlebar/resize・native file dialog複数選択・OLE DnD・backend listen/unlisten・両close経路を別に検証した。両形式で11 ACL拒否、2件選択/1件drop/削除、通常exit0、silent install/uninstall exit0が成功し、診断receiptのsource/run/exact manifestと必須runtime proofもlocalで再照合した。権限集合・production app/config/depsは変更していない。この時点では最終sourceの同run候補と通常6workflowが必要なため未完了としていた。

2026-10-05最終検証: source392247df19013c92710f44986619aca353f89a8cの[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628758)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628304)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628326)、[Windows実動作](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628319)、[設定権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628159)、[機能構成](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628321)と[同sourceからの配布候補全11jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37291628670)が成功した。実portable（PID724、25829ms）とNSIS installed（PID3148、16719ms）の両方で11 ACL拒否・実titlebar/resize・最大化/復元/最小化・backend listen/unlisten・native dialog2件/OLE drop1件の登録と削除・各close経路後の通常exit0を確認した。silent install/uninstallもexit0で、元manifest・形式別exe digest・source/run・必須runtime proof・GitHub実jobs/stepsをtrusted verifierで再照合した。Windows全体263件/実keyring/Launcher41件と明示headful3件、local133件/0skipが成功した。production app/config/depsと9 core/plugin permissionは変更しておらず、配布公開gateを強化した。タグ・Releaseは発行していない。以下の完了記録だけを追加したcommitは配布検証済みsourceとapp/build/test入力が同じである。

- [x] Issue #91: Windowsでlocked/all-targets/all-featuresの全体テストと実secure store/Launcher分岐を継続実行する。manifest/tag由来のexact artifact集合、portable ZIPのCRC/内容/PE、隔離Windowsでportable起動とNSIS silent install/起動/uninstallを検証し、同じartifact bytesのsmoke成功をRelease公開の必須条件にする。OS socket buffer量に依存する既存順序fixtureは実dispatcher permitを制御して検証する。タグ・Releaseの発行なしで候補を検証できるread-only経路も用意する。

2026-10-05進捗: exact artifact/source/lock/CRC/PE/checksum verifierとWindows NSIS/portable probe、同runのreceiptと実Windows jobsをtrusted publisherで照合するgateを追加した。新しいartifact/receipt検証36件を含むlocal49件とworkflow/context policyが成功。source63d45e3の両OS契約は成功したがWindows全体実行はCommon Controls manifest消失によるloader failureで失敗し、Cargo runnerで起動直前に付け直す。runnerはpackage rootで実行されるため絶対pathにする。実全体テストとDocker製の本物のNSIS/ZIPをcandidate dispatchで確認するまでは未完了。tag/Releaseは作成しない。

2026-10-05追加検証: source deac006 の[候補CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37263095537)でWindows全体263件と実keyringの保存・読込・失敗・削除が成功した。全体harnessでignoreされるheadful3件も別stepで各1件/0ignoreとして実行し、Launcher41件、focus、200 tile/16 styles/設定拒否、future schemaの保存保持と正常終了を確認した。[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37263098051)も成功。一方、Docker配布buildはRust Tauri 2.12.1に対してJS API 2.11.0が不一致で停止したため、API/CLIを2.12.1へexact pinし、通常frontend buildとDockerに互換性guardを追加する。新guard6件を含むlocal policy63件とfrontend322件は成功した。実installer/portableのsmokeはまだ未実行で、チェックを緩めず新sourceで再検証する。

2026-10-05追加進捗: source95fc65aの通常6workflowはすべて成功し、[候補内のWindows実動作](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264663033)も263件/実keyring/明示headful3件が成功した。Dockerで本番release exeを生成した後、bookwormのNSISにWin/RestartManager.nshがなくinstaller作成が停止した。NSIS3.11のnative compilerと同版のWindows headers/stubs/pluginsを公式配布のSHA-256固定入力で組み合わせ、実compiler版番号・内部packed version・必要header/pluginによるinstaller生成をlocalで確認した。入力policy13件を含むlocal74件とcontext/workflow/license guardが成功。compiler/source/Windows bundleのhashはbuild材料に残し、実Rice installer/portableのcandidate検証が成功するまでは未完了。

2026-10-05追加検証: source1c2c403の通常6workflowが成功し、[候補CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37270108901)ではDocker製の実installer/portable生成、exact artifact/CRC/PE/LICENSE/SBOMが成功した。Windowsは起動前のLICENSE byte比較で停止した。Gitのcore.autocrlf=trueによる同じ不一致を再現し、正本/LICENSE・2 lockfiles・Cargo.tomlをLF checkoutへ固定する。元bytesと検証条件は維持し、属性あり/なしのclone回帰2件を含むlocal83件が成功した。実portable起動/NSIS install/起動/uninstallは未確認であり、修正後sourceの候補で継続する。

2026-10-05追加検証: source2f5fdaeの[候補CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37272350673)でWindowsの改行回帰2件、portableのnative window/9305ms生存/正常exit0、NSIS silent installのexit0が成功した。installed比較の不一致は、固定Tauri CLIの正規のbundle-type patch（UNK→NSSの3byteのみ）と実payloadの独立展開で一致した。元portableを保持したまま、NSISのexact期待hashをmanifestへ記録し、両形式ごとの完全なdigest比較へ修正する。installed起動/uninstallと修正後sourceの再検証が終わるまでは未完了。

2026-10-05最終検証: source74ed02eの[品質9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393328)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274392934)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393311)、[Windows実動作](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393332)、[設定権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393175)、[機能構成](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393079)と[配布候補全工程](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393563)が成功した。Windows全体263件、実keyring、Launcher41件、明示headful3件が成功。実portableは6649ms、installedは5144ms生存してnative windowを表示し通常closeでexit0、NSIS silent install/uninstallもexit0となった。installed/LICENSE/version/registrationとuninstall後の除去、隔離cleanupも成功。同runのreceipt・exact manifest・GitHub最新jobsをtrusted verifierで再照合し、NSIS payloadの独立展開でも期待exeと正本LICENSEのdigest一致を確認した。local93件/全policyも成功。タグ・Releaseは作成しておらず、第三者通知の未完了境界#102は別に残す。

- [x] Issue #64: versioned persistence wireをdomainから分離し、旧schemaを段階migrationする。全fieldはoptionalとして欠落・型/範囲/意味違反を既定値へ戻し、#88の共通validatorでload/saveの不変条件を揃える。未知future version/未知fieldを黙って捨てず、設定・Launcher・window保存と終了からの上書きを防ぐ。fixture、primary/backup保持、writer、両OS/native CIで検証する。

2026-10-05 local検証: v0（番号なし/null/0）→v1、14 fixtureと7 schemaテスト、元bytesを保持するmigration backup、保存直前の将来版への交換、future backupの復旧を実装した。Rust all-features262件/no-default211件とfmt/strict clippy、frontend321件とformat/lint/typecheck/build/security/license、policy21件、Docker contextが成功。最大8MiB roundtripは4.56秒/追加Rust heap25.5MiBで既存予算内。Windows本番起動/Settings・Launcher IPC拒否/正常終了の追加テストと両OS CIが成功するまでは未完了とする。

2026-10-05最終検証: source276c720の[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260473477)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260470825)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260477961)、[Windows本番実動](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260475850)、[保存権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260479932)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260481897)がすべて成功した。両OSのschema7件と通常不正値のsilent fallback、重複版番号の安全な既定値/read-only、実primary/backup保持を確認した。Windows実WebViewで本番startup、Settings/Launcher IPCの保存拒否、memory/元file/backup非変更、正常exit、settings一覧3file、隔離storage cleanupが成功（verified:true、0 ignore）。既存Launcher41件/focus/16 stylesを維持し、200 tile描画626ms/設定IPC490ms/JS heap差分33.0MiB、8MiB roundtrip Windows3.50秒/Linux2.75秒で予算内。最終local frontend322件と全gate、Rust262/no-default211も成功。最初のnative失敗はWebView storageの隔離とHashRouterに消されない通知経路を修正して解消し、拒否・保存保持の検査や監査閾値を緩めていない。配布物全般のsmoke/第三者通知は#91/#102の未完了境界として残す。

- [x] main CI の依存監査: Tailwind 3 のビルド依存から入る braces（GHSA-vfj7-8cjw-p6xm）を依存グラフから除去する。公式 Tailwind 4/Vite 構成へ移行し、既存配色・日本語フォント・寸法・キーボード focus を保持する。作業ブランチの監査・品質・Windows native CI の成功を確認してから main へ反映する。監査の閾値・例外・失敗条件は緩めない。

2026-10-05: source af59b8c の作業ブランチで [audit](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928344)、[quality 全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928538)、[Windows native](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928782)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928450)、[settings permission](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928626)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928430)がすべて成功した。frontend 289 件（実Vite経路のCSS/runtime要件7件を含む）、Rust all-features244件、strict clippy/fmt、installed graph SBOM6件も成功した。実WebViewは既存色/日本語フォント/mono/156px tile/4rem icon/二層shadow/keyboard focus の16検査が成功し、200 tile描画704ms・設定IPC766ms・JS heap差分45.3MiB・不正4要求拒否/設定保持・2process復元/focusを確認した。braces/micromatch/chokidar/fast-glob の経路を除去し、npm High/Critical は0、Moderate8件と既存Rust例外2件は残る。新規releaseの実配布/通知/packaged smoke（#91/#102）は別に未完了で、CI成功を配布準備完了へ換算しない。以下の古いissue検証メモにある braces 配布停止は、各検証時点の履歴である。

- [x] Issue #88: 設定patch/Twitch login/endpoint/NG ruleをbackendの共通domain validatorで検証し、未知field・文字数/UTF-8/総量境界をstructured errorと共有fixtureで保証する。棒読みちゃんは通常loopback限定、remoteは明示mode・native consent・解決済みaddressの照合/固定を全送信経路へ適用し、renderer/file変更とDNS rebindingを拒否する。

2026-10-05 local検証: mainのCI復旧commit138fc80へ基点を更新し、Rust 1.90のall-features255件/no-default204件とfmt/strict clippy、frontend321件とformat/lint/typecheck/build/security/license、policy21件、Docker contextが成功した。実settings fileの偽consent拒否、primary/backup/memory非変更、DNS変更後に元IPへ戻っても再許可が必要な7つのdestinationテストを含む。最大200 icon/4MiBと有効NG rulesの設定を8MiBまでpaddingしたroundtripはlocal3.81秒/追加Rust heap25.5MiBで予算内。Windows本番IPC/両OS検証と作業ブランチ6workflowの成功を確認するまでは未完了とし、mainへ反映しない。

2026-10-05最終検証: source ec2d2ffをmain反映前に[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137470)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137491)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137393)、[Windows本番実動](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137483)、[settings権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137321)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137339)の全6workflowで確認した。両OSの共有domain fixture2件・destination7件・実file改変/backup非変更各1件が失敗/ignoreなし。実WebViewでstructured invalid invoke6件、remote flagだけのprobe/diagnostics2件、Launcher不正4要求を拒否し、設定復元と終了も成功。既存16 CSS検査、Launcher41件とfocusを保持した。200 tile描画692.8ms/設定IPC493.9ms/JS heap差分37.6MiB、8MiB設定roundtripはWindows3.58秒/Linux2.40秒で予算内。実LAN相手・手動native dialog操作・配布物全般のsmokeは代替せず、旧fileのoptional migrationは#64に残す。監査は既存Rust2例外（2026-10-21まで）を残して成功し、新しい例外や閾値変更はない。

- [x] Issue #76: Windows shortcutを起動直前に構造化して検証し、壊れた/移動したtargetをfailureへ返す。起動依頼の受付と対象起動の確認を区別し、日本語の修復/再登録案内、arguments/working directory、権限要求と直接exe/部分成功の回帰をWindows integrationとUIで検証する。

2026-10-05: UIの全failure表示/準備完了未確認、async serviceの4-worker/cancel-before-spawn、header/target/cwd/arguments検証を実装した。LinuxでRust all-features244件/no-default193件、strict clippy/fmt、frontend282件とbuild/security/license/quality policy3件/context検査が成功した。Windowsの39件は成功したが、WSH fixtureの日本語TargetPath代入に失敗し、Issueは未完了。実在/MZ/109 UTF-16 unitsを確認し、本番/fixtureをIShellLinkW Unicode APIへ切り替えた。既存lockのwindows 0.62.2を参照するだけで依存版は不変。実Windows再検証までは完了扱いにしない。

2026-10-05最終検証: commit e5791e3の[本番feature Windows CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246369941)でLauncher全41件（実Unicode COM link/args/cwd/移動/破損/RunAsUser/直接exe/部分成功を含む）、2process復元/focus、200 tile描画695ms/最大設定IPC715ms/JS heap差分43.2MiB/不正4要求拒否・設定保持・正常終了が成功した。[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246369853)は各Launcher39件、8MiB設定roundtripはWindows3.13秒/Linux2.26秒で予算内。[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246370049)、feature matrix、Windows writer/permissionも成功した。先のWSH失敗はUnicode APIへ切り替えて解消した。対話的UAC/外部アプリready/実UNC停止/配布物は未検証の別境界とし、[既知npm Highの配布停止](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246369811)を維持する。

- [x] Issue #82: Launcherをpure model・service・repository・commands・Windows icon/launch adapterへ分離し、本番serviceへ小さなtraitを注入する。timeout・抽出/shortcut/spawn/保存失敗・同時更新を全OSのfake adapterで再現し、Windows実動境界を文書化する。

2026-10-05: #82の責務分割と本番serviceの依存注入を実装した。9つのfake serviceテストを追加し、Rust all-features239件/no-default188件、strict clippy/fmt、frontend281件と全frontend/build/security/license gate、quality policy3件、Docker context検査が成功。元の200件/quota/atomic保存/4-worker・timeout境界とIPC契約を維持する。commit8579773の[両OS契約CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156624)でそれぞれLauncher34件、[Windows本番feature/実WebView](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156691)でLauncher34件と200 tile描画549ms・最大設定IPC547ms・JS heap差分43.7MiB・不正4要求拒否/設定保持・2process復元/focusが成功した。[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156974)、feature matrix、Windows writer/permissionも成功。実shortcutの意味論は#76、packaged smokeは#91と区別し、既知npm Highの配布停止を維持する。

- [x] Issue #71: Launcher request/文字列/PNG encoded・decoded/dimensions/合計icon量/settings JSONへ上限を設け、renderer編集DTOからbackend生成ID/target/iconを変更できないようにする。巨大・不正入力、重複path、quota境界、200件の予算、上限超過時のdisk/memory非変更を検証する。

2026-10-05: #71の資源上限、borrowed Request preflight、編集DTO、bounded読込/serializer、quota超過の全体拒否を実装した。Rust all-features230件/no-default179件、frontend281件とstrict clippy、frontend全gate、quality policy3件が成功。最大200件（icon合計4MiB、128×128 RGBA16 PNG200種類）の保存/backup/loadは、JSON自体8MiBでもlocal3.93秒/追加Rust heap39.4MiB、[Windows/Linux CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37239996196)でWindows2.69秒・Linux2.44秒だった（5秒/最大JSON40MiB予算）。[実native Windows](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37239996083)は200 tile/image decode完了617ms、最大設定IPC取得734ms、JS heap差分43.2MiBで2秒/64MiB以内、不正4要求の拒否と設定非変更・正常終了を確認した。同commit fd8a00aの[quality全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37239996525)、feature matrix、Windows writer/permissionも成功。JS heapは診断用推定であり全WebView RSS/GPUの保証ではない。frameworkの最初のIPC parse/実配布物smokeは別境界で、既知npm Highによる配布停止は維持する。

- [x] Issue #85: blocked/skipped/errorの型付き理由・安全な日本語説明・復旧操作・発生時刻を項目へ保持し、snapshot/reload/late subscriberとChatへ伝える。Queue/Chatの詳細導線、item ID付きwarning/log、履歴上限と機微情報非複製を契約/DOM/fake-workerで検証する。

2026-10-05: Issue #85で全21codeのkind別outcomeを導入し、formatter/連投/overflow/skip/remove/clear/adapter failureへ付与した。自動retry中は直前理由を保持し、manual retry/正常完了で消す。safe日本語へNG一致語/adapter detailをコピーせず、warning/logにitem IDを付ける。Chatの同status理由更新・非modal詳細paneとfocus、Queueのskip履歴toggle/削除・復旧routeを追加した。共通fixture、最新snapshot/reload/late subscriber、200件上限、取消/遅延結果9順序と6,144操作列を含むRust all-features219件/no-default168件、frontend279件、strict clippy/fmt、format/lint/typecheck/build/security/licenseとquality policy3件が成功。[Windows/Linux契約CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933123)、[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933304)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933100)、[Windows実動focus](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933133)も成功した。実棒読みちゃん/配布物全般のsmokeは#91と区別し、既知npm Highによるrelease blockは維持する。

- [x] Issue #66: 多重起動禁止を正式方針とし、2回目起動で既存main windowを復元/focusする。設定のprocess-lifetime writer lockを読込前に取得し、全saveを同じ所有権で保護する。2 processの競合と終了後の解放を自動検証する。

2026-10-05: Issue #66でsingle-instance pluginを最初に登録し、setup前のactivationを保留してshow/unminimize/focusする。固定lockを設定読込/初期化/復旧の前に取得してprocess lifetimeで保持し、全保存経路で所有権を確認する。別OS processによる競合、正常終了/kill後の解放、別section更新の保持、保存先/permission/link拒否を追加した。Rust all-features210件/no-default159件、strict clippy/fmt、frontend238件、format/lint/typecheck/build/security/license検査が成功。Windows native CI [37233983947](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233983947)で本番builder/pluginを使った実HWND復元/foreground一致、2回目の正常終了、設定非変更、owner正常終了と隔離WebView storage削除をすべて確認した。Windows headless writer/permissionと9 quality jobsも成功。Windows配布物全般のsmokeは#91と区別する。更新後の依存監査はRust既知例外2件だけで追加指摘なし；npm braces High1件によるrelease blockは維持する。

- [x] Issue #72: 本番enqueue/control/workerをfake adapter・clock・sinkで再生し、200件overflow・複数ユーザーの連投境界・取消と遅延結果・pause/resume・並行enqueue・復旧/手動再試行・snapshotと操作列の不変条件を網羅する。

2026-10-05: Issue #72で本番enqueue/control mutationをTauri wrapperから抽出し、#70のfake workerへ16シナリオを追加した。送信中を保持する200件overflow、0/1/2/30秒と複数ユーザー、取消/遅延結果9順序、pause/resumeと複数barrier、699/700msとretry待ち中clear、復旧後の手動再試行、async 2世代/20 native threadの単一worker、固定3seedの6,144操作、snapshot/NG/自動OFFを検証した。テストで満杯の手動再試行が201件になる不具合を再現し、空きがなければ履歴を保持して拒否するよう修正した。Rust all-features203件/no-default154件、strict clippy/fmt/build/securityが成功。Windows/Linux CIにもfake workerと純粋model検証を追加した。実棒読みちゃん/Windows配布WebViewの手動smokeは別に追跡する。

- [x] Issue #70: object-safeなSpeechAdapterとapp stateのfactory/共通dispatch境界を導入し、queue workerへadapter・clock・event sinkを注入する。health/test/controlも同じ選択を使い、fake adapterで成功・失敗・遅延・再試行を検証する。

2026-10-05: Issue #70でboxed futureのadapter、共通failure/completion/control、設定factoryとsession境界を導入した。workerはTauri/実TCP/具体adapterから独立し、同じ本番schedulerへfake adapter/clock/sinkを注入する。FIFO、1回再試行と上限、到達不明/受付後未確認の非再送、送信待ちの所有権/gate、完了待ち中の制御、全operationの共通選択、選択失敗の9件を追加した。設定された声質の実packet契約もsession経由で保持する。Rust all-features187件/no-default138件、strict clippy、fmt、frontend238件、format/lint/typecheck/build/securityが成功。操作列・並行enqueueの追加網羅はIssue #72で行う。

最終調査日: 2026-10-05


この TODO は `docs/06-implementation-roadmap.md` の Phase に沿って、現在の実装状況と次に進める作業を追跡するためのものです。作業を始める前後に該当項目を更新してください。

調査メモは [`docs/RESEARCH_NOTES.md`](./RESEARCH_NOTES.md) に分離し、日付が新しいものほど上に追記してください。

## 現在の進捗サマリ

- [x] Issue #211: speech の queue・formatter/URL・commands・runtime/event mapper を責務別 module へ分離し、明示 import と最小公開境界に整理する。既存回帰と no-default/DTO 契約を維持し、URL 検出 crate の比較と互換処理の範囲を記録する。

Issue #209では、接続taskをspawnしてからhandle登録・Connecting通知を行っていたため、即時lookup失敗のErrorをConnectingが後から上書きし、登録失敗時にtaskがdetachする競合を解消した。oneshot開始gateで登録・Connecting通知後にlookupを開始し、TwitchConnectionHandleのDropが所有taskをabortする。追補では予約・登録・cancelを同じmutex保護のTwitchConnectionOwnerへ集約し、古い予約の登録拒否を登録直前 barrier でstop／新接続の両順序から検証する。即時 lookup 成功／失敗はmulti-thread runtime上でconnect return前のlookup開始を同期し、Connectingとterminal statusの順を確認する。reviewed main `c868999` までを統合し、追加差分はfrontendと文書の変更でRustのTwitch接続ownerに重ならないことを確認した。Rust 1.90 all-features Twitch tests 100件、strict all-target clippy、fmt check、diff checkが成功した。親レビューで登録前stop/新接続・開始gate・Drop取消を確認した。reviewed main 928f1f6のspeech session制御を統合し、最終CIはPR #252で確認する。

Issue #220: 再生中itemのadapter所有権を保持し、設定変更後も制御と完了確認を同じ宛先へ送る。後続itemから新しい設定を使う。

Issue #219: 認証解除の要求を認証状態から分離し、失敗後に再試行できる調停を追加した。PR #264で最終検証を確認する。

Issue #218: 起動時認証復元より後発の手動ログインを優先する世代予約を追加した。PR #263で最終CIを確認する。

Issue #216: 設定初期化に読込状態・世代・更新番号を導入し、古い読込による保存結果の巻き戻りを防いだ。対象23件の回帰成功、最終 CI は PR #259 で追跡する。

Issue #261: 共通CIを止めた source-map-js advisoryを修正版へのlockfile統一で解消した。最終CIはPR #262で確認する。

2026-10-06 Issue #211: queue model/遷移を `queue.rs`、formatter/URL を `formatter.rs`、queue Tauri commands を `queue_commands.rs`、event snapshot/mapper を `events.rs` へ分離し、speech 回帰は `tests.rs` へ移した。`mod.rs` は共通型と境界の組立へ縮小。LinkFinder を候補抽出に使い、候補がない場合は旧 parser の全体走査、角括弧付き IPv6 は追加走査で互換性を保つ。厳格な http/https/www、authority/port、ASCII・メール境界、日本語隣接の条件は formatter 側に維持した。reviewed main `2a06b79` の #220 active playback session ownership と #209 Twitch task ownership、および #216/#218/#219/#261 の reviewed 更新を統合。他Issue branchは含めていない。統合後のRust 1.90 all-targets/all-features 312件は成功。no-default は 223件成功し、既存の5秒 launcher performance regression は全 suite 実行で7.04秒、serial実行で5.29秒と閾値を超えたため隔離して再実行し4.54秒で成功した。残りの no-default 223件、app-feature strict all-target Clippy、fmt check、diff check も成功。#209 統合後はTwitch関連回帰を追加検証する。

- [x] Issue #215: live 通知を ID・severity・correlation を持つ未通知 queue として扱い、command error・同文の別発生・同時障害の欠落を防ぐ。状態/event/log の同一障害は重複を抑え、実 DOM の配送・クリア・再通知を検証する。

2026-10-06 Issue #215: warning/error を ID・severity・correlation と明示の状態 context で受ける配送 queue に変更した。alert を優先し、同優先度は発生順に読み、同文の別IDには安定した live region の空更新を挟む。状態 summary とその event/log/command は同じ原因をまとめ、同時に起きた別障害や状態が変わらない別IDを残す。明示 clear と unmount では残る通知/timer を片付け、古い既存通知が次の障害を消費しない。frontend 全423件と追加の同一状態・別障害・長時間保持回帰14件、typecheck/format/lint/build が成功。最新 main 統合後の最終検証・CI は PR #258 に記録する。実スクリーンリーダーの発話確認は未実施。

2026-10-06: Issue #206 の認証更新は credential revision と共通更新 lock で統合し、subscription token identity/logout orderingと別Login generationの隔離を回帰化した。EventSubConnectionParamsは接続generationと認証generation/client/user identityを区別して保持し、旧接続をobsoleteとして再試行せず終了する。追加レビューで、auth generationのみ変更された際に現行Chat generationのsnapshotがConnectingのまま残る問題を修正し、AppEventState recorderで同一Chat generationはDisconnectedに、新しいChat generationは維持されることを回帰化した。reviewed main `d0c58b9` までのwire contract、認証復元型付け、共通chat delivery変更を統合した。Rust fmt / diff check、app-feature Twitch tests 95件、strict all-target app Clippy、generated wire contract test 1件が成功した。親レビューで obsolete 終端 snapshot と新世代保護を確認した。実装 head 9fcbab0 の全16 CI が成功し、#214 の reviewed main c58904d を統合した。最終 head の CI と統合結果は PR #243 に記録する。

- [x] Issue #214: コメントの受信と読み上げ受付を区別し、自動読み上げ OFF の対象外結果を backend の型付き outcome として通知・保持する。ON/OFF 切替、event の前後順、snapshot 復元を契約テストで確認する。

2026-10-06 Issue #214: backend の OFF 早期 return を理由付き skipped history の保存・通知に変更し、frontend の初期表示を received とした。message/queue の到着順、snapshot、現在の設定が受信時と逆の場合を実 AppShell で検証し、関連48件・frontend全420件が成功した。設定 snapshot の ON/OFF 判定、非 enqueue と履歴200件上限を含む no-default Rust 読み上げ107件、Rust由来の wire 型生成、typecheck/format/lint/build が成功した。PR #257 で app feature と最終 head の CI・統合結果を記録する。

2026-10-06: Issue #207 で EventSub 正規化時に ChatMessage へ接続 generation を付け、同じ値のまま UI と speech へ渡す。親レビュー対応で generation 検証と両sink配送を共有 `dispatch_chat_message` に集約し、本番 runtime と fake が同じ境界を使用する。回帰は同一 channel の世代交換、旧世代の遅延通知、停止後の通知、別 channel、UI/speech 両sinkの同一内容と順序を確認する。main `2802a4a`、`98bfd81`、`6bdb52c`、`6916a44`、`0d72925`、`493c57f` を統合。#203 の `7e880c4` と #213 の変更は frontend と docs に限られ、今回の Rust 本番処理との重複がないことを確認した。Tauri非依存 boundary test、strict app-feature Clippy、frontend 414件、format/lint/typecheck/build が成功。親レビューで本番/fake 共通配送と generation 保持、strict Clippy 指摘の修正を確認した。最終 app-feature runtime 回帰、exact-head CI と統合結果は PR #248 に記録する。no-default strict Clippy は既存 dead_code 警告群で失敗するが、警告抑制なしの通常 no-default Clippy と対象 unit test は成功した。実 Twitch 環境の手動確認は未実施。

- [x] Issue #212: 認証復元結果に scope 不足・保存先障害・破損・移行の型付き reason を保持し、composition root の日本語部分一致を除去する。文言に依存しない状態通知と secret 非公開・旧 store 移行・失敗時保持を検証する。

2026-10-06 Issue #212: AuthLoadResult に型付き reason と表示文を持つ notice を導入し、composition root の日本語部分一致を除去した。scope 不足は型付き error を復元まで保持し、破損 JSON は入力値を含まない固定文へ変換する。secure/legacy の分類・移行と失敗時保持、文言に依存しない起動時遷移の回帰を追加した。app feature で新規回帰4件と Twitch 回帰84件が成功し、実装 head f52025c の default/no-default・strict clippy・wire 型生成を含む CI も成功した。差分レビュー済み。文書追補後の最終 CI と統合結果は PR #256 に記録する。

Issue #199 は system timeline の中立モデルと型付き購読境界、source 別 transition 契約を実装した。初期 snapshot の認証/speech 通知、連続重複と復旧後の再通知、購読終了後の無視、不正 callback の型エラーを検証した。独立レビューで認証/接続の状態集合をさらに限定し、案内文を含む認証の重複抑制を維持した。最終 CI 結果と統合状況は PR #239 に記録する。

2026-10-06 Issue #203: appReducer/appStore.test の旧状態処理を除去し、AppState は読み取り用合成モデルとして appState.ts へ分離した。ログ ID／通知重複処理は logsStore を正本にし、明示 backend event ID の replay は抑制、IDなし同内容ログと表示IDの衝突は suffix で保持する。domain event bridge は store が受理したログにのみ通知等の副作用を行う。SettingsUpdateQueue と専用テストを削除し、失敗前から待機している後続保存の継続と idle 待機を createSettingsMutationOrchestrator で検証する。親レビューで旧 reducer の observable regression 4件を DomainStores/dispatchDomainAction の本番経路へ移し、最新 reviewed main 6bdb52c も統合した。frontend 366件、typecheck/build、変更ファイル Biome check と diff check が成功。親レビューと本番経路15 tests の再検証、実装 head 5185791 の全16 CI が成功した。その後 reviewed main 493c57f（Settings 分割・EventSub 終端状態・wire schema）を統合し、frontend 405件、format/lint/typecheck/build と diff check が成功した。最終 head の CI と統合結果は PR #251 に記録する。

2026-10-06: Issue #195の実装を専用 Draft PR #236 に分離した。AppShell配下へ controller/actions provider を組み立て、Twitch認証の非同期遷移、speech/queue/Launcher command、終了保護を責務別 controller/provider へ移した。画面はdomain別の安定action Contextと必要な selector を参照し、旧AppStateの再構成を除去した。初期レビューで見つかったDevice Code pollingのproduction lifecycle未接続、認証結果遷移の分散、実画面render計測の不足、手動操作/終了時の遅延応答競合を修正し、本番AppShell/provider/routes経由のtimer/render回帰へ更新した。親レビュー指摘を解消し、#205 の共通ラベルとの統合後は frontend 362件、format/lint/typecheck/build と diff check が成功。最終 CI と main 反映は PR #236 で確認する。

2026-10-06 レビュー対応: Device Code pollingをAppShellのprompt/status lifecycleへ接続し、初回interval、pending/slowDown後のinterval更新、手動start/validate/disconnectの競合、期限切れ、unmount中の遅延応答/restore callback抑制をcontrollerと本番AppShell経由の回帰で確認した。追加レビュー対応として期限切れtimerもschedule時のgenerationとprompt identityを照合し、手動start/validate進行中のdeadline callbackと期限到達済みpromptの即時expireを遅延応答テストで保護する。#200のdiscriminated state/runtime contractsを含む最新mainを統合して全 frontend gate を再実行する。timerはschedule時のgeneration/promptを照合し、AuthOperationControllerは世代付き完了、手動優先、poll排他、dispose invalidationを管理する。認証結果・prompt・profile・statusと通知/error副作用は小さな純粋遷移モデルへまとめた。render回帰は実AppShell/provider/routes上のSettings/Logs/Launcher各bodyをProfiler計測し、queue revisionのみの連続更新を確認する。PRは親レビュー再確認待ちのためDraft、Issueはmain反映まで未完了。

Issue #198 はテストの明示的 any を実 DTO／関数型へ置換し、既存 Biome gate に any・enum・const enum・namespace の検査を追加した。frontend 322件と品質 policy 5件、format/lint/typecheck/build が成功し、独立レビューを完了した。最終コミットの CI 結果と統合状況は PR #238 に記録する。

2026-10-05: Dependabot全13件（#179–#182、#184–#192）のレビューと互換性修正を完了した。Tauri dialogのJS/Rust版一致検査、React 19の型・ref初期値・TitleBar DOMテスト移行を実施した。マージと後片付けの条件・検証結果・既存Windows入力の不安定性はPhase 5のDependabot項目に記録する。

- [x] Issue #69: adapter healthとqueue phaseをbackend/frontendで独立保持し、paused中も無音probeを継続する。復旧は失敗項目を自動再送せず、UIの接続/準備完了を両状態から導出する。

2026-10-05: Issue #69でqueue活動から接続状態を推測する処理を除去し、明示的なhealth結果だけがadapterHealthを更新するようにした。無音probeはconnected/paused中も5秒周期で継続し、重複probeと終了後の遅延通知を防ぐ。復旧してもpaused/失敗項目の手動再試行待ちは維持し、失敗履歴があるだけで後続pendingを停止しない。Status Bar/Side Panel/live announcementは両状態を分離表示し、起動ガイドはhealth・phase・自動読み上げONから準備完了を判定する。終了保護とhotkeyもqueue phaseを使う。共通fixture5順序、paused中の切断/復旧DOM、probeのfake clockを含むfrontend238件、Rust all-features178件/no-default130件、fmt/clippy/format/lint/typecheck/build/securityとquality policy3件が成功。

- [x] Issue #77: connect/write/response/config/protocolを型付きerrorで区別し、queue・health・test・controlの状態と短い日本語案内を統一する。詳細原因はLogs、再試行は未送信の一時的接続失敗だけとする。

2026-10-05: Issue #77でBouyomiErrorと共通classificationを導入し、表示文/OS番号の部分一致を除去した。接続拒否は全経路でDisconnected、設定/protocol/unknownはError、connect/write/response timeoutは別codeとする。nativeの分類をfrontendの楽観的statusで上書きしない。cause chainはLogsへ保持し、write失敗/timeoutと受付後失敗は重複防止のため自動再送しない。Rust all-features176件/no-default129件、fmt・strict clippy、frontend227件（native reject分類6件を含む）・format/lint/typecheck/build/securityが成功。Windows/LinuxのErrorKind/native mappingとfake transportの継続CIを追加した。health/queueの独立保持は#69で追跡する。

- [x] Issue #92: Node unitとjsdom component projectを分離し、共通Tauri mock/cleanup・全routeのrender・フォーム操作/validation/focus・StrictModeの1event/1更新を検証する。

2026-10-05: Issue #92でVitestのNode/jsdom projectを分離し、同じpnpm testと共通CI gateで実行する構成を追加した。全7 route、Settings/Filterの入力・Tab・validation・保存・キャンセル/破棄、Launcherの部分成功/非Windows、Queue snapshot、command rejectのLogs表示、StrictModeの1 event/1 store更新/1 Chat行と遅延購読のcleanupを検証する。cleanup前のresetでlistener漏れを隠さず、未処理Promiseも失敗にする。unit209件+DOM12件、format/lint/typecheck/build/security、quality policy3件が成功。viewportは固定mockであり、実WebView/Windows smokeは#91で扱う。

- [x] Issue #79: backendの型付きplatform capabilityでLauncher登録/起動を制限し、非Windowsは保存前に拒否する。UIも選択・DnD・起動を無効化し、既存項目の表示/削除は維持する。

2026-10-05: Issue #79でapp_build_infoへ型付きLauncher capabilityを追加した。非Windowsは登録commandと設定patch経由の新規/target変更を保存前に拒否し、起動もfilesystem確認前に拒否する。UIは取得失敗/未取得時も安全側で無効化し、OS標準ランチャー/Windows版を案内する。native/browser/malformed契約6件、Windows/非Windows分岐と保存非変更を含むRust169件、frontend209件、fmt/clippy/typecheck/build/securityが成功。Windows実機smokeは#91で追跡する。

- [x] Issue #86: PR/main/releaseで共通のfrontend format・lint・typecheck・test・buildとRust fmt・all-features clippy/testを独立jobで実行する。固定toolchainで既存静的検査違反を解消する。

2026-10-05: Issue #86で共通workflowとquality-gate.shを導入した。Biome 2.5.15のformatter/linterを型検査から分離し、TS/TSXを機械整形した。frontend203件とformat/lint/typecheck/build、quality policy/負例3件、Rust 1.90 fmt・all-targets/all-features clippy（warnings deny）・165テストが成功。no-default詳細遷移、Windows実機検査、advisory gateは独立した検査として継続する。

- [x] Issue #81: MVPのSpeechRequestから未実装の項目単位音声overrideを除外し、未知の項目を明示拒否する。設定単位の声質は維持し、request/adapter契約をテストする。

2026-10-05: Issue #81で未使用の4つのrequest overrideを除外し、各項目の数値・文字列・null入力を拒否すること、およびtrait経由のTCP送信で設定された全声質値がpacketへ反映されることを確認した。Rust 1.90のdefault165件/no-default119件とfmtが成功。既存clippy違反4件は#86で対応する。

2026-09-21: Issue #173 で RustSec の残存7件を Tauri 上流由来として再確認した。公式 crates.io の Tauri 2.11.6 候補も GTK3 0.18 / webkit2gtk 2.0 / urlpattern 0.3 の制約を残し、今回の lockfile 互換更新で解消しない。Windows graph に unic 系が残り、glib / proc-macro-error は Linux GTK3 graph に限られる。glib の unsound API と呼出し有無を調査し、実 audit は exception を読んで成功した。例外には owner・根拠・2026-10-21 の期限を記録したが、期限検証と release audit gate は未統合の Issue #96 が担当するため、この時点を公開可能な clean audit と扱わない。

- [x] Issue #173: Tauri 2.12.1 / Utils 2.10.1と固定Rust 1.90.0への更新でunic系5警告を解消し、残るLinux GTK3の2警告だけをowner・根拠・2026-10-21の期限付き例外として検証する。

2026-10-05: Issue #173でTauri 2.12.1 / urlpattern 0.6へ更新し、unic系5crateと対応する例外を除去した。compilerのMSRV更新に伴いrelease/devcontainerのRust 1.90.0 imageをdigest固定した。Rust default163件/no-default117件、例外/immutable input/ bootstrap policy検査が成功。Windows MSVC graph306 nodeにglib/gtk/proc-macro-errorとunicはないことを確認した。RustSecの2件はLinux GTK3由来の残存リスクであり解消扱いにしない。全dependency監査は別のnpm braces Highで停止しており、公開可能なclean auditではない。

- [x] Issue #101: 既存MIT正本にpackage/Cargo/bundle metadataを揃え、NSIS/portableへLICENSEを同梱し、contributionと自動検査を追加する。

2026-10-05: Issue #101で権利者が既に配置したMIT正本を維持し、npm/Cargo/bundleとREADME、inbound=outboundの貢献条件を整合させた。NSIS license表示とinstalled resource、portable ZIP、直接Release assetへ同じLICENSEを同梱する。欠落/不一致/同梱漏れの7件のpolicy test、cargo check、Docker context/release guardsを確認した。Windows artifactの実行・同梱確認は配布smoke gateの検証と合わせて行う。

2026-10-05最終検証: [source74ed02eの実配布候補](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393563)で、直接artifact/portable ZIP/NSIS resourceのLICENSEが正本SHA-256 eeb4b00cfe4a9c135ab47b643c44f4c0b747318c0d52cee8580bf7c3d2ca0667と一致した。独立したNSIS展開とfresh Windowsのsilent install後のoffline LICENSE比較が成功し、実起動/正常終了/uninstallも成功した。同runのexact manifest・検証記録・実jobsを再照合した。GitHub license認識はMIT、最新基点でlicense policy7件も成功。Rice自身の利用許諾を明示する本Issueの確認が完了した。第三者依存の通知#102は別に未完了であり、候補成功だけで公開準備完了とは扱わない。

- [x] Issue #99: 設定本体・backup・temporary・退避fileをowner-onlyで保存し、読込前に所有者・type・permissionを検証する。umask 022/000、過剰permission補正、リンク/非regular/foreign owner拒否を自動検証する。Windowsはuser profile ACL継承を使用する。

2026-10-05: Issue #99 で settings storage のUnix permission invariantを実装した。共有ancestorを変更せず、読込前にowner-onlyへ補正する。Windows CIでは実ユーザーAppDataに本番SettingsStoreで保存し、directory、本体、backup、temporaryの所有者と許可SIDを検査する。packaged実機で独自profile ACLが設定された場合の確認は継続する。

- [x] Issue #96: pnpm/Cargoの監査・期限付き例外validator・定期scan・dependency更新PR・release SBOMを導入する。

2026-10-05: Issue #96でPR/main/weekly/releaseの共通advisory gate、期限/owner/根拠を必須とする例外validator、Dependabot、artifact digestとexact commitへ結び付けたCycloneDX 1.5 SBOMを追加した。policy/SBOMのunit10件と実installed graphのintegration1件を確認。RustSec DB ef6173cbc5c50ec8166f9a5b28f07834144373ee（1290 advisory）でRust警告7件、npm High1件をblockingとして検出した。gateが正常に失敗することを確認しており、clean auditではない。新規releaseの実配布は未実施。

- [x] Issue #93: release build の base image / Debian snapshot / toolchain を固定し、時刻と build material を記録・検証する。SDK/CRT feed と NSIS/PE metadata の非決定性は material inventory と文書で明示する。

2026-10-05: Issue #93 で release の immutable input manifest、test/build の compiler policy、commit時刻のZIP正規化と build material inventory を追加した。完全な byte 再現性を保証せず、残る非決定要因を文書化した。

2026-09-21: Issue #175 で Vite 8.0.16、PostCSS 8.5.18、nanoid 3.3.19、Browserslist 4.29.0 と関連する推移依存を更新した。npm audit の high 6件が解消し、high/critical は0件（low1件、moderate8件は残存）。frontend203件、typecheck/build、renderer security 検証が成功。

- [x] Issue #175: npm audit の high 6件を互換範囲の依存更新で解消し、frontend と再監査を確認する。

2026-09-20: Issue #65 でユーザーごとの連投抑制時刻を channel ID / 接続 generation ごとの期限付き cache に変更した。抑制なし・scope 切替は次の連投判定時に破棄し、background task の1秒 tick により期限30秒を1秒間隔で確認して解放する（実際の解放時刻はスケジューリングとmutex待ちの影響を受ける）。cache / expiry FIFO は4096件の受理記録、cleanup は一度に64件で、上限を超えて退避した記録が現行時刻ならそのユーザーは連投抑制を早く解除する。2秒と30秒の境界、接続 generation 切替、注入 clock の idle cleanup、期限 record の世代保護、大量ユニークユーザーでの上限制御を Rust テストで確認した。`cargo test --locked --no-default-features` は115件、default は161件、clippy が成功。

- [x] Issue #65: 連投抑制のユーザー時刻を期限付きかつ上限付きで保持し、channel / EventSub session の切替時にリセットする。cleanup はコメントごとの全件走査を避ける。

2026-09-20: Issue #172 で修正版のある RustSec 6 advisory を依存更新で解消した。quinn-proto 0.11.15、rustls 0.23.45、anyhow 1.0.103、event-listener 5.4.2、plist 1.10.1 / quick-xml 0.42.0 へ更新し、必要な推移的依存だけを lockfile に反映した。Rust 1.89 の default 156件/no-default 110件と Windows GNU check、現在の CI toolchain の clippy、frontend203件・typecheck/build が成功。保守終了等の7警告は #173 へ分離し、clean audit とは扱わない。

- [x] Issue #172: RustSec が検出した quinn-proto / rustls / anyhow / event-listener と plist 経由の quick-xml を修正版へ更新し、既存機能と監査結果を検証する。

2026-09-20: Issue #68 で Launcher の追加処理を設定 mutex の snapshot 後に最大4件の blocking worker へ移した。worker permit の取得待ちは6秒、実行開始後の各 worker は7秒で呼び出しを返す。PowerShell アイコン抽出は5秒で子プロセスを kill/reap する。複数選択は最大200件を4並列で処理するため、追加操作全体に7秒の上限はない。停止した同期 filesystem 操作そのものは強制取消できないため、実行中 worker は permit を保持し、残留数を全要求で最大4件に制限する。失敗時は汎用アイコンへフォールバックし、並行する設定変更は最新の Launcher 項目へ merge、抽出失敗の理由と所要時間は件数を制限して Logs へ残す。本番 worker に注入した fake extractor と停止 child process による timeout・終了確認・上限制御・競合・lock 非保持のテストを追加した。Windows で停止した shortcut と child process が残らないことの手動確認が必要。

2026-09-20: Issue #67 の残作業として、PR/main で default/no-default を独立実行する Rust feature matrix を追加した。GUI 非依存構成102件と Tauri 有効構成148件が成功し、不要 import warning はなかった。

- [x] Issue #67: Rust の default/no-default feature 構成を CI で検証し、GUI 非依存テストの型・import 境界を維持する。

2026-09-20: Issue #46 のエラー表示を共通化。command の文字列/Error/object reject を操作別の日本語案内へ変換し、元の詳細は Logs へ記録する。起動時認証の失敗は system Chat にも記録。frontend196件、typecheck/build が成功。

- [x] Issue #46: command エラーを日本語の原因・復旧操作へ正規化し、技術詳細を Logs へ分離する。

2026-09-20: Issue #48 で Tauri bridge の struct `Option` を JSON field omission に統一し、TypeScript の optional field と整合させた。認証、chat、status、queue、snapshot を runtime schema で検証し、Device Code 成功時の `storageWarning` を Rust enum variant field も含め camelCase で送る。Rust serialization と TypeScript が共通 fixture を検証し、Device Code の保存警告を通知と system Chat へ送る経路も回帰テストで確認した。

2026-09-20: `issue-fix-batch` スキルを撤去し、サブエージェント、修正作業、GitHub Issue 対応のルールへ分割した。`AGENTS.md` から作業内容に応じて必要なルールを読む構成へ移行し、関連する PR と Issue がすべて close されるまで worktree と修正用ブランチを保持する方針にした。

2026-09-20: Issue #58 の共有 dispatcher を fake TCP server で再検証し、遅延した talk の後に pause / skip / clear が到着すること、control が先に開始された場合は talk 接続を開かないこと、pause / resume の wire・ローカル queue・成功 status/log の順序が一致することを確認した。制御 command 失敗時はローカル queue が未変更、棒読みちゃん側は到達不明と明示して Logs / status へ残し、最後の control 失敗解除で pending worker を再開する。app 無効の Rust テスト全102件、app 有効の Rust テスト全148件、全 target の clippy が成功。

2026-09-11: Issue #83 で設定チャンネルと実接続チャンネルを分離。接続世代と購読成功時の broadcaster identity を status/chat に付与し、古い世代や別チャンネルの遅延イベントを frontend/backend の両方で拒否する。接続中に設定を変更した場合は現在の接続先と次回接続先を併記する。app無効のRust全95件、frontend全521件、typecheck、buildが成功。

2026-09-11: Issue #56 で棒読みちゃんへのTCP受付と再生完了を分離。残タスク数・再生中状態がともに0になるまで1件を in-flight に保持し、後続送信とキュー上限をリモート未再生分まで含めた。受付後の追跡失敗は重複防止のため自動再送しない。制御送信中は完了反映を保留し、skip/clear と再生完了の競合も防止する。app無効のRust全94件が成功。

2026-09-11: Issue #58 の棒読みちゃん共有 dispatcher を追加し、talk・test・health・control を短命TCP接続のまま直列化。遅延 talk と clear / pause / skip の接続順を検証する fake TCP server テストを追加し、app無効のRustテストで確認した。

2026-09-10: Issue #55 の in-flight 分離と worker 所有権の共通化を実装。取消・遅延成功/失敗・再試行待機・overflow・スナップショットの回帰テストを追加し、Rust 全137件と clippy が成功。

2026-09-10: Issue #59 の診断・接続確認・無音プローブを状態応答検証へ統一。Rust 全133件と clippy が成功。実機での確認は継続。

2026-09-10: Issue #87 の Client ID 検査を CI・ローカル・Docker で共通化し、生成 EXE の埋め込み確認を追加。未設定・不正値・正常値・埋め込み欠落の自動検証とセルフレビューを完了。実 Windows 成果物のビルド検証は未実施。

2026-09-10: Issue #90 の公開済み Release の上書きを禁止し、draft の再ダウンロード検証を追加。成功・不一致・不足・余分な成果物・通信失敗・tag 移動のスクリプトテストとセルフレビューを完了。

2026-09-09: Issue #42の状態復元とDraft残作業を実装・自動検証済み。Rust 130件（app無効85件）、frontend 176件、clippy、build、セキュリティ検査が成功した。PR #159/#160/#163/#166はmainへマージ済み。

2026-09-08: Issue #43のshadow harnessを本番処理の注入テストへ置換し、新welcome優先時の旧通知欠落とPingによるkeepalive期限延長を修正した。

2026-09-08: Issue #94を所有者一人の運用へ変更。外部承認設定を必須にせず、公開元と成果物の検証を維持した。


2026-09-08: ローカル worktree の整理を完了。Issue #29/#31/#33 は main 反映済み、#42/#43 は既存 PR #164/#163 に保持。詳細は調査メモを参照。


| Phase | 状態 | メモ |
| --- | --- | --- |
| Phase 0: プロジェクト作成 | 完了 | `app_events` の配信基盤と frontend 購読を接続し、`settings.json` の生成/読込、原子的保存、破損時のbackup/既定値復旧を確認した。Issue #50 で UI 倍率を名前付き radio group にし、現在の選択状態と表示倍率を支援技術へ公開した。Issue #49 で route ごとの document title 更新と、PUSH 遷移後の画面見出しへのフォーカス移動を追加した。Issue #16 で接続・認証・読み上げの状態変化を単一の live region へ集約し、重複通知を抑制した。Issue #202 で既定値 factory を UI feature 非依存の settings model に集約し、browser preview の連続 patch と入力配列の snapshot を保持し、接続診断にも保存済み host/port を反映するようにした。frontend unit 301 件、typecheck、format check が成功した。 |
| Phase 1: 棒読みちゃん連携 | 実装済み、自動検証済み、手動確認待ち | TCP 読み上げ、制御、接続診断、Settings 画面は実装済み。接続先は host/port を構造化し、IPv4・DNS・IPv6を共通の接続経路で扱う。接続確認は設定に応じて確認読み上げまたは無音の状態取得を行う。Issue #84 で接続エラーの復旧導線を Settings の［診断］へ統一し、backend から画面名を除去した。Issue #148 で起動後の自動復旧プローブを無音の状態取得だけに限定し、下流の音声合成アプリが未起動の間に読み上げ要求を送らないようにした。`cargo test` と `pnpm build` は成功。実機の棒読みちゃんでの確認が必要。 |
| Phase 2: Twitch 認証 | 実装中 | Device Code Flow、`/validate`、refresh、keyring、session-only 保存失敗処理、旧 Linux 平文ファイルの移行/削除、Login 画面、起動時の保存済み認証の自動検証は実装済み。Issue #4 で認証とチャット接続の状態イベントに domain を追加し、表示文言に依存せず独立更新するようにした。Device Code の絶対期限に基づく残り時間と期限切れ時の再発行導線、Issue #30 の必須 `user:read:chat` scope 検証と不足時の再ログイン案内も実装済み。Client ID は UI/設定JSONに出さずビルド時既定値を使う。実 Twitch 環境での確認が必要。 |
| Phase 3: EventSub チャット受信 | 実装中 | WebSocket 接続、`channel.chat.message` 購読、正規化、再接続をまたぐ期限付き重複排除、開始/停止 UI、フロントエンド反映、再購読時の最新 access token 取得と 401 時の一度だけの refresh/retry（Issue #23）、更新後 access token の `/validate` に基づく scope 再検証（Issue #30）を実装。Issue #9 で Twitch 指定の `reconnect_url` への接続と旧 socket の受信を並行し、新しい welcome 後にのみ切り替え、失敗時は25秒の猶予後に通常再接続へ移行するようにした。Issue #74 で `receivedAt` を Rust から TypeScript まで UTC RFC 3339 に統一し、非文字列を含む不正 timestamp と leap second の frame 取得時刻 fallback、ローカル時刻表示をテストした。Issue #83 で設定値と世代付き実接続 identity を分離し、遅延 status/chat による表示巻き戻りを防止した。Issue #207 で正規化済みの同一 `ChatMessage` を UI/speech へ渡し、停止・交換後の旧世代を backend 境界で拒否する。Rust feature matrix のdefault/no-default testとPR qualityのRust test/clippy/formatが成功した。実 Twitch 環境での手動確認が必要。 |
| Phase 4: 読み上げキュー統合 | 実装済み、自動検証済み、手動確認待ち | Issue #211 で queue model/transition、formatter/URL、queue Tauri commands、event snapshot mapper を責務別 module へ分離し、`mod.rs` の大規模テストを `tests.rs` へ移した。LinkFinder は候補抽出だけに用いて既存 URL 受理条件を維持する。reviewed main統合後のRust all-targets/all-features 312件、no-default 223件（既存性能予算テストを個別実行で確認）、app-feature strict Clippyを確認した。`SpeechFormatter`、FIFO `SpeechQueue`、EventSub チャットから棒読みちゃんへの自動読み上げ、Queue 画面を実装。Issue #63 で最大文字数をユーザー名 prefix・省略記号を含む最終読み上げ文へ適用し、Issue #34 で連投抑制の 0 秒を無効、1〜30 秒を指定間隔として実行時にも厳密に適用した。Issue #57 で失敗済み項目をエラー履歴へ隔離し、明示的な手動再試行のみで retry budget を復元するようにした。Issue #78 で正規化後に本文が空のチャットを理由付きで Blocked にした。Issue #52 で待機中の読み上げ制御と履歴 dismiss を分離し、blocked を含む履歴を個別・一括で削除可能にした。`cargo test`、`pnpm test`、`pnpm build` は成功。実 Twitch + 棒読みちゃん環境での統合確認が必要。 |
| Phase 5: 配信運用向け仕上げ | 実装中 | Launcher、dev ビルド識別、設定破損時の復旧通知、設定更新 transaction、用途別のエージェント作業ルール、ルート README と MIT License を実装。Issue #100 で Windows 利用者向け README を導入・検証・初回設定・障害復旧・データ保存まで拡充し、実在する route／操作名／Release asset 規則を確認するレビュー項目を追加した。Issue #1 で Activity Bar から Logs を開ける導線とナビゲーション回帰テストを追加した。Issue #2 で読み上げキューの `sourceMessageId` を Chat 行へ同期し、全終端状態を視覚・支援技術の両方で確認できる表示にした。Issue #3 で非同期の Tauri 購読を cleanup-safe な共通 helper へ統一し、遅延解決・部分失敗でもリスナーを残さないようにした。 Issue #12 で設定更新を leaf patch と直列処理に統一し、保存直後の接続も保存済みチャンネルを使うようにした。Issue #5 で変更のない保存ボタンを DOM から除外してフォーカス順とアクセシビリティツリーに残らないようにし、Issue #6 で NG 入力欄と声質スライダーのラベル・現在値を支援技術へ公開、Issue #7 で入力エラーを対象フィールドと関連付け、棒読みちゃんホスト空欄と保存不能理由を明示した。Issue #14 で通知を構造化して成功通知を Logs / system Chat に分離し、警告の重複を排除した。Issue #15 で通常文字を `zinc-400` に統一し、コントラストと低コントラスト文字の再導入を検査した。Issue #16 で接続・認証・読み上げの状態変化を単一の live region へ集約し、重複通知を抑制した。Issue #17 で配信中の Space / S / Cmd/Ctrl+, ショートカットを入力中・IME・キーリピートを妨げない共通 hook として実装した。Issue #18 で Launcher 削除メニューを WAI-ARIA Menu Button のキーボード操作とフォーカス管理に対応した。Issue #21 で接続・認証・復旧を重複抑止付きの system Chat timeline へ集約した。Issue #24 で Launcher の DnD listener を mount 中の単一購読とし、最新の追加 handler を ref 経由で参照するようにした。Issue #26 で最小幅 900px の Chat レイアウトを 100/125/150% に対応させた。Issue #28 で未保存変更を画面遷移・履歴戻る・終了時に共通確認するようにした。Issue #193 で Filter / Settings は保存値と世代付き編集patchを分離し、同値再読込・無関係更新・保存後の元値への追加入力をDOM回帰で確認した。接続先変更時だけ endpoint 許可メッセージを失効する。親レビューは commit 1f5df204 で完了し、最終CIとmergeはPR #235で管理する。 Issue #194 で React Hook Form 7 の FormProvider / Controller を Settings と Filter へ導入し、共通の draft 同期と field patch model を抽出。保存対象 field だけを pending に追跡し、同期外部値・保存中の追加入力・失敗・破棄・診断・native endpoint 許可の回帰を追加した。 Issue #27 で接続中または待機中の読み上げがある終了要求も保護し、承認後はチャット受信停止とキュークリアの完了を待って終了するようにした。 Issue #29 で EventSub の HTTP status/OAuth code/revocation reason を型付きで保持し、401/403 は認証復旧、400 等の永続障害は停止、timeout/5xx は再接続として分岐した。Issue #35 で Chat・Queue・Logs を読み取り用 ARIA table とし、列見出し、論理行位置・総行数、Queue 操作対象を支援技術へ公開した。Issue #36 で Chat 新着を重複なく集約したライブ通知と停止設定を追加した。Issue #37 で Chat 行の Twitch バッジを短縮ラベルと支援技術向け名称で表示した。Issue #39 で NG ルールの 200 件上限を frontend/backend ともに明示検証し、ASCII 大小文字を区別しない重複を除外した。Settings / Filter の設定群には統一した見出しを追加し、Issue #41 で同一内容の連続ログにも一意な表示 ID を割り当て、Issue #45 で Speech/Queue の内部状態値を日本語表示へ集約し、Queue 状態アイコンを支援技術から隠した。Issue #53 で Chat を遡っている場合の仮想スクロール可視アンカー保持と新着へ戻る導線を追加した。Issue #54 で Logs を仮想化し日時 formatter を再利用するようにした。Issue #31 で React の chat、queue、connection、settings、logs を独立 external store と selector に分離し、Chat event で無関係な画面を再 render しない計測テストと auth/event/settings orchestration テストを追加した。Issue #51 で keyboard focus indicator と forced-colors fallback を追加した。Issue #94 で tag push build を read-only にし、default branch の publish workflow、tag provenance、`main` 到達可能性と version の再検証へ公開境界を分離した。release-rice は 3 manifest と tag の version を共通 script で照合し、StatusBar の動的 build info は source 更新対象から除外した。devcontainer bootstrap を固定・build 時検証へ移し、SSH agent/Docker/host network を明示 profile に分離した。Windows 実機確認と詳細な運用エラー整理は継続。 |
| Phase 6: VOICEROID2 実験アダプタ | 未着手 | MVP 後に Windows 専用の実験アダプタとして追加する。 |

Phase 5 では Issue #73 として production CSP と明示的な Vite dev CSP、main window capability / custom command ACL を有効化し、Launcher icon を完全 decode・寸法検証済みの PNG data URL に限定した。

Issue #194 の親レビュー追補で、連投抑制秒の空欄/空白を拒否し `0` を有効な値として扱う回帰、同一fieldを含む重複保存の完了追跡、診断とテスト読み上げ section の個別操作回帰を追加した。親レビューと追加 DOM 14件の再検証、実装 head ed9a890 の全16 CI が成功した。最終 head の CI・統合結果は PR #246 に記録する。

通常 devcontainer には lock 済みの GitHub CLI feature を追加し、Codex の認証情報・履歴・セッションを `rice-codex-home` named volume に永続化した。

## Phase 0: プロジェクト作成

- [x] Issue #202: Settings 既定値を UI feature 非依存の共通 model factory に集約し、preview の leaf patch を現在値へ累積適用する。可変値の非共有、異 section/同 section field の連続保存、再読込を回帰検証する。
- [x] Issue #202 統合追補: 並列で追加された回帰テストの既定値参照を factory 呼び出しへ更新し、最新 main 全体の frontend 338 tests、型検査・lint・format・build が成功した。
  - 統合調査: #198 / #193 で並列追加された3テストが factory をオブジェクトとして参照していたため、clone 失敗や必須設定の欠落が起きた。製品の設定モデルを維持し、全呼び出しを factory 契約へ統一した。

- [x] Tauri + TypeScript + Tailwind の雛形を作る。
- [x] `src-tauri/src` に `twitch`, `speech`, `settings`, `app_events` の境界を作る。
- [x] Activity Bar、Side Panel、Main View、Status Bar の基本レイアウトを作る。
- [x] Activity Bar のビュー切り替えを `react-router-dom` ベースのルーティングへ移行する。
- [x] Issue #49: 画面遷移時に document title を更新し、ユーザー起因の遷移では新しい画面見出しへフォーカスを移す。
- [x] 未実装 route に Chat view ではなく仮ページを表示する。
- [x] 独自 Title Bar、ウィンドウ操作、リサイズハンドルを作る。
- [x] UI 倍率の自動/手動切替を作る。
- [x] Issue #50: UI 倍率セレクターへグループ名と現在の選択状態を公開し、キーボードで操作可能にする。
- [x] Issue #157: 終了時にウィンドウ座標を保存し、現在のモニター構成で操作可能な場合だけ次回起動時に復元する。
- [x] 一般設定を Tauri app data 配下の `settings.json` に保存する。
- [x] `settings.json` を原子的に保存し、破損時に backup または既定値で復旧して退避先を system Chat/Logs/警告へ表示する。
- [x] `app_events` からフロントエンドへ流すイベント設計を実装に接続する。
- [x] Phase 0 完了条件として、Tauri アプリ起動と設定 JSON 読み書きを手動確認する。

## Phase 1: 棒読みちゃん連携

- [x] Issue #58: 棒読みちゃん宛ての talk・test・health・control を共有 dispatcher で順序付け、キューワーカーの in-flight 予約から talk 書き込み、pause/skip/clear のローカル反映まで同じ順序に入れる。control 失敗時はローカル queue 未変更と棒読みちゃん側の到達不明を表示する。
- [x] Issue #56: TCP 受付済みと再生完了を分離し、棒読みちゃん側の backlog を含めて読み上げキューを追跡する。

- [x] Issue #59: 状態取得応答の検証と、無応答・不正応答・切断・接続拒否の診断を追加する。

- [x] `SpeechAdapter` trait の境界を作る。
- [x] `BouyomiAdapter` の短命 TCP 接続を実装する。
- [x] 棒読みちゃん読み上げパケットを生成する。
- [x] 一時停止、再開、スキップ、クリアの制御コマンドを実装する。
- [x] 接続確認と接続診断を実装する。
- [x] Issue #80: 棒読みちゃんの host/port を構造化して検証し、IPv4・DNS・IPv6 の接続先を全経路で同じ形式にする。
- [x] 棒読みちゃん未起動時の日本語エラーを返す。
- [x] Settings 画面から接続確認、診断、テスト読み上げ、ホスト/ポート/声質設定を操作できるようにする。
- [x] Issue #84: 接続拒否・timeout の復旧案内を［診断］へ統一し、読み上げエラー時に Side Panel から Settings の［診断］を開けるようにする。正式画面名と legacy redirect の区別を文書・テストで維持する。
- [x] 接続確認で空接続を送らず、「棒読みちゃんと接続しました」の確認読み上げを送る。
- [x] 接続成功時の読み上げ ON/OFF と読み上げ文カスタマイズを設定できるようにする。
- [x] Issue #148: 起動後の自動ヘルスプローブを無音の状態取得に限定し、後から VOICEVOX を起動したときに接続エラー文が発話されないようにする。
- [ ] 実機の棒読みちゃんでテスト読み上げできることを確認する。
- [ ] 棒読みちゃん未起動、ポート競合、アプリ連携 OFF の手動確認を行う。
- [x] 接続失敗時に読み上げキューを破棄しない挙動を Phase 4 で統合確認する。

## Phase 2: Twitch 認証

- [x] Issue #8: frontend/backend の認証操作に generation を導入し、認証開始・解除後の遅延 poll/validate 結果と資格情報保存を破棄する。Device Code poll は同一セッションで一件だけ実行し、denied/expired 後に pending を終了する。
- [x] Issue #4: Twitch の認証・チャット接続 status event を domain で識別し、frontend が表示文言で状態を判定しないようにする。
- [x] Issue #197: 接続中の認証確認の成功・一時失敗で Chat 状態や実接続 identity を消さず、Auth の command 応答と backend Chat event の所有権を分離して実 AppShell DOM で応答順序を検証する。
  - Auth controller の dispatch 型から Chat 更新を除外し、開始・停止・解除・終了処理の revision なし書き込みも除去した。実 AppShell の validate 成功/一時失敗、新しい接続 event が先行する各順序、古い停止要求の成功/失敗を6件の DOM 回帰で検証。frontend 372 tests が成功し、最終 CI・統合結果は PR #250 に記録する。実 Twitch 通信は未実施。
- [x] Twitch Client ID を `.env` / build env から内部既定値として読み込む。
- [x] Twitch Client ID を Settings UI と設定 JSON の公開項目から外す。
- [x] OAuth Device Code Flow の開始とポーリングを実装する。
- [x] `user:read:chat` スコープでトークンを取得する。
- [x] `/validate` でトークン有効性を確認する。
- [x] access token 検証失敗時に refresh token で更新する。
- [x] refresh 成功時に保存済み refresh token を差し替える。
- [x] OS keyring 優先の OAuth 保存/復元/削除を実装する。
- [x] Issue #103: keyring 保存に失敗しても OAuth token を平文ファイルへ自動保存せず、session-only として継続する。既存の Linux fallback file は keyring 復旧時に移行・削除し、移行できない場合は削除・token revoke・再ログインを案内する。
- [x] Issue #33: keyring/filesystem I/O を auth mutex から分離し、同期 credential store API を `spawn_blocking` に隔離する。遅延 fake store 中の profile/cancel 応答、logout 後の stale save／stale clear 非 commit、実 command 経路で遅延した credential delete 中にも新世代の認証を完了でき、delete 失敗後に保持されることを決定的テストで検証する。
- [x] 旧版の Linux Secret Service fallback `~/.rice/twitch-auth.json` を検出し、keyring への移行成功時に削除する。新規の fallback file は作成しない。
- [x] Login 画面に認証開始、確認、有効性確認、解除を実装する。
- [x] Login 画面の認証開始/解除を認証状態に応じた単一アクションへ整理する。
- [x] Device Code Flow の待機応答を正しく判定し、自動ポーリングが継続するよう修正する。
- [x] Issue #47: Device Code の絶対期限から残り時間を表示し、期限切れ時は確認を無効化してキーボード操作可能な再発行導線を表示する。
- [x] 有効性確認で認証更新にも失敗した場合は、保存済み情報を含む認証状態を解除する。
- [x] 有効性確認中の Loading 表示と、確認成功時の通知を Login 画面へ追加する。
- [x] 起動時に保存済み認証を `/validate` し、必要なら refresh してから認証済み状態へ遷移する。
- [x] 起動時の認証確認進捗と結果を system チャットへ表示する。
- [x] Issue #30: 初回認証・保存済み認証の復元・refresh 後に `user:read:chat` scope を検証し、不足時は `missingRequiredScope` を伴う再ログイン案内を出して EventSub を開始しない。
- [ ] 実 Twitch Client ID で Device Code Flow を手動確認する。
- [ ] 認可取り消し、401、期限切れ時の UI 表示を手動確認する。
- [ ] アプリ起動時の保存済み認証復元と refresh 更新を手動確認する。
- [x] Twitch ユーザー ID と接続チャンネルを EventSub 接続へ渡す command を実装する。

## Phase 3: EventSub チャット受信

- [x] Issue #83: 設定チャンネルと世代付きの実接続チャンネルを分離し、status/chat の遅延イベントで表示が巻き戻らないようにする。
- [x] Issue #207: 正規化済み ChatMessage に接続 generation を一度だけ付与し、同一モデルを UI と speech に渡す。停止・接続交換後の旧世代通知を配信/enqueue 境界で拒否し、本番runtime adapter と fake で同 channel の世代切替・別 channel・遅延旧世代通知を回帰検証する。dedupe と FIFO を維持する。

- [x] `tokio-tungstenite` を導入する。
- [x] `EventSubClient` 相当の接続ループを作り、`wss://eventsub.wss.twitch.tv/ws` へ接続する。
- [x] `session_welcome` 受信後に `channel.chat.message` を購読する。
- [x] EventSub 購読に User Access Token を使う。
- [x] `session_keepalive` 欠落を検出して状態とログへ出す。
- [x] `session_reconnect` を処理する。
- [x] Issue #9: `session_reconnect` 中は新しい welcome を受けるまで旧 WebSocket を維持し、失敗時は25秒後に通常再接続へ戻す。
- [x] `revocation` を処理し、UI に再ログインまたは再接続が必要な状態を出す。
- [x] `metadata.message_id` または `event.message_id` で重複排除する。
- [x] EventSub の重複排除キャッシュを再接続間で維持し、件数上限と有効期限を設ける。
- [x] `channel.chat.message` JSON fixture のパーステストを追加する。
- [x] `ChatMessage` に fragments / badges / received_at を含める。
- [x] `tauri::Emitter` events で `twitch://status` と `twitch://chat-message` を送る。
- [x] TypeScript client で Twitch events を購読し、store へ反映する。
- [x] Chat view にリアルタイムチャットを表示する。
- [x] Side Panel のキュー上にチャット受信の開始/停止ボタンを追加する。
- [x] Twitch 認証状態とチャット受信接続状態を UI store 上で分離する。
- [x] Issue #23: EventSub 再購読時に最新の access token を取得し、401 時は refresh token rotation を保存して一度だけ再試行する。
- [x] Issue #30: 必須 scope 不足の認証状態では EventSub 接続 task を開始せず、EventSub の更新後 access token も `/validate` した scope で再検証する。並行した再購読で古い refresh 結果が新しく回転済みの認証を解除しないよう、token lock 下で refresh token を照合する。
- [x] Issue #32: EventSub の welcome/購読成功を確立済みとして記録し、30 秒以上安定した session の後だけ再接続 backoff を最短へ reset する。welcome 直後の失敗、連続失敗の上限、通常再接続、安定済み旧 session からの handover で新 welcome 前に失敗する状態を決定的テストで確認する。
- [x] Issue #43: 本番session/handover/supervisorへfake socketとevent sinkを注入し、Tokio仮想時計で競合、跨ぎdedupe、keepalive期限、再接続を検証する。HTTP refresh/validateとgeneration-safe保存の実経路もテストする。
- [x] Issue #74: `ChatMessage.received_at` を `DateTime<Utc>` に統一し、offset・小数秒を UTC の `receivedAt` として bridge へ渡す。timestamp の欠落・空文字・タイムゾーンなし・非文字列を含む不正値と leap second は WebSocket frame 取得時刻へ fallback して警告し、frontend の境界検証、system/mock message、ローカル時刻表示をテストする。
- [ ] 実 Twitch 環境で `channel.chat.message` 購読と Chat view 表示を手動確認する。

## Phase 4: 読み上げキュー統合

- [x] Issue #55: in-flight を pending から分離し、取消後の遅延完了と overflow の競合を検証する。

- [x] `SpeechFormatter` を実装する。
- [x] URL、改行、制御文字、長文、emote の扱いを `SpeechFormatter` に閉じ込める。
- [x] チャット由来の棒読みちゃんタグを初期設定で無効化またはエスケープする。
- [x] FIFO の `SpeechQueue` を実装する。
- [x] 最大件数 200、1 件のチャット最大 120 文字、ユーザー単位 2 秒の連投抑制を実装する。
- [x] キュー溢れ時に古い未読を落とし、UI に警告を出す。
- [x] 読み上げ失敗時に 1 回だけ短い遅延で再試行する。
- [x] Issue #57: 自動再試行上限に達した項目をエラー履歴へ隔離し、明示的な手動再試行でのみキューへ戻す。
- [x] チャット受信から `SpeechFormatter`、`SpeechQueue`、`BouyomiAdapter` への流れを接続する。
- [x] `speech://queue-updated` と `speech://status` events を実装する。
- [x] Queue view を実装し、スキップ、クリア、再読込、削除を操作できるようにする。
- [x] Issue #52: 読み上げ待機の制御から履歴の dismiss を分離し、blocked を含む履歴を個別・一括で削除できるようにする。
- [x] `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] Issue #61: 日本語文中・括弧/引用符内の `http://`、`https://`、`www.` URL を空白に依存せず検出し、置換・遮断で同じ parser を使う。URL 前後の本文と末尾句読点・引用符、直後の開き delimiter を保持し、balanced path 括弧は URL 内に残す。メール/ASCII 識別子内の誤検出と不正な authority を除外するテーブルテストを追加する。
- [x] Issue #63: 最大文字数をユーザー名 prefix・省略記号を含む最終読み上げ文へ適用し、上限 1・既定 120・最大 500・長い表示名・multibyte・空本文・URL/NG 処理順の境界テストを追加する。
- [x] Issue #34: 連投抑制の 0 秒を無効、1〜30 秒を指定間隔として扱い、空欄入力と保存設定の範囲外値を拒否・復旧し、設定値から実キュー判定までの実効値を一致させる。
- [x] TypeScript の store reducer テストを追加する。

## Phase 5: 配信運用向け仕上げ

- [x] Issue #199: system timeline の source/transition を中立の判別可能 union へ移し、生成・購読・routing の共通型で型 assertion を除去する。起動・認証・speech 復旧の初回通知／重複抑制と不正 callback の型エラーを検証し、既存品質 gate で検証する。
  - 最新 main（#193 / #195 / #196 / #198 / #200 / #202 / #204 / #205 と追補 #245）との統合検証: frontend 369 tests、format/lint/typecheck/build、diff check が成功。

- [x] Issue #198: テスト mock の明示的 any を実 DTO／関数型へ置換し、既存 Biome 品質ゲートで any・enum・namespace の禁止と型レベル用途の限定例外を検証する。既存 quality policy に正負 fixture を追加し、関連テスト・format・lint・型検査・build を確認した。

- [x] Dependabot PR #179–#182、#184–#192 の全13件を一件ずつ専用 worktree でレビューし、必要なCI互換性修正を実装する。各PRは全PR checksと当該headのWindows開発build成功を確認してからマージし、worktreeと作業用ローカルbranchを削除して次へ進む。

2026-10-05最終PR進捗: #190はRust dialog 2.8.1との版一致とplugin回帰検査12件を追加し、全16 checksと開発build成功後にマージした。#192も使用中アイコンがv1のbrand icon削除に該当しないことを[公式移行ガイド](https://lucide.dev/guide/react/migration)で確認し、frontend322件・全16 checks・開発build成功後にマージした。両worktreeは削除済み。最後の[#191](https://github.com/hapo31/Rice-xwitch-comment-viewer/pull/191)では[React 19移行ガイド](https://react.dev/blog/2024/04/25/react-19-upgrade-guide)に合わせてReact DOM型も19.3.0へ揃え、5か所のuseRefへundefined初期値を明示した。TitleBarテストはHTML属性順の比較から、既存jsdom projectでgroup/radioの名前・値・全選択状態と現在倍率の検証へ移した。StrictModeの購読cleanupを含むfrontend322件、typecheck/build/format/lint、Tauri版・renderer権限検査は成功した。Dependabotの自動rebaseも取り込み、無関係なWASM更新を入れずに検証済みtreeを保持した。文書を含む最終headの全PR checksとWindows開発build成功をマージ条件とし、マージ済み・保存先から到達可能・cleanを確認してからworktreeを削除する。新たなRelease公開や新依存構成での配布物smokeは今回の対象に含めない。

2026-10-05追加進捗: #179–#182、#184–#189 は全PR checksとexact headの開発build成功後にマージし、各worktreeを削除した。#184/#186/#188では無関係なcssparser-macrosのsyn切替、#189では無関係なWASM runtime更新を除去した。#190のJS dialogは実装互換でもTauri CLIがRust2.7/JS2.8のminor不一致を拒否したため、Rust dialogも2.8.1へ揃え、必要なplugin helper/fsだけを更新する。通常frontend buildとDockerが共有する既存の版検査へ、pnpm/Cargo両lockのplugin対応・minor一致・installed一致を追加し、旧minor・欠落・重複・stale installの拒否と同minorのpatch差許容を回帰検証する。監査・renderer権限の検査条件は維持し、修正後のCI成功前にはマージしない。

2026-10-05進捗: #179（download-artifact 8.0.1）は公式の Node 24・digest mismatch の既定拒否・展開仕様を確認し、既存の name/path/run-id 指定と互換であることを確認した。ポリシー91件、PRの全16 checks、exact headの開発build、検証済み配布物の取得・digest照合と両形式のWindows native診断が成功してマージした。追加診断のtitlebar操作は初回・再試行で失敗し、同じsourceの3回目が成功したため、UI入力の不安定性は調査境界として記録する。#180以降を順次確認する。監査の閾値・例外や配布の検証条件は変更しない。

- [x] Issue #48: Tauri bridge の `Option` を JSON field omission に統一し、Rust/TypeScript の camelCase・nullability 契約、Device Code 後の保存警告経路を共通 fixture と runtime validation で検証する。
- [x] Issue #201: 全 command 応答と event を runtime schema で検証し、Rust wire 型生成と schema 型一致・生成差分検査を導入する。既存 domain 変換と IPC 境界を保持する。
  - 作業進捗: Zod 4 schema から frontend DTO を導出し、ts-rs 12 の Serde 型生成と全51 wire 型の双方向一致検査を導入。全32 command の正常/不正応答と unit null を検証し、frontend 414 tests・型検査・lint・format・build が最新 main（#194 / #208）統合後に成功。導入時の Rust no-default 218 tests と全機能 287 tests・型生成も成功。#200 の厳密な状態契約を維持し、#195 の解除テストも実際の unit null 応答に統一。独立レビュー済みで、#199 の snapshot テストにも必須 adapterHealth を補い、#197 の Chat 状態所有権も統合した。Windows checkout の CRLF は生成比較前に改行だけ正規化し、LF/CRLF 成功・型差分の失敗・復元後成功も確認した。最終 CI・統合結果は PR #247 に記録する。
  - 性能確認: 同一 Node 24 プロセス、1000 warmup 後の5回中央値で chat 1万件は旧 parser 5.74ms / schema 10.88ms、200件 queue 1000回は46.55ms / 39.67ms。Settings 分割も統合した production JS は600.33kB (gzip182.65kB)で、schema 導入前の469.75kB (gzip142.31kB)から増加し Vite の500kB警告が出る。警告上限は変更しない。

Issue #200 は読み上げ outcome の復旧契約と Twitch Auth/Chat の状態・付随情報を判別可能 union に揃えた。既存21 outcome fixtureと新しい型/runtime共通の正負 fixtureを検証し、frontend 327件とformat/lint/typecheck/buildが成功した。独立レビューを受け、再送不能かつ送達不明な理由には送達確認を必須にした。最終 CI 結果と統合状況は PR #240 に記録する。
- [x] Issue #200: 読み上げ outcome の kind/reason と retry/recovery、Twitch の domain と状態・付随情報を判別可能 union にし、不正組合せを型検査と bridge parser の両方で拒否する。既存 Rust payload と共通 fixture の互換性、品質 gate で検証する。
  - 最新 main（#193 / #196 / #198 / #202 と統合追補 #245）との統合検証: frontend 343 tests、format/lint/typecheck/build、diff check が成功。
- 2026-10-06 Issue #200: Rust の auth service と app_events の本番送信は、Auth に接続世代/identityを持たせず、missingRequiredScope を AuthRequired にだけ付ける。Frontend の domain 別状態配列を型/parserで共用し、14種類の不正組合せを同じ値でコンパイル時とruntimeの両方から拒否した。既存21 outcome fixture、旧payloadのoptional省略、errorの安全な再送と送達確認の既存分岐は保持する。追加のschema libraryやRust wire変更は行わず、汎用parserの整理は別Issue #201に残す。
- Issue #200 レビュー確認: Rust `SpeechQueueOutcome::error` は `accepted=true` なら全 FailureCode に confirmDelivery を返す。configuration/confirmDelivery もこの経路では正常な契約のため拒否しない。全理由の受付済み fixture を維持し、accepted=false でも送達不明になる再送不能 write/lost/unknown に diagnoseSpeech を指定する組合せは排除する。


- [x] `issue-fix-batch` スキルを用途別ルールへ分解し、`AGENTS.md` から必要時に参照する構成へ移行する。関連する PR／Issue の close 後に worktree と修正用ブランチを削除する。

- [x] Issue #87: Client ID の共通 release gate と実行ファイルの埋め込み検査を追加する。

- [x] Issue #90: 公開済み Release を変更せず、全 asset の一致時だけ再実行を成功させ、draft は検証後に公開する。

- [x] PRレビューで確認したRust CIのビルド時間超過に対応し、Rust test/clippyの実行上限を15分へ変更する。

- [x] ローカル worktree と切れたリンクを整理し、main／既存 PR への保存状況を確認する。ルートの worktree 専用パスを gitignore に追加する。

- [x] Issue #73: production CSP を有効化し、Vite HMR 用 dev CSP を明示し、Tauri capability / custom command ACL と完全 decode 済み Launcher PNG の renderer 権限境界を最小化する。
- [x] Issue #3: 非同期イベント購読を遅延 cleanup と部分失敗に安全な共通 helper へ統一する。
- [x] Issue #12: frontend の設定更新を backend と同じ leaf patch 契約にし、保存要求を直列化して遅い応答や blur 直後の接続で変更を取りこぼさないようにする。
- [x] Issue #13: Helix による接続準備中にも接続 task を予約し、停止・解除で後続の EventSub 起動を取消せるようにする。
- [x] Issue #19: 一時的な validate/refresh 障害では資格情報を保持し、401 と `invalid_grant` の確定失効時だけ再ログインへ切り替える。
- [x] Issue #20: OS 資格情報ストアの削除が失敗した場合は認証解除を成功扱いにせず、メモリ状態も維持して再試行可能にする。
- [x] Issue #22: Twitch の HTTP 接続・応答と EventSub WebSocket handshake に明示的な deadline を設定する。
- [x] Issue #27: Twitch 接続中、読み上げ中、未保存変更がある終了要求を確認し、承認後に接続と待機キューを安全に停止する。
- [x] Issue #29: Twitch API HTTP/OAuth/revocation エラーを型付きで保持し、再試行・認証要求・永続停止を分岐する。
- [x] Issue #208: EventSub の恒久失敗を終端エラーから generation 付き Chat Error/AuthRequired と Logs/system Chat に一貫して反映してから task を終了し、HTTP 400 等で Connecting が残らず再試行しないことを fake で検証する。
  - supervisor が API/revocation の終端 Chat 状態を一元更新し、handover 中の revocation も直ちに終端処理へ返す。HTTP 400/410/401/403、全 revocation 分岐、実 AppEventState snapshot、再試行回数と task 終了を本番 service の fake で確認した。Rust app feature Twitch 78 tests、実 AppShell の event/snapshot 復元2 tests、frontend build/lint/format と Rust fmt が成功。最終 CI・統合結果は PR #249 で追跡し、実 Twitch 通信は未実施。
- [x] Issue #2: Chat 行へ読み上げ状態を表示し、キュー更新時に `sourceMessageId` で状態を同期する。
- [x] Issue #1: Activity Bar から Logs view を開ける導線を追加し、リンク名・現在地表現を回帰テストする。
- [x] `main` 向け PR で frontend/Rust の unit test と lint を並列実行する read-only GitHub Actions workflow を追加する。
- [x] PR 作成時と手動 dispatch 時だけ dev build を実行し、結果を job summary へ出す workflow を追加する。
- [x] devcontainer bootstrap 検証は `.devcontainer/**` または workflow 自体を変更したときだけ実行する。
- [x] 単独管理の方針ではrequired status checksのrepository設定を必須にせず、PRレビュー時にCIを確認する（2026-09-08所有者判断）。
- [x] Issue #16: 接続・認証・読み上げの非同期状態変化を、重複を抑制した live region で支援技術へ通知する。
- [x] Issue #17: Space / S / Cmd/Ctrl+, による配信向け読み上げ操作と Settings 遷移を追加し、入力中・IME 変換中・キーリピート時は無効化する。
- [x] Issue #39: NG ユーザー/NG ワードを 200 件で明示的に検証し、大小文字を区別しない重複を除外して超過時の保存を防止する。
- [x] Issue #15: 通常文字のコントラストを WCAG 1.4.3（4.5:1）以上へ引き上げ、主要画面のトークン利用を自動検査する。
- [x] Issue #35: Chat・Queue・Logs の列表示へ table/row/header/cell セマンティクスを追加し、仮想行の論理位置・総行数を View 経由で検証し、同一ユーザーの Queue 操作対象を論理行・本文付きラベルで区別する。
- [x] Issue #37: Chat 行に Twitch バッジの簡易表示と支援技術向け名称を追加する。
- [x] Issue #36: Chat の新着を重複なく集約して支援技術へ通知し、Settings でライブ通知を ON/OFF できるようにする。
- [x] Issue #5: 変更のない Filter / Settings で非表示の保存ボタンをフォーカス順とアクセシビリティツリーから除外し、キーボード回帰テストを追加する。
- [x] Issue #14: 通知を severity/source/correlation を持つ構造化モデルへ移し、成功通知を警告から分離し、同一障害の重複表示を抑止する。OAuth 認可待ち/待機延長の info 進捗も Logs と system Chat に記録する。

- [x] Issue #203: 旧 `appReducer` と `SettingsUpdateQueue` を削除し、reducer/ID/通知重複判定と設定更新直列化の正本を domain store/orchestrator に統合する。既存回帰を本番経路へ移し、ID衝突と replay 重複を区別して検証した。
  - 最新 reviewed main（6bdb52c）との統合後: frontend 366 tests、typecheck/build、変更ファイル Biome check、diff check が成功。
- [x] Issue #203: 親レビューと実装 head の全16 CI を確認した。文書追補後の最終 CI と統合は PR #251 で管理する。
Issue #204 は対処待ち通知と情報履歴を各100件の別領域へ分離し、成功操作で警告を失わないようにした。frontend 326件、format/lint/typecheck/build を確認済み。独立レビュー・最終CI・統合状況は PR #242 に記録する。
- [x] Issue #204: 対処待ち warning/error と info/success 履歴の保持上限を分離し、大量の成功通知で警告を失わないようにする。明示クリア・severity昇格・correlation重複排除・容量上限の logsReducer 回帰を追加した。
  - 最新 main（#193 / #196 / #198 / #202 と統合追補 #245）との統合検証: frontend 342 tests、format/lint/typecheck/build、diff check が成功。
- 2026-10-06 Issue #204: 実際に使用する logsReducer で warning/error を notifications、info/success を notificationHistory へ分離した。昇格時は元のIDを保ち履歴から対処待ちへ移し、correlationId と本文/5秒の重複排除、明示クリア、独立した保持上限を回帰する。旧 appReducer の整理は別Issue #203 の範囲とし、runtime の正本を直接検証した。

- [x] Issue #213: 未保存の保存と保存後の終了/画面遷移を操作 token で分離し、キャンセル・破棄・新しい終了要求・unmount で古い継続を無効化する。同一要求の重複保存を拒否し、別の確認中に以前の保存が完了しても現在の要求を自動承認しない。実 AppShell の deferred 応答でキャンセル、再試行、失敗、連打、古い応答、別の遷移先、unmount を検証する。最終 CI・統合結果は PR #254 に記録する。
- [x] Issue #28: Filter / Settings の未保存変更を Activity Bar 遷移・履歴戻る・ウィンドウ終了で共通確認し、保存・破棄・キャンセルをキーボード操作可能にする。native close listener は mount 中に一度だけ登録し、直後の終了要求も保護する。
- [x] Issue #194: React Hook Form 7 を採用し、Settings / Filter の値・dirty・reset を責務別 FormProvider / Controller へ移行する。Rice 固有の saved-value 同期、保存対象 field ごとの pending、leaf patch、日本語 validation、空欄/数値、NG リスト正規化、native 接続許可を維持し、外部更新ごとの全フォーム reset を避ける。テストで外部同期、保存中の追加入力、失敗、破棄、診断、許可、空欄と 0 の区別、重複する保存完了、診断・テスト読み上げ section の単独操作を確認する。
- [x] Issue #194: 親レビューと実装 head の全16 CI を確認した。文書追補後の最終 CI と統合は PR #246 で管理する。
- [x] Issue #193: Filter / Settings の保存済み値と世代付き編集patchを分離し、保存応答が開始後の追加入力・元値への編集を上書きしない。接続先が変わったときだけ endpoint 許可メッセージを消す。
- [x] Issue #193: 親レビュー指摘を反映し、最終レビュー対象 commit `1f5df20411f48cbdfd96b31f4c21110004c9a175` を確認した。PR #235 で最終 CI と統合を管理する。
- [x] Issue #38: Settings / Filter の設定群へ同一階層・同スタイルの見出しを追加し、見出し一覧のアクセシビリティテストを追加する。
- [x] Issue #6: NG ユーザー/NG ワードと速度/音程/音量のフォームコントロールへラベルを関連付け、既定値を含む現在値を支援技術へ公開する。
- [x] Issue #7: 入力エラーを対象フィールドと関連付け、棒読みちゃんホスト空欄と保存不能理由を明示する。
- [x] プロジェクト概要、主な機能、導入方法、ライセンスを案内するルート README と MIT License を追加する。
- [x] Issue #100: Windows 利用者向け README に製品範囲、WebView2 を含む installer／portable の前提と選択、SHA-256 検証と未署名時の注意、初回設定、実在する状態に沿った障害復旧、設定・認証・ログの保存／削除方針を記載し、route／操作名／Release asset 規則のレビュー項目を追加する。
- [x] Issue #97: devcontainer の bootstrap を固定し、通常開発と SSH/Docker/host network 利用を明示的な profile に分離する。
- [x] 通常 devcontainer に lock 済みの GitHub CLI feature を追加する。
- [x] 通常 devcontainer の Codex 状態を named volume に保持し、Rebuild 後も認証情報と履歴を復元する。
- [x] Issue #60: 設定更新を候補へ適用・保存成功後に commit するトランザクションに統一し、失敗時にメモリと永続設定を変更しない。
- [x] 画面実装を `features` 単位へ分割し、ルーティング層を画面配線のみに整理する。
- [x] Windows 10 スタートメニュー風の Launcher 画面を追加する。
- [x] Launcher でアプリの選択/DnD登録、削除、単体起動、一斉起動を実装する。
- [x] Launcher の登録内容を永続化し、将来の色変更・グループ・並べ替え・Webリンクに拡張できるモデルにする。
- [x] Issue #196: Launcher追加処理が`added`/`duplicate`/`rejected`の操作結果を明示し、共有state更新とPromise解決の順序や並行追加に依存せず正しい通知を表示する。実装・回帰テスト・親レビューを完了した。最終コミットの CI 結果と統合状況は PR #237 に記録する。

2026-10-06 Issue #196: `launcher_add`はsettings transactionで実際に追加した`addedCount`と更新後のitemsを返すようにし、Launcher通知は共有stateの件数差分を参照しない。実DomainStoresとLauncherViewを接続し、`flushSync`で共有stateをPromise解決前に更新してから結果を返すDOM回帰を追加した。新規/混在/重複通知と同一targetの並行結果独立性を確認する。Frontend全gate（format/lint/typecheck/test 324件/build）とsecurity/license policyが成功。Rust toolchainがこの実行環境にないためRust回帰のローカル実行は未確認。親レビューで追加した実store更新順序のDOM回帰2件も成功。Rustを含む最終CI結果はPR #237に記録する。
- [x] Issue #68: Launcher のアイコン抽出を timeout/kill/reap 付きの上限制御 worker へ移し、設定 lock 外で実行して競合する設定変更を merge する。抽出失敗は汎用アイコンと bounded Logs へフォールバックする。
- [x] Issue #18: Launcher の削除メニューを WAI-ARIA Menu Button のキーボード操作とフォーカス管理に対応させる。
- [x] Issue #24: チャット・ログ・状態更新時にも Launcher の DnD listener を再登録せず、mount 中の購読を維持し、最新 handler と遅延登録後の cleanup をテストする。
- [x] Settings 画面から Login 画面を分離し、認証専用の画面として整理する。
- [x] Settings 画面へ読み上げ基本設定を集約し、Login/Filter 側に重複した読み上げ設定を残さない。
- [x] `v[0-9]*` タグ push で Windows NSIS ビルドと GitHub Release 作成を行う Actions workflow を追加する。
- [x] Windows リリースビルド用 Dockerfile と `.dockerignore` を追加する。
- [x] Windows リリースでインストーラーに加えて portable zip を作成する。
- [x] devcontainer に Docker outside Docker feature を追加し、手元でも Dockerfile 経由で Windows 成果物を作れるようにする。
- [x] 手元 Docker ビルドでは `.env` の `RICE_TWITCH_CLIENT_ID` を build arg として渡すラッパースクリプトを使う。
- [x] リリース workflow では build/release job を分離し、release job のみ `contents: write`、build cache は未使用にする。
- [x] Windows リリースビルドが Client ID 未設定で即失敗しないようにし、Windows 用 `icon.ico` を追加する。
- [x] Windows リリース版を GUI サブシステムで起動し、付随するコマンドウィンドウを表示しないようにする。
- [x] main 同期確認、ローカル検証、SemVer 判断、差分リリースノート作成、注釈付きタグ発行までを扱う非同期リリーススキルを追加する。
- [x] リリース作業スキルで 3 manifest の version 同期と、StatusBar の動的 build info 契約を検証する。
- [x] Issue #89: 動的な StatusBar の build info に合わせ、リリース時の version 更新対象を 3 manifest と tag の共通検証へ更新する。
- [x] `v0.1.2` パッチリリース向けにバージョンを同期し、TypeScript/Rust/Windows Docker ビルドを検証する。
- [x] Launcher と dev ビルド識別を含む `v0.2.0` リリース向けにバージョンを同期し、自動検証する。
- [x] リリースビルド以外のステータスバーへ dev 表示とコミットハッシュを追加する。
- [x] 注釈付きタグの本文から GitHub Release を非同期・冪等に公開するフローへ移行する。
- [x] `actions/checkout` がタグ event の注釈付きタグを軽量タグへ置き換える場合に、検証前にリモートのタグ object を復元する。
- [x] リポジトリ公開後に `v0.2.1` の失敗 run を再実行し、注釈付きタグの復元と検証が成功することを確認する。
- [x] Release workflow の Rust テスト前に Tauri が必要とする Linux 開発パッケージを導入し、`v0.2.2` として再リリースする。
- [x] `gh release create` の `--notes-from-tag` / `--repo` 非互換を解消し、`v0.2.3` Release workflow の build / publish 成功を確認する。
- [x] Release 公開ジョブでも annotated tag を復元・検証し、`--notes-from-tag` がコミットメッセージへフォールバックしないようにする。
- [x] Issue #94: 単独管理の方針で、read-only tag build、default branchのtrusted publisher、main ancestry、event/checkout/tag object、versionと成果物checksumの照合を実装する。Team・別承認者・ruleset・environmentは必須にしない。
- [x] Docker build context を default-deny allowlist 化し、Codex state/credential の送信前検査と退避先の workspace 外移動を行う（#98）。
- [x] UI 倍率変更時に Activity Bar、Side Panel、Status Bar が操作部品と同じ比率で拡大するよう、アプリシェル寸法を rem に統一する。
- [x] Issue #26: 最小ウィンドウ幅 900px と 100/125/150% の UI 倍率で、Chat の主要列を横スクロールなしに表示する。
- [x] Logs view を実装する。
- [x] Issue #54: Logs の日時 formatter を再利用し、500件のログ行を仮想化して連続追加時の描画負荷を抑える。
- [x] `app://log` event をフロントエンドへ接続する。
- [x] Issue #41: 同一内容の `app://log` event を連続受信しても Logs view の表示 ID を一意にする。
- [x] Issue #42: backend の operational event/status replay、未検証 Twitch credential の `Validating` 状態、revision 付き speech queue/status snapshot と subscribe-first reconcile を実装・検証する。
- [x] EventSub、認証、読み上げアダプタのログを Logs view に表示する。
- [x] ステータスバーに Twitch 接続状態、棒読みちゃん状態、キュー件数、警告状態を集約する。
- [x] 起動時自動接続を実装する。
- [x] Issue #21: 自動接続、EventSub の切断・再接続・認証状態、棒読みちゃん自動復旧を重複抑止付きの system Chat timeline に表示する。
- [x] チャット受信停止時の確認ダイアログを設定で省略できるようにする。
- [x] 自動読み上げ ON/OFF を実装する。
- [x] 棒読みちゃん接続エラー後、成功するまで接続確認をポーリングして状態を復帰する。
- [x] SidePanel の「待機中」件数を読み上げ未完了の項目数に揃える。
- [x] Filter view を実装する。
- [x] NG ユーザー、NG ワード、URL 処理、長文処理の設定を実装する。
- [x] 各画面のタイトル下説明を、画面機能が分かる日本語の概要文へ整理する。
- [x] Filter/Settings は設定変更時だけ保存ボタンを右下からスライドイン表示し、Login は認証操作ボタンと自動保存に分離する。
- [x] Rules の表示名・内部 route・view 名を Filter へ変更し、読み上げ対象を設定する画面だと分かる説明にする。
- [x] Auth の表示名を Login に変更する。
- [x] 「起動時にチャット受信を開始」を Settings 画面の先頭に移動する。
- [x] Voices 画面を Settings へ改名し、`/voices` から `/settings` へリダイレクトする。
- [x] 左ペインのチャンネル行から Login 画面へ移動できるようにする。
- [x] 左ペインから Settings と重複するテスト読み上げ、Logs ナビゲーションを削除する。
- [x] Activity Bar 左下の未使用アイコンを非表示にし、領域だけ残す。
- [x] Twitch 文脈の「コメント」表記を「チャット」へ統一する。
- [x] 配信者向け文言を「読み上げ」へ統一する。
- [x] Chat view のチャットリストを仮想スクロール化する。
- [x] Issue #53: Chat を遡っている間も prepend 後の可視アンカーを維持し、新着件数から先頭へ戻れるようにする。
- [x] Queue view を読み上げ待ち・エラー・フィルターによる読み飛ばしだけに絞り、Chat view と同じ新着順にする。
- [x] 起動時の仮チャットを設定状態に応じた system 操作案内へ置き換える。
- [x] 配信中に判断しやすい日本語エラー文言を整理する（Issue #46: 操作別の原因・復旧案内と Logs の技術詳細）。
- [x] Issue #45: 内部の Speech/Queue 状態値を日本語の表示文言へ集約し、状態アイコンの重複した支援技術向け読み上げをなくす。

- [x] Issue #205: Twitch認証・接続ラベルをpresentationに集約し、短い視覚表示と詳しい読み上げの意図した差を明示する。状態の型網羅性と実AppShellの画面/支援技術の代表状態を検証した。frontend全326件、format/lint/typecheck/buildが成功。最終レビューとCI・統合状況はPR #244に記録する。
  - 最新 main（#193 / #196 / #198 / #202 と統合追補 #245）との統合検証: frontend 340 tests、format/lint/typecheck/build、diff check が成功。

Issue #205 調査メモ: 接続ラベルは4か所で同じ内容、認証ラベルは視覚表示2か所とライブ通知で長短差があった。共通の網羅したmappingと読み上げ用の明示的な差分へ統一し、暗黙の英語fallbackを設けない。

- [x] キュー行の状態表示テストを追加する。
- [x] 設定フォームのバリデーションテストを追加する。
- [x] Issue #31: chat、queue、connection、settings、logs の state/action 境界を独立 store に分離し、Chat event で無関係な画面を再 render しない selector 購読と auth/event/settings orchestration のテストを追加する。
- [x] Issue #51: キーボード操作時に十分な focus indicator を実装し、Windows 高コントラスト向け fallback と自動テストを追加する。

## Phase 6: VOICEROID2 実験アダプタ

- [ ] MVP 完了後に着手可否を再判断する。
- [ ] Windows 専用 feature として隔離する方針を維持する。
- [ ] C# sidecar の PoC を作る。
- [ ] Rust から stdio JSON-RPC で `speak`, `stop`, `health` を呼ぶ。
- [ ] VOICEROID2 のバージョン、bitness、起動状態を診断する。
- [ ] 失敗時に棒読みちゃんアダプタへ戻せる UI を作る。

## テストと確認

- [x] Issue #43レビュー: app無効時のqueue status importとTwitchテストfixtureのcfgを修正し、app有無のcargo testとappのclippyを確認する。

- [x] Rust: 棒読みちゃんパケット生成テストを追加する。
- [x] Rust: 棒読みちゃん制御パケットテストを追加する。
- [x] Rust: 外部 URL 許可リストのテストを追加する。
- [x] Rust: `channel.chat.message` JSON fixture のパーステストを追加する。
- [x] Rust: `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] Rust: Issue #57 の初回失敗、再試行成功/失敗、新着 enqueue/通常再開、手動復旧の状態遷移テストを追加する。
- [x] Rust: WebSocket 再接続状態遷移テストを追加する。
- [x] Rust: 通常再接続と reconnect ハンドオーバーをまたぐ重複排除テストを追加する。
- [x] Rust: Issue #43 の DI harness で HTTP/WS/credential store/clock を外部環境なしに駆動し、OAuth と EventSub の状態遷移・競合を検証する。
- [x] Rust: Launcher の拡張子、重複、順序、予約種別、旧設定互換テストを追加する。
- [x] Rust: 設定JSONの原子的保存、disk full/replace failure、構文・設定値が不正な本体/backup復旧テストを追加する。
- [x] Rust: Issue #157 の旧設定互換、座標のJSON保存、画面外位置の復元抑止をテストする。
- [x] TypeScript: store reducer テストを追加する。
- [x] TypeScript: キュー行の状態表示テストを追加する。
- [x] TypeScript: 設定フォームのバリデーションテストを追加する。
- [x] TypeScript: Issue #37 の複数／未知／バッジなしの Chat バッジ表示テストを追加する。
- [x] TypeScript: Issue #47 の Device Code 期限境界を fake timer でテストする。
- [x] TypeScript: Launcher の表示順、背景色、DnDパス判定、起動結果表示、削除メニューのキーボード操作のテストを追加する。
- [x] TypeScript: 100/125/150% と自動倍率でアプリシェル寸法が一貫して拡大するレイアウト回帰テストを追加する。
- [x] TypeScript: Issue #53 の仮想スクロール prepend 後の可視アンカー回帰テストを追加する。
- [x] TypeScript: Issue #26 の最小幅 900px における Chat 主要列の 100/125/150% レイアウト回帰テストを追加する。
- [x] TypeScript: Issue #36 の起動済みチャット除外、連投集約、重複・system チャット抑制、通知停止中の既読化をテストする。
- [x] TypeScript: Issue #49 の route 別 document title と PUSH/POP/REPLACE ごとのフォーカス方針をテストする。
 - [x] TypeScript: Issue #17 の Space / S / Cmd/Ctrl+, と入力中・IME・キーリピート・Shift 修飾時のショートカット抑止をテストする。
- [x] Security: production CSP、明示的な Vite dev CSP、main capability、custom command ACL、bundled asset source を自動検査する。
- [x] Security: Issue #94のnon-main tag、event/checkout target、moved tag object、manifest/tag version、upload前後のremote tag移動、draft/公開済みRelease再実行、workflow権限境界、default branchとAPI取得失敗を自動検査する。
- [x] Issue #94の運用方針: 2026-09-08の所有者指示により単独管理とし、Team・別承認者・ruleset・release environmentの設定を完了条件から外す。
- [ ] 手動: 棒読みちゃん未起動/起動中/ポート競合を確認する。
- [ ] 手動: Twitch トークン期限切れ/認可取り消しを確認する。
- [ ] 手動: 配信中チャット連投を確認する。
- [ ] 手動: ネットワーク切断と復帰を確認する。
- [ ] 手動: Windows 10/11 で `.exe` / `.lnk` の選択・DnD登録、実アイコン、単体/一斉起動、削除、再起動後の復元を確認する。
- [ ] 手動: 空白・日本語・`&` を含むアプリパスと、移動済みアプリを含む一斉起動の部分失敗表示を確認する。
- [ ] 手動: Issue #68 として、停止する UNC 上の `.lnk` を追加して7秒以内に戻り、PowerShell 子プロセスが残らず、その間に設定の読込・保存と読み上げ操作が続けられることを Windows で確認する。
- [ ] 手動: マウス操作では不要な focus ring が出ず、Tab 操作では各入力・ボタンの位置を確認できること、および Windows 高コントラストで focus indicator を確認する（Issue #51）。
- [ ] 手動: Windows release package の DevTools で CSP violation がないことと、Tauri event/invoke、タイトルバー、Dialog、Launcher icon、主要 API 操作を確認する（Issue #73）。
- [ ] 手動: Issue #157 として、ウィンドウを別モニターへ移動して終了後に復元されること、モニターを外した後は画面外で起動しないことを Windows 10/11 で確認する。
- [ ] 手動: Issue #27 として、Twitch 接続中・読み上げ待機中・未保存変更ありの X、Alt+F4、OS close-request で終了確認とキャンセル、承認後の接続停止・キュークリアを Windows 10/11 で確認する。

## 調査メモ

- 2026-10-06 Issue #194 親レビュー追補: Filter の連投抑制秒は空文字列を `Number("") === 0` として扱うと無効化を意図せず保存できるため、共有 validator で空欄/空白を拒否し、数値 `0` は引き続き許可する。保存応答の追跡は transaction ごとの field baseline を保持し、同一更新に対する複数成功応答を保存値の到着で個別に解放する。診断とテスト読み上げ section は限定 action provider を使う単独 DOM 操作テストを追加する。

- 2026-10-06 Issue #199: `SystemTimelineEvent` を presentation から `models/systemTimeline.ts` へ移し、domain orchestration と AppShell も同じ型を使う。source/transition は判別可能 union とし、認証の状態＋案内文による重複抑制は維持する。実購読＋snapshot replay＋router の回帰と `@ts-expect-error` の型契約回帰を追加した。frontend 325件、format/lint/typecheck/build が成功した。実Twitch/棒読みちゃんとの手動通信は未実施。

- Issue #193: Settings / Filter の useEffect は保存済み設定を全入力stateへ毎回複写し、項目と無関係な更新でも編集中の値を消していた。保存開始時点の編集世代snapshotで応答を照合し、開始後に元の保存値へ戻した入力も保存中はpatchとして保持する。親Harnessで設定更新と保存応答が同一batchに入る場合も回帰する。保存APIがrejectした場合はfinallyでpendingを解放し、失敗時の下書きと明示破棄を維持する。保存済みhost/port/remoteModeの変更だけを接続許可メッセージの失効条件にする。親レビューは最終レビュー対象 commit `1f5df20411f48cbdfd96b31f4c21110004c9a175` で完了した。

- 2026-10-06 Issue #198: [Biome noExplicitAny](https://biomejs.dev/linter/rules/no-explicit-any/) の型引数制約の例外を維持する。条件型で任意の引数列から戻り値を推論する場合に限り、理由付きの行単位 `biome-ignore lint/suspicious/noExplicitAny` を使える。DTO、mock、値のキャストには使わず、ファイル単位の無効化はしない。`noEnum` は const enum を検出しないため `noConstEnum` も有効にした。既存 quality policy の正負 fixture で named/alias import と許容例外を含め検証し、別の AST 検査器や workflow は追加していない。
- Issue #18: 削除メニューは ARIA `menu` / `menuitem` を使うため、Menu Button pattern に従い、開いた直後は最初の項目へフォーカスする。矢印キーと Home/End は項目間を循環移動し、Escape はトリガーへ戻す。Tab はフォーカスを閉じ込めずにメニューだけを閉じ、外側クリックで閉じる既存動作は維持する。

2026-10-06 Issue #213: 保存待ちキャンセル後の app_exit と reset 済み blocker の proceed 例外を実 AppShell で再現し、保存後の操作を現在の token と blocker location key で照合する。取り消した保存そのものは完了してよいが、旧終了・遷移の副作用は実行しない。保存済みになった新しい確認要求も続行/キャンセルを明示選択できる。

2026-10-06 Issue #213 統合確認: reviewed main 493c57f の wire schema を取り込み、本番 AppShell と保存継続の DOM 33件、format/lint/typecheck/build が成功した。最終 head の CI とマージは PR #254 に記録する。

## Issue #218: 起動時認証復元と手動操作の優先順位

- [x] 起動開始時の認証操作を予約し、遅い snapshot/auth 復元が後発の手動 start/poll を無効化しない。実 AppShell と認証世代の逆順完了を回帰化する。

2026-10-06 Issue #218: 起動effectで予約した世代をsnapshot復元後も使い、後発手動操作から世代を奪わない。Auth snapshotは手動開始後には反映せず、古いstored auth取得後にvalidate commandを追加起動しない。実AppShellのStrictMode、snapshot対start/poll、stored auth/validateの遅い成功・失敗7件を回帰化した。最終検証とCIはPR #263で追跡する。


## Issue #216: 設定初期化の競合防止

- [x] 設定初期化の loading/ready/error と取得世代を明示し、遅い取得応答が新しい保存結果を上書きしない共通境界を設ける。初期値表示と保存可否を区別し、読込失敗後の再試行・逆順完了・StrictMode・unmount を回帰化する。

2026-10-06 Issue #216: 設定読込・直列保存を SettingsController へまとめ、load generation / store publication revision / effect lifetime で応答を照合する。Settings/Filter は読込中に既定値フォームを編集させず、失敗時に明示再試行を出す。接続用の二重 settingsSnapshot をなくし、store を正本にした。StrictMode の一回限りの復旧通知は load 間で共有し、現在の load が一度だけ通知する。読込対保存（成功/失敗）、同 lifetime の取得逆順、StrictMode、unmount、Launcher 更新を含む対象23件と typecheck が成功。最終 frontend 全体、CI と統合結果は PR #259 に記録する。

## Issue #261: dependency audit の共通 blocker

- [x] source-map-js の影響版を修正版へ統一し、既存監査・frozen install・frontend gates を通す。監査例外は追加しない。

2026-10-06 Issue #261: @tailwindcss/node 4.3.3 の許容範囲 ^1.2.1 内で source-map-js を1.2.2へ統一し、未使用の1.2.1 entryを除去した。他の依存とpackage.jsonは変更しない。frozen offline install、frontend全429件、production build成功。pnpm auditはhigh/critical 0件で対象GHSAが消え、既存moderate 5件のみ。全CIの結果は PR #262 に記録する。

## Issue #220: 再生中 session と制御先の一致

- [x] 設定上の宛先と再生中sessionを区別し、Pause/Resume/Skip/Clear・完了確認を同じadapterへ送る。A再生中のB設定保存と後続itemの選択をfakeで検証する。

2026-10-06 Issue #220: fake adapter A/Bで4種類の制御、送信完了待ちとの競合、制御失敗時の状態維持、後続itemのB選択を検証した。no-default全224件・app構成speech112件・strict Clippy・frontend build・format/diff検査成功。実棒読みちゃんの手動確認は未実施。最終headのCIはPR #265で確認する。

## Issue #219: 認証解除失敗後の再試行

- [x] 解除中の表示と認証正本を調停し、削除失敗後に現行認証と操作性を維持する。keyring失敗後の再試行と後発認証/eventとの競合を回帰化する。

2026-10-06 Issue #219: 解除中はUI操作世代付きの要求として認証状態と分離し、削除失敗後はbackendの現在profileを照合する。後発操作・Auth revision変更後の古い解除/調停応答を拒否する。実AppShellで失敗後再試行、認証保持/消失、後発event、再取得失敗とcontrollerの後発loginを回帰化した。追加レビューで、解除成功eventがcommand応答より先だと古いprofileが残ることを再現し、revision検証済みのAuth disconnectedをprofile/promptと同時反映する。最終検証・CIはPR #264に記録する。
