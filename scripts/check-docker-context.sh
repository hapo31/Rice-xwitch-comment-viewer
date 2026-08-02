#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

expected_exceptions=(
  '!Dockerfile'
  '!package.json'
  '!pnpm-lock.yaml'
  '!index.html'
  '!postcss.config.js'
  '!tailwind.config.js'
  '!tsconfig.json'
  '!vite.config.ts'
  '!src/'
  '!src/**'
  '!src-tauri/'
  '!src-tauri/Cargo.toml'
  '!src-tauri/Cargo.lock'
  '!src-tauri/build.rs'
  '!src-tauri/tauri.conf.json'
  '!src-tauri/capabilities/'
  '!src-tauri/capabilities/**'
  '!src-tauri/icons/'
  '!src-tauri/icons/**'
  '!src-tauri/src/'
  '!src-tauri/src/**'
)

expected_copy_sources=(
  index.html
  package.json
  pnpm-lock.yaml
  postcss.config.js
  src
  src-tauri/Cargo.lock
  src-tauri/Cargo.toml
  src-tauri/build.rs
  src-tauri/capabilities
  src-tauri/icons
  src-tauri/src
  src-tauri/tauri.conf.json
  tailwind.config.js
  tsconfig.json
  vite.config.ts
)

fail() {
  echo "Docker build context policy error: $*" >&2
  exit 1
}

first_rule="$(sed -n '/^[[:space:]]*#/d; /^[[:space:]]*$/d; { s/[[:space:]]*$//; p; q; }' .dockerignore)"
[[ "${first_rule}" == '**' ]] || fail '.dockerignore must begin with the default-deny ** rule'

sensitive_deny_rules=(
  '.codex-state-backup/**' \
  '**/codex-state-backup*.zip' \
  '**/auth.json' \
  '**/credentials.json' \
  '**/history.jsonl' \
  '**/sessions/**' \
  '**/.env' \
  '**/.env.*' \
  '**/*.key' \
  '**/*.pem' \
  '**/*.p12' \
  '**/*.pfx' \
  '**/id_rsa' \
  '**/id_ed25519' \
  '**/*.zip' \
  '**/*.tar' \
  '**/*.tar.gz' \
  '**/*.tgz' \
  '**/*.7z' \
  '**/*.rar'
)
for deny_rule in "${sensitive_deny_rules[@]}"; do
  grep -Fqx "${deny_rule}" .dockerignore || fail "missing sensitive-path deny rule: ${deny_rule}"
done

mapfile -t actual_exceptions < <(sed -n '/^!/p' .dockerignore)
if ! diff -u \
  <(printf '%s\n' "${expected_exceptions[@]}") \
  <(printf '%s\n' "${actual_exceptions[@]}"); then
  fail 'Docker context exceptions differ from the reviewed allowlist'
fi

last_exception_line="$(grep -n '^!' .dockerignore | tail -n 1 | cut -d: -f1)"
for deny_rule in "${sensitive_deny_rules[@]}"; do
  deny_line="$(grep -Fnx "${deny_rule}" .dockerignore | cut -d: -f1)"
  (( deny_line > last_exception_line )) || fail "sensitive deny must follow all exceptions: ${deny_rule}"
done

if grep -Eq '^[[:space:]]*ADD[[:space:]]' Dockerfile; then
  fail 'Dockerfile ADD is not permitted; review and extend the context policy explicitly'
fi

mapfile -t actual_copy_sources < <(
  awk '
    /^[[:space:]]*COPY[[:space:]]/ && $0 !~ /--from=/ {
      for (i = 2; i < NF; i++) {
        if ($i !~ /^--/) print $i
      }
    }
  ' Dockerfile | LC_ALL=C sort -u
)
if ! diff -u \
  <(printf '%s\n' "${expected_copy_sources[@]}") \
  <(printf '%s\n' "${actual_copy_sources[@]}"); then
  fail 'Dockerfile COPY sources changed; review and update the allowlist together'
fi

manifest_files=(
  Dockerfile
  package.json
  pnpm-lock.yaml
  index.html
  postcss.config.js
  tailwind.config.js
  tsconfig.json
  vite.config.ts
  src-tauri/Cargo.toml
  src-tauri/Cargo.lock
  src-tauri/build.rs
  src-tauri/tauri.conf.json
)
manifest_dirs=(
  src
  src-tauri/capabilities
  src-tauri/icons
  src-tauri/src
)

for path in "${manifest_files[@]}" "${manifest_dirs[@]}"; do
  [[ -e "${path}" ]] || fail "allowlisted path does not exist: ${path}"
done

if find "${manifest_dirs[@]}" -type l -print -quit | grep -q .; then
  fail 'symlinks are not permitted in allowlisted context directories'
fi

emit_manifest() {
  printf '%s\n' "${manifest_files[@]}"
  find "${manifest_dirs[@]}" -type f -print
}

while IFS= read -r path; do
  case "/${path}" in
    */.codex/*|*/.codex-state-backup/*|*/auth.json|*/credentials.json|*/history.jsonl|*/sessions/*|*/.env|*/.env.*|*.key|*.pem|*.p12|*.pfx|*/id_rsa|*/id_ed25519|*.zip|*.tar|*.tar.gz|*.tgz|*.7z|*.rar)
      fail "sensitive path would enter the Docker build context: ${path}"
      ;;
  esac
done < <(emit_manifest)

if [[ "${1:-}" == '--print-manifest' ]]; then
  emit_manifest | LC_ALL=C sort -u
elif [[ -n "${1:-}" ]]; then
  fail "unknown option: ${1}"
else
  echo 'Docker build context policy is valid.'
fi
