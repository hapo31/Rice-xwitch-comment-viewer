# 調査メモ

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
