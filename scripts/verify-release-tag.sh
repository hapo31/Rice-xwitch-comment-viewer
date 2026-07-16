#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 vX.Y.Z" >&2
  exit 64
fi

tag_name="$1"

if [[ ! "${tag_name}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "エラー: タグ名は vX.Y.Z 形式で指定してください: ${tag_name}" >&2
  exit 64
fi

if ! git show-ref --verify --quiet "refs/tags/${tag_name}"; then
  echo "エラー: ローカルタグが見つかりません: ${tag_name}" >&2
  exit 1
fi

object_type="$(git cat-file -t "refs/tags/${tag_name}")"
if [ "${object_type}" != "tag" ]; then
  echo "エラー: ${tag_name} は注釈付きタグではありません。軽量タグはリリースに使用できません。" >&2
  exit 1
fi

annotation="$(git for-each-ref --format='%(contents)' "refs/tags/${tag_name}")"
if [ -z "${annotation//[[:space:]]/}" ]; then
  echo "エラー: ${tag_name} の annotation message が空です。" >&2
  exit 1
fi

target_commit="$(git rev-parse "${tag_name}^{commit}")"
printf 'tag=%s\ntarget=%s\nannotation:\n%s\n' "${tag_name}" "${target_commit}" "${annotation}"
