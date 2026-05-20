# Devcontainer notes

## Windows 側の棒読みちゃんへ接続する

この devcontainer は、WSL2 の `networkingMode=mirrored` と WSL2 上の Docker を前提に、`--network=host` で起動します。

この構成では、devcontainer から Windows 側の TCP サーバーへ `127.0.0.1` で到達できることを期待します。棒読みちゃんを Windows 側で起動し、アプリ連携の TCP ポートを有効にしたうえで、Rice の棒読みちゃん接続先を次にします。

```text
host: 127.0.0.1
port: 50001
```

新規に作られる設定 JSON では、devcontainer の `RICE_BOUYOMI_HOST=127.0.0.1` が既定値として使われます。既に設定 JSON がある場合は、Voices 画面でホストを手動で `127.0.0.1` に変更してください。

`host.docker.internal` は Docker Desktop では便利ですが、WSL2 上の Docker では Windows ホストではなく Docker/WSL 側の gateway を指すことがあります。このリポジトリの devcontainer では使わない方針にしています。

設定を反映するには devcontainer の Rebuild が必要です。

接続できない場合は、まず次を確認してください。

- 棒読みちゃんが起動している
- 棒読みちゃんのアプリ連携/TCP 受付が有効
- 棒読みちゃんが `0.0.0.0:50001` または `127.0.0.1:50001` で LISTEN している
- Voices 画面のホストが `127.0.0.1`
- Windows Defender Firewall やセキュリティソフトが WSL/Docker からの接続を遮断していない

Windows 側では次で待ち受け状態を確認できます。

```powershell
netstat -ano | findstr ":50001"
```
