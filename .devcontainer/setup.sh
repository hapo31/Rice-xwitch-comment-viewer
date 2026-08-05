#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  sudo mkdir -p "${CARGO_TARGET_DIR}"
  sudo chown -R "$(id -u):$(id -g)" "$(dirname "${CARGO_TARGET_DIR}")"
fi

codex_version="$(jq -r '.packages.codex.version' /usr/local/share/rice-devcontainer/bootstrap-lock.json)"
codex --version | grep -F -- "${codex_version}"

pnpm install --frozen-lockfile --prefer-offline
