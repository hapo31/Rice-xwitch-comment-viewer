# 調査メモ


## 2026-10-05 TypeScript 7 と互換ツールチェーン

- [TypeScript 7正式版](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)とnpm metadataを照合し、`typescript@7.0.2`を採用した。`tsc`は正式native compilerを起動する。Biome/Vite/Vitestの既存経路にCompiler API直接依存はなく、preview packageやTypeScript 6 API互換aliasは不要。
- 元の`moduleResolution: Node`でTS5108を再現し、[Bundler解決](https://www.typescriptlang.org/tsconfig/moduleResolution.html)へ移行した。strict・ES2020・既存検査対象を保ち、`rootDir: ./src`、`types: ["vite/client"]`、`noUncheckedSideEffectImports: true`を明示する。[Vite client型](https://vite.dev/guide/features#client-types)でCSS/import.metaを解決する。型検査やside-effect import検査は無効化していない。
- Vite 8.3.2、React plugin 6.1.2、PostCSS 8.5.29、React Virtual 3.14.13、React Router 6.30.6へ更新した。Vite/Vitest用の`@types/node`22.20.5とTesting Library共通peerの`@testing-library/dom`10.4.2を直接宣言する。main 934f318のReact/DOM/型19.3.0、Vitest5.0.3、Lucide1.49.0、Tauri2.12.1とdialog2.8.1のJS/Rust整合・useRef初期値・DOMテスト修正を保持する。
- 初期source0341101のReact18/Vitest4構成はNode20/22で検証したが、その後mainにReact19/Vitest5が入ったため最終構成を再検証する。Vitest5はNode22.12以上の対応LTSを要求する。package enginesを明示し、開発コンテナをCI/releaseと同じNode22.22.0 image/digestに揃え、bootstrap guardで両imageの一致を検証する。jsdom27.4.0、jest-dom6.9.1など既に互換性のある固定依存は継続する。
- サブエージェントの元headレビューで、JS dialog2.8.1とRust2.7.2のminor不一致が指摘された。main統合でRust2.8.1とplugin対応の版guard・回帰検査を取り込み、`verify-tauri-versions.mjs --installed`が成功した。通常frontendのVite buildだけではnative CLIによる版拒否を検出できないため、GitHubのnative/dev buildでも確認する。
- pnpm8.11.0のlockfileにはTypeScript7のLinux/Windowsを含むOS別optional binaryとintegrityが含まれる。install scriptを無効にしたインストールでnative compilerを実行できる。main統合後もNode22.22.0で型検査・全322件・本番buildが成功した。bootstrap/release input/Docker context guardも成功。GitHub CIとWindows実動作は検証中で、成功前に完了扱いにしない。
- ユーザーがpushとPR作成を明示承認したため、専用branchをpush済み。サブエージェントの最終差分レビューとCI結果をDraft PRへ記録する。タグやReleaseは発行しない。

## 2026-10-05 Issue #101: Rice自身のMIT正本と実配布物の最終照合

- 権利者が既に配置したMIT正本のSHA-256 eeb4b00cfe4a9c135ab47b643c44f4c0b747318c0d52cee8580bf7c3d2ca0667を維持した。npm/Cargo/bundle metadata、README、inbound=outboundの貢献条件、外部GPL参考実装をコピーしない方針を照合し、現在のGitHub repository license APIもSPDX MIT/正本LICENSEを返す。欠落/改変/metadata不一致/installer・portable同梱漏れ/貢献条件欠落のpolicy7件が成功した。
- [実配布候補37274393563](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393563)のsource74ed02eは正本LICENSEを直接artifact、portable ZIP、NSIS installer resourceへ含む。manifest/CRC/checksumと同runのWindows検証記録を再照合した。NSISの独立展開でもLICENSEの正本digest一致を確認し、fresh Windowsでsilent install後のoffline LICENSE比較、installed実起動、uninstallとresource除去が成功した。この候補のタグ・Releaseは作成していない。Rice自身のMIT表示と、まだ未完了の第三者通知#102の配布条件を混同しない。

## 2026-10-05 Issue #91: Windows全体テストと配布物の公開前検証

- source74ed02eの[候補37274393563](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37274393563)は全11jobsが成功し、通常6workflowも成功した。Windows全体263件/0failure/3ignoredと、別stepで明示実行するheadful3件の各1件/0ignore、実Credential ManagerとLauncher41件を確認した。実WebViewの16 style検査、200 tile624.4ms/設定get501.7ms/追加JS heap30.4MiB、future schema verified:trueも成功した。receiptはportable PID3116/6649ms/正常exit0とinstalled PID5996/5144ms/正常exit0、silent install/uninstall各exit0の4probeを記録した。manifest SHA-256はa6c3017edd12574f76c9da5e05bbd7b8f7b6d65520b0a92f8757baf93ee16e9e、portable exeは1f7732cfef13e0879eb6f3e04cc18052d8d8da6038f13de0137ca512bbdb28f3、期待NSIS exeは1dac1847aca3631138f91f3819f73cc0435f3de0377133773cfb635d46885cafで、同run最新jobs/stepsと実receiptをtrusted verifierで再照合した。NSISを独立展開し、実payloadが期待exeと一致しLICENSEも正本eeb4b00cfe4a9c135ab47b643c44f4c0b747318c0d52cee8580bf7c3d2ca0667と一致することを確認した。Windowsでもinstalled LICENSE/version/registration、uninstall後のapp/resource/registration除去と今回のscratch/profile cleanupが成功した。SBOM7093 componentのNSIS3.11実compiler hashは5d034cfda6635fd7e281df852dcf1aad304614542db3665044c99a24f55b078fでbuild材料と一致した。候補はmanual event/tag:nullであり、タグ・Releaseは作成していない。第三者通知#102やpackaged IPC/UI追加検査#75は、この成功だけで完了扱いにしない。
- NSIS期待exeの導出/別literal保持/印の欠落・重複・別形式/descriptor改ざん、receiptでの形式別hashの誤用等の追加10件を含むlocal関連93件とworkflow/context policyが成功した。実旧候補のpayloadとも期待digestの完全一致を確認し、候補を新sourceで作り直す。旧runを再実行したり、失敗receiptや取得済みmanifestを成功へ書き換えたりはしない。
- source2f5fdaeの[候補CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37272350673)では通常6workflow、Windows全体263件/実keyring/Launcher41件/明示headful3件、Docker buildが成功した。Windowsのclone回帰2件も成功し、実portableはnative windowを表示して9305ms生存し通常closeでexit0、NSIS silent installもexit0となった。installed exeの比較で停止したが、取得したNSISを展開してportableと照合すると差分はoffset10726400からの3byte（UNK→NSS）だけであり、LICENSEは正本と一致した。[固定CLI2.12.1のbundle.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle.rs)はNSISへ__TAURI_BUNDLE_TYPE_VAR_NSSを書き込み、bundle後に元のUNK executableを復元する。portableからその唯一のUNK変数だけを置換した期待hashはac3e756d97c1665046f460e5600e64285a6e88f94d9f9dd26e41d87db4f14b3cで実NSIS payloadと一致した。manifestにこのexactなNSIS executable descriptorを加え、installed/portableそれぞれのdigestを照合する。印の欠落/重複/別形式、その他byteの変更、portable hashのinstalledへの誤用は拒否し、Tauriのpackaging挙動や元exeを変更しない。installed起動/uninstallはまだ未確認で、失敗した旧候補のreceiptを成功へ書き換えない。
- [source1c2c403の候補](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37270108901)はNSIS3.11のtools probe、Rust release exe、NSIS installer/portable、exact集合/CRC/PE/manifest/checksum、SBOM生成まで成功した。取得した実bundleをlocalでも再照合し、LICENSEは正本のeeb4b00cfe4a9c135ab47b643c44f4c0b747318c0d52cee8580bf7c3d2ca0667、SBOMの実nsis componentはv3.11/実compiler SHA-256で7093 componentを記録することを確認した。Windowsは実起動前にBundle LICENSE mismatchで停止した。[Git公式eol属性](https://git-scm.com/docs/gitattributes#_eol)とGit cloneのcore.autocrlf=trueによる再現を照合し、LICENSE/2 lockfiles/Cargo.tomlのcheckoutをLFへ固定する。元の正本・digest・byte比較は変更せず、属性なしの負例で4fileのCRLF化、属性ありの正例でexact byte保持を確認した。local関連83件が成功し、Windows候補でもこの2つのclone回帰を実行する。実portable/NSIS install/launch/uninstallはまだ未確認で、失敗した候補を公開しない。
- 配布前レビューでcompiler digestをtoolsのversion mapへ入れると、SBOMがhash専用の架空toolを作ることを確認した。digestをtoolHashesへ分離し、実NSIS componentのSHA-256に結び付ける。reviewed NSISの欠落hash・不正hash・未知tool（prototype名を含む）・版不一致を拒否する回帰テストを追加し、installed graphを含むSBOM7件とartifact/input/license/receiptの合計63件、全関連policy81件が成功した。compiler/provenanceの記録修正なので、旧sourceの候補はsupersedeし、本物のcandidateも修正後commitで再確認する。観測timeoutによるrestartではなく記録コードの修正による新candidateであり、他の正常な6workflowは止めない。
- source95fc65aの[品質9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264663324)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264733977)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264733741)、[native Windows](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264733919)、[保存権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264733777)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264734020)はすべて成功した。[候補](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37264663033)でもWindows263件/実keyring/Launcher41/明示headful3件が成功し、16 styles、200 tile674.6ms/設定get418.8ms/追加JS heap30.4MiB、future schema verified:trueを確認した。Dockerはversion mismatchを解消して本番release exeまで生成したが、bookwormのNSISにWin/RestartManager.nshがなくinstaller生成で失敗し、artifact smokeはskipとなった。
- [NSIS公式POSIX build手順](https://nsis.sourceforge.io/Docs/AppendixG.html#g3)に従い、3.11のnative compilerと同じ版のWindows zipを組み合わせる。[Tauri CLI2.12.1の正本](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs)もNSIS3.11とRestartManagerを要求する。source archiveは[Debianの3.11-1 DSC](https://deb.debian.org/debian/pool/main/n/nsis/nsis_3.11-1.dsc)のSHA-256、Windows zipはTauri binary-releasesのasset digestを照合してmanifestへ固定した。SConsはversion文字列だけでなくVER_MAJOR/MINOR/REVISION/BUILDを渡し、packed versionが0x03011000であることをprobeが要求する。compiler版/必要headers/Windows System pluginを使うlocal installer生成とruntime NSIS guardが成功した。これはtools専用probeであり、本物のRice NSIS/portable実起動を代替しない。追加の入力拒否7件を含むpolicy74件とcontext/workflow/license検査が成功し、実native compilerのSHA-256もBUILD-MATERIALSへ記録する。
- [source deac006の候補CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37263095537)でWindows全体263件が成功した（headful3件はharnessでignoreされ、後続の明示stepでそれぞれ1件/0ignoreが成功）。実Windows Credential Managerのdedicated namespaceで保存・読込・上書き・実write failure時の旧値保持・削除が成功し、Launcher41件、focus、16 styles、200 tile描画473.5ms/設定get326ms/追加JS heap38.8MiB、future schemaのverified:trueと通常終了を確認した。PowerShell2scriptのParser.ParseFileも成功した。[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37263098051)も成功したが、配布DockerはTauriのRust 2.12.1 / JS API 2.11.0のmajor-minor不一致でbuild前に停止し、artifact smokeはskipとなった。NSIS/ZIPの実動作の証拠にはしない。
- [Tauri公式release一覧](https://v2.tauri.app/release/)の2.12系列とnpm registryのexact 2.12.1/integrity/licenseを照合し、API/CLIをRust coreと同じ2.12.1へ固定する。pnpm 8.11でlockを再生成し、変更はAPI/CLIとCLIのplatform binary、およびplugin-dialogから同じAPIへ向くedgeだけに限定した。Rust lock/compiler/監査例外は変更しない。新しい互換性guardはRust lockの単一coreとexactなAPI/CLI input・installed packageを照合し、old minor/other major/floating input/stale installを拒否する6件が成功した。通常frontend buildとDockerで実行し、Tauri本来のversion checkも無効化しない。既存artifact/license/inputs/quality/auditを含むlocal63件とDocker context、frontend322件、全frontend/build/security/license gate、installed graph SBOM6件、Rust1.90 fmt/strict clippyも成功した。npm監査はHigh/Critical0、Moderate8件が残り、既存Rust例外の解消を意味しない。実配布smokeは修正後sourceのcandidateで再確認する。
- 新しいartifact/receipt verifierはNode標準APIだけでexact集合、source/3manifest/tag/lockfile/build材料、ZIP local/central layout・bounded inflate・CRC・flatなrice.exe/LICENSE、GUI/x64 PE、元LICENSE、checksumの完全な一覧を照合する。構造fixture18件とreceipt/job失敗fixture18件が成功し、既存license/inputを含むlocal49件、workflow/context policyも成功。構造fixtureの合成PEは実起動の証拠ではなく、実Docker candidateをWindowsへ転送してportable/native window/正常exit、NSIS silent install/同一exe・LICENSE/registry/installed起動/uninstallを検証する。レシートはexact commit/runId/manifest hashへ結び、trusted publisherはGitHub APIの同run latest jobsを全page取得して必要な実Windows stepのsuccessも要求する。
- [source63d45e3両OS](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37261813615)は成功したが、[Windows全体](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37261811789)はSTATUS_ENTRYPOINT_NOT_FOUNDで起動前に失敗した。no-run後に付けたmanifestが次のcargo testによる再linkで失われるため、実起動直前にCommon Controls v6を付けるCargo target runnerへ変更する。[Cargo公式のworking directory](https://doc.rust-lang.org/cargo/commands/cargo-test.html#working-directory-of-tests)はpackage rootなのでrunner pathはgithub.workspace由来の絶対pathとする。Windowsの成功までは未確認とし、ignore/テスト除外は増やさない。
- [source46ec8e5の再検証](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37262341109)は相対runner pathがpackage rootから見つからず失敗した。上記の絶対path修正で対応し、新sourceのWindows実行を必要とする。ローカルにYAML/PowerShell parserはないため、その構文検証も実Windows CIのParser.ParseFileへ追加した。構造fixture成功をWindows実動成功へ換算しない。
- 現在のmainはRust/Node/Docker digestを既に1.90.0/22.22.0へ固定しているため、compiler policyを変更しない。既存Windows native CIはlibtestをcompileしてLauncher/focus/budget/schemaの選択実行だけだった。all-targets/all-featuresのharnessをbuildし、従来どおりCommon Controls v6 manifestをlibtestへ付けてから、要求されたlocked/all-targets/all-features全体テストを実行する。
- #77で確認したWindows順序fixture3件の失敗は、8MiBがOSのsend bufferを必ず超えるという前提が原因。実production factory/runtime/SpeechSessionで小さなtalk packetを送信し、同じsessionを保持したままcontrol futureを一度pollしてPendingを確認、permit解放後にcontrol packetを確認する。人工的な待ち時間やOS buffer量を使わず、本番の共有gateとexact packetの順序を検証する。localで3/3とstrict全targets/allfeatures clippyが成功した。
- production KeyringAuthStoreのservice/accountを小さなinstanceへ移し、従来のservice/accountは定数instanceとして維持する。Windows testだけPID/nonce由来の専用serviceへ偽credentialを書き、missing/read/save/overwrite/real write failure/last-value保持/delete/idempotent cleanupを確認する。実ユーザーのOAuth service/accountは参照しない。source deac006の実Windowsで成功した。
- 配布物検証はmanifest/tag由来のexact集合・ZIP CRC/内容・PEと実NSIS install/launch/uninstallを公開gateへ結ぶ。NSISの[公式command line](https://nsis.sourceforge.io/Docs/Chapter3.html)に従い、/S、末尾のquoteなし/D=、uninstaller待機のため末尾の_?=を用い、CRCを無効化する/NCRCは使わない。tag/Releaseを発行せず実candidateを検証するread-only dispatchも実装した。まだ実NSIS/ZIPの実行成功前であり、Issueは未完了。

## 2026-10-05 Issue #64: optional/versionedな設定と非対応データの保護

- 最終source276c720は[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260473477)、[監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260470825)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260477961)、[Windows本番実動](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260475850)、[保存権限](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260479932)、[feature matrix](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37260481897)の全6workflowが成功した。重複schemaVersionを含むschema7テストが両OSで0失敗/0ignore、Rust通常262/no-default211、local frontend322と全gateも成功。Windows実WebViewでfuture schemaの本番startup/Settings・Launcher拒否/memory・primary・backup保持/正常exit/settings一覧3file/cleanupがverified:true（7.20秒、0ignore）。既存Launcher41/focus/16 stylesは保持し、200 tile render625.9ms/設定get490ms/JS heap差分34,560,078bytes（33.0MiB）、8MiB設定roundtrip Windows3.503秒/Linux2.745秒・追加Rust heap26,737,227bytesで既存予算内。下記の失敗・未確認記録は各source時点の履歴であり、最終headでは解消した。実配布物と第三者通知は#91/#102で別途検証する。
- [owner方針](https://github.com/hapo31/Rice-xwitch-comment-viewer/issues/64#issuecomment-5154975526)に従い、すべての永続fieldをoptionalとして不正値だけ既定値へ戻し、通常の補正通知は追加しない。番号なし/null/0の既存v0→v1だけを実在するmigrationとして扱う。domain/IPC DTOは厳格なまま、保存時だけschemaVersionを付け、loadは#88の実際のvalidated patchと共通domain/Launcher構造検証を使う。supported migrationの元bytesは既存の1世代backupに残し、次の通常保存でrotationする。
- 将来version/不正version型はno-auto-connectの既定値と復旧案内、未知field/重複wire keyは正常な既知fieldを読めるread-onlyとする。temporary/backupに触れる前にディスク上の現行fileを再検査し、起動後の外部変更でもSettings/Launcher/window保存のすべてから拒否する。設定の保存エラーと終了の可否は分け、位置保存の拒否を理由に終了できなくしない。構文破損primaryとfuture backupの組み合わせでもbackupをverbatimに復旧し、その後も保存を拒否する。
- 8MiBのowned Value treeや無制限配列を新たに作らず、[RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html)の借用と[map Visitor](https://serde.rs/deserialize-map.html)を使う。Cargoのserde_json版は1.0.149のままraw_value featureだけ有効にする。rules/Launcherは上限件数、文字列はescape-awareなraw長を検査してからowned domainへ渡す。未知objectのkeyをStringに解釈できない場合もread-onlyにし、情報を落として自動再保存しない。PNG検証・8MiB・5秒・既存heap上限は緩めない。
- fixture14例と7 Rustテストでoptional/null/型/範囲/正規化、無効Launcher/重複ID、rule・path合計、10万node配列、未知版・field/重複wire key、migration backup、save faultとfile交換時のmemory/primary/backup/listing非変更、future backup復旧を確認した。local Rust262/no-default211、strict clippy/fmt、frontend321と全gate、policy21/contextが成功。最大8MiB roundtripは4.56秒・追加Rust heap26,737,227bytes（25.5MiB）。
- Windows libtestは本番builder/ACL/commandをそのまま使い、隔離app-dataをtest-only plugin setupで事前にseedする。実際の本番startupから将来版を読み、Settings更新と現在のtest binaryのLauncher登録を拒否し、[WebviewWindow URL](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.url)の同一document fragmentでnative親へ結果を返す。実行ファイルは登録試験だけで起動せず、追加IPC/permissionを作らない。その後、本番app_exitをinvokeして正常終了と元primary/backup bytes・temporaryなしを検査する。実Windowsの成功はCI確認まで未完了であり、コード追加だけでは実動済みとしない。
- source6c11dafの両OS schema7件と8MiB予算（Windows3.54秒/Linux2.80秒）は成功した。nativeの元file/backup保持・正常終了も通ったが、隔離directoryの一覧検査が失敗した。[AppDirectoriesOverrideの公式source](https://docs.rs/tauri/2.12.1/src/tauri/path/mod.rs.html)を照合するとRootはconfig/data/local-dataを同じpathへ向け、WebView storageもその中に作る。future schema fixtureだけ各directoryを分離し、settings本体・backup・writer lock以外なしという厳格な検査を維持する。通常の本番保存コードや拒否条件は変更せず、修正後のWindows成功を改めて確認する。
- sourcefb76cf1のnative検証は結果通知のtimeoutで失敗した。`src/main.tsx`の[HashRouter](https://reactrouter.com/6.30.1/routers/create-hash-router)と`MainView`の未知route redirectが、裸の完了fragmentを`/chat`へ戻すことを本番AppShellのDOMテストで再現した。結果と段階通知を既存`/chat`のqueryへ置き、native側も観測URLと段階を診断に残す。DOMでは旧markerの消失とsuccess/failure queryの保持を確認し、frontend322件と全frontend gateが成功した。これは通知経路の修正であり、production IPC拒否・memory/file保持・正常終了・隔離settings一覧の検査は省略しない。
- sourcec267435の[Windows本番実動](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37258934906)は成功し、future schemaの結果`verified:true`、本番startup/Settings・Launcher拒否/memory・primary・backup保持/正常exit/隔離settings一覧3file/cleanupを実行した。既存Launcher41件、focus1件、最大表示1件も成功し、16 styles、render824ms/IPC492ms/JS heap差分30.4MiBが予算内。追加レビューでは先頭がv1・後方がfutureの重複schemaVersionに対し「unsupported/no-auto-connect」を求めるテストを先に失敗させ、同値の重複や解釈不能なroot keyを含め版番号が曖昧ならsafe defaultsとread-only/復旧案内にするよう補強した。4つのraw documentで実SettingsStoreのload/save拒否とprimary/backup保持も確認した。通常fieldの重複とは区別し、修正後headのCI成功を確認するまでは完了扱いにしない。

## 2026-10-05 main CI: braces の除去

- 最新 main 6d17a3a の 6 workflow は audit だけが失敗した（run37246805086）。blocking は [GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm) の braces <=3.0.3。GitHub advisory に修正版はなく、Tailwind 3.4.19 → chokidar/micromatch/fast-glob のビルド依存を除去する必要がある。無承認の例外や audit の閾値変更は行わない。
- [公式 Vite 統合](https://tailwindcss.com/docs/installation/using-vite)に従い Tailwind / @tailwindcss/vite を MIT の 4.3.3 に固定する。autoprefixer を除去し、PostCSS の余分な Tailwind plugin は使わない。[upgrade guide](https://tailwindcss.com/docs/upgrade-guide)の @config 読込、outline-hidden、shadow-xs、backdrop-blur-xs 等の変更を適用する。元の sRGB 色と日本語フォント、placeholder/cursor の挙動を維持する。icon の bare drop-shadow は v4 の互換定義が v3 と同じ二層/alpha のため保持し、変更された sm の値へ置換しない。Chromium 111+ の最低 CSS 要件を README に明示し、Windows 10/11 と最新 Evergreen WebView2 を正式経路とする。
- ローカルの初回 frozen install/build、frontend 288 test（生成 CSS 6 件を含む）、format/lint/typecheck/security/license、policy 20 件、Docker context が成功し、pnpm audit --audit-level high が成功（High/Critical 0、Moderate 8）。installed graph の SBOM も別途実行して 6/6 が成功し、braces の不存在と Tailwind の build scope を検査した。Rust 1.90 で all-features 244 件と strict clippy/fmt が成功した。lockfile の braces/micromatch/chokidar/fast-glob は除去された。Cargo 既知例外 2 件の解消を意味しない。
- [最低 WebView2 設定](https://v2.tauri.app/reference/config/#minimumwebview2version-1)を 111.0.0.0 に設定して、古い runtime の更新を installer が試みるようにした。portable は別に手動更新を案内する。設定/文書の一致を生成 CSS と同じ test group に追加（合計 7 件）し、最終 frontend 289 件が成功した。実際の旧 runtime 更新/packaged smoke は #91 の境界であり、設定だけで実動済みとは扱わない。
- source af59b8c を main 反映前の作業ブランチで検証し、[audit37250928344](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928344)、[quality 全9jobs37250928538](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928538)、[native37250928782](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928782)、[両OS37250928450](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928450)、settings permission/feature matrix の全6workflowが成功した。nativeのLauncher41件、focus1件、最大表示1件が失敗/ignoreなし。実WebViewの16 computed-style/geometry検査が成功し、200 tile render704.4ms、settings get766.1ms、JS heap差分47,509,465bytes（45.3MiB）、4 invalid request拒否/設定保持/正常終了を確認した。auditは npm High/Critical 0、既存のRust reviewed exception 2件（2026-10-21まで）と非blocking Moderate8件を報告して成功した。例外は解消ではなく、実配布物検証と第三者通知は依然 #91/#102 の境界である。

## 2026-10-05 Issue #88: 入力domainとTCP destination trust

- 最終source ec2d2ffはmain反映前の作業ブランチで[全9品質jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137470)、[依存監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137491)、[両OS契約](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137393)、[Windows本番実動](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37253137483)、settings権限/feature matrixの6workflowが成功した。両OSでfile改変からの同意迂回拒否とprimary/backup非変更、7 destination testsを確認し、実WebViewでinvalid invoke6件・remote flagのみ2経路を拒否/設定復元した。Launcher41件、focus、既存16 CSS checksも成功し、200 tile render692.8ms/IPC493.9ms/JS heap差分39,453,007bytes（37.6MiB）、8MiB設定roundtrip Windows3.58秒/Linux2.40秒で予算内。local Rust255/no-default204/frontend321、strict clippy/fmtと全frontend gate、policy21/contextも成功。実LANの相手・手動native dialogの操作や配布物smokeは別の実機境界であり、fake consentを実操作の証拠にはしない。既存Rust2例外は解消していない。
- 追加レビューで、観測したDNS/host/port変更や解決失敗の時点で許可を失効させる。DNSが元のIPへ戻っても古い許可を再利用せず、loopbackへの一時変化も同じ扱いとする。fake resolver/consentは7テストへ拡張した。実settings.jsonを直接編集してremote modeと偽のconsentを記録したfixtureを本番loader/factory/runtimeへ渡し、再起動相当の2 runtimeでもTCP開始前に拒否する。不正patch12種はprimaryだけでなく実backup bytesとmemoryの非変更を比較する。
- UIだけのlogin/文字列制約と任意hostnameへの平文TCPをbackendの境界へ移した。TwitchLoginをsettings_update/twitch_connectで共用し、wire未知fieldと256KiB patchをpreflight、NG rule/host/文字列/範囲を共通validatorで拒否する。保存は最新candidateの検証/永続化後だけ公開し、field/code/message/recoveryを返す。旧fileのoptional migration/fallbackはownerの判断どおり#64で同じvalidatorを使う。
- [Rust loopback定義](https://doc.rust-lang.org/std/net/enum.IpAddr.html#method.is_loopback)を使い、127/8・::1・IPv4-mapped loopbackだけを通常接続とした。[Tokio TcpStream::connect](https://docs.rs/tokio/1.52.3/tokio/net/struct.TcpStream.html#method.connect)へ検証済みSocketAddr集合を渡し、全DNS結果の検査後に再名前解決してpolicyを迂回しない。remoteはRFC1918 IPv4/ULA IPv6のprivate宛先だけがeligibleで、hostname/port/address集合のnative承認がない送信は拒否する。public/link-local（metadata endpointを含む）/multicast/未指定宛先は許可しない。
- [Tauri公式native dialog](https://v2.tauri.app/plugin/dialog/#build-an-ask-dialog)のnon-blocking Rust show callbackをoneshotへ渡す。frontend message/confirm permissionは追加せず、明示app commandだけを既存manifest/capabilityへ登録した。利用者に保存済みhost/portと全解決address、平文で送るTwitch user/chat/test/control、TLS/peer authなし、VPN/暗号化トンネル注意を示す。設定lockはDNS/dialog中に保持しない。許可はopaque/backend-only/process-localで、fileやrendererにconsent flagを持たない。全address再確認/最新設定比較後だけinstallし、変更・拒否・再起動は再許可を要する。同時prompt1つ/30秒rate limitを設ける。既に送ったbyteを回収する機構ではない。
- 30ケースの共通fixtureでTS/Rustのlogin・DNS/IPv6・Unicode/UTF-8・NG総量・成功文の境界を合わせる。fake resolver/consentの7テストでmixed DNS、private consent、public/link-local拒否、DNS変更、再起動/endpoint変更/拒否/同時prompt、全talk/query/control/diagnosticsの非再送拒否を検証する。実Windows WebViewにはstructured invalid invoke6件、remote flagのみのprobe/diagnostics2件拒否と設定復元を追加し、結果確認までIssueは未完了とする。
- 最大quota fixtureのmulti-MiB NG wordは新domainでは不正なので、200 icon/4MiBとvalidな131×500字NG ruleへ変更した。8MiB設定ファイルは同じ有効JSONへwhitespaceを付け、primary/backup/readの従来5秒/40MiB予算を維持する。native結果をTwitch loginへ書く検証迂回は行わず、500文字以内のNG wordをtest-only transportにした。本番command/permissionのテスト除外は追加しない。

## 2026-10-05 Issue #76: shortcutの受付と対象process生成

- Explorerのspawn成功を対象アプリの成功へ換算していた。通常exeへのlinkだけを起動都度解決し、target/cwdを再検証した上で直接CreateProcessする。結果のlaunchedCountはprocess生成確認だけと定義し、UIに準備完了未確認と全failureの名前・原因・修復/再登録を表示する。設定に解決targetをcacheしない。
- [Microsoft Shell Links](https://learn.microsoft.com/ja-jp/windows/win32/shell/links)のtarget/arguments/working directory/icon sourceを構造化する。[MS-SHLLINK header](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/c3376b21-0931-45e4-b2fc-a48ac0e60d15)の76bytes/CLSID/flagsと[LinkFlags](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/ae350202-3ba9-4790-9e9e-98935f4ee5af)に基づき、HasDarwinID/RunAsUserをCOM読込前に拒否する。Resolve/installer修復/UAC promptを呼ばない。URL/仮想folder/入れ子linkはunsupportedとして手動起動/通常exe登録を案内する。
- argumentsは[標準CommandExt::raw_arg](https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html)でliteral tailを渡す。最大1MiB link、展開後4096byte path、64KiB UTF-8/16Ki UTF-16 argumentを検証する。[CreateProcessのUAC error740](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/user-account-control-for-game-developers)を固定日本語で案内し、OS errorの英語/日本語textには依存しない。
- 新COM処理でUI threadを塞がないようcommand/serviceをasync化し、OS作業を共有4-worker poolへ移す。order/部分成功/lock非保持は維持する。timeout後もpermitを実終了まで保持し、cancel/deadlineをOS呼出し前に再確認する。実行中CreateProcessの強制取消は保証せず、不確定な起動はfailure/画面確認を案内して自動再試行しない。仮想時計＋channelのテストで遅延resolverがprocessを起動しないことを確認する。
- Stateを借用するasync commandが単純なDTO returnだとTauri macroのlifetime検査で失敗した。[公式async commandのResult回避策](https://v2.tauri.app/develop/calling-rust/#async-commands)に従ってOkへ包み、frontendの解決済みJSON契約は変えない。本番ACL/command名/依存版は変更しない。Unicode COMは既存lockのwindows 0.62.2（MIT OR Apache-2.0）をdirect dependencyにも固定参照する。lock変更はRiceの参照1行で、依存の版更新はない。
- Windows integrationはreviewedなstandalone Rust probeを隔離directoryへcompileし、実IShellLinkWでlinkを作成して本番serviceから起動する。正常link、日本語/空白/メタ文字引数、cwd/空欄fallback、登録後のlink編集、missing/moved target、missing cwd、RunAsUser拒否、直接exeとbulk部分成功を検査し、成功probeの実args/cwdと失敗の成功件数0/marker非生成を比較する。これは対話的UAC承認/外部アプリready/配布物smokeの証拠ではない。Windows実行結果を確認するまでIssue/TODOは未完了とする。
- 初回native Windows run37244260983は39件が成功し、実link fixtureの最初のPowerShell作成処理が5秒timeoutで失敗した。childの終了とpipe回収は確認したが、実link成功とは扱わない。helperのstdinをWindows NULから起動直後に閉じるpipe（明示EOF）へ変更し、fixtureの4つの段階だけをstderrへ記録してtimeout時に最大400文字を回収する。timeout/並列/メモリ予算の引き上げやskipは行わず、再検証する。
- run37244668257ではPowerShell開始/COM生成まで進んだが、fixtureのTargetPath代入がArgumentExceptionで失敗した。stdin変更だけで直ったとは扱わない。fixtureも本番と同じDOS/UNC canonical pathへ統一し、自作probeの生成/MZを明示検査、Rust/PowerShell両側のfixture-owned pathと存在確認で入力境界を診断する。実linkの失敗をignoreや許可拡張で回避しない。
- run37245084941でもTargetPath代入で失敗した。自作exeのMZ/実在、DOS path、109 UTF-16 units（MAX_PATH以内）、PowerShell側Test-Path=Trueを確認した。日本語を除いて弱いfixtureにせず、本番/fixtureともUnicode APIへ変更する。[CoInitializeEx](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex)のS_FALSEを含む成功を同threadでbalanceし、[IPersistFile::Load](https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-ipersistfile-load)はread-only、Resolve/Save/activationは本番では行わない。
- [IShellLinkW::GetPath](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishelllinkw-getpath)はMAX_PATHで、raw targetの259文字目を超過扱いにして切り詰めを受理しない。[GetWorkingDirectory](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishelllinkw-getworkingdirectory)等もbuffer上限で切り詰められるため、すべて上限+余分な1文字+NULを確保し長さ/終端/Unicodeを検証する。target/cwd/iconの環境変数だけ展開する。同期COM停止の強制取消はできないため既存7秒/4-worker/cancel-before-spawn境界を使う。shortcut経路のPowerShell子process/JSON輸送は削除した（icon抽出の既存5秒child境界は不変）。
- 最終e5791e3の[Windows native run37246369941](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246369941)はLauncher41件・失敗0・ignore0で実Unicode COM起動を確認した。実WebViewの200 tiles/全iconは695ms、設定IPC715ms、JS heap差分45,329,006bytes、不正4要求拒否・設定保持・正常終了、2process restore/focusも成功した。[両OS Launcher39件](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246369853)、[品質全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37246370049)、feature matrix/settings permissionも成功。Windows permissions testの範囲はRunAsUser要求を起動前に拒否することとnative error codeの案内で、対話的UAC承認や配布物の実動は含まない。auditは既知braces High1件でfail-closedのまま（run37246369811）。

## 2026-10-05 Issue #82: Launcherのレイヤと依存注入

- model/ports/service/workers/repository/commands/eventsとplatformのtarget/process/windows icon/launchを分離した。pure model/normalizationとserviceはTauri、PowerShell、filesystem/process実装をimportしない。AppStateの1つのLauncherRuntimeで全commandがadapter/poolを共有し、commandsはborrowed IPC変換・wiring・service呼出しだけを担当する。ApplicationLauncherにはkindを渡さず、Website dispatchの予約/拒否をservice側に置く。
- repositoryはLauncher item mutationを最新AppSettings candidateへ1回だけ適用し、既存settings transactionの検証/保存後に公開する。serviceがsettings全体や具象SettingsStoreへ依存しない。save失敗は成功ログ・fallback通知も発行しない。追加workerのtimeout後もpermitを実終了まで保持し、遅延した結果をsaveしない。
- fake resolver/icon extractor/application launcher/repository/sinkで9件の本番serviceテストを追加した。icon timeout/failure fallback、invalid/resource-limit拒否、shortcut解決失敗、spawn部分成功、read/save failure、全section rollback、同時add/remove・重複target・別section更新の保持、非対応OS・WebsiteのOS adapter非呼出しを確認する。実filesystem/canonical pathとUnix child kill/reapは別境界として維持する。
- 仮想timeoutテストの初回は待機したまま停止し、検証processだけを終了して原因を調べた。使用中のTokio 1.52.3 sourceと[公式pause説明](https://docs.rs/tokio/latest/tokio/time/fn.pause.html)でblocking taskによるauto-advance停止を確認した。開始channel handshake後にpauseし、job/permitの両deadlineを明示advanceする。millisecondに丸められるtimerを1ms越えて待つ。壁時計sleepやbusy loop、production timeout/並列上限の変更は行わず、9件が0.00秒で成功した。
- 既存worker integrationのdummy exe/lnkにはfake NoIconExtractorを使い、Windowsで実PowerShellを誤って起動するtest fixture couplingを除去した。本番SystemIconExtractor/PowerShell/Explorerの処理自体は分割前の動作を維持する。Windows native CIで本番featureの全Launcher suiteを明示実行し、既存200 tile/IPC拒否・2process focusも再検証する。
- ローカルのRust all-features239件/no-default188件、strict clippy/fmt、frontend281件、format/lint/typecheck/build/security/license、quality policy3件、Docker contextが成功。最大JSON8MiBの200件roundtripは3.95秒・追加Rust heap39.4MiBで元の5秒/40MiB予算以内。実shortcutのbroken/moved/引数/UACの意味論は#76、実配布物の検証は#91に残す。依存・本番ACL/command manifestは変更せず、既知npm Highの配布停止も維持する。
- commit8579773の[Windows/Linux契約CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156624)が成功し、両OSの全Launcher34件（fake service9件を含む）を確認した。最大JSON8MiB roundtripはWindows3139ms・Linux1498ms、追加Rust heap約39.4MiB。[native Windows](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156691)も本番featureのLauncher34件/ignored0と、明示実行のfocus1件・最大Launcher1件が成功した。200 image decode/2frame描画548.7ms、最大設定IPC547.4ms、JS heap差分45,821,351bytes（43.7MiB）、不正4要求reject/設定完全一致、正常終了と隔離storage削除を確認した。JS heapと全WebView RSS/GPUは区別する。[quality全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37242156974)、feature matrix37242156631、Windows writer/permission37242156633も成功。audit37242156673は既知braces GHSA-vfj7-8cjw-p6xmだけをblockingとして正常に失敗し、Rust既知例外2件以外の追加指摘はなかった。

## 2026-10-05 Issue #71: Launcherと設定の資源境界

- 設定取得も本番IPCで初回/拒否要求後の2回を測定し、それぞれ2秒以内を予算とする。
- 最初のWindows/Linux契約CI 37239473912ではLinuxが成功し、Windowsの既存concurrent-merge fixtureがverbatim prefix付きcanonical pathを期待して失敗した。本番は既にDOS/UNCへ正規化するため、fixtureもその保存形式を厳密に期待するよう修正し、WindowsのDOS/UNC変換を別のpure testで明示する。失敗をskip/ignoreに置換しない。
- native run 37239473874で実200tileは617ms、最大設定IPC取得940ms、4つの不正要求の拒否/設定非変更が成功した一方、JS heap差分79,513,898bytesで64MiB予算を超えた。fixtureの比較が最大JSONを2回stringifyし、Unicodeを含む巨大な追加bufferを作っていたため、field/型/array/key/valueを厳密に比較する再帰処理へ変更して、その計測専用の全量複製をなくす。sampling範囲/64MiB予算は維持し、GC強制や測定停止で失敗を隠さない。
- 最終コードfd8a00aの[native Windows run 37239996083](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37239996083)が成功した。200個のimage.decodeと2frame描画616.9ms、最大設定の2回のIPC取得の最大734.2ms、JS heap差分45,296,687bytes（43.2MiB）。不正4要求は全て拒否し、field/key/array/valueの完全一致、正常終了・隔離storage削除を確認した。通常の2process復元/focusも再度成功した。[Windows/Linux run 37239996196](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37239996196)では全Launcher/resource/rollback/recoveryを成功し、8MiB最大JSONのtransaction/backup/loadがWindows2689ms・Linux2439ms、追加Rust heap約39.4MiBだった。quality全9jobs/feature matrix/Windows writerも同commitで成功した。依存版や本番ACLは変更せず、既知braces High1によるaudit/release blockは維持する。
- Tauri 2.12.1のCommandItemはIPCでparse済みのValueを借用し、Requestもbodyを借用することを公式crate sourceで確認した。Launcher追加と設定patchはRequest経由で、owned DTOへのclone/deserialize前にJSON byte/node/depthとpath件数/総量を検査する。framework自身の最初のIPC parseをアプリ側で制限できたとは扱わない。handler manifest/ACLを広げない。
- 既存の検証済みPNG data URLとassetProtocol無効を維持し、per-icon encoded/decoded/dimensionと合計quotaを下げる。renderer patchはbackend生成ID/target/iconを持たない編集DTOへ分離する。元の永続modelは互換を維持する。quota超過はtransaction保存前に拒否し、同時更新のmergeでも部分保存しない。
- settingsの読込とserializerは8MiBを上限にし、bounded readとwriterで上限超過file/stringの全量複製を避ける。最大200件・合計4MiB・128×128 RGBA16の200種類のCRC-valid PNG・最大文字列を使う別processで保存/backup/loadを測る。通常は5秒/追加Rust heap32MiB、JSON自体8MiBの最悪構成は5秒/40MiBを予算にする。test-only System allocator wrapperは[標準allocator API](https://doc.rust-lang.org/std/alloc/struct.System.html)へ同じpointer/layoutを委譲し、Rust-owned live heapだけを計測する（RSS/native library/GPUは含めない）。
- native Windowsはproduction frontend/commands/ACLを維持した隔離fixtureで200 tilesと全128×128 iconのdecode完了・2frame描画を2秒以内、JS heap差分64MiB以内と定義する。[performance.memory](https://developer.mozilla.org/en-US/docs/Web/API/Performance/memory)はChromium限定の非標準推定値で、全WebView process memoryの保証ではない。[GoogleChrome診断flag](https://github.com/GoogleChrome/chrome-launcher/blob/main/docs/chrome-flags-for-tools.md)と[Microsoftのtest-only browser flag手順](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags)に基づく精密化flagをtest childだけへ渡し、20ms/各IPC完了でsamplingする。欠落/zero deltaは失敗扱い。本番flag/permissionは追加しない。最終結果は上記37239996083で確認した。

## 2026-10-05 Issue #85: 項目に保持する読み上げ結果

- 一時warningだけへ保存していたformatter/連投理由をBlockedReasonへ、取消を4つのSkippedReasonへ、worker失敗を既存FailureCodeへ統一した。outcomeはkindで判別する共通契約とし、安全な固定日本語、retryable/recoveryAction、遷移時刻をbounded item/historyへ持たせる。adapter detailやNG一致語を理由へコピーせず、warning/logへitem IDを付ける。
- auto retry中は直前の失敗を保持し、manual retry/正常完了で消す。取消後に到着する結果は理由も変更しない。occurredAtMsはUTC wall clockの説明用値で、既存のmonotonic revision/IDによる順序を変更しない。
- Queueは既定のskip非表示を維持しつつ履歴toggleを追加する。Chatは状態buttonから非modal下部詳細paneを開き、既存の行高さを保つ。復旧routeはfrontendに留め、keyboard opening/Escape/return focus、同status理由更新、理由消失時のfocus、snapshot/reload/late subscriptionをDOMとbridge/storeで検証する。
- 最終コード2c5182eでRust all-features219件/no-default168件、frontend279件、strict clippy/fmtと全frontend/build/security/license検査、quality policy3件を確認した。[Windows/Linux CI](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933123)では各platformの新outcome6件とlatest snapshot1件を明示実行し、本番worker/既存操作列も成功した。[quality全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933304)と[Windows実動single-instance](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37235933133)も成功した。依存追加はなく、既知advisoryのrelease blockを変更しない。

## 2026-10-05 Issue #66: 多重起動禁止と設定writer所有権

- ownerの[多重起動禁止の判断](https://github.com/hapo31/Rice-xwitch-comment-viewer/issues/66#issuecomment-5154957888)に従う。CASや複数profile機能は追加しない。[Tauri公式single-instance手順](https://v2.tauri.app/plugin/single-instance/)に従い最初のpluginとして登録する。公式plugin 2.5.2のmanifestでMSRV 1.90 / Apache-2.0 OR MITを確認し、既存固定compilerに合わせて版を固定した。新規Linux zbus系を含む10依存も許可済みpermissive licenseを確認した。
- pluginのWindows/Linux実装を確認した。通知可能な既存ownerへcallbackを送って2回目を終了する一方、Windowsの初期化競合やLinuxのDBus登録失敗ではpluginだけで設定排他を保証できない。固定lock fileの所有権をsettings loadの前に取得し、所有者でないsaveも拒否する。終了時にlock fileを削除すると別inodeへの二重lockを許すため削除しない。[std File::try_lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)はRust 1.89以降、Unix flock / Windows LockFileExに対応し、Fileをprocess lifetimeのmanaged stateで保持する。
- native Windows CIはproduction builder/plugin/setupと実embedded frontendを使う。最小化した実HWNDの復元・foreground一致、2回目の正常終了、設定bytes非変更をassertする。desktop/WebView2不足は失敗として扱う。Windowsのknown-folder APIはAPPDATA環境変数だけでは隔離できないため、[Tauri appDirectoriesOverride](https://v2.tauri.app/reference/config/#appdirectoriesoverride)をtest contextだけで指定し、全app directory/WebView保存先を固有tempへ隔離する。資格情報は既存store injectionへ空のfake backendを渡し、実keyringへ触れない。production command/capabilityやtest環境変数による本番設定変更は追加しない。
- headlessの別OS process検証はownerがNG設定を保存後、contenderを読込前に拒否し、正常終了/kill後のsuccessorが再読込して別sectionを更新してもNGを維持する。lockの保存先一致、Unix permission/link拒否も検証する。native focusの成否とWindows runtime全般・配布物のsmoke（#91）は区別する。
- 最初のnative Windows run [37232558548](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37232558548)はtest main前にSTATUS_ENTRYPOINT_NOT_FOUNDで失敗した。local rfdがTaskDialogIndirectをstatic importし、tauri-winresは通常binだけへmanifestをlinkすることを確認した。[MicrosoftのAPI要件](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect)と[Tauri上流の同症状](https://github.com/tauri-apps/tauri/issues/11028)に対応し、productionと同じCommon Controls v6依存manifestを[Windows SDK mt.exe](https://learn.microsoft.com/en-us/windows/win32/sbscs/mt-exe)で生成済みlibtest exeだけへembedしてから明示実行する。本番manifest/ACL/依存版を変更したり、失敗をskipに置き換えたりしない。
- 次のrun 37233158467でtest mainは起動したが、fixtureがbuild直後にmain windowを取得して失敗した。Tauri 2.12.1のApp::make_run_event_loop_callbackを確認し、window作成/setupはevent loop Readyで実行されるため、fixtureのReady callbackで初めてHWND/設定を取得して通知するよう修正した。2回目がそのReadyへ到達した場合は引き続き失敗する。
- run [37233548545](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233548545)では既存HWNDの復元/foreground一致、2回目の正常終了、設定bytes非変更の全assertが成功し、最後のtemp削除だけがWindows sharing violationで失敗した。[WebView2のUDF削除要件](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder#deleting-user-data-folders)に従い、fixture自身のwindowへWM_CLOSEを送りhostの正常終了を確認してから、browserの非同期解放を上限30秒で待つ。retryはsharing/lock/nonemptyの既知エラーだけで、削除失敗の握り潰しや他アプリのkillは行わない。
- 最終コード3f1d112のnative Windows run [37233983947](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233983947)が成功した。headful testは明示実行1件/成功1件/ignored0で、2回目の正常終了、復元/foreground、設定bytes保持、owner正常終了と隔離directory削除を確認した。同commitの[quality全9jobs](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233984123)、[Windows writer/permission](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233983993)、[Windows/Linux transport/queue](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37233984010)も成功。libtest exeに通常appのicon resourceがない警告は本番bundleの問題と断定せず、実配布物検証は#91で別に扱う。

## 2026-10-05 Issue #173: Tauri 2.12への更新と例外縮小

- 最新の公式crates.io indexでTauri 2.12.1 / Tauri Utils 2.10.1が公開済みであることを確認した。両者のMSRVはRust 1.90。Utilsのurlpatternが0.6へ更新されたため、Rust/compilerの固定policyとdevcontainer bootstrapを同じimmutable Rust 1.90.0 image digestへ変更し、Tauriを2.12.1へ更新した。依存解決によりunic系5crateがlockfileから除去された。
- cargo-audit 0.22.2をignore前のreportで実行し、Rust警告が7件からLinux GTK3のglib/proc-macro-errorの2件へ減ったことを確認した。残る2件は互換範囲のglib更新ができず、ownerと2026-10-21の期限付き例外だけを維持する。これらが解消したという意味ではない。Windowsの非選択も別途graphで確認する。
- 共通監査ゲートは新規npm braces Highを理由に停止する。Rust例外の正常評価と、全依存のclean auditは区別する。npm例外を勝手に追加しない。

## 2026-09-23 Issue #173: 再開時の統合検証

- 最新 main bb7f324 を既存 branch へ取り込み、並行した調査メモと TODO を保持した。cargo-audit 0.22.2 の online 監査は RustSec DB `1e640cd56d7604993e3a9ec392060666e3b95ccc` と registry 更新を含め成功した。7件の期限付き例外を適用した結果であり、上流指摘は残っている。
- Issue #96 の validator で現在の例外を受理し、2026-10-22 では期限切れとして拒否することを確認した。GUI 非依存 Rust テスト115件、Tauri 有効構成161件が成功。最初の sandbox 内実行は loopback bind が拒否されたため、許可された環境で再実行した。#96 の main 統合後に完了を判定する。

## 2026-09-21 Issue #173: Tauri 上流由来 RustSec 指摘

- 親レビューで `cargo-audit audit --file src-tauri/Cargo.lock --deny warnings` をonline実行し、RustSec DB `57ad4063bb49c1deb04b6fcee30cfbac6b508474` と crates.io index の取得を含めexit 0を確認した。これは7件の期限付き例外を適用した結果であり、依存自体の修正や警告の消滅を意味しない。

- 参照: [glib advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)、[Tauri 2.11.6](https://crates.io/crates/tauri/2.11.6)、[tauri-runtime-wry 2.11.4](https://crates.io/crates/tauri-runtime-wry/2.11.4)、[tauri-utils 2.9.3](https://crates.io/crates/tauri-utils/2.9.3)。親レビューでも Windows MSVC target の glib 非選択、unic の build/runtime 経路、Linux の proc-macro-error 経路を確認した。#96 の validator に例外案を渡す独立検証では現日付で成功し、2026-10-22 で期限切れとして拒否された。cached audit（no-fetch/no-yanked）もexit 0だが、mainへの例外反映と完了判定は #96 の期限検証統合後とする。

- 公式 crates.io API と crate manifest を online で確認した。Tauri 2 系の最新は 2.11.6、Tauri Runtime Wry 2 系の最新は 2.11.4 である。`cargo update --dry-run -p tauri --precise 2.11.6` は tauri / tauri-build / tauri-codegen / tauri-macros / tauri-runtime / tauri-runtime-wry / tauri-utils と tray-icon を更新候補として表示したが、advisory の経路を解消しないため lockfile 更新は行わない。Tauri 2.11.6 は tauri-runtime-wry 2.11.4 と tauri-utils 2.9.3 を要求する。前者は Linux target に GTK 0.18 と webkit2gtk =2.0 を、後者は urlpattern 0.3 を要求する。Wry の独立最新 0.57.0 は GTK 0.18 / webkit2gtk =2.0.2 を維持し、Tauri Runtime Wry 2.11.4 は Wry 0.55 系を要求する。webkit2gtk 2.0.2 は glib ^0.18、urlpattern 0.6.0 は公開済みでも tauri-utils の 0.3 制約と互換でない。Tauri 3.0.0-alpha.1 は Rust 1.95 が必要な pre-release であり、固定 Rust 1.89 の本アプリには更新候補としない。
- 現在の `Cargo.lock` と `cargo tree --locked --offline` では、glib 0.18.5 と proc-macro-error 1.0.4 は Tauri/Wry の Linux GTK3 graph にある。Windows platform を指定した `cargo metadata --locked --offline --filter-platform x86_64-pc-windows-gnu` は gtk / glib / webkit2gtk / proc-macro-error を含まず、urlpattern 0.3.0 と unic-ucd-ident 0.9.0 は含む。したがって Windows runtime / build graph の unic 系5 package と、Linux target だけの GTK3系を区別するが、lockfile だけから実行時到達可能性を断定しない。
- RUSTSEC-2024-0429 が示す unsound API は `glib::VariantStrIter::{next, nth, last, next_back, nth_back}` である。Rice source と lockfile の直近上流 source（tauri 2.11.2、tauri-runtime-wry 2.11.2、wry 0.55.1、webkit2gtk 2.0.2）を `rg` で調べ、この API / `g_variant_get_child` の呼出しは見つからなかった。ただし GTK3 graph 全体の非到達性を証明するものではない。Linux build/runtime がこの API を利用すれば undefined behavior / crash のリスクが残り、glib 0.20 への個別 override は Tauri/Wry / webkit2gtk の 0.18 API 制約を破る。
- proc-macro-error と unic の6 advisory は patched release を持たない unmaintained advisory である。更新不能な7件は owner `hapo31`、期限 2026-10-21 として `.cargo/audit.toml` と `security/advisory-exceptions.json` に記録した。cargo-audit 0.22.2 を RustSec DB の取得済み snapshot で `audit --file src-tauri/Cargo.lock --no-fetch --no-yanked --deny warnings` 実行し、例外を読んで exit 0 となった。`cargo test --locked --no-default-features` は110件成功した。空の frontendDist directory を一時的に用意して `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` を実行し、warning は既存の unused import だけで成功した。
- Issue #96 の validator と release audit gate は main に未統合である。この Issue はそれらを複製しない。例外の owner / 根拠 / 期限を #96 の policy で検証し、期限前に Tauri/Wry または urlpattern の更新を再確認するまで、公開可能な clean audit と主張しない。

## 2026-09-21 Issue #175: npm high advisory の互換更新

- main ba44b3e の全依存監査で high 6件を確認した。Vite の Windows path 経由の deny bypass（GHSA-fx2h-pf6j-xcff）、nanoid の不正サイズ時 loop（GHSA-28wg-ghj8-5hjv / GHSA-2v37-7h3g-55p8）、PostCSS の source map path traversal（GHSA-r28c-9q8g-f849）、Browserslist の無制限 cache と custom stats 処理（GHSA-c83g-rgw3-j3cx / GHSA-73wf-gq98-2v4g）が対象。主に開発・ビルド依存であり、配布アプリでの到達を断定しない。
- Vite 8.0.16、PostCSS 8.5.18、nanoid 3.3.19、Browserslist 4.29.0 へ互換更新し、関連する bundler / browser data の推移依存も lockfile に反映した。直接依存の宣言変更は Vite と PostCSS の最小版だけ。major upgrade や waiver は追加していない。
- frozen install、frontend203件、typecheck、build、renderer security check、diff check が成功。pnpm audit --audit-level=high は exit 0。JSON audit の件数は high/critical 0、low1、moderate8であり、全指摘が解消したとは扱わない。Windows実機確認は未実施。


## 2026-09-20 Issue #65: 連投抑制時刻の期限・容量境界

- `last_user_enqueue` は受理済みコメントの user ID を無期限に保持していた。連投判定は最大30秒までしか参照しないため、channel ID と接続 generation を scope にした期限付き cache へ変更し、scope 切替または抑制なしへの変更を次の連投判定時に破棄する。
- 各受理時刻は FIFO の expiry record と対応付ける。background task は1秒ごとに期限切れ先頭を最大64件だけ取り出し、同一時刻に大量の期限が来た場合も mutex を解放して batch を続ける。期限30秒を1秒間隔で確認するが、解放時刻の厳密な上限はスケジューリングや mutex 待ちに依存する。enqueue 側も同じ cleanup を行い、HashMap 全体をコメントごとに走査しない。期限 FIFO が map entry を所有する不変条件により、FIFO の4096件の受理記録上限だけで map も上限内になる。
- 保持する受理記録が4096件を超える場合は最も古い期限 record を退避する。退避した記録が現行の時刻を指していれば、そのユーザーの連投抑制が window より早く解除される。2秒と30秒の境界、接続 generation 切替、idle cleanup の注入 clock、大量ユニークユーザーの容量上限と分割 cleanup、古い expiry が新しい時刻を削除しないことをテストした。`CARGO_TARGET_DIR=/tmp/rice-issue-65-cargo-target cargo test --locked --no-default-features` は115件、default feature は161件、`cargo clippy --locked --all-targets -- -D warnings` は成功した。

## 2026-09-20 Issue #172: RustSec 修正版の依存更新

- cargo-audit 0.22.2 と RustSec DB `d5c17953a895cf19e8d3ce66eaa42b6fcfe1fb16` で既存 lockfile の advisory を確認した。quinn-proto 0.11.15、rustls 0.23.45、anyhow 1.0.103、event-listener 5.4.2 と、plist 1.10.1 経由の quick-xml 0.42.0 へ互換更新した。rustls-webpki 0.103.15 と base64 0.23.1 はこれらに必要な推移的更新である。
- `cargo-audit audit --file src-tauri/Cargo.lock --json --no-fetch --no-yanked --deny warnings` の JSON で RUSTSEC-2026-0185 / 0190 / 0194 / 0195 / 0221 / 0285 の消失を確認した。vulnerabilities は0件、残存 warning は7件で strict gate は失敗する。残存する glib の unsound と保守終了の警告は #173 で追跡し、waiver は追加していない。最初の online audit は yanked の registry 確認が timeout したため、今回の照合は取得済み DB の advisory 検証に限定し、全項目の clean audit とは主張しない。
- Rust 1.89.0 の `cargo test --locked` は default 156件/no-default 110件、Windows GNU target の `cargo check --locked --all-targets` が成功した。現在の CI toolchain の `cargo clippy --locked --all-targets -- -D warnings`、frontend203件、typecheck、build、diff check も成功。1.89 の clippy が検出した既存 source の4指摘は toolchain 固定を行う #93 で扱い、この変更には含めない。Windows 実機の動作確認は未実施。

## 2026-09-20 Issue #58: dispatcher barrier の予約・反映順序

- キューワーカーは shared dispatcher を取得してから control-in-progress を確認し、pending の in-flight 予約と talk packet 書き込みを同じ guard 内で行う。control が先に開始されていれば worker は予約せず、control の local queue 反映後に状態を再確認する。
- pause/resume/skip/clear は packet の write、local queue 反映、成功 status/log を同じ dispatcher guard に収め、wire 順と local 適用・通知順を一致させる。送信失敗は local 未変更・リモート到達不明、送信済み後の local 反映失敗はリモート送信済み・local 未反映として Logs/status に出す。失敗した最後の control barrier は、保留中の processable な項目があれば worker を再開する。
- fake TCP server で遅延 talk 後の pause/skip/clear、実際の `SpeechQueueState` を使う control 先行時に talk 接続を開かないこと、pause/resume の wire と local 適用順を検証した。`cargo test --locked --no-default-features` 全102件、app feature の `cargo test --locked` 全148件、`cargo clippy --locked --all-targets -- -D warnings` を実行して成功した。

## 2026-09-20 Issue #68: Launcher icon extraction の timeout と残留 worker 境界

- 最新 main 上で Rust default 全154件と clippy `--all-targets -- -D warnings` が成功。Windows GNU target の `cargo check --locked --all-targets` も成功した。async command の State と Result の制約は [Tauri 公式資料](https://v2.tauri.app/develop/calling-rust/) で確認した。

- Launcher 追加では settings mutex 内で Launcher 項目の snapshot だけを取得し、path 検証とアイコン抽出は設定 lock 外の `spawn_blocking` worker で実行する。worker pool はアプリ全体で最大4件、permit の取得待ちは6秒、実行開始後の各 worker の呼び出し待ちは7秒とする。最大200件の選択は4並列で複数 batch に分かれるため、追加操作全体に7秒の期限は設けない。
- PowerShell icon extraction は5秒で `kill` し、`wait` による reaping と stdout/stderr pipe の回収が成功した場合だけ終了確認済みとして Logs に残す。kill または wait に失敗した場合は終了したとは表現せず、確認できなかった原因を記録する。
- `std::fs` の UNC 確認など、実行済み blocking worker を安全に強制停止できない操作は残る。呼び出し側は7秒で戻るが、停止した worker は終了まで permit を保持するため、同時に残留できる worker は全 Launcher 追加要求を合算して4件までである。permit が全て残留した場合、次の追加は6秒で混雑エラーを返す。
- worker / timeout / extractor を本番共通関数へ注入し、停止 fake extractor が期限内に返ること、上限並列数、settings lock 非保持、最新 snapshot への merge、実際の停止 child process の kill/reap を Linux の回帰テストで確認する。Windows の PowerShell/COM/UNC 実機確認は TODO に残す。

## 2026-09-20 Issue #67: Rust feature matrix

- モデルと emitter の import 分離は既に main にあり、no-default でも formatter・packet・queue state のテストが実行できる。残っていた CI の両構成検証を専用 workflow へ追加した。
- PR/main と手動実行で default/no-default を別 job にし、`fail-fast: false` で片方の失敗が他方の結果を隠さない。`RUSTFLAGS: -D unused-imports` で不要 import の再導入を拒否する。no-default job は GTK/WebKit をインストールしない。
- `cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features` は102件、`mkdir -p dist` 後の default test は148件成功。不要 import warning はなし。YAML parse と `git diff --check` も成功。Windows の実機操作には変更なし。

## 2026-09-20 Issue #46: command エラーの表示と詳細の分離

- AppShell、起動時認証、Launcher の例外表示を `presentation/errors.ts` へ統一。接続拒否・期限切れ・認証無効・権限不足・対象不存在を説明し、settings/auth/chat/speech/queue/launcher 等の操作別復旧手順を付ける。backend が既に返す日本語の復旧案内は重複させない。
- unknown reject の構造化データと Error stack は Logs 用に保持する。循環参照、空値、throwing getter でもエラー表示自体を壊さない。起動時認証は system Chat と技術ログへ別々に出力する。
- 文字列/Error/object の接続拒否、空値、部分成功の日本語説明、技術ログと通知の分離、循環参照、既知エラー、起動時認証を回帰テスト。frontend196件、typecheck、build、diff check が成功。実機の Windows 操作は未実施。

## 2026-09-20: Issue #48 Tauri bridge の Option / field 名契約

- Rust の struct field にある `Option<T>` は `None` を JSON `null` ではなく field omission として統一した。TypeScript 側は同じ field を `?: T` とし、property 存在確認後に `null` で例外になる穴をなくした。struct 全体を `Option<T>` として返す `settings_take_recovery_notice` と `twitch_get_stored_auth` だけは command result の `null` を維持し、client 層で `undefined` に変換する。
- `TwitchAuthPollResult` は enum variant field にも `rename_all_fields = "camelCase"` を設定し、keyring 保存失敗時の `storageWarning` を Login の既存警告表示へ到達させた。Rust serializer test と TypeScript bridge test は同じ fixture を使い、field omission、camelCase、status message、queue warning / `sourceMessageId`、fragment option、authorized payload を検証する。shell が呼ぶ routing helper も Device Code の保存警告が notification と system Chat の両方へ届くことを確認する。
- frontend は Tauri の generic を runtime validation とみなさず、client 層で主要 payload の required field・enum・任意 field を検証する。その他の command result は再帰的な null 排除だけで shape は検証しないため、新しい利用箇所で shape が必要になれば個別 parser を追加する。最新 main 上で Rust default 156件/no-default 110件、clippy `--all-targets -- -D warnings`、fmt、frontend 203件、typecheck、build、diff check が成功した。

## 2026-09-20: エージェント作業ルールの分離

- `issue-fix-batch` は、サブエージェントの報告形式、一般的な修正の隔離方法、GitHub Issue 固有の選定／レビュー／PR 手順を一つのスキルに混在させていたため、用途別の `rules/` 文書へ分離してスキルを撤去した。
- `AGENTS.md` は全ルールを常時展開せず、サブエージェント利用時は `rules/subagent.md`、修正時は `rules/fixing.md`、GitHub Issue 対応時は `rules/github-issues.md` を読む索引として扱う。
- サブエージェントは途中経過を送らず、調査では事実だけ、ファイル編集では編集ファイル一覧だけを最終報告する。親エージェントが成果物、commit、検証結果を直接確認する。
- 修正は原則として一時 worktree と専用ブランチへ隔離する。「すぐに動作確認したい」という明示指示がある場合だけ通常 workspace の専用ブランチを使う。関連する PR または Issue がある場合は、それらがすべて close されるまで worktree とローカル／リモートブランチを保持し、close 後に削除する。

## 2026-09-11 Issue #83: 設定チャンネルと実接続チャンネルの分離

- 設定の `twitch.channel` は次回接続用として維持し、接続試行ごとの generation と、EventSub購読が成功した broadcaster user ID/login を別の実接続状態として status snapshot/event に保存する。chat eventにもgenerationを付与した。
- backendのstatus replayは古いgenerationを拒否し、frontend store/orchestrationも古いstatus、世代不一致または実接続identityと異なるchatを拒否する。旧backendとの互換用にgenerationなしeventは従来どおり受理する。
- Chat headerとSide Panelは実接続先を優先し、設定だけが変更された場合は「次回接続先」を併記する。Authにも設定反映が次回接続からであることを表示する。
- app無効のRust全95件、app featureを含むWindows GNU targetの`cargo check`、frontend全521件、typecheck、buildが成功した。実WindowsでのEventSub接続確認は継続する。

## 2026-09-11 Issue #56: 棒読みちゃん受付後の再生完了追跡

- 棒読みちゃんTCPの talk は送信成功だけでは発声完了を保証しないため、送信後に `0x130`（残タスク数）と `0x120`（再生中）を照会し、両方が0になるまで当該項目を in-flight / Speaking に保持する。後続 talk は送らないため、アプリが送信したリモート未再生分もローカルの200件上限に含まれる。
- 状態照会は共有 dispatcher をポーリング間で解放し、pause/resume/skip/clear が待ち続けないようにした。受付後に照会不能または5分超過となった項目は、再送による二重発声を避けて自動 retry を消費済みの Error 履歴へ移す。
- 制御コマンドの送信開始からローカルqueue反映までを control-in-progress として記録し、その間に完了pollが返っても反映を待機する。これによりskip/clearが送信済みin-flight項目ではなく次項目へ誤適用される競合を防ぐ。
- fake TCP server が talk 受付後に busy/残1、次に idle/残0を返す回帰テストと、受付後の追跡失敗が自動再送されないqueueテストを追加した。応答が1 byteであることは既存参照実装 `bouyomi4rs` の `send_command_with_response` と `get_remaining_tasks` も確認した。`cargo test --locked --no-default-features` は全94件成功した。

## 2026-09-11 Issue #58: 棒読みちゃん送信の順序保証

- `AppState` が共有する async dispatcher を全 `BouyomiAdapter` へ渡し、talk、テスト読み上げ、接続確認、無音プローブ、pause/resume/skip/clear の接続・送受信全体を直列化した。接続確認の query と任意の確認読み上げも一つの transaction として扱う。
- 巨大な talk の `write_all` を fake TCP server 側で意図的に停滞させ、clear が二本目の接続を開始できず、talk 完了後に clear packet が届く回帰テストを追加した。
- `cargo fmt --all` と `git diff --check` は成功。`libdbus-1-dev` を一時領域へ展開して `cargo test --locked --no-default-features` を実行し、Issue #56追加後の全94件（#58のTCP回帰テストを含む）が成功した。app有効のビルドはローカル環境のGTK/WebKit開発依存不足のため後段CIで確認する。

## 2026-09-09: Issue #42 Draftレビューと状態復元の仕上げ（完了）

- mainのdomain store分割を維持してsnapshot reconciliationをorchestrationへ移した。全listenerの成功後にのみsnapshotを取得し、各streamのrevisionで古い応答と重複を除外する。起動処理と自動接続は復元後に開始し、既存のTwitch接続状態をローカルでdisconnectedへ上書きしない。
- build.rsのapp_events_snapshot ACL登録漏れを修正。snapshotはキャッシュしたqueue payloadとrevisionを組で保持し、clear後の架空の空queue通知を除いた。自動ヘルスプローブは無音を維持し、一時停止・読み上げ中のqueue状態を保持する。
- 起動ログとemit errorに安定したIDを付与し、late subscriberのLogs/system Chatへ復元する。state replayの購読遅延、古いsnapshot、unmount、購読/query失敗を回帰テストした。
- native command bridgeのsnapshot応答も検証。Rust 130件、app無効85件、frontend 176件が成功し、clippy、frontend build、セキュリティ検査、format/diff検査も成功した。Windows実機での外部アプリ連携は既存の手動確認項目として残る。

## 2026-08-29: Issue #42 backend state replay と speech snapshot

- Tauri の setup emit は WebView listener 登録前に実行されるため、backend に bounded operational log/status store と `app_events_snapshot` command を追加する。emit の失敗は stderr と診断 snapshot の双方で観測する。保存済み credential は `/validate` 完了まで `Validating` とし、未検証の `Connected` を送らない。
- speech queue/status は monotonic revision 付きの単一 `SpeechStateSnapshot` として `speech_queue_reload` から取得する。frontend は listener 登録完了 callback 後に snapshot を取得し、並行 event と revision 比較して reload 後の paused queue を保持する。

## 2026-09-09: Issue #43 最終検証

- app feature全126件とclippyが成功。app無効のビルドで既存のqueue status importとcredential storeテストのcfg不足を検出したため修正し、app無効のテストも成功した。本番のTwitch fixture・scope・dedupeテストはapp無効でも維持した。

## 2026-09-08: Issue #43 本番Twitch処理の回帰テスト

- Draftの模擬状態機械は本番のselect!/timeoutを通らず競合を見逃していたため撤去。本番のsession/handover/supervisorに通信とevent sinkを注入し、Tokioの仮想時計で新welcome先行、旧通知、再接続跨ぎdedupe、handshake/welcome失敗25秒維持と2秒backoff、keepalive期限を検証した。Pingでは期限を延長しない。
- OAuth refresh/validateとrotation保存を本番共通境界へ抽出し、再購読前の永続化、logout後の古いvalidate/rotation破棄をテストする。既存mainの遅延credential store、scope不足、部分失敗テストも維持する。
- Twitch公式 https://dev.twitch.tv/docs/eventsub/handling-websocket-events/ を確認し、通知/keepaliveのみが期限を更新する契約と、新welcome前の旧接続維持を反映した。Rust app feature全125件は成功。


## 2026-09-08: Issue #94 単独管理方針でのレビュー完了

- 所有者の明示指示により、下記過去メモのTeam・別承認者・ruleset・environment必須条件を撤回した。公開workflowはmainのtrusted script、同一runのprovenance、tag objectの継続照合を維持し、取得した成果物のchecksum検証を追加した。外部のrepository設定は変更しない。
- repository policy 4件、workflow policy、tag/version/publicationのshell回帰テストが成功。古いGitでも動くようテストfixtureの未作成branch名変更をsymbolic-refへ変更した。
- rulesetがない状態の短いtag照合競合と過去のwrite workflowは所有者を信頼する運用上の制約としてdocs/releasing.mdへ明記した。

## 2026-09-06: Issue #94 保護設定の fail-closed 検証

- 最新 main を PR branch へ merge し、TODO の並行修正を保持した。API 再確認では rulesets 0 件、environments 0 件、main protection は404であり、Issue の運用完了条件は未達。外部設定と担当 identity の選定は変更していない。
- default branch の読み取り専用 job で repository ruleset / environment を検証し、未設定の environment を公開 job が自動作成する経路を停止した。承認後の公開直前にも設定を再検証する。creation の bypass が update/delete も許可しないよう tag ruleset は独立させ、workflow_run の deployment ref は main であるため tag 限定の誤った手順を修正した。
- release-rice の version bump も branch / worktree と PR review / required checks を経由させ、保護された main への直接 push と文書の矛盾を解消した。
- 未設定・無効・除外・API に見える bypass・review/check 不足・self-review・誤った deployment ref、API failure、複数ページの API 読み取りを自動検査する。公開前の remote tag 移動だけでなく create 後 / upload 後の移動と draft / 公開済み Release の再実行も検証する。GitHub 公式 REST schema / endpoint 説明を確認した結果、read-only token には ruleset bypass が非公開となり、environment 管理者 bypass は API schema に存在しない。これらと team/App・承認者の人員/credential 分離は管理者の確認事項として明示し、自動検証成功だけでは運用完了と扱わない。

## 2026-08-26: Issue #94 release tag / publish 権限境界

- 調査時点の repository settings は rulesets 0 件、`main` branch protection なし、environments 0 件だった。workflow だけでは tag update/delete の TOCTOU と、過去の tag commit に残る旧 workflow の write 権限を無効化できないため、`refs/tags/v*` の作成主体・update・delete を制限する active ruleset、review 済み `main`、tag 作成者と分離した required reviewer を持つ `release` environment を管理者の必須手順として残す。外部設定はこの変更では操作していない。
- tag push workflow から `contents: write` を除去し、検証済み tag object / event commit を provenance artifact に固定した。default branch SHA の trusted script を使う `workflow_run` publish job が、current tag object、workflow run commit、checkout `HEAD`、`origin/main` 到達可能性、3 manifest version を再照合し、remote tag を Release 作成・upload・公開の前後で確認する。non-main、event/checkout mismatch、同じ commit 上で annotation だけを変えた moved tag、manifest/tag mismatch、remote moved tag、write 権限の tag workflow 再導入を自動テストした。release script tests、workflow policy / YAML 構文、`pnpm test`（38 files / 154 tests）、`pnpm build`、`git diff --check` は成功した。


## 2026-09-08: PR #159/#166 レビューとCI実行時間

- #159の自動プローブをレビューし、mainを統合した。既存開発コンテナでRust 116件の成功を確認した。#166は文書とignoreのみで、アプリコードへの変更はない。
- 両PRのRust CIは依存ビルド込みの3分上限でcancelledとなった。Rust test/clippyを15分へ変更し、実テスト失敗と区別する。Windowsでの棒読みちゃん・VOICEVOX実機確認は従来どおり残る。

## 2026-09-08: ローカル worktree の整理

- 未追跡の `.issue31-worktree`、`issue33-tmp`、`rice-issue42` は存在しない `/tmp` 配下を参照するリンクだったため削除した。`.issue42` は旧 `/workspaces/rice` を参照していた登録パスを修復し、未コミット変更がないことを確認して `git worktree remove` で削除した。存在しない worktree の登録も prune した。
- GitHub の PR 一覧と fetch 後の `origin/main`（`3ad8d97`）を照合した。Issue #29 の型付き Twitch エラー分類は PR #161、#31 の frontend domain store 分割は PR #162、#33 の認証 credential I/O 分離は PR #165 でマージ済み。#31 は squash 後の `b4ae28f` とローカル branch の tree が一致した。
- Issue #42 の backend state replay snapshot は未マージの PR #164 にあり、ローカル HEAD `54123d6` と PR HEAD が一致した。Issue #43 の決定的 OAuth/EventSub state harness は未マージの PR #163 にあり、ローカル `3739147` に続く `c2aa03e` がリモートへ提出済み。重複 PR は作成せず、修正ブランチは保持した。既存 PR の未完了検証は今回の整理では完了扱いにしない。
- 今後の repo 内 worktree 用に `/.worktrees/` と既存の Issue 番号付きパスをルート限定で ignore した。`git check-ignore` で対象パスが無視され、通常のソースファイルは無視されないこと、`git diff --check` を確認した。アプリコードの変更はない。


## 2026-08-29: Issue #33 認証 credential I/O の mutex 隔離（実装中）

- Twitch の認証状態 mutex は generation、pending、token、profile の短い状態更新だけを担当し、keyring と旧 Linux fallback file の同期 API は `TwitchAuthStore` に注入できる backend として分離する。`load`、`save`、`clear` は `spawn_blocking` 上で実行し、I/O 自体を async runtime の worker から外す。
- save/clear は専用 I/O mutex で直列化し、save は I/O mutex を取得した後に auth generation を再確認する。logout は先に generation を無効化してから clear を待つため、遅延した古い保存が logout 後に資格情報を復活させない設計とする。
- 遅延 fake store のテストを追加し、keyring 保存中の profile 取得と logout 後の stale save 無効化を検証する。cargo test/clippy の結果は実装完了時に追記する。

## 2026-08-26: Issue #148 起動後の自動読み上げ接続プローブ

- VOICEVOX の直接アダプタは未実装で、現行の正式経路は本アプリから棒読みちゃん TCP へ送り、VOICEVOX 連携は棒読みちゃん側へ委ねる。frontend は読み上げ状態の初期値を `disconnected` とし、設定読込後から 5 秒周期で `speech_health_probe` を実行するが、この自動プローブがユーザー向け［接続確認］と同じ「接続成功時に読み上げる」設定を流用していた。そのため下流の VOICEVOX が未起動でも Talk パケットが送られ、棒読みちゃん側の VOICEVOX 接続エラー発話を誘発していた。
- 自動復旧プローブ専用の `health_probe` を追加し、設定にかかわらず無音の再生状態取得 `0x120` だけを送るようにした。ユーザーが明示的に実行する［接続確認］では従来どおり設定に応じた確認読み上げを維持する。ローカル TCP listener で自動プローブの受信 packet が `0x120` の 2 bytes だけであることを回帰テストし、`cargo test --locked`（98件）、`pnpm test`（154件）、`pnpm build`、security check、Rust fmt/clippy が成功した。本アプリ、棒読みちゃん、VOICEVOX の順に起動した Windows 実機でエラー文が発話されないことは手動確認として残る。




## 2026-08-08: Issue #53 Chat 仮想スクロールの prepend アンカー

- Chat は新着を先頭へ追加するため、過去ログを読んでいると同じ `scrollTop` が別の行を指す状態だった。更新直前に最初の可視メッセージ ID とコンテナ先頭からの相対 offset を記録し、prepend 後はその ID へ仮想スクロールして offset を戻す。先頭閲覧時は offset 0 を維持して新着を即時表示する。
- 過去ログ閲覧中の新着は件数ボタンとして重ねて表示し、操作時のみ先頭へ戻す。pure helper の回帰テストで、2件 prepend 後も最初の可視 ID と 8px の相対 offset が保持されることを確認した。`pnpm test -- src/features/chat/scrollAnchor.test.ts` と `pnpm build` は成功。実 Twitch の連続受信中に可変高行をまたぐ操作確認は手動確認として残る。

## 2026-08-08: Issue #7 入力エラーのフィールド関連付け

- 共通の数値入力、Login の Twitch チャンネル、Settings の棒読みちゃんホスト・ポート・声質は、エラー表示があっても対象 input と支援技術上の関係を持っていなかった。共通 `FieldError` で一意なエラー ID と `role="alert"` を統一し、無効な input に `aria-invalid` と `aria-describedby` を付与した。
- ホスト空欄では「棒読みちゃんのホストを入力してください。」を表示し、無効な形式では許容するアドレス形式を示す。保存ボタンが無効な場合も、エラーの要約を `aria-describedby` で関連付けた。server-rendered markup の自動テストでホスト、ポート、声質、Twitch チャンネル、保存不能理由の関連付けを検証する。

## 2026-08-08: Issue #17 配信向けキーボードショートカット

- UI 設計の `Space`、`S`、`Cmd/Ctrl+,` は未配線だったため、document の keydown を集約する `useStreamHotkeys` を追加した。Space は speech status が `paused` の場合だけ resume、それ以外は pause、S は skip、Cmd/Ctrl+, は Settings route へ遷移する。Cmd/Ctrl+K は設計どおり MVP 後の対象として追加していない。
- input、textarea、select、contenteditable、標準 button、IME 変換、キーリピート、および別の修飾キーを使うショートカットを除外する。Windows の Ctrl+, と macOS の Cmd+,、Space/S と抑止条件を TypeScript テストで検証した。`pnpm test`、`pnpm build`、`git diff --check` は成功。実機で日本語 IME 変換中とブラウザ標準 button の操作を確認する。

## 2026-08-08: Issue #14 警告キューの成功通知・重複排除

- `warning.added` が文字列 5 件だけを保持していたため、棒読みちゃんの確認、テスト読み上げ、Twitch 認証・接続の成功通知が対処中の障害を押し出していた。通知を severity/source/correlation を持つ構造化モデルへ変更し、Warnings は warning / error だけを最新 5 件表示する。成功・情報は Logs と system Chat へ残す。
- backend の同一障害は `app://log`、status event、command reject から同文で届くことがある。`correlationId` があればそれを、なければ本文と 5 秒の到着時間を使って重複排除し、経路ごとに severity が異なる場合は error を優先する。reducer テストで severity 別の表示上限と log/event/command の統合重複を検証した。`pnpm test`（36件）と `pnpm build` は成功。Windows の Tauri 実機で、棒読みちゃん接続失敗時に警告が 1 件だけ表示され、成功操作を繰り返しても残ることは手動確認として残る。

## 2026-08-08: Issue #36 Chat の制御可能なライブ通知

- 仮想スクロールされた Chat 行をそのまま live region にすると、再レンダーや起動案内も通知対象になり得る。そこで Chat リストは識別可能な `role="log"` のまま live 通知を停止し、Twitch 由来の新着だけを 500ms 単位で `role="status"` へ「新しいチャットが N 件届きました。」と集約する構成にした。通知領域はフォーカスを移動せず、system の起動案内は対象外にする。
- Settings の Twitch 設定に既定 ON の `liveChatAnnouncements` を追加した。OFF 中に受信したチャットは既読として記録して再有効化後に遡及通知せず、既存 `settings.json` にフィールドがない場合も serde の既定値で ON を維持する。起動済みチャット、連投集約、重複/system チャット、抑制中の既読化と Rust 設定互換を自動テストした。Windows のスクリーンリーダーで連投時の読み上げ密度と ON/OFF の実動作は手動確認として残る。

## 2026-08-08: Issue #16 非同期状態の支援技術への通知

- Side Panel の警告と Status Bar は同じ store を表示するため、両方へ live region を付けると一つの接続イベントを二重に読み上げる。`LiveStatusAnnouncer` を App に一つだけ置き、前回の認証・Twitch 接続・読み上げ状態と最新警告を比較して、状態変化がない再描画では通知しないようにした。
- 通常の状態変化と警告は `role="status"` で控えめに通知する。認証期限切れ/エラー、Twitch 接続エラー/再ログイン要求、読み上げの切断/エラーだけは `role="alert"` を使い、同じ更新に警告も含まれる場合は alert を優先して一度だけ伝える。高頻度のログは通知対象に加えない。
- 自動テストで接続エラーの一度だけの通知、通常状態・警告の polite 通知、同一更新のエラー優先を検証した。`pnpm test`（38件）、`pnpm build`、`git diff --check` は成功。Windows のスクリーンリーダーで実際の再認証・EventSub 切断・棒読みちゃん切断時の読み上げ優先度は手動確認として残る。

## 2026-08-08: Issue #37 Chat 行の Twitch バッジ表示

- EventSub から `ChatMessage.badges` までは正規化済みで、UI 設計も「バッジ簡易表示」を要求していたが、Chat 行は表示名だけを描画していた。`ChatBadges` を表示境界として追加し、代表的な Twitch バッジを短縮ラベル、未知のバッジを set ID の切り詰め表示へ変換した。未知バッジを含め、幅を固定上限と省略表示にしてユーザー列のレイアウトを維持する。
- 各バッジは色に依存しない可視ラベルと `role="img"` の accessible name を持つ。複数の代表バッジ、未知バッジ、バッジなしの静的レンダリングをテストし、`pnpm test`（38件）、`pnpm build`、`git diff --check` が成功した。実 Twitch 受信時の視認性とスクリーンリーダーでの読み上げは手動確認として残る。

## 2026-08-07: Issue #15 通常文字のコントラスト

- `zinc-500` / `zinc-600` を通常文字へ使うと `zinc-900` / `zinc-950` 背景で WCAG 1.4.3 の 4.5:1 基準を満たさないため、説明文、列見出し、時刻、状態補助文字を `zinc-400` に統一した。コントラスト比と低コントラスト文字の再導入を Vitest で検査する。

## 2026-08-07: Issue #54 Logs 描画負荷

- Logs view は新着順と 500 件の保持上限を維持したまま、既存の Chat view と同じ `@tanstack/react-virtual` で表示中と周辺行だけを描画するようにした。列ヘッダーは仮想リストのスクロール要素の外に置き、行 offset と実スクロール位置の基準を一致させる。行 key にはログ ID を使うため、連続追加時も既存行の識別とスクロールコンテナを維持する。
- 日時表示の `Intl.DateTimeFormat("ja-JP", ...)` は presentation モジュールで一度だけ作成して再利用する。500件を整形しても formatter が1回しか生成されない回帰テストと、`pnpm build` を確認した。実 WebView での連続ログ投入時の commit 時間は未計測。

## 2026-08-07: Issue #51 focus indicator

- `outline-none focus:border-sky-400` の入力は、色だけの 1px border 変化に依存していた。共通の `focusIndicatorClass` に `focus-visible` の 2px ring と 2px offset を集約し、マウス操作で ring を表示しないようにした。すべての keyboard-focusable control には CSS fallback を置き、Windows 高コントラストではシステムの `Highlight` 色による 2px outline を使う。
- 共通クラスの keyboard-only ring / offset と forced-colors fallback を Vitest で回帰確認した。Windows の通常表示と高コントラスト表示で Tab 移動を確認する手動項目は残す。

## 2026-08-07: Issue #26 最小幅での Chat 横スクロール

- Tauri の最小幅は 900px だが、Chat の内容には `min-w-[640px]` があり、Activity Bar と Side Panel を除いた 572px の Main View を必ず超過していた。UI 倍率ではシェルも拡大するため、同じ最小幅で Chat に残る幅は 100%/125%/150% で 572/490/408px となる。
- 固定最小幅を削除し、時刻・ユーザーは下限を持つ可変列、本文は残り幅の列へ変更した。Chat のスクロール領域は縦方向だけを許可し、狭幅時は既存の省略表示と2行制限で主要情報を維持する。境界幅の TypeScript テストで3倍率の本文列が正の幅を保つことを確認した。Windows の実機で各倍率の見た目と操作性を確認する必要がある。
## 2026-08-08: Issue #21 自動処理の system Chat timeline

- Chat message を user/system の discriminated union にし、Twitch status event を一箇所で timeline event に変換した。自動接続の開始・失敗、EventSub の接続/切断/再接続/復旧、認証更新・取消、棒読みちゃん probe の再到達を既存の Logs/Warnings とともに Chat view の `system` 行へ残す。keepalive は対象外とし、同じ発信元の直前の状態遷移は抑止する。
- timeline routing・重複抑止・system 表示の presentation・store の型をテストし、`pnpm test`（40件）、`pnpm build`、`git diff --check` が成功した。実 Twitch の EventSub 切断/再接続、認可取消、棒読みちゃん停止後の自動復旧は手動確認として残る。

## 2026-08-08: Issue #52 フィルターで除外された Queue 項目の dismiss

- Queue view は `blocked` を表示対象にしていた一方、削除操作は `queued`/`error` に限られ、読み上げ側の clear は pending だけを消していた。待機中の読み上げをキャンセルする `speech_queue_remove` と、表示履歴を消す `speech_queue_dismiss` / `speech_queue_dismiss_history` を分離した。blocked と error は履歴 dismiss の対象で、読み上げ中の項目は従来どおり削除できない。
- UI は確認付きの「待機中の読み上げをクリア」と「表示履歴をクリア」を別操作として表示する。Rust で queued のキャンセル、error/blocked の個別 dismiss と一括 clear が pending に影響しないことを、React の静的レンダリングで blocked の削除操作と両方の clear の表示を検証した。`cargo test`（59件）、`pnpm test`（36件）、`pnpm build`、`git diff --check` が成功。Twitch と棒読みちゃんを接続した実機での UI 操作確認は残る。
## 2026-08-08: Issue #49 SPA 画面遷移時の title / focus

- route ごとの document title は `Rice - {画面名}` に統一した。履歴操作を含めて title は常に更新し、ユーザー操作で作られる `PUSH` 遷移だけは、新しい Main View の `h1`（`tabIndex={-1}`）へフォーカスを移して画面名とコンテンツ開始位置を通知する。戻る/進むの `POP` と旧 route からのリダイレクトの `REPLACE` ではフォーカスを保持し、ブラウザの履歴復元を壊さない。
- route 別 title と PUSH/POP/REPLACE のフォーカス方針を TypeScript テストで検証した。`pnpm exec tsc --noEmit`、`pnpm test`（13 files / 37 tests）、`pnpm build`、`git diff --check` は成功した。Windows WebView とスクリーンリーダーで Activity Bar 操作後の読み上げ・戻る/進む時のフォーカス保持を手動確認として残す。
## 2026-08-08: Issue #84 棒読みちゃん接続エラーの診断導線

- 現行の正式画面は `src/routes.ts` の Chat / Launcher / Queue / Filter / Settings / Login / Logs であり、`/rules` と `/voices` は redirect 専用だった。一方、棒読みちゃんの接続拒否・timeout は backend の文字列で存在しない Voices 画面を案内していた。
- backend は route 名を含めず［診断］操作だけを案内し、読み上げが `Disconnected` / `Error` のとき Side Panel の行を `appRoutes` の Settings 定義から作るリンクにした。これにより route の改名では backend 文言が陳腐化しない。接続拒否・timeout の復旧文言、Settings 診断リンク、正式画面名と legacy redirect の契約を自動テストで確認する。Windows 側の棒読みちゃん停止・timeout を使った実機確認は残る。

## 2026-08-07: Issue #78 正規化後に空となる読み上げ本文

- `SpeechFormatter` は raw text の空判定だけでは BEL などの制御文字のみのチャットを通してしまい、名前読み上げ OFF では空の talk packet、ON ではユーザー名だけを送る可能性があった。制御文字/空白の正規化、URL・タグ処理、emote 除外の後に本文が空なら、設定にかかわらず `読み上げる本文がありません。` の理由で `Blocked` とする。既存の enqueue 経路はこの理由を Queue history と UI の警告へ渡す。
- 制御文字のみ、改行/タブのみ、emote のみ、通常文字＋制御文字を、ユーザー名読み上げ ON/OFF の両方で検証するテーブルテストを追加した。`CARGO_TARGET_DIR=/tmp/rice-issue-78-cargo-target cargo test` は 56 件すべて成功。実 Twitch + 棒読みちゃんで本文なしチャットが UI の Blocked 履歴として表示され、talk packet が送られないことは手動確認として残る。

## 2026-08-07: Issue #38 Settings / Filter の設定群見出し

- Settings と Filter は区切り線だけで設定群を分けていたため、共有 `SettingsSection` に `section`、関連付けた `h2`、静かな小見出しスタイルを集約した。両画面とも画面名の `h1` の下で同じ階層を使い、見出しナビゲーションから設定群へ移動できる。
- React の静的レンダリングで両画面の見出し一覧を検証し、`pnpm test`（31件）と `pnpm build` が成功した。画面上での密度と読みやすさは次回の手動 UI 確認で確認する。

## 2026-08-07: Issue #50 UI倍率セレクターのアクセシビリティ

- UI倍率は視覚的な背景色のみで現在値を表していた。名前付き `fieldset` 内の native radio group に変更し、各選択肢の選択状態をブラウザ標準の支援技術 API と矢印キー操作へ委譲した。実際に適用中の倍率は `output` として別途公開し、radio の `checked` と視覚的な選択表示は同じ `scaleMode` から生成する。
- server-rendered markup のテストで、グループ名、4つの radio、選択済みの倍率、現在の表示倍率を検証した。`pnpm test`（31件）と `pnpm build` が成功した。Windows のスクリーンリーダーでの実機確認は未実施。

## 2026-08-07: Issue #47 Device Code の期限表示

- Device Code 開始レスポンスが相対的な `expiresIn` だけを返し、Login 画面が初期値を固定表示していた。Rust command で発行時刻から算出した `expiresAtMs`（UNIX epoch milliseconds）を返し、UI はこの絶対期限と現在時刻で残り秒数を更新するようにした。
- 期限到達時はコードと認可 URL を隠し、確認操作を無効化する。標準の button による「認証をやり直す」はキーボード操作でき、新しい Device Code を発行する。fake timer で期限直前と期限到達の境界を検証した。実 Twitch の Device Code を用いた表示・再発行の手動確認は残る。

## 2026-08-07: Issue #41 Logs view の重複 React key

- Rust が送る `app://log` payload には ID がなく、従来の frontend reducer は timestamp・level・message だけから ID を作っていた。同一 payload を連続受信すると ID が重複するため、保持するログは dedupe せず、frontend store が既存 ID と衝突したときだけ `-1` 以降の連番 suffix を付けるようにした。これにより Logs view の key は一意で、同文ログの件数と新着順を維持する。

## 2026-08-07: Issue #45 UI 状態ラベルのローカライズ

- Speech/Queue の enum 値が Side Panel、Status Bar、Queue アイコンの tooltip/accessible name に直接渡されていた。presentation mapping を表示専用の日本語ラベルへ統一し、Queue 行では隣接する可視状態テキストだけを支援技術に公開するため、装飾アイコンを `aria-hidden` にした。全 Speech/Queue 状態を対象に日本語ラベルと色を検証する TypeScript テストを追加し、`pnpm test`（31件）、`pnpm build`、`git diff --check` が成功した。
## 2026-08-07: Issue #80 棒読みちゃん IPv6 接続先

- 従来の `"{host}:{port}"` 連結は `::1:50001` を作り、IPv6のhost/port境界を失っていた。`BouyomiAddress` にhostとportを分離して保持し、TCP接続は `(host, port)` の `ToSocketAddrs` を使うよう統一した。これによりqueue、health、test、control、diagnosticsが同じ検証・接続経路を使う。
- hostはIPv4、DNS名、または角括弧なしのIPv6を受け付け、portを含むhostや角括弧付きIPv6などは設定保存時とadapter構築時に日本語エラーで拒否する。IPv6 zone identifierは今回の初期実装では受け付けない。diagnosticsとStatus BarはIPv6を `[::1]:50001` と曖昧さなく表示する。Rust/TypeScriptのunit testでIPv4・DNS・IPv6・不正値を確認する。

## 2026-08-06: Issue #23 EventSub 再購読時の認証更新

- EventSub 接続 task が開始時点の access token を `EventSubConnectionParams` に保持していたため、Login 画面などで token を更新しても、後続の通常再接続で古い token を使っていた。接続パラメータから認証情報を除き、購読のたびにアプリの認証状態から現在の token を取得するようにした。
- 購読が 401 の時だけ refresh を一度実行し、更新された access token / rotation 後の refresh token を既存の OS credential store 保存経路へ直ちに渡してから再購読する。refresh 失敗または更新後の再試行の 401 は認証状態を解除して Login での再認証を案内する。期限切れ token の再試行、refresh 失敗時に再試行しないこと、rotation が保存対象へ反映されることを Rust の非同期テストで確認した。

## 2026-08-28: 通常 devcontainer の Codex state 永続化

- 通常 profile に `rice-codex-home` named volume を `/home/vscode/.codex` として追加した。これにより devcontainer の Rebuild でも Codex の認証情報、履歴、セッションを Docker 環境内に保持する。`setup.sh` は初回 volume の所有者を実行ユーザーへ変更し、ディレクトリを `0700` にする。
- Docker named volume は Git と Docker build context の外にあり、workspace へ資格情報を保存しない。コンテナや Docker environment 自体を削除した場合は残らないため、その場合は既存の手動 backup/restore 手順を使う。

## 2026-08-05: Issue #97 devcontainer bootstrap と capability 分離

- 通常 devcontainer では host `.ssh`、`.gitconfig`、Codex state volume、Docker socket、`--network=host` が `postCreateCommand` と同居しており、`@openai/codex@latest` を未固定で global install していた。これを、base/Node/Rust image digest、Codex 0.98.0 と pnpm 8.11.0 の tarball SHA-512、Rust 1.89.0 を `.devcontainer/bootstrap-lock.json` に記録する構成へ変更した。
- Codex/pnpm は build stage で integrity を検証し lifecycle script を無効化して導入し、Rust/rustfmt/clippy も固定 Rust image から build 時に導入する。通常 profile の post-create は baked Codex の version 確認と project の `pnpm install --frozen-lockfile` だけにした。
- SSH agent + Codex state、Windows Bouyomi 用 host network、ローカル release 用 Docker socket を別 config に分離した。SSH profile は `${SSH_AUTH_SOCK}` の scoped agent を使い、秘密鍵ディレクトリを bind mount しない。新しい CI は bootstrap 更新時に lock 検証、image rebuild、tool version と `pnpm test` / `pnpm build` / `cargo test` を実行する。

## 2026-08-05

- 複数の GitHub Issue を並列修正する際は、親が重要度・変更範囲・既存 PR を基に選定し、各 Luna へ Issue の解釈を委ねつつ、独立 worktree／branch／commit に隔離する。`issue-fix-batch` スキルでは並列数を wave 単位で制御し、実装担当を `gpt-5.6-terra` に限定して、親のレビュー後に Issue ごとの Draft PR を作る手順と固定プロンプトを定義した。
- Issue #103 として、OS keyring 保存失敗時の Linux 固有の平文 fallback を廃止した。認証は session-only として続行し、再起動後の再ログインを UI 警告で案内する。旧版の `~/.rice/twitch-auth.json` は keyring が利用できる場合だけ移行・削除し、失敗時は読み込まず、削除・Twitch のアクセス取り消し・再ログインを案内する。fake credential store による保存成功/失敗、復旧移行、移行不能、解除、keyring 読込失敗時の復旧案内の自動テストを追加し、`cargo test`（36件）、`pnpm test`（25件）、`pnpm build` が成功した。実 Linux Secret Service と旧ファイルを使う手動確認は残る。
- 一般設定の直接上書きは保存途中の終了で `settings.json` を破損し、Tauri setup の失敗でアプリ全体を起動不能にしていた。保存は同一ディレクトリの一時ファイルを `sync_all` 後に atomic replace する方式へ変更し、Windows は `MoveFileExW` の replace/write-through を使う。保存前の正常版は `settings.json.bak` 1世代だけ保持する。
- 起動時に本体が不正なら破損データを日時付きで退避し、正常な backup から復旧する。backup も不正なら両方を退避して既定値で起動する。復旧通知は backend log に加え、起動後に一度だけ取得する command を通じて system Chat、Logs、警告へ表示する。

## 2026-08-02

- Codex state backup は認証情報・履歴・セッションを含む一方、従来の既定保存先は repository 内で `.dockerignore` の対象外だった。既定保存先を XDG state directory へ移し、Docker context を root Dockerfile の必要 source だけに限定する default-deny allowlist とした。local/CI build は送信前検査を実行し、allowlist、Dockerfile の `COPY` source、credential/state archive、秘密鍵、`.env` の混入を検出する。
- UI 倍率はルートフォントサイズを変更するため、`rem` の Activity Bar 操作部品だけが拡大し、固定 `px` のシェル列・Status Bar 行からはみ出していた。Activity Bar 3rem、Side Panel 17.5rem、Status Bar 1.5rem に統一し、100/125/150% と自動倍率でも親子が同じ比率で拡大する回帰テストを追加した。
- EventSub の重複排除キャッシュは WebSocket セッション内で生成されていたため、通常再接続と `session_reconnect` ハンドオーバーで既知 ID が失われていた。接続ループのライフタイムへ移し、最大 5,000 件・10 分の期限付きキャッシュとして、再接続直後の再配送によるチャット二重表示と二重読み上げを防ぐようにした。

## 2026-07-21

- `v0.2.3` の local / remote tag object には annotation message が正しく保存されていたが、Release 公開ジョブの `actions/checkout@v6.0.0` が peeled commit をローカルのタグ ref へ割り当て、`gh release create --notes-from-tag` がコミットメッセージへフォールバックしていた。build job と同じ tag object の再取得・検証を release job にも追加した。次回のタグリリースで実動作確認が必要。
- `v0.2.2` の Release workflow は Windows installer / portable zip のビルドと artifact upload まで成功し、公開ジョブだけが失敗していた。GitHub Actions 上の `gh 2.92.0` では `gh release create --notes-from-tag` と `--repo` の併用が拒否されるため、作成時だけ対象リポジトリを `GH_REPO` で指定するよう変更した。既存 Release の確認・asset upload・draft 公開は従来どおり明示的な `--repo` を使う。修正を含む注釈タグ `v0.2.3` で workflow run `29834476552` を起動し、build / publish 両ジョブと GitHub Release 公開の成功を確認した。

## 2026-07-20

- ステータスバーのバージョン直書きを廃止し、Rust の Cargo package version と `debug_assertions` から表示を組み立てるようにした。通常のリリースビルドは `Rice X.Y.Z`、それ以外は `Rice X.Y.Z (dev abcdef0)` と表示する。コミットはビルド時に `RICE_GIT_COMMIT`、`GITHUB_SHA`、ローカル Git の順で取得し、取得不能でも `(dev)` は維持する。
- 画面追加前の整理として、約1,200行に集約されていた `MainView.tsx` から Chat / Queue / Filter / Settings / Login / Logs を `src/features` 配下へ分離した。`MainView.tsx` は route と props 配線に限定し、共通設定フォーム部品と既定値も別ファイルへ移した。
- Tauri v2 のファイルDnDは `getCurrentWebview().onDragDropEvent` から絶対パスを取得できる。ファイル選択はWeb標準の `<input type=file>` では絶対パスを保持できないため、公式 Dialog plugin の複数選択を使用する。初期 Launcher は `.exe` / `.lnk` に限定し、Rust側でも存在、ファイル種別、重複を再検証する。
- Launcher項目は `kind`, `target`, `displayName`, `order`, `backgroundColor`, `groupId`, `iconDataUrl` を持たせた。現在はapplicationだけを登録し、website種別は予約として拒否する。これにより後続の色編集、枠付きグループ、pointer sensorによる並べ替え、Webリンク追加を既存項目の置換なしで拡張できる。
- Windowsの関連アイコンは登録時に非表示PowerShellから抽出する。対象パスはスクリプトへ連結せず子プロセス環境変数で渡し、抽出失敗時はUIの汎用アプリアイコンへフォールバックする。Linux上の自動検証は完了したが、Windows実機で日本語や記号を含むパス、`.lnk`、管理者権限要求、一斉起動の部分失敗を確認する必要がある。

## 2026-07-16

- 既存リリースは `vX.Y.Z` タグを起点に、Linux Docker + cargo-xwin で Windows x86_64 の NSIS installer と portable zip を生成し、build/release の2ジョブで GitHub Release を公開していた。これを、タグ annotation message を初期 Release 本文に使い、タグ push 後はエージェントが待機しない方式へ変更した。Release は draft 作成、Assets upload、公開の順とし、再実行時は既存本文を保持して Assets を `--clobber` 更新する。
- `git tag -F` の既定 cleanup では Markdown 見出しがコメントとして除去されるため、リリースタグ作成時は `--cleanup=verbatim` を指定する。

調査や作業中に分かった補足情報を記録するファイルです。日付が新しいものほど上に追記してください。

## 2026-08-26

- Issue #157: ウィンドウの物理ピクセル座標を一般設定の `window.position` として、OS の close request と独自タイトルバー経由の終了操作の両方で原子的に保存する。起動時は、現在のモニター作業領域に少なくとも 64 x 32px が残る位置だけを復元し、モニターの取り外し・再配置で画面外になる保存座標は初期の中央配置へ安全にフォールバックする。旧設定に `window` がない場合は既定値（未保存位置）として互換読み込みする。

## 2026-08-15

- Issue #73: Tauri v2 の `csp` は production HTML へ注入され、bundled script/style の hash / nonce は build 時に Tauri が補う。`devCsp` が `null` または未指定の場合は production `csp` へ fallback するため、Vite HMR を production policy へ許可せず、development policy だけに `ws://localhost:1420` と Vite style injection を明示した。renderer は外部 API へ直接接続しないので、production の `connect-src` は公式 IPC source の `ipc:` / `http://ipc.localhost` だけでよい。動的 style 属性は仮想スクロール、倍率、tile 色に必要なため `style-src-attr 'unsafe-inline'` に限定して残した。
- capability directory の全ファイルは設定未指定時に自動有効化されるため、`tauri.conf.json` から `default` だけを明示した。`core:default`、event/window の default set は未使用 command を含むので個別 permission へ縮小した。custom command は app manifest がない local renderer では暗黙許可されるため、`tauri_build::AppManifest` で invoke handler と同じ command 一覧を ACL 化し、main capability へ明示した。
- Launcher icon は保存済み JSON を直接 deserialize する経路でも remote URL が表示され得た。`data:image/png;base64,`、encoded/decoded 上限、base64 decode、PNG chunk/checksum/IEND、単一 frame、最大 512 x 512 px を deserialize と更新の両方で検証し、違反値は `None` に落として汎用アイコンへ戻す。CSP の `img-src 'self' data:` と合わせ、remote image request、SVG active content、署名だけを模した壊れた PNG を許可しない。

## 2026-08-12

- Issue #1 の確認で、Logs は接続・認証・読み上げ障害から復旧するための正式画面である一方、2026-05-26 に Activity Bar の導線だけが削除されており、通常操作で到達不能になっていた。`docs/05-ui-ux.md` の全正式画面を並べる方針に合わせ、Activity Bar に Logs を戻した。`AGENTS.md` の「Logs を除く」はこの旧判断を残した不整合だったため、通常操作から 1 回で Logs へ到達できる方針に訂正した。

## 2026-08-07

- StatusBar は version literal を持たず、`app_build_info` が `CARGO_PKG_VERSION` を返して動的に表示する。Issue #89 では release-rice とリリース手順の旧 4 箇所更新を 3 manifest に訂正し、`scripts/verify-release-version.sh` で 3 manifest と release tag を照合するようにした。version bump 時は同 script の `--changed-from` で変更対象が 3 manifest だけであることも検証する。

## 2026-07-14

- Queue view は読み上げ済み・手動スキップ済みの履歴を除外し、待機中・読み上げ中・エラー・フィルター設定による除外だけを表示する役割に整理した。キュー ID の連番で降順に並べ、Chat view と同じく新しい項目が上になる表示方向へ統一した。
- リリース時のバージョン更新対象は `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` に加え、ステータスバーのアプリ内表示にも存在する。表示が `0.1.0` のまま、マニフェストが `0.1.1` になっていたため現在値を揃え、`release-rice` スキルで4箇所を同じバージョン commit に含めて旧表示の残存を検査する手順にした。

## 2026-07-11

- Login 画面の Twitch 有効性確認は非同期処理中にボタンを無効化し、スピナーと「確認中...」を表示するようにした。確認成功後は従来の通知一覧に加え、認証設定内にも成功メッセージを表示する。
- Twitch 認証の有効性確認で access token の検証と refresh の両方に失敗した場合、EventSub 接続を停止し、メモリ上および保存済みの認証情報を削除するようにした。Login 画面も未認証状態へ戻し、古いプロフィールを残さない。
- Login 画面の認証アクションを状態連動に整理し、未認証時は認証開始、認証済みでは認証解除を同じ位置に表示するようにした。Device Code Flow のポーリング停止は、Twitch が待機状態を OAuth の `error` ではなく `message: "authorization_pending"` で返すのに、実装が `error` だけを判定していたことが原因だった。両形式を判定して待機ポーリングを継続するよう修正し、応答形式の回帰テストを追加した。

## 2026-05-26

- 左ペイン整理として、Side Panel 末尾の「テスト読み上げ」を削除し、操作場所を Settings 画面へ一本化した。Activity Bar から Logs ナビゲーションも削除し、ステータスバーの警告/状態表示と Logs view `/logs` は維持した。
- UI、Rust の通知/ログ、設計文書、TODO の日本語表記を、配信者向けに一般的な「読み上げ」へ統一。内部 API 名の `speech` はコード境界として維持。

## 2026-05-24

- Twitch 公式用語に合わせ、UI のチャット受信/停止/キュー/ログ説明、Rust 側の日本語ログ、設計ドキュメント/TODO の「コメント」表記を「チャット」へ統一した。型名や EventSub の `channel.chat.message` 境界は既存実装のまま維持。
- UI 整理として Settings route を `/auth` / Auth 表示へ改名し、Activity Bar アイコンを認証用に変更。Auth 画面は Twitch 認証、チャンネル、起動時自動接続だけを扱う。読み上げ基本設定の自動読み上げ/ユーザー名読み上げ/emote 読み上げは Voices へ集約し、Rules は NG/URL/長文の規則に絞った。`pnpm test`、`pnpm build`、`CARGO_TARGET_DIR=/workspaces/Rice-xwitch-comment-viewer/src-tauri/target pnpm tauri build --bundles deb` は成功。通常の `pnpm tauri build` は AppImage bundling が読み取り専用 FS で失敗するため、この devcontainer では bundle 対象指定が必要。
- GitHub Actions の Windows リリースビルド失敗を確認。`v0.0.3` は `RICE_TWITCH_CLIENT_ID` 未設定で `test -n` が即失敗、`v0.0.2` は Tauri の Windows リソース生成で `src-tauri/icons/icon.ico` がなく失敗していた。workflow は Client ID 未設定を警告に変更し、ビルド自体は継続するようにした。Twitch ログインは従来どおりビルド時 Client ID がない場合に UI へ設定エラーを出す。`icon.png` から Windows 用 `icon.ico` を追加し、`tauri.conf.json` の bundle icon に明示した。

## 2026-05-23

- devcontainer が重い原因を調査。メモリ/ディスク容量の枯渇はなく、主因候補は `src-tauri/target` が 7.0GB まで肥大化していること、特に `debug/deps` 4.3GB、`debug/incremental` 883MB、`release/deps` 1.2GB。`postCreateCommand` は `npm install -g @openai/codex@latest` と `pnpm install --frozen-lockfile` を毎回走らせるため、Rebuild/作成時の待ち時間要因になり得る。VS Code 拡張は rust-analyzer/Tailwind/Error Lens があり、watcher exclude は設定済みだが Cargo/Rust 側の target I/O とは別。改善候補は Cargo target をワークスペース外の volume/tmpfs へ逃がす、`setup.sh` を冪等化して Codex CLI の再インストールを避ける、不要時は release 成果物を削除する、rust-analyzer の実行条件をさらに絞ること。
- devcontainer 軽量化を実施。`CARGO_TARGET_DIR=/home/vscode/.cargo-target/rice` と named volume `rice-cargo-target` を追加し、Cargo の重い build artifacts をワークスペース外へ移した。rust-analyzer は専用 target dir を使う設定にした。`setup.sh` は `CODEX_NPM_PACKAGE` 未指定かつ `codex` 既存時に npm global install を省略し、`pnpm install` は `--prefer-offline` を付けた。廃止済み desktop-lite の 6080/5901 port forwarding も削除した。既存の `src-tauri/target` は自動削除していない。
- devcontainer rebuild 後に Codex の認証情報と履歴が消える問題を防ぐため、`rice-codex-home` named volume を `/home/vscode/.codex` にマウントするようにした。`setup.sh` で所有者と `700` 権限を整える。
- Codex 状態永続化のために作成する Docker volume 名が分かるよう、`.devcontainer/README.md` に `docker volume create rice-codex-home` を明記した。
- devcontainer rebuild 前後で Codex 状態を手動退避/復元できるよう、書き捨ての `.devcontainer/codex-state-transfer.sh` を追加した。バックアップ zip は git 管理外の `.codex-state-backup/codex-state-backup.zip` に置く。
- Settings 表示時に `TypeError: undefined is not an object (evaluating 's.trim')` が出る問題を修正。Tauri client 層で取得/更新後の設定を既定値とマージし、Settings / Voices のフォーム初期値も部分的な設定オブジェクトで欠けた項目を既定値で補完するようにした。`pnpm build` は成功。
- `ViewId` と `AppState.activeView` による独自ビュー切り替えを廃止し、`react-router-dom` の `HashRouter` / `NavLink` / `Routes` へ移行した。Tauri の file/custom protocol 配信でも直接パス再読込に依存しないよう hash routing を採用。未実装の Queue / Rules / Logs route は専用の仮ページを表示し、画面遷移したことが分かる状態にした。
- アプリ内の日本語が豆腐表示になる問題を調査。devcontainer に日本語フォントが入っておらず、WebKit/WSLg で CJK fallback が成立しない状態だった。Tailwind の `fontFamily.sans` / `fontFamily.mono` に Windows 標準の日本語フォントと Noto CJK 系 fallback を追加し、devcontainer には `fonts-noto-cjk` を追加した。
- GitHub Actions の Windows リリースビルドで `RICE_TWITCH_CLIENT_ID` を repository variable または secret から Docker build arg として渡すようにした。Client ID は Settings UI と `settings_get` の返却値から外し、OAuth 開始時はビルド時に埋め込まれた内部既定値だけを使う。古い `settings.json` に `clientId` が残っていても serde の未知フィールドとして無視される。
- Windows リリース用に Linux Docker + `cargo-xwin` + NSIS の Dockerfile と、タグ `v[0-9]*` push でビルド/リリースする GitHub Actions workflow を追加。Tauri 公式では Windows 上の `tauri build` が本筋で、Linux/macOS からの Windows クロスビルドは NSIS 限定かつ caveat ありのため、workflow は `--bundles nsis` に固定した。Actions は build job と release job を分離し、build job は `contents: read` のみ、release job のみ `contents: write`。キャッシュ poisoning 回避のため `actions/cache` と Docker GHA cache は使わず、`docker build --pull --no-cache` と短期 artifact 受け渡しにした。
- `cargo-xwin 0.22.0` の MSRV が Rust 1.89 だったため、Dockerfile の Rust image を `rust:1.89.0-bookworm` に更新した。

## 2026-05-22

- Phase 1 実装確認として `cargo test` と `pnpm build` を実行し、どちらも成功。棒読みちゃん実機でのテスト読み上げ、未起動、ポート競合、アプリ連携 OFF の手動確認は未実施。
- Phase 3 の初期実装として `tokio-tungstenite` による EventSub WebSocket 接続、Welcome 後の `channel.chat.message` 購読、keepalive 欠落/reconnect/revocation 処理、`event.message_id` fallback の重複排除、`twitch://chat-message` のフロントエンド購読を追加。`cargo test` と `pnpm build` は成功。実 Twitch チャンネルでの受信確認は未実施。
- Side Panel のキュー上へチャット受信の開始/停止ボタンを追加し、`twitch_stop_chat` で認証解除せずに EventSub 接続だけ停止できるようにした。UI store では Twitch 認証状態とチャット受信接続状態を分離。`cargo test` と `pnpm build` は成功。

## 状態メモ

- Git 作業ツリーは調査開始時点で clean。
- `src-tauri/target` と `dist` がローカルに存在するため、ビルド済み成果物はある。
- `src/components/MainView.tsx` の Chat view は EventSub 由来のチャット表示に接続済み。未受信時のみサンプルメッセージを表示する。
- `src/stores/appStore.ts` は `twitch://status` / `twitch://chat-message` / `speech://status` の購読反映を実装済み。実キュー連携は Phase 4 で実装する。
- `src/tauri/client.ts` で `app://log`, `twitch://status`, `twitch://chat-message`, `speech://status`, `speech://queue-updated` を購読できる。
- `src-tauri/src/app_events/mod.rs` にイベント payload と `tauri::Emitter` helper を実装し、設定/認証/棒読みちゃん操作から発火する。
- `src-tauri/src/twitch/mod.rs` は認証、Helix ユーザー解決、EventSub WebSocket 接続、Helix subscription 作成まで実装済み。

## 2026-09-10: Issue #90

公開済み Release は download と全ファイル比較だけを行い、upload/edit しない。draft は checksum と upload 後の完全一致を公開条件とする。mock gh の失敗注入テスト、bash 構文検査、workflow policy 検査が成功。セルフレビューで余分な remote asset も拒否することを確認。外部管理者の同時操作との API 間競合は既存の運用制約として残る。

## 2026-09-10: Issue #87

Client ID 共通 gate と EXE のバイト列検証を追加。未設定、空白、不正形式、正常値、異なる埋め込み、EXE 不在とログ非露出を自動テスト。Docker context 検査と workflow policy が成功。セルフレビューで直接 Docker build の迂回を塞ぎ、実際に COPY しない devcontainer bootstrap 例外を context から除去した。実際の Windows build／Twitch ログインは未実施。

## 2026-09-10: Issue #59

TCP 到達性だけで成功とせず 0x120 の boolean 応答を期限付きで検証する。接続確認音声も検証後に送信。公式 ReadMe (https://chi.usamimi.info/Program/Application/BouyomiChan/ReadMe.txt) のアプリ連携説明と既存設計参照の bouyomi4rs プロトコル資料を照合し、外部コードはコピーしていない。fake TCP server で正常0/1、不正値、HTTP、EOF、無応答、接続拒否、検証失敗時の音声未送信を検証。Rust 全133件、clippy -D warnings、fmt、diff check が成功。セルフレビューで診断と health が同じ判定を使い、TCP 成功のみでは接続成功にならないことを確認。Windows 実機の棒読みちゃん確認は未実施。

## 2026-09-10: Issue #55

送信開始時に pending から in-flight へ移し、完了はそのIDへ適用する。取消後も worker 所有権を維持し、遅延した結果は新項目へ適用しない。overflow は pending のみを落とし、snapshot は in-flight を含む。取消済み送信の失敗はログへ記録し、現キューのエラー状態を上書きしない。Rust 全137件、frontend 全176件、clippy -D warnings、fmt、diff check が成功。セルフレビューでは pause/再試行待ち中の取消、snapshot の件数、既存retry budgetとの整合を確認した。TCP制御の順序保証は #58、受付と発声完了の区別は #56 に残る。
