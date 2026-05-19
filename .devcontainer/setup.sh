#!/usr/bin/env bash
set -euo pipefail

rustup component add rustfmt clippy
pnpm install --frozen-lockfile
