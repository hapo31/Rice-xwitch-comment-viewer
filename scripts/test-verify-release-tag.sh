#!/usr/bin/env bash
set -euo pipefail

fixture_dir="$(mktemp -d)"
trap 'rm -rf "${fixture_dir}"' EXIT

git -C "${fixture_dir}" init --quiet
git -C "${fixture_dir}" config user.name test
git -C "${fixture_dir}" config user.email test@example.invalid
git -C "${fixture_dir}" branch -M main

printf 'first\n' > "${fixture_dir}/file.txt"
git -C "${fixture_dir}" add file.txt
git -C "${fixture_dir}" commit --quiet -m first

printf 'second\n' >> "${fixture_dir}/file.txt"
git -C "${fixture_dir}" commit --quiet -am second
main_commit="$(git -C "${fixture_dir}" rev-parse HEAD)"
git -C "${fixture_dir}" tag -a v1.2.3 -m 'release 1.2.3'
original_tag_object="$(git -C "${fixture_dir}" rev-parse v1.2.3^{tag})"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
verify_script="${script_dir}/verify-release-tag.sh"

verify() {
  (
    cd "${fixture_dir}"
    "${verify_script}" v1.2.3 \
      --expected-commit "${main_commit}" \
      --checkout-ref HEAD \
      --main-ref refs/heads/main \
      "$@"
  )
}

expect_failure() {
  expected_message="$1"
  shift
  output_file="${fixture_dir}/failure-output.txt"
  if "$@" >"${output_file}" 2>&1; then
    echo "エラー: 失敗すべき検証が成功しました: ${expected_message}" >&2
    exit 1
  fi
  if ! grep -Fq "${expected_message}" "${output_file}"; then
    echo "エラー: 期待する失敗理由がありません: ${expected_message}" >&2
    sed -n '1,120p' "${output_file}" >&2
    exit 1
  fi
}

verify --expected-tag-object "${original_tag_object}"

first_commit="$(git -C "${fixture_dir}" rev-parse HEAD^)"
expect_failure 'event commit' bash -c '
  cd "$1"
  "$2" v1.2.3 --expected-commit "$3" --checkout-ref HEAD --main-ref refs/heads/main
' _ "${fixture_dir}" "${verify_script}" "${first_commit}"

git -C "${fixture_dir}" branch checkout-mismatch "${first_commit}"
expect_failure 'checkout HEAD' bash -c '
  cd "$1"
  "$2" v1.2.3 --expected-commit "$3" --checkout-ref refs/heads/checkout-mismatch --main-ref refs/heads/main
' _ "${fixture_dir}" "${verify_script}" "${main_commit}"

git -C "${fixture_dir}" switch --quiet -c outside-main
printf 'outside\n' > "${fixture_dir}/outside.txt"
git -C "${fixture_dir}" add outside.txt
git -C "${fixture_dir}" commit --quiet -m outside-main
outside_commit="$(git -C "${fixture_dir}" rev-parse HEAD)"
git -C "${fixture_dir}" tag -a v1.2.4 -m 'outside main'
expect_failure '上にありません' bash -c '
  cd "$1"
  "$2" v1.2.4 --expected-commit "$3" --checkout-ref HEAD --main-ref refs/heads/main
' _ "${fixture_dir}" "${verify_script}" "${outside_commit}"

git -C "${fixture_dir}" switch --quiet main
git -C "${fixture_dir}" tag -f -a v1.2.3 -m 'moved annotation' "${main_commit}" >/dev/null
expect_failure 'build 時から変更されています' verify --expected-tag-object "${original_tag_object}"

git -C "${fixture_dir}" tag -f v1.2.3 "${main_commit}"
expect_failure '注釈付きタグではありません' verify

printf 'release tag verification tests passed\n'
