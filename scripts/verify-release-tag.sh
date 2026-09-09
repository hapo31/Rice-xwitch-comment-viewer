#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 vX.Y.Z --expected-commit COMMIT --checkout-ref REF --main-ref REF [--tag-ref REF] [--expected-tag-object SHA]" >&2
  exit 64
}

if [ "$#" -lt 1 ]; then
  usage
fi

tag_name="$1"
shift

if [[ ! "${tag_name}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "エラー: タグ名は vX.Y.Z 形式で指定してください: ${tag_name}" >&2
  exit 64
fi

tag_ref="refs/tags/${tag_name}"
expected_commit=""
checkout_ref=""
main_ref=""
expected_tag_object=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --tag-ref)
      [ "$#" -ge 2 ] || usage
      tag_ref="$2"
      shift 2
      ;;
    --expected-commit)
      [ "$#" -ge 2 ] || usage
      expected_commit="$2"
      shift 2
      ;;
    --checkout-ref)
      [ "$#" -ge 2 ] || usage
      checkout_ref="$2"
      shift 2
      ;;
    --main-ref)
      [ "$#" -ge 2 ] || usage
      main_ref="$2"
      shift 2
      ;;
    --expected-tag-object)
      [ "$#" -ge 2 ] || usage
      expected_tag_object="${2,,}"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

if [ -z "${expected_commit}" ] || [ -z "${checkout_ref}" ] || [ -z "${main_ref}" ]; then
  usage
fi

if ! git check-ref-format "${tag_ref}" >/dev/null 2>&1; then
  echo "エラー: tag ref が不正です: ${tag_ref}" >&2
  exit 64
fi

if ! git show-ref --verify --quiet "${tag_ref}"; then
  echo "エラー: 検証対象の tag ref が見つかりません: ${tag_ref}" >&2
  exit 1
fi

object_type="$(git cat-file -t "${tag_ref}")"
if [ "${object_type}" != "tag" ]; then
  echo "エラー: ${tag_name} は注釈付きタグではありません。軽量タグはリリースに使用できません。" >&2
  exit 1
fi

tag_object="$(git rev-parse "${tag_ref}^{tag}")"
if [ -n "${expected_tag_object}" ]; then
  if [[ ! "${expected_tag_object}" =~ ^[0-9a-f]{40}([0-9a-f]{24})?$ ]]; then
    echo "エラー: 期待する tag object SHA が不正です: ${expected_tag_object}" >&2
    exit 64
  fi
  if [ "${tag_object}" != "${expected_tag_object}" ]; then
    echo "エラー: ${tag_name} の tag object が build 時から変更されています (${tag_object} != ${expected_tag_object})。タグを移動せず、新しい version を発行してください。" >&2
    exit 1
  fi
fi

declared_tag="$(git cat-file tag "${tag_ref}" | sed -n 's/^tag //p' | head -n 1)"
if [ "${declared_tag}" != "${tag_name}" ]; then
  echo "エラー: tag object 内の名前 (${declared_tag:-不明}) が期待値 ${tag_name} と一致しません。" >&2
  exit 1
fi

annotation="$(git for-each-ref --format='%(contents)' "${tag_ref}")"
if [ -z "${annotation//[[:space:]]/}" ]; then
  echo "エラー: ${tag_name} の annotation message が空です。" >&2
  exit 1
fi

if ! expected_commit_resolved="$(git rev-parse --verify "${expected_commit}^{commit}" 2>/dev/null)"; then
  echo "エラー: event commit を解決できません: ${expected_commit}" >&2
  exit 1
fi
if ! checkout_commit="$(git rev-parse --verify "${checkout_ref}^{commit}" 2>/dev/null)"; then
  echo "エラー: checkout ref を解決できません: ${checkout_ref}" >&2
  exit 1
fi
if ! main_commit="$(git rev-parse --verify "${main_ref}^{commit}" 2>/dev/null)"; then
  echo "エラー: main ref を解決できません: ${main_ref}" >&2
  exit 1
fi

target_commit="$(git rev-parse "${tag_ref}^{commit}")"
if [ "${target_commit}" != "${expected_commit_resolved}" ]; then
  echo "エラー: ${tag_name} の対象 commit (${target_commit}) が event commit (${expected_commit_resolved}) と一致しません。移動されたタグはリリースできません。" >&2
  exit 1
fi
if [ "${target_commit}" != "${checkout_commit}" ]; then
  echo "エラー: ${tag_name} の対象 commit (${target_commit}) が checkout HEAD (${checkout_commit}) と一致しません。" >&2
  exit 1
fi
if ! git merge-base --is-ancestor "${target_commit}" "${main_commit}"; then
  echo "エラー: ${tag_name} の対象 commit (${target_commit}) は ${main_ref} (${main_commit}) 上にありません。" >&2
  exit 1
fi

printf 'tag=%s\ntag_object=%s\ntarget=%s\ncheckout=%s\nmain=%s\nannotation:\n%s\n' \
  "${tag_name}" "${tag_object}" "${target_commit}" "${checkout_commit}" "${main_commit}" "${annotation}"
