#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 4 ]; then
  echo "Usage: $0 TAG REPOSITORY EXPECTED_TAG_OBJECT ASSET..." >&2
  exit 64
fi

tag_name="$1"
repository="$2"
expected_tag_object="${3,,}"
shift 3
assets=("$@")
verification_dir="$(mktemp -d)"
trap 'rm -rf "${verification_dir}"' EXIT
mkdir "${verification_dir}/expected" "${verification_dir}/remote"
for asset in "${assets[@]}"; do
  name="$(basename "${asset}")"
  if [ ! -f "${asset}" ] || [ -e "${verification_dir}/expected/${name}" ]; then
    echo 'エラー: 成果物が存在しないか、ファイル名が重複しています。' >&2
    exit 1
  fi
  cp -- "${asset}" "${verification_dir}/expected/${name}"
done
(cd "${verification_dir}/expected" && sha256sum --check --strict SHA256SUMS.txt)

verify_remote_assets() {
  gh release download "${tag_name}" --repo "${repository}" --dir "${verification_dir}/remote"
  if ! diff -qr "${verification_dir}/expected" "${verification_dir}/remote"; then
    echo 'エラー: Release の成果物が不一致または不足しています。公開済み成果物は変更せず、新しい patch version を発行してください。draft は成果物を揃えて再実行してください。' >&2
    exit 1
  fi
}

if [[ ! "${tag_name}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "エラー: タグ名は vX.Y.Z 形式で指定してください: ${tag_name}" >&2
  exit 64
fi
if [[ ! "${repository}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  echo "エラー: repository は owner/name 形式で指定してください: ${repository}" >&2
  exit 64
fi
if [[ ! "${expected_tag_object}" =~ ^[0-9a-f]{40}([0-9a-f]{24})?$ ]]; then
  echo "エラー: 期待する tag object SHA が不正です: ${expected_tag_object}" >&2
  exit 64
fi

assert_remote_tag_unchanged() {
  remote_tag_object="$(
    gh api "repos/${repository}/git/ref/tags/${tag_name}" --jq '.object.sha'
  )"
  if [ "${remote_tag_object}" != "${expected_tag_object}" ]; then
    echo "エラー: remote tag ${tag_name} が build 時から変更されています (${remote_tag_object} != ${expected_tag_object})。公開を中止します。" >&2
    exit 1
  fi
}

assert_remote_tag_unchanged

if gh release view "${tag_name}" --repo "${repository}" >/dev/null 2>&1; then
  is_draft="$(gh release view "${tag_name}" --repo "${repository}" --json isDraft --jq .isDraft)"
else
  GH_REPO="${repository}" gh release create "${tag_name}" \
    --title "${tag_name}" \
    --notes-from-tag \
    --verify-tag \
    --draft
  is_draft=true
fi

assert_remote_tag_unchanged

if [ "${is_draft}" = false ]; then
  verify_remote_assets
  assert_remote_tag_unchanged
  echo '公開済み Release の全成果物が一致しました。変更はありません。'
  exit 0
elif [ "${is_draft}" != true ]; then
  echo 'エラー: Release の公開状態を確認できません。' >&2
  exit 1
fi

gh release upload "${tag_name}" "${assets[@]}" \
  --repo "${repository}" \
  --clobber

verify_remote_assets
assert_remote_tag_unchanged

if [ "${is_draft}" = "true" ]; then
  gh release edit "${tag_name}" \
    --repo "${repository}" \
    --draft=false
fi
