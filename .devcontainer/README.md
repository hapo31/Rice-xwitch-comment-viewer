# Devcontainer notes

## Codex CLI

`postCreateCommand` で OpenAI Codex CLI を確認し、未インストールの場合だけ npm のグローバルパッケージとしてインストールします。

```bash
npm install -g @openai/codex@latest
```

devcontainer を Rebuild したあと、コンテナ内で次を実行して確認できます。

```bash
codex --version
```

特定バージョンに固定したい場合は、devcontainer 作成時の環境変数 `CODEX_NPM_PACKAGE` に `@openai/codex@<version>` を指定してください。
`CODEX_NPM_PACKAGE` を指定した場合は、既存の `codex` があっても指定バージョンをインストールします。

Codex の認証情報、履歴、セッション状態は named volume の `rice-codex-home` を `/home/vscode/.codex` にマウントして保持します。

devcontainer を作成する前に手動で用意する場合、作成する volume 名は `rice-codex-home` です。

```bash
docker volume create rice-codex-home
```

これにより devcontainer を Rebuild しても `~/.codex/auth.json` や `~/.codex/history.jsonl` が残ります。初回だけ、既に別コンテナ内にあった未永続化の Codex 状態は自動移行されないため、必要なら再認証してください。

## Cargo target

Cargo のビルド成果物はワークスペース内の `src-tauri/target` ではなく、named volume の `/home/vscode/.cargo-target/rice` に出力します。

これにより、Tauri/Rust の大きな `deps` や `incremental` を VS Code のワークスペース監視対象から外します。既存の `src-tauri/target` は自動削除しないため、不要になったら次で削除できます。

```bash
cargo clean --manifest-path src-tauri/Cargo.toml
```

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

## Windows リリース成果物をローカルで作る

devcontainer は Docker outside Docker feature でホスト側 Docker を使えるようにします。devcontainer を Rebuild したあと、`.env` に Twitch OAuth の public client ID を設定します。

```bash
cp .env.example .env
$EDITOR .env
```

その後、コンテナ内で次を実行すると、GitHub Actions と同じ Dockerfile 経由で Windows の NSIS インストーラーと portable zip を `release-artifacts/` に出力できます。Docker build は `.env` を自動では読まないため、このスクリプトが `.env` を読み込み、`RICE_TWITCH_CLIENT_ID` を build arg として渡します。

```bash
scripts/build-windows-docker.sh
```
