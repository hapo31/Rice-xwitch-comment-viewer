#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  sudo mkdir -p "${CARGO_TARGET_DIR}"
  sudo chown -R "$(id -u):$(id -g)" "$(dirname "${CARGO_TARGET_DIR}")"
fi

sudo mkdir -p "${HOME}/.codex"
sudo chown -R "$(id -u):$(id -g)" "${HOME}/.codex"
chmod 700 "${HOME}/.codex"

rustup component add rustfmt clippy

if [[ -n "${CODEX_NPM_PACKAGE:-}" ]]; then
  npm install -g "${CODEX_NPM_PACKAGE}"
elif ! command -v codex >/dev/null 2>&1; then
  npm install -g @openai/codex@latest
fi

codex --version
pnpm install --frozen-lockfile --prefer-offline
