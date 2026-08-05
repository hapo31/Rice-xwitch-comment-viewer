#!/usr/bin/env bash
set -euo pipefail

CODEX_HOME="${CODEX_HOME:-${HOME}/.codex}"
DEFAULT_STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
if [[ -n "${CODEX_BACKUP_ZIP:-}" ]]; then
  BACKUP_ZIP="${CODEX_BACKUP_ZIP}"
  BACKUP_DIR="$(dirname "${BACKUP_ZIP}")"
else
  BACKUP_DIR="${CODEX_BACKUP_DIR:-${DEFAULT_STATE_HOME}/rice-xwitch-comment-viewer}"
  BACKUP_ZIP="${BACKUP_DIR}/codex-state-backup.zip"
fi

usage() {
  cat <<'USAGE'
Usage:
  .devcontainer/codex-state-transfer.sh backup
  .devcontainer/codex-state-transfer.sh restore

Environment:
  CODEX_HOME        Source/restore directory. Default: $HOME/.codex
  CODEX_BACKUP_DIR  Backup directory. Default: $XDG_STATE_HOME/rice-xwitch-comment-viewer
  CODEX_BACKUP_ZIP  Backup zip path. Default: $CODEX_BACKUP_DIR/codex-state-backup.zip
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
