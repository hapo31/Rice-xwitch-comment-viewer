#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 3 ]; then
  echo "Usage: $0 TAG REPOSITORY ASSET..." >&2
  exit 64
fi

tag_name="$1"
repository="$2"
shift 2
assets=("$@")

if gh release view "${tag_name}" --repo "${repository}" >/dev/null 2>&1; then
  is_draft="$(gh release view "${tag_name}" --repo "${repository}" --json isDraft --jq .isDraft)"
else
  gh release create "${tag_name}" \
    --repo "${repository}" \
    --title "${tag_name}" \
    --notes-from-tag \
    --verify-tag \
    --draft
  is_draft=true
fi

gh release upload "${tag_name}" "${assets[@]}" \
  --repo "${repository}" \
  --clobber

if [ "${is_draft}" = "true" ]; then
  gh release edit "${tag_name}" \
    --repo "${repository}" \
    --draft=false
fi
