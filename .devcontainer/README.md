# Devcontainer notes

## Default profile and bootstrap lock

`.devcontainer/devcontainer.json` is the normal development profile. It does not mount host SSH keys or Git configuration, the Docker socket, or host networking. Codex state is retained in the local Docker named volume `rice-codex-home` at `/home/vscode/.codex`, so a Rebuild keeps Codex authentication, history, and sessions. The volume is local to the Docker environment; it is not committed to Git or sent in the Docker build context.

Node 20.19.4, pnpm 8.11.0, Codex CLI 0.98.0, Rust 1.89.0, and the base images are fixed in [`bootstrap-lock.json`](./bootstrap-lock.json). The Dockerfile downloads the two npm tarballs during the image build, checks their SHA-512 integrity, and installs them with lifecycle scripts disabled. Rust, rustfmt, and clippy are copied from the fixed Rust image. Thus these tools are installed before any profile can mount host credentials or Docker access.

GitHub CLI is installed through the pinned [`github-cli` Feature](./devcontainer-lock.json).

After a normal Rebuild, `postCreateCommand` only verifies the baked Codex version and runs the project's lockfile-based `pnpm install`. Check the installed tools with:

```bash
codex --version
gh --version
pnpm --version
rustc --version
```

Do not change a version, digest, or integrity value in isolation. Update `bootstrap-lock.json` and `.devcontainer/Dockerfile` in the same reviewed dependency/tool update PR, then run:

```bash
scripts/verify-devcontainer-bootstrap.sh
docker build --pull --file .devcontainer/Dockerfile --tag rice-devcontainer-bootstrap .
docker run --rm rice-devcontainer-bootstrap codex --version
```

The `Devcontainer bootstrap` workflow runs this rebuild, version verification, and `pnpm test`, `pnpm build`, and `cargo test` whenever these bootstrap files change.

## Opt-in capability profiles

Open these configuration files explicitly (for example, with `devcontainer up --workspace-folder . --config <path>`, or by selecting that configuration in the Dev Containers UI). They deliberately do not inherit the normal profile's `postCreateCommand`: the image is already verified before the capability is mounted, and project setup remains a user-initiated `pnpm install --frozen-lockfile`.

| Profile | Capability | Use only when |
| --- | --- | --- |
| [`profiles/ssh-agent/devcontainer.json`](./profiles/ssh-agent/devcontainer.json) | Scoped SSH agent socket | Git-over-SSH is required. |
| [`profiles/windows-bouyomi/devcontainer.json`](./profiles/windows-bouyomi/devcontainer.json) | `--network=host` | Connecting to a Windows-side BouyomiChan server from mirrored WSL2 networking. |
| [`profiles/release/devcontainer.json`](./profiles/release/devcontainer.json) | Docker outside of Docker | Running `scripts/build-windows-docker.sh` locally. |

The SSH profile requires a running agent and uses `${SSH_AUTH_SOCK}`. It never bind-mounts `~/.ssh`, so private-key files are not readable by the container. Load only the key needed for this repository and remove it from the agent when finished. Do not combine the release profile with SSH or host-network profiles unless the task genuinely requires every capability.

Codex state in the normal profile is retained in `rice-codex-home` at `/home/vscode/.codex`. Docker creates the volume automatically on the first container creation. To create it before opening the devcontainer, run:

```bash
docker volume create rice-codex-home
```

## Credential incident response

Treat a suspected malicious rebuild or bootstrap-integrity failure as a credential incident. Stop the container, revoke and recreate any SSH keys loaded into the agent, revoke Codex sessions/tokens and remove `rice-codex-home`, rotate any GitHub/Twitch credentials used from the container, and review Docker activity before enabling the release profile again. Rebuild only from a reviewed commit after verifying the lock with `scripts/verify-devcontainer-bootstrap.sh`.

手動退避には `.devcontainer/codex-state-transfer.sh backup` を使います。バックアップは認証情報、履歴、セッションを含むため、既定では workspace 外の `${XDG_STATE_HOME:-$HOME/.local/state}/rice-xwitch-comment-viewer/codex-state-backup.zip` に `0600` で保存します。保存先を変更する場合は `CODEX_BACKUP_DIR` または `CODEX_BACKUP_ZIP` を指定してください。workspace 内へ置く場合も `.codex-state-backup/` を使用し、Git や Docker など外部へ送信しないでください。

## Cargo target

The normal profile writes Cargo build artifacts to the `rice-cargo-target` named volume at `/home/vscode/.cargo-target/rice`, keeping large artifacts out of workspace file watching. Existing `src-tauri/target` content is never removed automatically.

## Windows-side BouyomiChan connection

Use the explicit Windows Bouyomi profile when WSL2 mirrored networking requires host networking. Start BouyomiChan on Windows with its TCP integration enabled, then set:

```text
host: 127.0.0.1
port: 50001
```

If it cannot connect, confirm that BouyomiChan is running, TCP integration is enabled, it is listening on `127.0.0.1:50001` (or `0.0.0.0:50001`), and Windows Firewall permits the connection. In the app, open **Settings** and select **診断** to see the configured address and the connection result. Apply profile changes with a Rebuild.

## Local Windows release artifacts

Use only the explicit release profile to access the host Docker daemon. Set the Twitch OAuth public client ID in `.env` and run:

```bash
scripts/build-windows-docker.sh
```

The script creates the NSIS installer and portable zip in `release-artifacts/`. Docker builds do not read `.env` automatically; the script passes `RICE_TWITCH_CLIENT_ID` as a build argument.

このスクリプトは送信前に `scripts/check-docker-context.sh` を実行します。root Dockerfile が必要とする source だけを `.dockerignore` の allowlist として許可し、Codex state、`.env`、秘密鍵、credential archive が context に入る構成なら停止します。remote/shared Docker daemon を使う場合、allowlist 内の source code は daemon、BuildKit frontend、cache 管理者から参照できるデータ境界に入るものとして扱ってください。

過去に secret-bearing context を remote/shared builder へ送った可能性がある場合は、対象 builder の利用を停止し、管理者へ context/cache/log の削除を依頼してください。そのうえで `codex logout` を実行し、OpenAI アカウントのセキュリティ設定で該当セッションや API key を失効させ、`codex login` で再認証します。影響範囲を確認できるまで、漏えいした可能性がある archive は再利用しないでください。
