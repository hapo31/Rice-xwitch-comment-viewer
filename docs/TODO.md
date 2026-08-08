# 実装 TODO

最終調査日: 2026-08-07

この TODO は `docs/06-implementation-roadmap.md` の Phase に沿って、現在の実装状況と次に進める作業を追跡するためのものです。作業を始める前後に該当項目を更新してください。

調査メモは [`docs/RESEARCH_NOTES.md`](./RESEARCH_NOTES.md) に分離し、日付が新しいものほど上に追記してください。

## 現在の進捗サマリ

| Phase | 状態 | メモ |
| --- | --- | --- |
| Phase 0: プロジェクト作成 | 完了 | `app_events` の配信基盤と frontend 購読を接続し、`settings.json` の生成/読込、原子的保存、破損時のbackup/既定値復旧を確認した。Issue #50 で UI 倍率を名前付き radio group にし、現在の選択状態と表示倍率を支援技術へ公開した。 |
| Phase 1: 棒読みちゃん連携 | 実装済み、自動検証済み、手動確認待ち | TCP 読み上げ、制御、接続診断、Settings 画面は実装済み。接続確認は設定に応じて確認読み上げまたは無音の状態取得を行う。`cargo test` と `pnpm build` は成功。実機の棒読みちゃんでの確認が必要。 |
| Phase 2: Twitch 認証 | 実装中 | Device Code Flow、`/validate`、refresh、keyring、session-only 保存失敗処理、旧 Linux 平文ファイルの移行/削除、Login 画面、起動時の保存済み認証の自動検証は実装済み。Device Code の絶対期限に基づく残り時間と期限切れ時の再発行導線も実装済み。Client ID は UI/設定JSONに出さずビルド時既定値を使う。実 Twitch 環境での確認が必要。 |
| Phase 3: EventSub チャット受信 | 実装中 | WebSocket 接続、`channel.chat.message` 購読、正規化、再接続をまたぐ期限付き重複排除、開始/停止 UI、フロントエンド反映、再購読時の最新 access token 取得と 401 時の一度だけの refresh/retry（Issue #23）を実装。実 Twitch 環境での手動確認が必要。 |
| Phase 4: 読み上げキュー統合 | 実装済み、自動検証済み、手動確認待ち | `SpeechFormatter`、FIFO `SpeechQueue`、EventSub チャットから棒読みちゃんへの自動読み上げ、Queue 画面を実装。Issue #57 で失敗済み項目をエラー履歴へ隔離し、明示的な手動再試行のみで retry budget を復元するようにした。Issue #78 で正規化後に本文が空のチャットを理由付きで Blocked にした。`cargo test`、`pnpm test`、`pnpm build` は成功。実 Twitch + 棒読みちゃん環境での統合確認が必要。 |
| Phase 5: 配信運用向け仕上げ | 実装中 | Launcher、dev ビルド識別、設定破損時の復旧通知、設定更新 transaction、重要 Issue の並列修正スキル、ルート README と MIT License を実装。Issue #26 で最小幅 900px の Chat レイアウトを 100/125/150% に対応させた。Settings / Filter の設定群には統一した見出しを追加し、Issue #41 で同一内容の連続ログにも一意な表示 ID を割り当て、Issue #45 で Speech/Queue の内部状態値を日本語表示へ集約し、Queue 状態アイコンを支援技術から隠した。devcontainer bootstrap を固定・build 時検証へ移し、SSH agent/Docker/host network を明示 profile に分離した。Windows 実機確認と詳細な運用エラー整理は継続。 |
| Phase 6: VOICEROID2 実験アダプタ | 未着手 | MVP 後に Windows 専用の実験アダプタとして追加する。 |

## Phase 0: プロジェクト作成

- [x] Tauri + TypeScript + Tailwind の雛形を作る。
- [x] `src-tauri/src` に `twitch`, `speech`, `settings`, `app_events` の境界を作る。
- [x] Activity Bar、Side Panel、Main View、Status Bar の基本レイアウトを作る。
- [x] Activity Bar のビュー切り替えを `react-router-dom` ベースのルーティングへ移行する。
- [x] 未実装 route に Chat view ではなく仮ページを表示する。
- [x] 独自 Title Bar、ウィンドウ操作、リサイズハンドルを作る。
- [x] UI 倍率の自動/手動切替を作る。
- [x] Issue #50: UI 倍率セレクターへグループ名と現在の選択状態を公開し、キーボードで操作可能にする。
- [x] 一般設定を Tauri app data 配下の `settings.json` に保存する。
- [x] `settings.json` を原子的に保存し、破損時に backup または既定値で復旧して退避先を system Chat/Logs/警告へ表示する。
- [x] `app_events` からフロントエンドへ流すイベント設計を実装に接続する。
- [x] Phase 0 完了条件として、Tauri アプリ起動と設定 JSON 読み書きを手動確認する。

## Phase 1: 棒読みちゃん連携

- [x] `SpeechAdapter` trait の境界を作る。
- [x] `BouyomiAdapter` の短命 TCP 接続を実装する。
- [x] 棒読みちゃん読み上げパケットを生成する。
- [x] 一時停止、再開、スキップ、クリアの制御コマンドを実装する。
- [x] 接続確認と接続診断を実装する。
- [x] 棒読みちゃん未起動時の日本語エラーを返す。
- [x] Settings 画面から接続確認、診断、テスト読み上げ、ホスト/ポート/声質設定を操作できるようにする。
- [x] 接続確認で空接続を送らず、「棒読みちゃんと接続しました」の確認読み上げを送る。
- [x] 接続成功時の読み上げ ON/OFF と読み上げ文カスタマイズを設定できるようにする。
- [ ] 実機の棒読みちゃんでテスト読み上げできることを確認する。
- [ ] 棒読みちゃん未起動、ポート競合、アプリ連携 OFF の手動確認を行う。
- [x] 接続失敗時に読み上げキューを破棄しない挙動を Phase 4 で統合確認する。

## Phase 2: Twitch 認証

- [x] Twitch Client ID を `.env` / build env から内部既定値として読み込む。
- [x] Twitch Client ID を Settings UI と設定 JSON の公開項目から外す。
- [x] OAuth Device Code Flow の開始とポーリングを実装する。
- [x] `user:read:chat` スコープでトークンを取得する。
- [x] `/validate` でトークン有効性を確認する。
- [x] access token 検証失敗時に refresh token で更新する。
- [x] refresh 成功時に保存済み refresh token を差し替える。
- [x] OS keyring 優先の OAuth 保存/復元/削除を実装する。
- [x] Issue #103: keyring 保存に失敗しても OAuth token を平文ファイルへ自動保存せず、session-only として継続する。既存の Linux fallback file は keyring 復旧時に移行・削除し、移行できない場合は削除・token revoke・再ログインを案内する。
- [x] 旧版の Linux Secret Service fallback `~/.rice/twitch-auth.json` を検出し、keyring への移行成功時に削除する。新規の fallback file は作成しない。
- [x] Login 画面に認証開始、確認、有効性確認、解除を実装する。
- [x] Login 画面の認証開始/解除を認証状態に応じた単一アクションへ整理する。
- [x] Device Code Flow の待機応答を正しく判定し、自動ポーリングが継続するよう修正する。
- [x] Issue #47: Device Code の絶対期限から残り時間を表示し、期限切れ時は確認を無効化してキーボード操作可能な再発行導線を表示する。
- [x] 有効性確認で認証更新にも失敗した場合は、保存済み情報を含む認証状態を解除する。
- [x] 有効性確認中の Loading 表示と、確認成功時の通知を Login 画面へ追加する。
- [x] 起動時に保存済み認証を `/validate` し、必要なら refresh してから認証済み状態へ遷移する。
- [x] 起動時の認証確認進捗と結果を system チャットへ表示する。
- [ ] 実 Twitch Client ID で Device Code Flow を手動確認する。
- [ ] 認可取り消し、401、期限切れ時の UI 表示を手動確認する。
- [ ] アプリ起動時の保存済み認証復元と refresh 更新を手動確認する。
- [x] Twitch ユーザー ID と接続チャンネルを EventSub 接続へ渡す command を実装する。

## Phase 3: EventSub チャット受信

- [x] `tokio-tungstenite` を導入する。
- [x] `EventSubClient` 相当の接続ループを作り、`wss://eventsub.wss.twitch.tv/ws` へ接続する。
- [x] `session_welcome` 受信後に `channel.chat.message` を購読する。
- [x] EventSub 購読に User Access Token を使う。
- [x] `session_keepalive` 欠落を検出して状態とログへ出す。
- [x] `session_reconnect` を処理する。
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
- [ ] 実 Twitch 環境で `channel.chat.message` 購読と Chat view 表示を手動確認する。

## Phase 4: 読み上げキュー統合

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
- [x] `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] TypeScript の store reducer テストを追加する。

## Phase 5: 配信運用向け仕上げ

- [x] Issue #38: Settings / Filter の設定群へ同一階層・同スタイルの見出しを追加し、見出し一覧のアクセシビリティテストを追加する。
- [x] プロジェクト概要、主な機能、導入方法、ライセンスを案内するルート README と MIT License を追加する。
- [x] Issue #97: devcontainer の bootstrap を固定し、通常開発と SSH/Docker/host network 利用を明示的な profile に分離する。
- [x] Issue #60: 設定更新を候補へ適用・保存成功後に commit するトランザクションに統一し、失敗時にメモリと永続設定を変更しない。
- [x] 画面実装を `features` 単位へ分割し、ルーティング層を画面配線のみに整理する。
- [x] Windows 10 スタートメニュー風の Launcher 画面を追加する。
- [x] Launcher でアプリの選択/DnD登録、削除、単体起動、一斉起動を実装する。
- [x] Launcher の登録内容を永続化し、将来の色変更・グループ・並べ替え・Webリンクに拡張できるモデルにする。
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
- [x] リリース作業スキルでマニフェストとステータスバーの表示バージョンを同時に更新・検証する。
- [x] `v0.1.2` パッチリリース向けにバージョンを同期し、TypeScript/Rust/Windows Docker ビルドを検証する。
- [x] Launcher と dev ビルド識別を含む `v0.2.0` リリース向けにバージョンを同期し、自動検証する。
- [x] リリースビルド以外のステータスバーへ dev 表示とコミットハッシュを追加する。
- [x] 注釈付きタグの本文から GitHub Release を非同期・冪等に公開するフローへ移行する。
- [x] `actions/checkout` がタグ event の注釈付きタグを軽量タグへ置き換える場合に、検証前にリモートのタグ object を復元する。
- [x] リポジトリ公開後に `v0.2.1` の失敗 run を再実行し、注釈付きタグの復元と検証が成功することを確認する。
- [x] Release workflow の Rust テスト前に Tauri が必要とする Linux 開発パッケージを導入し、`v0.2.2` として再リリースする。
- [x] `gh release create` の `--notes-from-tag` / `--repo` 非互換を解消し、`v0.2.3` Release workflow の build / publish 成功を確認する。
- [x] Release 公開ジョブでも annotated tag を復元・検証し、`--notes-from-tag` がコミットメッセージへフォールバックしないようにする。
- [x] 重要 Issue を独立 worktree と個別 PR で並列修正する `issue-fix-batch` スキルを追加する。
- [x] Docker build context を default-deny allowlist 化し、Codex state/credential の送信前検査と退避先の workspace 外移動を行う（#98）。
- [x] UI 倍率変更時に Activity Bar、Side Panel、Status Bar が操作部品と同じ比率で拡大するよう、アプリシェル寸法を rem に統一する。
- [x] Issue #26: 最小ウィンドウ幅 900px と 100/125/150% の UI 倍率で、Chat の主要列を横スクロールなしに表示する。
- [x] Logs view を実装する。
- [x] `app://log` event をフロントエンドへ接続する。
- [x] Issue #41: 同一内容の `app://log` event を連続受信しても Logs view の表示 ID を一意にする。
- [x] EventSub、認証、読み上げアダプタのログを Logs view に表示する。
- [x] ステータスバーに Twitch 接続状態、棒読みちゃん状態、キュー件数、警告状態を集約する。
- [x] 起動時自動接続を実装する。
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
- [x] Queue view を読み上げ待ち・エラー・フィルターによる読み飛ばしだけに絞り、Chat view と同じ新着順にする。
- [x] 起動時の仮チャットを設定状態に応じた system 操作案内へ置き換える。
- [ ] 配信中に判断しやすい日本語エラー文言を整理する。
- [x] Issue #45: 内部の Speech/Queue 状態値を日本語の表示文言へ集約し、状態アイコンの重複した支援技術向け読み上げをなくす。
- [x] キュー行の状態表示テストを追加する。
- [x] 設定フォームのバリデーションテストを追加する。

## Phase 6: VOICEROID2 実験アダプタ

- [ ] MVP 完了後に着手可否を再判断する。
- [ ] Windows 専用 feature として隔離する方針を維持する。
- [ ] C# sidecar の PoC を作る。
- [ ] Rust から stdio JSON-RPC で `speak`, `stop`, `health` を呼ぶ。
- [ ] VOICEROID2 のバージョン、bitness、起動状態を診断する。
- [ ] 失敗時に棒読みちゃんアダプタへ戻せる UI を作る。

## テストと確認

- [x] Rust: 棒読みちゃんパケット生成テストを追加する。
- [x] Rust: 棒読みちゃん制御パケットテストを追加する。
- [x] Rust: 外部 URL 許可リストのテストを追加する。
- [x] Rust: `channel.chat.message` JSON fixture のパーステストを追加する。
- [x] Rust: `SpeechFormatter` の NG/URL/長文処理テストを追加する。
- [x] Rust: Issue #57 の初回失敗、再試行成功/失敗、新着 enqueue/通常再開、手動復旧の状態遷移テストを追加する。
- [x] Rust: WebSocket 再接続状態遷移テストを追加する。
- [x] Rust: 通常再接続と reconnect ハンドオーバーをまたぐ重複排除テストを追加する。
- [x] Rust: Launcher の拡張子、重複、順序、予約種別、旧設定互換テストを追加する。
- [x] Rust: 設定JSONの原子的保存、disk full/replace failure、破損本体/backup復旧テストを追加する。
- [x] TypeScript: store reducer テストを追加する。
- [x] TypeScript: キュー行の状態表示テストを追加する。
- [x] TypeScript: 設定フォームのバリデーションテストを追加する。
- [x] TypeScript: Issue #47 の Device Code 期限境界を fake timer でテストする。
- [x] TypeScript: Launcher の表示順、背景色、DnDパス判定、起動結果表示テストを追加する。
- [x] TypeScript: 100/125/150% と自動倍率でアプリシェル寸法が一貫して拡大するレイアウト回帰テストを追加する。
- [x] TypeScript: Issue #26 の最小幅 900px における Chat 主要列の 100/125/150% レイアウト回帰テストを追加する。
- [ ] 手動: 棒読みちゃん未起動/起動中/ポート競合を確認する。
- [ ] 手動: Twitch トークン期限切れ/認可取り消しを確認する。
- [ ] 手動: 配信中チャット連投を確認する。
- [ ] 手動: ネットワーク切断と復帰を確認する。
- [ ] 手動: Windows 10/11 で `.exe` / `.lnk` の選択・DnD登録、実アイコン、単体/一斉起動、削除、再起動後の復元を確認する。
- [ ] 手動: 空白・日本語・`&` を含むアプリパスと、移動済みアプリを含む一斉起動の部分失敗表示を確認する。
