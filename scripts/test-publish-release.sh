#!/usr/bin/env bash
set -euo pipefail

fixture_dir="$(mktemp -d)"
trap 'rm -rf "${fixture_dir}"' EXIT
mkdir -p "${fixture_dir}/bin"

mock_gh="${fixture_dir}/bin/gh"
mock_log="${fixture_dir}/gh.log"
touch "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"
(cd "${fixture_dir}" && sha256sum asset.zip > SHA256SUMS.txt)
export GH_MOCK_ASSET_DIR="${fixture_dir}"

cat > "${mock_gh}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${GH_MOCK_LOG}"
case "$1" in
  api)
    calls="$(grep -c '^api ' "${GH_MOCK_LOG}")"
    if [ "${calls}" -ge "${GH_MOCK_MOVE_AT:-99}" ]; then
      printf '%s\n' bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    else
      printf '%s\n' "${GH_MOCK_REMOTE_TAG_OBJECT}"
    fi
    ;;
  release)
    if [ "${2:-}" = "upload" ] && [ "${GH_MOCK_FAIL_UPLOAD:-0}" = 1 ]; then
      exit 1
    fi
    if [ "${2:-}" = "download" ]; then
      if [ "${GH_MOCK_FAIL_DOWNLOAD:-0}" = 1 ]; then exit 1; fi
      destination="${@: -1}"
      cp "${GH_MOCK_ASSET_DIR}/asset.zip" "${GH_MOCK_ASSET_DIR}/SHA256SUMS.txt" "${destination}/"
      case "${GH_MOCK_DIFFERENCE:-none}" in
        bytes) echo changed > "${destination}/asset.zip" ;;
        missing) rm "${destination}/asset.zip" ;;
        extra) touch "${destination}/extra.zip" ;;
      esac
    fi
    if [ "${2:-}" = "view" ]; then
      if [ "${GH_MOCK_EXISTING:-missing}" = missing ]; then
        exit 1
      fi
      printf '%s\n' "${GH_MOCK_EXISTING}"
    fi
    ;;
  *)
    echo "unexpected gh command: $*" >&2
    exit 2
    ;;
esac
EOF
chmod +x "${mock_gh}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
expected_tag_object="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
moved_tag_object="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

if PATH="${fixture_dir}/bin:${PATH}" \
  GH_MOCK_LOG="${mock_log}" \
  GH_MOCK_REMOTE_TAG_OBJECT="${moved_tag_object}" \
  "${script_dir}/publish-release.sh" \
    v1.2.3 owner/repository "${expected_tag_object}" "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"; then
  echo "エラー: 移動された remote tag を拒否できませんでした。" >&2
  exit 1
fi
if grep -Eq '^release (create|upload|edit)' "${mock_log}"; then
  echo "エラー: remote tag の不一致後に Release を変更しました。" >&2
  exit 1
fi

: > "${mock_log}"
PATH="${fixture_dir}/bin:${PATH}" \
  GH_MOCK_LOG="${mock_log}" \
  GH_MOCK_REMOTE_TAG_OBJECT="${expected_tag_object}" \
  "${script_dir}/publish-release.sh" \
    v1.2.3 owner/repository "${expected_tag_object}" "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"

grep -Fq 'release create v1.2.3' "${mock_log}"
grep -Fq 'release upload v1.2.3' "${mock_log}"
grep -Fq 'release edit v1.2.3' "${mock_log}"
if [ "$(grep -c '^api ' "${mock_log}")" -ne 3 ]; then
  echo "エラー: remote tag を作成前・upload 前・公開前に検証していません。" >&2
  exit 1
fi

printf 'release publication guard tests passed\n'

for move_at in 2 3; do
  : > "${mock_log}"
  if PATH="${fixture_dir}/bin:${PATH}" \
    GH_MOCK_LOG="${mock_log}" GH_MOCK_MOVE_AT="${move_at}" \
    GH_MOCK_REMOTE_TAG_OBJECT="${expected_tag_object}" \
    "${script_dir}/publish-release.sh" \
      v1.2.3 owner/repository "${expected_tag_object}" "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"; then
    echo "エラー: API 呼び出し ${move_at} 回目で移動された tag を拒否できませんでした。" >&2
    exit 1
  fi
  if grep -Eq '^release edit' "${mock_log}"; then
    echo 'エラー: 移動された tag の draft を公開しました。' >&2
    exit 1
  fi
  if [ "${move_at}" -eq 2 ] && grep -Eq '^release upload' "${mock_log}"; then
    echo 'エラー: upload 前の tag 移動を検出しても Assets を変更しました。' >&2
    exit 1
  fi
done

for existing in true false; do
  : > "${mock_log}"
  PATH="${fixture_dir}/bin:${PATH}" \
    GH_MOCK_LOG="${mock_log}" GH_MOCK_EXISTING="${existing}" \
    GH_MOCK_REMOTE_TAG_OBJECT="${expected_tag_object}" \
    "${script_dir}/publish-release.sh" \
      v1.2.3 owner/repository "${expected_tag_object}" "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"
  if grep -Eq '^release create' "${mock_log}"; then
    echo 'エラー: 再実行で既存 Release を再作成しました。' >&2
    exit 1
  fi
  if [ "${existing}" = true ]; then
    grep -Fq 'release upload v1.2.3' "${mock_log}"
    grep -Fq 'release edit v1.2.3' "${mock_log}"
  elif grep -Eq '^release (upload|edit)' "${mock_log}"; then
    echo 'エラー: 公開済み Release の状態を変更しました。' >&2
    exit 1
  fi
done

printf 'release publication race and retry tests passed\n'

for existing in true false; do
  for failure in bytes missing extra download upload; do
    if [ "${existing}" = false ] && [ "${failure}" = upload ]; then continue; fi
    : > "${mock_log}"
    if PATH="${fixture_dir}/bin:${PATH}" \
      GH_MOCK_LOG="${mock_log}" GH_MOCK_EXISTING="${existing}" \
      GH_MOCK_DIFFERENCE="${failure}" \
      GH_MOCK_FAIL_DOWNLOAD="$([ "${failure}" = download ] && echo 1 || echo 0)" \
      GH_MOCK_FAIL_UPLOAD="$([ "${failure}" = upload ] && echo 1 || echo 0)" \
      GH_MOCK_REMOTE_TAG_OBJECT="${expected_tag_object}" \
      "${script_dir}/publish-release.sh" v1.2.3 owner/repository \
        "${expected_tag_object}" "${fixture_dir}/asset.zip" "${fixture_dir}/SHA256SUMS.txt"; then
      echo "Expected failure: ${existing} ${failure}" >&2
      exit 1
    fi
    if grep -Eq '^release edit' "${mock_log}"; then exit 1; fi
    if [ "${existing}" = false ] && grep -Eq '^release (create|upload|edit)' "${mock_log}"; then exit 1; fi
  done
done
printf 'release immutable asset and failure tests passed\n'
