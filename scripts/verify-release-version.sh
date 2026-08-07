#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 X.Y.Z [--tag vX.Y.Z] [--changed-from REF]" >&2
  exit 64
}

if [ "$#" -lt 1 ]; then
  usage
fi

version="$1"
shift

if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "エラー: バージョンは X.Y.Z 形式で指定してください: ${version}" >&2
  exit 64
fi

tag_name=""
changed_from=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --tag)
      [ "$#" -ge 2 ] || usage
      tag_name="$2"
      shift 2
      ;;
    --changed-from)
      [ "$#" -ge 2 ] || usage
      changed_from="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

root_dir="${RICE_RELEASE_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "${root_dir}"

package_version="$(jq -er '.version | strings' package.json)"
cargo_version="$(sed -nE 's/^version = "([^"]+)"$/\1/p' src-tauri/Cargo.toml | head -n 1)"
tauri_version="$(jq -er '.version | strings' src-tauri/tauri.conf.json)"

for manifest in \
  "package.json:${package_version}" \
  "src-tauri/Cargo.toml:${cargo_version}" \
  "src-tauri/tauri.conf.json:${tauri_version}"; do
  if [ "${manifest#*:}" != "${version}" ]; then
    echo "エラー: ${manifest%%:*} の version (${manifest#*:}) が期待値 ${version} と一致しません。" >&2
    exit 1
  fi
done

if [ -n "${tag_name}" ] && [ "${tag_name}" != "v${version}" ]; then
  echo "エラー: tag (${tag_name}) が manifest version (${version}) と一致しません。" >&2
  exit 1
fi

if [ -n "${changed_from}" ]; then
  changed_files="$(git diff --name-only "${changed_from}" -- | LC_ALL=C sort)"
  expected_files=$'package.json\nsrc-tauri/Cargo.toml\nsrc-tauri/tauri.conf.json'
  if [ "${changed_files}" != "${expected_files}" ]; then
    echo "エラー: version bump の変更対象は 3 manifest だけにしてください。" >&2
    printf '変更されたファイル:\n%s\n' "${changed_files:-（なし）}" >&2
    exit 1
  fi
fi

printf 'version=%s\npackage.json=%s\nsrc-tauri/Cargo.toml=%s\nsrc-tauri/tauri.conf.json=%s\n' \
  "${version}" "${package_version}" "${cargo_version}" "${tauri_version}"
if [ -n "${tag_name}" ]; then
  printf 'tag=%s\n' "${tag_name}"
fi
