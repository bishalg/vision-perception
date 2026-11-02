#!/usr/bin/env bash
# owner: WP-B.18 (DPL-7)
#
# Restore the clinic server from one encrypted backup archive:
#
#   sudo ./restore.sh /var/lib/asd/backups/asd-20261102T023000Z.tar.age
#   sudo ./restore.sh <archive> --key-file /media/usb/backup-key.txt
#   sudo ./restore.sh <archive> < backup-key.txt
#
# It needs the BACKUP KEY from the key card (the line that starts with AGE-SECRET-KEY-1),
# from --key-file, from stdin, or typed when asked. It also needs this server's .env to
# hold the DATA KEY (ASD_DATA_KEK) the backup was made with: on a new server, run
# install.sh first, then put the data key from the key card into .env (README "Restore").
#
# What it does: decrypts the archive, checks every part against the manifest's sha256,
# stops api/worker, replaces the database with the backup (drop + pg_restore --create),
# puts the blob store back exactly as it was, runs `migrate` (the backup may be older than
# this version), replays the erasure ledger (anyone erased after the backup is erased
# again, WP-B.22), starts the stack and prints the row counts next to the manifest.
# Anything written AFTER the backup was made is lost.
#
# `--inside restore|drill ARCHIVE` is the part that runs in the `backup` container
# (restore-drill.sh uses `drill`); do not call it yourself.
set -euo pipefail

# ------------------------------------------------------------------ inside the container
if [[ "${1:-}" == "--inside" ]]; then
  MODE=$2
  ARCHIVE=$3
  WORK=$(mktemp -d /tmp/asd-restore.XXXXXX)
  KEY=$(mktemp -p /dev/shm asd-key.XXXXXX 2>/dev/null || mktemp "$WORK/key.XXXXXX")
  trap 'rm -rf "$WORK" "$KEY"' EXIT
  umask 077
  fail() { echo "restore: FAIL: $*" >&2; exit 1; }
  # A usage line is the bare command name, optionally followed by --options (not indented).
  has_admin() { asd-backend admin --help 2>&1 | grep -qE "^[[:space:]]*$1([[:space:]]+--|[[:space:]]*\$)"; }

  cat >"$KEY"
  grep -q '^AGE-SECRET-KEY-1' "$KEY" || fail "no backup key given (expected a line starting with AGE-SECRET-KEY-1)"

  echo "restore: decrypting $(basename "$ARCHIVE")"
  if ! age -d -i "$KEY" "$ARCHIVE" | tar -x -C "$WORK"; then
    fail "cannot decrypt or unpack the archive: wrong backup key, or the file is damaged"
  fi
  rm -f "$KEY"
  [[ -s "$WORK/manifest.json" ]] || fail "the archive has no manifest.json"
  for part in db.dump blobs.tar erasure-ledger.jsonl; do
    want=$(jq -r --arg p "$part" '.sha256[$p] // empty' "$WORK/manifest.json")
    [[ -f "$WORK/$part" ]] || fail "the archive is missing $part"
    got=$(sha256sum "$WORK/$part" | cut -d' ' -f1)
    [[ "$want" == "$got" ]] || fail "$part does not match the manifest (sha256 $got, expected $want): damaged archive"
  done
  echo "restore: manifest OK (backup of $(jq -r .created_at "$WORK/manifest.json"), version $(jq -r .asd_version "$WORK/manifest.json"))"

  echo "restore: replacing the database"
  psql -v ON_ERROR_STOP=1 -qAtX -d postgres <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'asd_app') THEN
    CREATE ROLE asd_app NOLOGIN;
  END IF;
END
$$;
DROP DATABASE IF EXISTS asd WITH (FORCE);
SQL
  pg_restore --create --exit-on-error -d postgres "$WORK/db.dump" || fail "pg_restore failed"

  echo "restore: putting the blob store back"
  export MC_HOST_asd="http://${AWS_ACCESS_KEY_ID}:${AWS_SECRET_ACCESS_KEY}@minio:9000"
  tar -x -C "$WORK" -f "$WORK/blobs.tar"
  mc mirror --quiet --overwrite --remove "$WORK/blobs" "asd/${S3_BUCKET}" >/dev/null || fail "mc mirror into the bucket failed"
  objects=$(mc ls --recursive "asd/${S3_BUCKET}" | wc -l | tr -d ' ')

  counts=$(psql -qAtX -v ON_ERROR_STOP=1 -c "SELECT json_build_object(
    'participants', (SELECT count(*) FROM participants),
    'sessions', (SELECT count(*) FROM sessions),
    'audit_log', (SELECT count(*) FROM audit_log),
    'audit_log_max_ts', (SELECT to_char(max(ts) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM audit_log))")
  mismatch=0
  printf '  %-18s %12s %12s\n' "" "restored" "backup"
  for k in participants sessions audit_log; do
    a=$(jq -r ".$k" <<<"$counts"); b=$(jq -r ".counts.$k" "$WORK/manifest.json")
    printf '  %-18s %12s %12s\n' "$k" "$a" "$b"
    [[ "$a" == "$b" ]] || mismatch=1
  done
  a=$(jq -r .audit_log_max_ts <<<"$counts"); b=$(jq -r .audit_log_max_ts "$WORK/manifest.json")
  printf '  %-18s %s / %s\n' "max audit ts" "$a" "$b"
  [[ "$a" == "$b" ]] || mismatch=1
  b=$(jq -r .bucket_objects "$WORK/manifest.json")
  printf '  %-18s %12s %12s\n' "blob objects" "$objects" "$b"
  [[ "$objects" == "$b" ]] || mismatch=1
  if [[ $mismatch -ne 0 ]]; then
    [[ "$MODE" == drill ]] && fail "restored counts differ from the manifest"
    echo "restore: WARNING: restored counts differ from the manifest" >&2
  else
    echo "restore: counts match the manifest"
  fi

  echo "restore: migrate (the backup may predate this version)"
  asd-backend migrate >/dev/null || fail "asd-backend migrate failed"

  if [[ "$MODE" == drill ]]; then
    if has_admin 'verify'; then
      asd-backend admin verify || fail "asd-backend admin verify failed (exit $?): the data key in .env does not open this backup, or a table is unreadable"
      echo "restore: admin verify OK (tables readable, data key opens the org keys)"
    else
      echo "restore: admin verify SKIPPED (B.22 pending: this asd-backend has no 'admin verify')"
    fi
    echo "restore: drill PASS"
    exit 0
  fi

  if has_admin 'requeue-erasure'; then
    cat "$WORK/erasure-ledger.jsonl" /state/backups/erasure-ledger.jsonl 2>/dev/null | sort -u >"$WORK/ledger.jsonl" || true
    if [[ -s "$WORK/ledger.jsonl" ]]; then
      asd-backend admin requeue-erasure --ledger "$WORK/ledger.jsonl" || fail "erasure ledger replay failed"
      echo "restore: erasure ledger replayed ($(wc -l <"$WORK/ledger.jsonl" | tr -d ' ') entries)"
    else
      echo "restore: erasure ledger is empty; nothing to replay"
    fi
  else
    echo "restore: erasure ledger replay SKIPPED (B.22 pending: this asd-backend has no 'admin requeue-erasure')"
  fi
  exit 0
fi

# ------------------------------------------------------------------ on the server
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

usage() {
  echo "usage: sudo $0 <archive.tar.age> [--key-file FILE] [--yes]"
  echo "The backup key (AGE-SECRET-KEY-1...) comes from --key-file, stdin, or is typed when asked."
}
ARCHIVE=""; KEY_FILE=""; YES=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --key-file) KEY_FILE=${2:-}; shift 2 ;;
    --yes) YES=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) usage >&2; exit 2 ;;
    *) ARCHIVE=$1; shift ;;
  esac
done
[[ -n "$ARCHIVE" ]] || { usage >&2; exit 2; }
require_root
require_env_file
require_docker

# Trap 19: the KEK is never in the backup. Without it the clinical text cannot be read.
[[ -n "$(env_get ASD_DATA_KEK)" ]] || die "ASD_DATA_KEK (the DATA KEY) is not set in $ASD_ENV_FILE." \
  "Backups never contain the data key, on purpose: it is on the key card." \
  "Add the line  ASD_DATA_KEK=<data key from the key card>  to $ASD_ENV_FILE, then run this again."
[[ -f "$ARCHIVE" ]] || die "no such archive: $ARCHIVE" "Backups are in $(state_dir)/backups/"
ARCHIVE=$(cd "$(dirname "$ARCHIVE")" && pwd)/$(basename "$ARCHIVE")

KEY_TEXT=""
if [[ -n "$KEY_FILE" ]]; then
  [[ -r "$KEY_FILE" ]] || die "cannot read the key file $KEY_FILE"
  KEY_TEXT=$(grep -m1 '^AGE-SECRET-KEY-1' "$KEY_FILE" || true)
elif [[ ! -t 0 ]]; then
  KEY_TEXT=$(grep -m1 '^AGE-SECRET-KEY-1' || true)
else
  printf 'Paste the BACKUP KEY from the key card (starts with AGE-SECRET-KEY-1), then press Enter:\n> '
  read -r -s KEY_TEXT
  echo
fi
[[ "$KEY_TEXT" == AGE-SECRET-KEY-1* ]] || die "that is not a backup key." \
  "The backup key is the line on the key card that starts with AGE-SECRET-KEY-1."

say "Restore from $(basename "$ARCHIVE")"
cat <<EOF
    This REPLACES ALL DATA on this server with the backup.
    Anything recorded after the backup was made is LOST.
    People erased after the backup are erased again automatically.
EOF
if [[ -z "$YES" ]]; then
  [[ -t 0 ]] || die "not confirmed. Add --yes when the key comes from stdin."
  read -r -p "    Type RESTORE to continue: " answer
  [[ "$answer" == RESTORE ]] || die "cancelled; nothing was changed."
fi

say "Stopping the API and the worker"
dc stop api worker >/dev/null
dc up -d --wait postgres minio >/dev/null
dc run --rm minio-init >/dev/null

say "Restoring (decrypt, check, database, blobs, migrate, erasure ledger)"
if ! printf '%s\n' "$KEY_TEXT" | dc --profile backup run --rm -T \
    -v "$(dirname "$ARCHIVE"):/restore-src:ro" backup \
    /opt/asd/restore.sh --inside restore "/restore-src/$(basename "$ARCHIVE")"; then
  die "the restore FAILED (details above). The API and worker are stopped." \
    "If the archive is damaged, try the previous night's:  ls $(state_dir)/backups/" \
    "To start the server again on the current data:      sudo $ASD_DIR/install.sh"
fi
KEY_TEXT=""

say "Starting the stack"
dc up -d >/dev/null
wait_ready 180 || die "the API did not become ready after the restore." "Look at:  sudo $ASD_DIR/status.sh"
ok "restore finished; the server is running version $(backend_version)"
