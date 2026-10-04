#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

scripts/check-docker-context.sh
node scripts/verify-release-build-inputs.mjs

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

RICE_TWITCH_CLIENT_ID="${RICE_TWITCH_CLIENT_ID:-${TWITCH_CLIENT_ID:-}}"

export RICE_TWITCH_CLIENT_ID
node scripts/verify-twitch-client-id.mjs

rm -rf release-artifacts
mkdir -p release-artifacts

DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}" docker build \
  --build-arg "RICE_GIT_COMMIT=$(git rev-parse HEAD)" \
  --build-arg "SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)" \
  --build-arg "RICE_TWITCH_CLIENT_ID=${RICE_TWITCH_CLIENT_ID}" \
  --target artifacts \
  --output type=local,dest=release-artifacts \
  "$@" \
  .
