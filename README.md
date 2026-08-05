# Rice

Rice は、Twitch 配信中のチャット確認と読み上げをひとつにまとめる Windows 向けデスクトップアプリです。Twitch EventSub WebSocket でチャットをリアルタイムに受信し、棒読みちゃんへ送信します。

## 特徴

- VS Code 風の落ち着いた画面で、配信中の状態をひと目で確認
- Twitch の OAuth Device Code Flow に対応し、アプリへパスワードを入力せずにログイン
- 読み上げ処理をキューで管理し、接続エラーや読み飛ばしも画面に表示
- 配信で使う Windows アプリを登録し、まとめて起動できるランチャーを搭載

## 主な機能

- Twitch チャットのリアルタイム受信・表示
- 棒読みちゃん TCP 連携による自動読み上げ
- 読み上げキューの確認、スキップ、削除、クリア
- NG ユーザー・NG ワード、URL、長文などの読み上げフィルター
- Twitch／棒読みちゃんの接続状態、警告、ログの表示
- Windows アプリの登録、個別起動、一斉起動

## ダウンロードとインストール

対応環境は Windows 10／11（64 bit）です。読み上げ機能を使う場合は、[棒読みちゃん](https://chi.usamimi.info/Program/Application/BouyomiChan/)を別途用意し、アプリ連携の TCP ポートを有効にしてください。既定の接続先は `127.0.0.1:50001` です。

1. [GitHub Releases](https://github.com/hapo31/Rice-xwitch-comment-viewer/releases/latest) を開きます。
2. 通常は Assets から `Rice_*_x64-setup.exe` をダウンロードし、実行して画面の案内に従います。
3. インストールせずに使う場合は `Rice_*_portable.zip` をダウンロードして展開し、`rice.exe` を実行します。
4. Rice を起動し、Login 画面から Twitch にログインします。読み上げを使う場合は、Settings 画面で棒読みちゃんとの接続を確認します。

## ライセンス

Rice は [MIT License](./LICENSE) のもとで公開されています。

設計や開発に関する資料は [`docs/`](./docs/README.md) を参照してください。
