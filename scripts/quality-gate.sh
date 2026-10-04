#!/usr/bin/env bash
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The same commands are used by PR, main and release. Each CI job chooses one
# gate, so a failure remains attributable to formatting, lint, types or tests.
case "${1:-}" in
  frontend-format) pnpm run format:check ;;
  frontend-lint) pnpm run lint; pnpm run check:security ;;
  frontend-typecheck) pnpm run typecheck ;;
  frontend-test) pnpm test ;;
  frontend-build) pnpm build ;;
  rust-format) cargo fmt --check --manifest-path src-tauri/Cargo.toml ;;
  rust-clippy) cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings ;;
  rust-test) cargo test --locked --manifest-path src-tauri/Cargo.toml --all-features ;;
  *) echo 'Usage: bash scripts/quality-gate.sh {frontend-format|frontend-lint|frontend-typecheck|frontend-test|frontend-build|rust-format|rust-clippy|rust-test}' >&2; exit 64 ;;
esac
