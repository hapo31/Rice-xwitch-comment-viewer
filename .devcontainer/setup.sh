#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  sudo mkdir -p "${CARGO_TARGET_DIR}"
  sudo chown -R "$(id -u):$(id -g)" "$(dirname "${CARGO_TARGET_DIR}")"
fi
codex_home="${CODEX_HOME:-${HOME}/.codex}"
sudo mkdir -p "${codex_home}"
sudo chown -R "$(id -u):$(id -g)" "${codex_home}"
chmod 700 "${codex_home}"


codex_version="$(jq -r '.packages.codex.version' /usr/local/share/rice-devcontainer/bootstrap-lock.json)"
codex --version | grep -F -- "${codex_version}"

pnpm install --frozen-lockfile --prefer-offline
