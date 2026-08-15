# Rice

Rice は、Twitch 配信中のチャット確認と読み上げを一つにまとめる Windows 向けデスクトップアプリです。Twitch EventSub WebSocket でチャットを受信し、[棒読みちゃん](https://chi.usamimi.info/Program/Application/BouyomiChan/)へ TCP 接続で送信します。

> Rice は Twitch の非公式クライアントです。Twitch のアカウント情報やパスワードを Rice に入力する必要はありません。ログインには Twitch のブラウザ画面を使います。

## できること・現在の範囲

- Twitch チャットのリアルタイム受信と Chat 画面での表示
- 棒読みちゃん TCP 連携による自動読み上げ、読み上げキューの確認・制御
- NG ユーザー・NG ワード、URL、長文などの読み上げフィルター
- Twitch／棒読みちゃんの接続状態、警告、診断、Logs の確認
- 配信で使う Windows アプリを登録して起動する Launcher

正式な読み上げ先は棒読みちゃんです。VOICEROID2 の直接連携は、Windows 専用の実験的アダプタとしてもまだ実装していません。Twitch と棒読みちゃんを使う実機での統合確認には、未完了の項目があります。詳しい進捗は [docs/TODO.md](./docs/TODO.md) を参照してください。

## 必要なもの

- Windows 10 または 11 の 64 bit PC（公開 Release は `x86_64-pc-windows-msvc` 向け）
- Twitch アカウントと、Twitch へ接続できるネットワーク
- 読み上げを使う場合: 棒読みちゃん。本体側で TCP アプリ連携を有効にします。既定の接続先は `127.0.0.1:50001` です。

Rice 本体だけでも、読み上げなしでチャット受信・表示の準備を進められます。棒読みちゃん、Twitch、ネットワークはいずれも別サービス／別アプリです。

## 入手、インストール、検証

1. [GitHub Releases](https://github.com/hapo31/Rice-xwitch-comment-viewer/releases/latest) を開き、最新 Release の **Assets** を表示します。
2. 通常は NSIS installer（`Rice_<version>_x64-setup.exe`）を選び、ダウンロード後に実行して案内に従います。スタートメニューなどへ登録して使う場合に適しています。
3. インストールせずに試す場合は portable ZIP（`Rice_<version>_x86_64-pc-windows-msvc_portable.zip`）を選びます。任意の書き込み可能なフォルダーへ展開してから、展開先の `rice.exe` を実行してください。ZIP の中から直接実行しないでください。

各 Release には `SHA256SUMS.txt` も含まれます。ダウンロード後のファイル名とチェックサムが Release のものと一致することを、PowerShell で確認できます。

```powershell
Get-FileHash .\Rice_<version>_x64-setup.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

portable ZIP を選んだ場合は、展開前の ZIP に対して同じ方法で確認します。ハッシュが一致しない、または Release に `SHA256SUMS.txt` がない場合は実行せず、再ダウンロードして Release ページを確認してください。現行 Release workflow はチェックサムを公開しますが、コード署名・provenance の扱いはこの README で保証しません。

## 最初の設定

初回起動時は Chat 画面に準備状況が表示されます。次の順に進めると、チャット受信と読み上げを開始できます。

1. **Login** を開き、読み上げたい Twitch チャンネル名を「チャンネル」に入力します。自分のチャットを読む場合は自分のチャンネル名です。
2. **Login** の「認証開始」を押します。表示された認証コードのリンクを開き、Twitch のブラウザ画面でコードを入力して認可します。Rice に Twitch パスワードを入力しません。
3. Login が「ログイン済み」になったことを確認します。必要な権限はチャット読み取り用の `user:read:chat` です。
4. 読み上げを使う場合は棒読みちゃんを起動し、**Settings** を開きます。TCP アプリ連携のホストとポートを確認し（初期値は `127.0.0.1:50001`）、変更した場合は保存します。
5. Settings の「接続確認」で接続を試し、必要に応じて「診断」と「テスト読み上げ」を使います。自動読み上げも Settings でオン／オフできます。
6. Chat 画面の左ペイン「チャット受信」で「開始」を押します。チャットが Chat に表示され、読み上げが有効なら Queue で状態を確認できます。

自動接続を使う場合は Settings の「起動時にチャット受信を開始」を有効にします。自動接続前にも、ログイン状態、チャンネル名、棒読みちゃんの起動状態を確認してください。

## 画面と困ったとき

| 画面 | 用途 | 代表的な復旧操作 |
| --- | --- | --- |
| **Chat** | 受信したチャットと system の案内を表示。左ペインで受信を開始・停止します。 | チャットを受信しない場合、Login のログイン／チャンネルを確認してから左ペインの「開始」を押します。 |
| **Login** | Twitch 認証と接続先チャンネルを管理します。 | 認証切れ、権限不足、購読取り消しは「認証開始」で再ログインします。「有効性確認」も利用できます。 |
| **Settings** | 棒読みちゃん接続、診断、テスト読み上げ、声質、自動読み上げを設定します。 | 読み上げエラーなら棒読みちゃんを起動し、host／port と TCP アプリ連携を確認して「診断」を実行します。 |
| **Queue** | 待機中、読み上げ済み、読み飛ばし、エラーの項目を確認・操作します。 | 読み上げが止まった場合、失敗項目と接続状態を確認し、必要に応じて Settings の診断へ戻ります。 |
| **Filter** | NG ユーザー・NG ワード、URL、長文の読み上げ規則を設定します。 | 期待したチャットが読まれない場合、フィルターと Queue の「読み飛ばし」状態を確認します。 |
| **Logs** | 認証、チャット受信、読み上げ連携の動作ログを表示します。 | 原因が分からない場合に、直前の警告・エラーを確認します。 |
| **Launcher** | 登録した Windows アプリを起動します。 | Twitch／読み上げ設定とは独立しています。 |

棒読みちゃんが未起動、ポートが違う、またはアプリ連携が無効な場合、Queue を安易に破棄せず接続エラーとして扱います。Settings の「診断」の結果を優先して確認してください。ネットワーク切断中に届かなかった Twitch チャットは再送されないため、復帰後に Chat と Logs で再接続状態を確認します。

## 設定、認証情報、ログ

- 一般設定（チャンネル名、棒読みちゃん接続、フィルター、Launcher など）はアプリのデータフォルダーに `settings.json` として保存されます。保存時にはバックアップを作り、破損を検出した場合はバックアップまたは既定値で復旧して Chat と Logs に通知します。
- Twitch の access token と refresh token は OS の資格情報ストアに保存します。`settings.json` や新しい平文ファイルには保存しません。資格情報ストアが使えない場合は、そのログインは起動中だけ有効で、再起動後に再ログインが必要です。
- Login の「認証解除」は保存済みの Twitch 認証情報を削除します。設定を初期化またはアプリのデータフォルダーを削除すると、チャンネル、読み上げ、フィルター、Launcher の設定を失うため、必要なら先にコピーしてバックアップしてください。認証解除の詳細は [認証情報の保存と安全性](./docs/07-oauth-storage-security.md) を参照してください。
- Chat、Queue、Logs は現在のアプリ起動中の表示です。ログとコメント履歴を永続保存する機能は、現時点ではありません。必要な障害情報はアプリを終了する前に控えてください。

## 開発・設計・リリース情報

利用手順はこの README を正とし、以下は開発・保守向けの資料です。

- [設計文書の入口](./docs/README.md)
- [実装状況と手動確認項目](./docs/TODO.md)
- [リリース手順](./docs/releasing.md)
- [開発コンテナの説明](./.devcontainer/README.md)
- [MIT License](./LICENSE)

### README 更新時の確認（maintainer 向け）

Release、route、画面名を変更する PR では、README の導線も同時にレビューしてください。

- `src/routes.ts` の正式 route と画面名（Chat / Launcher / Queue / Filter / Settings / Login / Logs）がこの README と一致すること。`/rules` と `/voices` は legacy redirect であり、新しい利用者向け導線に使わないこと。
- `Dockerfile` の portable ZIP 名、Windows target、`release-windows.yml` の `.exe`／`.zip`／`SHA256SUMS.txt` 生成が「入手、インストール、検証」と一致すること。
- Settings、Login、Side Panel にある操作名（「認証開始」「接続確認」「診断」「開始」）と、障害からの復旧先が一致すること。
