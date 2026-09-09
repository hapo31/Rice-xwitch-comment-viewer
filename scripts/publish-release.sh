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

gh release upload "${tag_name}" "${assets[@]}" \
  --repo "${repository}" \
  --clobber

assert_remote_tag_unchanged

if [ "${is_draft}" = "true" ]; then
  gh release edit "${tag_name}" \
    --repo "${repository}" \
    --draft=false
fi
