#!/usr/bin/env bash
# owner: WP-B.18 (DPL-8, HIP-7)
#
# Monthly restore test (asd-drill.timer, 03:30 on the first Sunday). Proves the newest
# backup can actually be restored, without touching the live server:
#
#   sudo ./restore-drill.sh                       newest archive, key from BACKUP_KEY_FILE
#   sudo ./restore-drill.sh --archive FILE --key-file FILE
#
# Restores into a separate scratch compose project (`asd-drill`: its own database and blob
# volumes, no published ports, no Caddy), checks the participants / sessions / audit_log
# counts and max(audit_log.ts) against the backup manifest, runs `asd-backend admin
# verify` (every table readable; the data key in .env opens the org keys, i.e. the key
# card matches), then deletes the scratch project. The result is written to
# <state>/drills/<date>.PASS or .FAIL with the log next to it; status.sh shows it.
#
# The drill needs the BACKUP KEY (private) on this server to run unattended: save the
# AGE-SECRET-KEY-1 line from the key card in the file named by BACKUP_KEY_FILE in .env
# (default /etc/asd/backup-key.txt, owner root, mode 0600). README.md "Restore drill"
# explains the trade-off. Without it the drill records FAIL and says why.
set -euo pipefail
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ARCHIVE=""; KEY_FILE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --archive) ARCHIVE=${2:-}; shift 2 ;;
    --key-file) KEY_FILE=${2:-}; shift 2 ;;
    -h|--help) echo "usage: sudo $0 [--archive FILE] [--key-file FILE]"; exit 0 ;;
    *) echo "usage: sudo $0 [--archive FILE] [--key-file FILE]" >&2; exit 2 ;;
  esac
done
require_root
require_env_file
require_docker

STATE=$(state_dir)
DRILLS="$STATE/drills"
mkdir -p "$DRILLS"
chmod 700 "$DRILLS"
DAY=$(date -u +%Y-%m-%d)
LOG="$DRILLS/$DAY.log"
KEY_FILE=${KEY_FILE:-$(env_get BACKUP_KEY_FILE)}
KEY_FILE=${KEY_FILE:-/etc/asd/backup-key.txt}

cleanup_drill() { dc_drill --profile backup down -v --remove-orphans >/dev/null 2>&1 || true; }

result() { # result PASS|FAIL MESSAGE
  rm -f "$DRILLS/$DAY.PASS" "$DRILLS/$DAY.FAIL"
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$2" >"$DRILLS/$DAY.$1"
  if [[ "$1" == PASS ]]; then ok "restore drill PASS: $2"; else
    printf '    FAIL  restore drill: %s (log: %s)\n' "$2" "$LOG" >&2
  fi
}

run_drill() {
  if [[ -z "$ARCHIVE" ]]; then
    ARCHIVE=$(find "$STATE/backups" -maxdepth 1 -name 'asd-*.tar.age' 2>/dev/null | sort | tail -n 1)
    [[ -n "$ARCHIVE" ]] || { echo "no backup archive in $STATE/backups"; return 1; }
  fi
  [[ -f "$ARCHIVE" ]] || { echo "no such archive: $ARCHIVE"; return 1; }
  [[ -r "$KEY_FILE" ]] || {
    echo "no backup key at $KEY_FILE: save the AGE-SECRET-KEY-1 line of the key card there (README 'Restore drill')"
    return 1
  }
  ARCHIVE=$(cd "$(dirname "$ARCHIVE")" && pwd)/$(basename "$ARCHIVE")
  echo "drill: archive $(basename "$ARCHIVE") into scratch project $ASD_DRILL_PROJECT"
  cleanup_drill
  dc_drill up -d --wait postgres minio
  dc_drill run --rm minio-init
  grep -m1 '^AGE-SECRET-KEY-1' "$KEY_FILE" | dc_drill --profile backup run --rm -T \
    -v "$(dirname "$ARCHIVE"):/restore-src:ro" backup \
    /opt/asd/restore.sh --inside drill "/restore-src/$(basename "$ARCHIVE")"
}

say "Restore drill ($DAY)"
set +e
run_drill >"$LOG" 2>&1
rc=$?
set -e
cleanup_drill
chmod 600 "$LOG"
grep -E '^restore:|^  |^drill:|^no ' "$LOG" || true
if [[ $rc -eq 0 ]]; then
  result PASS "$(basename "$ARCHIVE") restored; counts match the manifest"
  exit 0
fi
result FAIL "$(grep -E 'FAIL|no backup|no such' "$LOG" | tail -n 1 | sed 's/^restore: //')"
exit 1
