#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

RICE_TWITCH_CLIENT_ID="${RICE_TWITCH_CLIENT_ID:-${TWITCH_CLIENT_ID:-}}"

if [ -z "${RICE_TWITCH_CLIENT_ID}" ]; then
  cat >&2 <<'EOF'
RICE_TWITCH_CLIENT_ID が未設定です。

.env に次のように設定するか、環境変数として指定してから再実行してください。

RICE_TWITCH_CLIENT_ID=your_twitch_public_client_id
EOF
  exit 1
fi

rm -rf release-artifacts
mkdir -p release-artifacts

DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}" docker build \
  --build-arg "RICE_TWITCH_CLIENT_ID=${RICE_TWITCH_CLIENT_ID}" \
  --target artifacts \
  --output type=local,dest=release-artifacts \
  "$@" \
  .
