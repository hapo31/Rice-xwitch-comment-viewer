# リリースの固定入力

`build/release-inputs.json` が Windows release の Rust / Node.js image digest、compiler version、Debian snapshot、pnpm、cargo-xwin、target の正本です。Dockerfile の値との不一致は policy check が拒否します。release の事前テストも同じ manifest の Node.js / Rust version を使用します。

Debian の署名・package hash 検証は維持します。過去 snapshot の `Valid-Until` だけを無効化し、通常 mirror へ fallback しません。取得障害は build failure として扱います。snapshot は実行日の最新版を選ばず、manifest を変更する通常のレビューで更新します。

ローカル wrapper と CI は exact commit SHA とその commit 時刻を `RICE_GIT_COMMIT` / `SOURCE_DATE_EPOCH` として渡します。ZIP は UTC の commit 時刻と `zip -X` により付随 metadata を正規化します。`BUILD-MATERIALS.json` に source、image digest、snapshot、lockfile hash、インストール済みOS package version、tool version、Windows SDK/CRT cache の全file hash、EXE/ZIP hashを記録し、checksum 対象のRelease assetとして同梱します。

固定入力は byte 単位の再現性の証明ではありません。cargo-xwin の MSVC SDK/CRT feed 選択と NSIS/PE linker metadata は残る非決定要因です。前者は実際に取得したcacheのSHA-256 inventory、後者は使用tool versionとmanifest内の明示記録で追跡します。完全な再現性が必要なら、同一materialによる2回の独立buildとEXE/installer差分調査を行います。

更新時はmanifest、Dockerfile、必要ならbootstrap policyを同じ変更としてレビューし、`node --test scripts/test-release-build-inputs.mjs`、Docker context policy、実際のtools stage buildを確認します。通常のdependency PRとして扱い、tagだけを動かしてcompilerを更新しません。機能テスト用CIのhost OS自体はrelease binaryを生成しません。

## Windows候補と公開gate

NSISは3.11のnative compiler source、同版のWindows headers/stubs/plugins ZIP、その2つの取得URL・SHA-256・compiler build時刻をmanifestへ固定します。Debian snapshot/Rust/Nodeの固定は維持します。buildは取得後にdigestを検証し、公開版の内部version情報を付けてcompilerだけを構築、必要なRestartManager/MUI2/System pluginで小さなinstallerを生成してからRice本体へ進みます。Docker runtimeはcompilerの版とheaderを照合し、BUILD-MATERIALSにはnative compiler自体のSHA-256も残します。このtools probeは配布installerの実Windows検証の代わりではありません。

`package.json` と `pnpm-lock.yaml` の Tauri API/CLI はレビュー済みのexact versionに固定し、Rust lockのTauri coreとmajor/minorが一致する必要があります。`verify-tauri-versions.mjs` は通常frontend buildとDocker buildのinstalled graphも検査します。compiler policyの更新やTauri自身のversion checkを無効化する代わりにはしません。

`Release Windows` の手動dispatchは選択したcommitを同じDocker経路でbuildし、tagやGitHub Releaseを作らずに候補を検証します。公開workflowは成功したtag push eventだけを対象とし、dispatch結果を公開へ換算しません。

`ARTIFACT-MANIFEST.json` は3つのversion manifestとtag（候補はnull）、source commit、reviewed target、exactなinstaller/portable/support file集合、各file hash・容量、portable内のflatなrice.exe/LICENSEのCRC・hashを記録します。検査はZIPのlocal/central一致、CRC、bounded inflate、PE GUI/x64、元LICENSE、build材料とlock digest、SHA256SUMSの完全一致を要求します。

LICENSE、npm/Cargo lockfile、Cargo metadataは`.gitattributes`でLF checkoutを指定し、Git for Windowsのcore.autocrlf=trueでもcanonicalな正本bytesを保持します。CIのclone正例/負例で属性の効果を確認します。受け取ったartifactを正規化してdigestを合わせたり、license/lock比較を省略したりはしません。

同じrunのartifactをfreshなGitHub-hosted Windowsへ渡し、portable起動、隔離NSIS silent install、exactな期待exe・offline LICENSE・version/registry、installed起動、silent uninstallを検証します。固定Tauri CLI2.12.1はNSIS格納時に唯一の`__TAURI_BUNDLE_TYPE_VAR_UNK`変数を`__TAURI_BUNDLE_TYPE_VAR_NSS`へ置換し、bundle後に元exeを復元します。その3byteだけを反映したNSIS期待exeの容量・SHA-256をmanifestへ記録し、installed hashはこの期待値、portable hashは元bytesと完全一致させます。印の欠落/重複や他形式を拒否し、任意のbyte差分は許容しません。各appは実main windowを表示して5秒以上異常終了せず、通常closeでexit0になる必要があります。smoke scriptは既存Rice profile/installationやself-hosted環境を拒否し、終了処理で対象PIDと新規GUID scratch/今回作成したprofileだけを扱います。

trusted publisherは同runのmanifest hashを持つreceiptだけでなく、GitHub APIでWindows全体/明示native/実installer smokeの各job・stepがsuccessであることも確認します。失敗・skip・欠落・古いtagにgateがない場合はRelease変更前に停止します。第三者ライセンス通知の整備（#102）はこの実動作gateとは別の未完了事項です。
