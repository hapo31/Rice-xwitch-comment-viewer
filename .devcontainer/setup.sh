#!/usr/bin/env bash
set -euo pipefail

rustup component add rustfmt clippy
npm install -g "${CODEX_NPM_PACKAGE:-@openai/codex@latest}"
codex --version
pnpm install --frozen-lockfile
