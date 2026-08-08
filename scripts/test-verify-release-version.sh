#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(mktemp -d)"
trap 'rm -rf "${repo_dir}"' EXIT

mkdir -p "${repo_dir}/src-tauri"
printf '{"version":"1.2.3"}\n' > "${repo_dir}/package.json"
printf '[package]\nname = "rice"\nversion = "1.2.3"\n' > "${repo_dir}/src-tauri/Cargo.toml"
printf '{"version":"1.2.3"}\n' > "${repo_dir}/src-tauri/tauri.conf.json"

git -C "${repo_dir}" init --quiet
git -C "${repo_dir}" add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git -C "${repo_dir}" -c user.name=test -c user.email=test@example.invalid commit --quiet -m fixture

printf '{"version":"1.2.4"}\n' > "${repo_dir}/package.json"
sed -i 's/1.2.3/1.2.4/' "${repo_dir}/src-tauri/Cargo.toml"
printf '{"version":"1.2.4"}\n' > "${repo_dir}/src-tauri/tauri.conf.json"

RICE_RELEASE_ROOT="${repo_dir}" "$PWD/scripts/verify-release-version.sh" 1.2.4 --tag v1.2.4 --changed-from HEAD

if RICE_RELEASE_ROOT="${repo_dir}" "$PWD/scripts/verify-release-version.sh" 1.2.4 --tag v1.2.5; then
  echo "エラー: tag の不一致を検出できませんでした。" >&2
  exit 1
fi
