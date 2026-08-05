# Devcontainer notes

## Default profile and bootstrap lock

`.devcontainer/devcontainer.json` is the normal development profile. It does not mount host SSH keys or Git configuration, a Codex state volume, the Docker socket, or host networking.

Node 20.19.4, pnpm 8.11.0, Codex CLI 0.98.0, Rust 1.89.0, and the base images are fixed in [`bootstrap-lock.json`](./bootstrap-lock.json). The Dockerfile downloads the two npm tarballs during the image build, checks their SHA-512 integrity, and installs them with lifecycle scripts disabled. Rust, rustfmt, and clippy are copied from the fixed Rust image. Thus these tools are installed before any profile can mount host credentials or Docker access.

After a normal Rebuild, `postCreateCommand` only verifies the baked Codex version and runs the project's lockfile-based `pnpm install`. Check the installed tools with:

```bash
codex --version
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
| [`profiles/ssh-agent/devcontainer.json`](./profiles/ssh-agent/devcontainer.json) | Scoped SSH agent socket and `rice-codex-home` state volume | Git-over-SSH or a persisted Codex login is required. |
| [`profiles/windows-bouyomi/devcontainer.json`](./profiles/windows-bouyomi/devcontainer.json) | `--network=host` | Connecting to a Windows-side BouyomiChan server from mirrored WSL2 networking. |
| [`profiles/release/devcontainer.json`](./profiles/release/devcontainer.json) | Docker outside of Docker | Running `scripts/build-windows-docker.sh` locally. |

The SSH profile requires a running agent and uses `${SSH_AUTH_SOCK}`. It never bind-mounts `~/.ssh`, so private-key files are not readable by the container. Load only the key needed for this repository and remove it from the agent when finished. Do not combine the release profile with SSH or host-network profiles unless the task genuinely requires every capability.

Codex state in the SSH-agent profile is retained in `rice-codex-home` at `/home/vscode/.codex`. Create it manually if needed:

```bash
docker volume create rice-codex-home
```

## Credential incident response

Treat a suspected malicious rebuild or bootstrap-integrity failure as a credential incident. Stop the container, revoke and recreate any SSH keys loaded into the agent, revoke Codex sessions/tokens and remove `rice-codex-home`, rotate any GitHub/Twitch credentials used from the container, and review Docker activity before enabling the release profile again. Rebuild only from a reviewed commit after verifying the lock with `scripts/verify-devcontainer-bootstrap.sh`.

## Cargo target

The normal profile writes Cargo build artifacts to the `rice-cargo-target` named volume at `/home/vscode/.cargo-target/rice`, keeping large artifacts out of workspace file watching. Existing `src-tauri/target` content is never removed automatically.

## Windows-side BouyomiChan connection

Use the explicit Windows Bouyomi profile when WSL2 mirrored networking requires host networking. Start BouyomiChan on Windows with its TCP integration enabled, then set:

```text
host: 127.0.0.1
port: 50001
```

If it cannot connect, confirm that BouyomiChan is running, TCP integration is enabled, it is listening on `127.0.0.1:50001` (or `0.0.0.0:50001`), and Windows Firewall permits the connection. Apply profile changes with a Rebuild.

## Local Windows release artifacts

Use only the explicit release profile to access the host Docker daemon. Set the Twitch OAuth public client ID in `.env` and run:

```bash
scripts/build-windows-docker.sh
```

The script creates the NSIS installer and portable zip in `release-artifacts/`. Docker builds do not read `.env` automatically; the script passes `RICE_TWITCH_CLIENT_ID` as a build argument.
