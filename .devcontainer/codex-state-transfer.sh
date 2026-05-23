#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CODEX_HOME="${CODEX_HOME:-${HOME}/.codex}"
BACKUP_DIR="${CODEX_BACKUP_DIR:-${REPO_ROOT}/.codex-state-backup}"
BACKUP_ZIP="${CODEX_BACKUP_ZIP:-${BACKUP_DIR}/codex-state-backup.zip}"

usage() {
  cat <<'USAGE'
Usage:
  .devcontainer/codex-state-transfer.sh backup
  .devcontainer/codex-state-transfer.sh restore

Environment:
  CODEX_HOME        Source/restore directory. Default: $HOME/.codex
  CODEX_BACKUP_ZIP  Backup zip path. Default: .codex-state-backup/codex-state-backup.zip
USAGE
}

backup() {
  if [[ ! -d "${CODEX_HOME}" ]]; then
    echo "Codex home not found: ${CODEX_HOME}" >&2
    exit 1
  fi

  mkdir -p "${BACKUP_DIR}"
  chmod 700 "${BACKUP_DIR}" 2>/dev/null || true

  local tmp_zip
  tmp_zip="$(mktemp "${BACKUP_DIR}/codex-state-backup.XXXXXX.zip")"
  rm -f "${tmp_zip}"

  (
    cd "${CODEX_HOME}"
    zip -r -q "${tmp_zip}" .
  )

  mv "${tmp_zip}" "${BACKUP_ZIP}"
  chmod 600 "${BACKUP_ZIP}" 2>/dev/null || true
  echo "Backed up ${CODEX_HOME} to ${BACKUP_ZIP}"
}

restore() {
  if [[ ! -f "${BACKUP_ZIP}" ]]; then
    echo "Backup zip not found: ${BACKUP_ZIP}" >&2
    exit 1
  fi

  mkdir -p "${CODEX_HOME}"
  chmod 700 "${CODEX_HOME}"
  unzip -o -q "${BACKUP_ZIP}" -d "${CODEX_HOME}"
  chmod -R go-rwx "${CODEX_HOME}"
  echo "Restored ${BACKUP_ZIP} to ${CODEX_HOME}"
}

case "${1:-}" in
  backup)
    backup
    ;;
  restore)
    restore
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
