# 依存関係監査と release SBOM

PR、main push、毎週月曜日03:17 UTC、手動実行で `dependency-audit.yml` がlockfileを監査する。release buildも同じ `scripts/dependency-audit.mjs` を必須とする。Cargo scannerはcargo-audit 0.22.2、npm scannerはpnpm 8.11.0に固定し、更新はpolicyとworkflowを同じreviewで変更する。RustSec databaseは新規advisoryを検出するため実行時の最新databaseを取得し、取得commit・advisory件数を生reportへ保存する。固定すると新規advisoryを検出できないためdatabase自体は固定しない。

Cargoは脆弱性、unmaintained、unsound、yankedをすべてblockingとし、npmはhigh/criticalをblocking、低いseverityもreportへ残す。OS/SDKのscannerは導入していないため、OS inventoryがあることを脆弱性検査済みとは扱わない。scanner取得・実行・database/registry取得失敗、不明JSON、空databaseは失敗する。scanner outageは「問題なし」ではない。

例外は `security/advisory-exceptions.json` のscope、advisory ID、具体的な到達性/残存リスクの根拠、owner、`expiresOn`（UTC日付・その日まで）を必須とする。期限切れ、不明scope、重複、`.cargo/audit.toml`の無審査ignoreは失敗する。Cargo scannerはrepository外で実行し、ignore前のすべてのfindingを保存する。例外は解決ではなく期限付きの残存リスク受容である。reportは30日保持する。

Dependabotはnpm/Cargo/GitHub Actions/Dockerを毎週確認し、更新PRのlockfileと通常テストをreviewする。Docker digest/toolchain更新は `build/release-inputs.json` とDockerfileの両方を揃える。botの変更を自動mergeしない。

2026-10-05: braces の High finding（GHSA-vfj7-8cjw-p6xm）は、Tailwind 3 の build path を公式 Tailwind 4.3.3/Vite plugin へ移行して依存グラフから除去した。[source af59b8c の監査](https://github.com/hapo31/Rice-xwitch-comment-viewer/actions/runs/37250928344)は例外/閾値を変えずに成功し、npm High/Critical 0・Moderate8件、Rust の期限付き例外2件を報告した。全 advisory が解消した、配布物の第三者通知や Windows packaged smoke まで完了したという意味ではない。新規変更は作業ブランチで品質・監査・native Windows を確認してから main へ反映する。

## SBOMの対象と制約

各新規Windows releaseに `Rice.sbom.cdx.json` を添付する。形式は [CycloneDX 1.5](https://cyclonedx.org/docs/1.5/json/)。generator `rice-sbom` のversionをmetadataへ記録する。exact source commit、2つのlockfile digest、installer/portable ZIP digestをbuild materialと照合し、ずれた場合はreleaseを止める。公開workflowは従来の全asset checksum検証を維持する。

- npm: frozen installされたLinux cross-build環境のdirect/transitive依存。frontend runtimeはrequired、dev/buildのみはexcluded。未導入の他OS用optional packageは対象外で、lockfile全体のdigestも保存する。
- Cargo:Windows MSVC/default featureのresolved graphとmetadata inventory。Windows runtimeをrequired、build/dev/他targetのみをexcludedにする。registry crateはCargo.lockのchecksumを付ける。
- build:imageのimmutable digest、Debian実インストールpackage/version、Rust/Node/cargo-xwin/NSIS等tool version、取得されたWindows SDK/CRTのfile SHA-256 inventory。
- EXE/ZIP:buildが記録したasset digestと実際の配布物を再照合してfile componentへ記録する。

scope excludedは配布実行時のcomponentでないことを表す。静的linkやbundlingで最適化されたbyteの網羅的binary解析を意味しない。利用PCのWindows/WebView2、棒読みちゃん、外部音声合成ソフト、runtimeで取得するTwitchコンテンツはこのbuild SBOMの対象外。license/NOTICEの全文同梱は別のlicense policyで扱う。過去公開済みreleaseは後から書き換えない。

## 新規advisoryの影響検索

対象releaseのchecksumを検証後、SBOMを取得して `jq '.components[] | select(.name == "crate-or-package-name") | {name, version, purl, scope, properties}' Rice.sbom.cdx.json` で検索する。nameだけでなくpurlのecosystem、version、runtime/build scope、source commitを確認してadvisoryのaffected rangeと比較する。該当したらsource commitのlockfileと到達経路をreviewし、修正または期限付き例外を判断する。古いreleaseでSBOMがない場合はtag commitのlockfileを調査し、SBOM不存在を影響なしとみなさない。
