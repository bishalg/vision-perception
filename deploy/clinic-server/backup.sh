#!/usr/bin/env bash
# owner: WP-B.18 (DPL-6)
#
# Nightly encrypted backup of the clinic server. The systemd timer (asd-backup.timer,
# 02:30) runs it; you can also run it by hand at any time:
#
#   sudo ./backup.sh
#
# Writes <state>/backups/asd-<UTC timestamp>.tar.age (default state: /var/lib/asd), an
# `age`-encrypted tar of:
#   db.dump               pg_dump -Fc of the `asd` database (schema owner role)
#   blobs.tar             every object of the MinIO bucket (session files, PDFs, exports)
#   erasure-ledger.jsonl  who was erased and when (replayed by restore.sh; WP-B.22)
#   manifest.json         time, ASD_VERSION, row counts, max(audit_log.ts), object count,
#                         sha256 of each part
# Counts and the dump come from one database snapshot, so a restore must match them
# exactly. Only the PUBLIC backup key is on this server: the archive cannot be read here
# without the private key from the key card. `.env` (the secrets) is NOT in the backup on
# purpose; the key card is the backup of the keys.
# Keeps BACKUP_KEEP_DAILY (14) newest archives plus one per week for BACKUP_KEEP_WEEKLY (8)
# weeks; copies new archives to BACKUP_REMOTE with rclone when set. A failure writes
# <state>/backups/LAST_FAILED, which status.sh shows.
#
# `--inside` is the part that runs in the `backup` container; do not call it yourself.
set -euo pipefail

# ------------------------------------------------------------------ inside the container
if [[ "${1:-}" == "--inside" ]]; then
  BK=/state/backups
  mkdir -p "$BK"
  chmod 700 "$BK"
  TS=$(date -u +%Y%m%dT%H%M%SZ)
  WORK=$(mktemp -d /tmp/asd-backup.XXXXXX)
  STEP=start
  # shellcheck disable=SC2329 # called by the EXIT trap
  finish() {
    local rc=$?
    rm -rf "$WORK"
    rm -f "$BK"/*.partial
    if [[ $rc -ne 0 ]]; then
      printf '%s failed at step: %s\n' "$TS" "$STEP" >"$BK/LAST_FAILED"
      echo "backup: FAILED at step '$STEP'" >&2
    fi
    exit "$rc"
  }
  trap finish EXIT
  umask 077

  STEP="database dump"
  echo "backup: database snapshot, counts and pg_dump"
  psql -v ON_ERROR_STOP=1 -qAtX <<SQL
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_export_snapshot() AS snap \gset
\setenv ASD_SNAPSHOT :snap
\o $WORK/counts.json
SELECT json_build_object(
  'participants', (SELECT count(*) FROM participants),
  'sessions', (SELECT count(*) FROM sessions),
  'audit_log', (SELECT count(*) FROM audit_log),
  'audit_log_max_ts', (SELECT to_char(max(ts) AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"') FROM audit_log));
\o
\! pg_dump -Fc --snapshot="\$ASD_SNAPSHOT" -f $WORK/db.dump || touch $WORK/dump.failed
COMMIT;
SQL
  [[ ! -e "$WORK/dump.failed" && -s "$WORK/db.dump" ]] || { echo "backup: pg_dump failed" >&2; exit 1; }

  STEP="blob store copy"
  echo "backup: copying the blob store"
  export MC_HOST_asd="http://${AWS_ACCESS_KEY_ID}:${AWS_SECRET_ACCESS_KEY}@minio:9000"
  mkdir -p "$WORK/blobs"
  mc mirror --quiet --overwrite "asd/${S3_BUCKET}" "$WORK/blobs" >/dev/null
  OBJECTS=$(find "$WORK/blobs" -type f | wc -l | tr -d ' ')
  tar -C "$WORK" -cf "$WORK/blobs.tar" blobs
  rm -rf "$WORK/blobs"

  STEP="erasure ledger"
  # A command line of `admin --help` is the bare name, optionally followed by --options
  # (the usage text is not indented; prose lines that start with a name are not matched).
  if asd-backend admin --help 2>&1 | grep -qE '^[[:space:]]*erasure-ledger([[:space:]]+--|[[:space:]]*$)'; then
    asd-backend admin erasure-ledger >"$WORK/erasure-ledger.jsonl"
    # The cumulative ledger outside the archives: restore.sh replays it on top of an
    # older archive so erasures made after that night are not undone.
    touch "$BK/erasure-ledger.jsonl"
    sort -u "$BK/erasure-ledger.jsonl" "$WORK/erasure-ledger.jsonl" >"$BK/erasure-ledger.jsonl.partial"
    mv "$BK/erasure-ledger.jsonl.partial" "$BK/erasure-ledger.jsonl"
    echo "backup: erasure ledger $(wc -l <"$WORK/erasure-ledger.jsonl" | tr -d ' ') rows"
  else
    : >"$WORK/erasure-ledger.jsonl"
    echo "backup: erasure ledger SKIPPED (B.22 pending: this asd-backend has no 'admin erasure-ledger')"
  fi

  STEP="manifest"
  sha() { sha256sum "$WORK/$1" | cut -d' ' -f1; }
  jq -n \
    --arg created_at "$TS" \
    --arg asd_version "${ASD_VERSION:-unknown}" \
    --argjson counts "$(cat "$WORK/counts.json")" \
    --argjson objects "$OBJECTS" \
    --arg db "$(sha db.dump)" --arg blobs "$(sha blobs.tar)" --arg ledger "$(sha erasure-ledger.jsonl)" \
    '{format: 1, created_at: $created_at, asd_version: $asd_version,
      counts: {participants: $counts.participants, sessions: $counts.sessions, audit_log: $counts.audit_log},
      audit_log_max_ts: $counts.audit_log_max_ts, bucket_objects: $objects,
      sha256: {"db.dump": $db, "blobs.tar": $blobs, "erasure-ledger.jsonl": $ledger}}' \
    >"$WORK/manifest.json"

  STEP="encrypt"
  OUT="$BK/asd-$TS.tar.age"
  tar -C "$WORK" -cf - manifest.json db.dump blobs.tar erasure-ledger.jsonl \
    | age -r "$BACKUP_AGE_RECIPIENT" -o "$OUT.partial"
  mv "$OUT.partial" "$OUT"
  chmod 600 "$OUT"
  echo "backup: wrote $(basename "$OUT") ($(du -h "$OUT" | cut -f1)); counts $(jq -c .counts "$WORK/manifest.json"), objects $OBJECTS"

  STEP="retention"
  mapfile -t ALL < <(find "$BK" -maxdepth 1 -name 'asd-*.tar.age' -printf '%f\n' | sort -r)
  declare -A WEEKS=()
  kept=0
  for f in "${ALL[@]}"; do
    if ((kept < ${BACKUP_KEEP_DAILY:-14})); then kept=$((kept + 1)); continue; fi
    d=${f#asd-}; d=${d:0:8}
    wk=$(date -u -d "${d:0:4}-${d:4:2}-${d:6:2}" +%G-%V 2>/dev/null || echo "?")
    if [[ -z "${WEEKS[$wk]:-}" && ${#WEEKS[@]} -lt ${BACKUP_KEEP_WEEKLY:-8} ]]; then
      WEEKS[$wk]=1
      continue
    fi
    rm -f "$BK/$f"
    echo "backup: removed old $f"
  done

  if [[ -n "${BACKUP_REMOTE:-}" ]]; then
    STEP="off-site copy"
    rclone copy --config "${RCLONE_CONFIG:-/state/rclone.conf}" "$OUT" "$BACKUP_REMOTE"
    echo "backup: copied to $BACKUP_REMOTE (encrypted bytes only)"
  fi

  rm -f "$BK/LAST_FAILED"
  printf '%s\n' "$TS" >"$BK/LAST_OK"
  STEP=finished
  exit 0
fi

# ------------------------------------------------------------------ on the server
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
require_root
require_env_file
require_docker
STATE=$(state_dir)
mkdir -p "$STATE/backups"
chmod 700 "$STATE" "$STATE/backups"

say "Backup of the clinic server"
if dc --profile backup run --rm -T backup /opt/asd/backup.sh --inside; then
  ok "backup finished: $(find "$STATE/backups" -maxdepth 1 -name 'asd-*.tar.age' | sort | tail -n 1)"
else
  rc=$?
  [[ -f "$STATE/backups/LAST_FAILED" ]] \
    || printf '%s failed before the backup container ran (exit %s)\n' "$(date -u +%Y%m%dT%H%M%SZ)" "$rc" >"$STATE/backups/LAST_FAILED"
  die "the backup FAILED (details above; also in $STATE/backups/LAST_FAILED)." \
    "Check that the stack is running:  sudo $ASD_DIR/status.sh" \
    "Then try again:                   sudo $ASD_DIR/backup.sh"
fi
