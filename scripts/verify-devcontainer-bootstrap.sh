#!/usr/bin/env bash
set -euo pipefail

lock_file=".devcontainer/bootstrap-lock.json"
dockerfile=".devcontainer/Dockerfile"

test -f "${lock_file}"
test -f "${dockerfile}"

jq -e '
  .schemaVersion == 1
  and (.images.base.reference | type == "string")
  and (.images.base.digest | test("^sha256:[0-9a-f]{64}$"))
  and (.images.nodeBootstrap.reference | test("node:20\\.19\\.4-bookworm-slim$"))
  and (.images.nodeBootstrap.digest | test("^sha256:[0-9a-f]{64}$"))
  and (.images.rustBootstrap.reference | test("rust:1\\.89\\.0-bookworm$"))
  and (.images.rustBootstrap.digest | test("^sha256:[0-9a-f]{64}$"))
  and (.packages.codex.name == "@openai/codex")
  and (.packages.codex.version | test("^[0-9]+\\.[0-9]+\\.[0-9]+$"))
  and (.packages.codex.integrity | test("^sha512-"))
  and (.packages.pnpm.version == "8.11.0")
  and (.packages.pnpm.integrity | test("^sha512-"))
  and (.toolchains.rust.version == "1.89.0")
  and (.toolchains.rust.components == ["rustfmt", "clippy"])
' "${lock_file}" >/dev/null

base_digest="$(jq -r '.images.base.digest' "${lock_file}")"
node_digest="$(jq -r '.images.nodeBootstrap.digest' "${lock_file}")"
rust_digest="$(jq -r '.images.rustBootstrap.digest' "${lock_file}")"

grep -F -- "mcr.microsoft.com/devcontainers/base:bookworm@${base_digest}" "${dockerfile}" >/dev/null
grep -F -- "node:20.19.4-bookworm-slim@${node_digest}" "${dockerfile}" >/dev/null
grep -F -- "rust:1.89.0-bookworm@${rust_digest}" "${dockerfile}" >/dev/null
! rg -n '@openai/codex@latest|CODEX_NPM_PACKAGE|"version": "latest"' .devcontainer
! jq -e '.runArgs[]? == "--network=host"' .devcontainer/devcontainer.json >/dev/null
! rg -n 'target=/home/vscode/\.ssh|source=\$\{localEnv:HOME\}/\.ssh|docker-outside-of-docker' .devcontainer/devcontainer.json

default_feature_lock=".devcontainer/devcontainer-lock.json"
jq -e '.features | keys == ["ghcr.io/devcontainers/features/github-cli:1"]' .devcontainer/devcontainer.json >/dev/null
jq -e '
  .features["ghcr.io/devcontainers/features/github-cli:1"] as $feature
  | $feature.resolved == ("ghcr.io/devcontainers/features/github-cli@" + $feature.integrity)
' "${default_feature_lock}" >/dev/null

jq -e '
  .containerEnv.SSH_AUTH_SOCK == "/tmp/vscode-ssh-auth.sock"
  and ([.mounts[]] | any(test("SSH_AUTH_SOCK")))
  and ([.mounts[]] | all(test("\\.ssh") | not))
' .devcontainer/profiles/ssh-agent/devcontainer.json >/dev/null
jq -e '.runArgs == ["--network=host"]' .devcontainer/profiles/windows-bouyomi/devcontainer.json >/dev/null
jq -e '.features | keys == ["ghcr.io/devcontainers/features/docker-outside-of-docker:1"]' .devcontainer/profiles/release/devcontainer.json >/dev/null

release_feature_lock=".devcontainer/profiles/release/devcontainer-lock.json"
jq -e '
  .features["ghcr.io/devcontainers/features/docker-outside-of-docker:1"] as $feature
  | $feature.resolved == ("ghcr.io/devcontainers/features/docker-outside-of-docker@" + $feature.integrity)
' "${release_feature_lock}" >/dev/null

echo "devcontainer bootstrap lock is internally consistent"
