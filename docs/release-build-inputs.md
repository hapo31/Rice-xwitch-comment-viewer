# リリースの固定入力

`build/release-inputs.json` が Windows release の Rust / Node.js image digest、compiler version、Debian snapshot、pnpm、cargo-xwin、target の正本です。Dockerfile の値との不一致は policy check が拒否します。release の事前テストも同じ manifest の Node.js / Rust version を使用します。

Debian の署名・package hash 検証は維持します。過去 snapshot の `Valid-Until` だけを無効化し、通常 mirror へ fallback しません。取得障害は build failure として扱います。snapshot は実行日の最新版を選ばず、manifest を変更する通常のレビューで更新します。

ローカル wrapper と CI は exact commit SHA とその commit 時刻を `RICE_GIT_COMMIT` / `SOURCE_DATE_EPOCH` として渡します。ZIP は UTC の commit 時刻と `zip -X` により付随 metadata を正規化します。`BUILD-MATERIALS.json` に source、image digest、snapshot、lockfile hash、インストール済みOS package version、tool version、Windows SDK/CRT cache の全file hash、EXE/ZIP hashを記録し、checksum 対象のRelease assetとして同梱します。

固定入力は byte 単位の再現性の証明ではありません。cargo-xwin の MSVC SDK/CRT feed 選択と NSIS/PE linker metadata は残る非決定要因です。前者は実際に取得したcacheのSHA-256 inventory、後者は使用tool versionとmanifest内の明示記録で追跡します。完全な再現性が必要なら、同一materialによる2回の独立buildとEXE/installer差分調査を行います。

更新時はmanifest、Dockerfile、必要ならbootstrap policyを同じ変更としてレビューし、`node --test scripts/test-release-build-inputs.mjs`、Docker context policy、実際のtools stage buildを確認します。通常のdependency PRとして扱い、tagだけを動かしてcompilerを更新しません。機能テスト用CIのhost OS自体はrelease binaryを生成しません。
