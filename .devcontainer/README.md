# Devcontainer notes

## Windows 側の棒読みちゃんへ接続する

この devcontainer では、コンテナから Windows ホストへ `host.docker.internal` で到達できるようにしています。

棒読みちゃんを Windows 側で起動し、アプリ連携の TCP ポートを有効にしたうえで、Rice の棒読みちゃん接続先を次にします。

```text
host: host.docker.internal
port: 50001
```

新規に作られる設定 JSON では、devcontainer の `RICE_BOUYOMI_HOST=host.docker.internal` が既定値として使われます。既に設定 JSON がある場合は、Voices 画面でホストを手動で変更してください。

VS Code/devcontainer の `forwardPorts` は、コンテナ内で起動した Vite などのサービスをホスト側へ公開するための設定です。Windows 側で動いている棒読みちゃんへコンテナから接続する用途では、`forwardPorts` ではなく `host.docker.internal` を使います。

接続できない場合は、まず次を確認してください。

- 棒読みちゃんが起動している
- 棒読みちゃんのアプリ連携/TCP 受付が有効
- ポートが `50001`
- Windows Defender Firewall やセキュリティソフトが Docker/WSL からの接続を遮断していない
