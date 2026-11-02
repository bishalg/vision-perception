#!/usr/bin/env bash
# owner: WP-B.18 (DPL-9) - the definition of done for the clinic-server stack.
#
#   bash deploy/clinic-server/test/compose_smoke.sh
#
# Installs the real stack (install.sh --non-interactive --tls internal) into a throwaway
# compose project and state folder, then drives it the way a clinic would: console and API
# through Caddy (TLS verified against Caddy's root), a session posted on the device plane,
# a clinical note, a backup, the restore drill (PASS, and FAIL on a corrupted archive), a
# full restore with the erasure-ledger replay, upgrade.sh to the same version and
# status.sh. Tears everything down at the end (KEEP=1 keeps the stack and the folder).
#
# Runs as a normal user that may use Docker (CI runner, macOS Docker Desktop); install.sh
# is called with ASD_DEV_NON_ROOT=1, so the systemd timers are not installed here. The
# images are built by install.sh unless they already exist (asd-backend/asd-console/
# asd-backup:<version>). On macOS there is no lsblk: the disk-encryption refusal is only
# exercised on Linux.
#
# Environment: SMOKE_HTTP_PORT (18080), SMOKE_HTTPS_PORT (18443), KEEP=1.
# Needs: docker (compose plugin), curl, jq, openssl; shellcheck is used when present.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
FIXTURE="$ROOT/apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/asd-smoke.XXXXXX")"
export ASD_ENV_FILE="$WORK/.env"
export ASD_STATE_DIR="$WORK/state"
export ASD_PROJECT=asd-smoke
export ASD_DRILL_PROJECT=asd-smoke-drill
export HTTP_PORT="${SMOKE_HTTP_PORT:-18080}"
export HTTPS_PORT="${SMOKE_HTTPS_PORT:-18443}"
export ASD_DEV_NON_ROOT=1
DOMAIN=clinic.asd-smoke.test
EMAIL=owner@asd-smoke.test
PASSWORD_CLIN="smoke-clinician-$(date +%s)"
STEP=init
BODY="$WORK/body"
RUN_ID="$(date +%s)$RANDOM"

pass() { printf '   PASS  %s\n' "$*"; }
skip() { printf '   SKIPPED  %s\n' "$*"; }
fail() {
  printf '\n   FAIL  [%s] %s\n' "$STEP" "$*" >&2
  [[ -s "$BODY" ]] && { echo "   last response body:" >&2; head -c 1500 "$BODY" >&2; echo >&2; }
  exit 1
}

# shellcheck disable=SC1091 # lib.sh is linted on its own
. "$HERE/lib.sh"

cleanup() {
  local rc=$?
  if [[ -n "${KEEP:-}" ]]; then echo "KEEP set: stack $ASD_PROJECT and $WORK left running"; exit $rc; fi
  if [[ $rc -ne 0 && -f "$ASD_ENV_FILE" ]]; then
    echo "---- api/worker/caddy logs (tail)" >&2
    dc logs --tail 40 api worker caddy console 2>/dev/null | sed 's/^/   /' >&2 || true
  fi
  if [[ -f "$ASD_ENV_FILE" ]]; then
    dc --profile backup down -v --remove-orphans >/dev/null 2>&1 || true
    dc_drill --profile backup down -v --remove-orphans >/dev/null 2>&1 || true
  fi
  # Files written by the root-run backup container: remove them through a container.
  docker run --rm -v "$WORK:/w" --entrypoint /bin/sh caddy:2.8.4-alpine -c 'rm -rf /w/state' >/dev/null 2>&1 || true
  rm -rf "$WORK" 2>/dev/null || true
  exit $rc
}
trap cleanup EXIT INT TERM

for tool in docker curl jq openssl; do command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 2; }; done
docker compose version >/dev/null || { echo "docker compose plugin missing" >&2; exit 2; }

# ------------------------------------------------------------------ 1 static checks
STEP="1 static"
say "$STEP"
if command -v shellcheck >/dev/null; then
  shellcheck -x "$HERE"/*.sh "$HERE"/test/*.sh || fail "shellcheck"
  pass "shellcheck clean"
else
  skip "shellcheck not installed"
fi
CFG=$(docker compose --project-directory "$HERE" -f "$HERE/docker-compose.yml" --env-file "$HERE/.env.example" \
  --profile backup --profile tunnel config --format json) || fail "docker compose config"
published=$(jq -r '.services | to_entries[] | select((.value.ports // []) | length > 0) | .key' <<<"$CFG" | sort | tr '\n' ' ')
[[ "$published" == "caddy " ]] || fail "only caddy may publish ports; got: $published"
pass "docker compose config: only caddy publishes ports (postgres, minio, api unpublished)"
for svc in api worker; do
  jq -e --arg s "$svc" '.services[$s].environment.DATABASE_URL | startswith("postgres://asd_app:")' <<<"$CFG" >/dev/null \
    || fail "$svc does not connect as asd_app"
done
pass "api and worker configured as asd_app; migrate/backup as the owner"

# ------------------------------------------------------------------ 2 disk check (Linux)
STEP="2 encrypted-disk refusal"
say "$STEP"
if command -v lsblk >/dev/null && command -v findmnt >/dev/null; then
  if bash "$HERE/install.sh" --non-interactive --domain "$DOMAIN" --tls internal --admin-email "$EMAIL" \
      --org-name "Smoke Clinic" >"$WORK/refuse.log" 2>&1; then
    if grep -q "data disk is encrypted" "$WORK/refuse.log"; then
      skip "this runner's disk is encrypted; refusal not exercised"
      dc --profile backup down -v >/dev/null 2>&1 || true
      rm -f "$ASD_ENV_FILE"; rm -rf "$ASD_STATE_DIR"
    else
      fail "install.sh ran on an unencrypted disk without --allow-unencrypted-disk"
    fi
  else
    grep -q "NOT encrypted" "$WORK/refuse.log" || { cat "$WORK/refuse.log" >&2; fail "install failed for another reason"; }
    [[ ! -f "$ASD_ENV_FILE" ]] || fail ".env was written although the install was refused"
    pass "install.sh refuses an unencrypted disk and writes nothing"
  fi
else
  skip "no lsblk/findmnt (macOS): encrypted-disk check runs in CI only"
fi

# ------------------------------------------------------------------ 3 install
STEP="3 install"
say "$STEP"
bash "$HERE/install.sh" --non-interactive --domain "$DOMAIN" --tls internal --admin-email "$EMAIL" \
  --org-name "Smoke Clinic" --allow-unencrypted-disk 2>&1 | tee "$WORK/install.log" \
  || fail "install.sh failed"
[[ ${PIPESTATUS[0]} -eq 0 ]] || fail "install.sh failed"
card() { sed -n "s/^ $1: *//p" "$WORK/install.log" | head -n 1; }
OWNER_PW=$(card "Owner password")
BACKUP_KEY=$(card "BACKUP KEY")
[[ -n "$OWNER_PW" && "$BACKUP_KEY" == AGE-SECRET-KEY-1* && -n "$(card "DATA KEY")" ]] || fail "key card incomplete"
pass "key card printed (owner password, data key, backup key, bundle public key)"
mode=$(stat -c %a "$ASD_ENV_FILE" 2>/dev/null || stat -f %Lp "$ASD_ENV_FILE")
[[ "$mode" == 600 ]] || fail ".env mode is $mode, expected 600"
pass ".env mode 0600"
[[ ! -e "$ASD_STATE_DIR/install.pending" ]] || fail "install.pending left behind"
grep -q "$OWNER_PW" "$ASD_ENV_FILE" && fail "the owner password is in .env"
grep -q "${BACKUP_KEY#AGE-SECRET-KEY-1}" "$ASD_ENV_FILE" && fail "the backup private key is in .env"
pass "owner password and backup private key are on the key card only"
ports=$(dc ps --format json | jq -rs 'flatten | .[] | select([.Publishers[]? | select(.PublishedPort > 0)] | length > 0) | .Service' | sort -u | tr '\n' ' ')
[[ "$ports" == "caddy " ]] || fail "running containers publishing ports: $ports"
pass "running stack: only caddy has host ports"
dc logs api 2>/dev/null | grep '"starting api"' | grep -q 'postgres://asd_app:' || fail "the API log does not show user asd_app"
pass "API log line 'starting api' shows database user asd_app"
# install.sh reuses images that already exist; the backup image carries copies of the
# scripts, so a stale one would test yesterday's backup.sh / restore.sh.
BIMG="$(env_get ASD_IMAGE_PREFIX)asd-backup:$(env_get ASD_VERSION)"
for f in lib.sh backup.sh restore.sh; do
  docker run --rm --entrypoint cat "$BIMG" "/opt/asd/$f" | cmp -s - "$HERE/$f" \
    || fail "$BIMG has an old /opt/asd/$f: rebuild it (docker compose --profile backup build backup)"
done
pass "the backup image carries this checkout's backup/restore scripts"

STEP="3b install again"
sum1=$(openssl dgst -sha256 "$ASD_ENV_FILE")
bash "$HERE/install.sh" --non-interactive >"$WORK/install2.log" 2>&1 || true
grep -q "already installed" "$WORK/install2.log" || { cat "$WORK/install2.log"; fail "second install did not detect the installation"; }
[[ "$(openssl dgst -sha256 "$ASD_ENV_FILE")" == "$sum1" ]] || fail ".env changed on the second run"
grep -q "KEY CARD" "$WORK/install2.log" && fail "the key card was printed twice"
pass "install.sh again: nothing changed, no second key card"

# ------------------------------------------------------------------ 4 console through Caddy
STEP="4 console via caddy"
say "$STEP"
CA="$ASD_STATE_DIR/caddy-root.crt"
[[ -s "$CA" ]] || fail "no Caddy root certificate at $CA"
C="https://$DOMAIN:$HTTPS_PORT"
A="https://api.$DOMAIN:$HTTPS_PORT"
CURL=(curl -sS --max-time 20 --cacert "$CA" --resolve "$DOMAIN:$HTTPS_PORT:127.0.0.1" --resolve "api.$DOMAIN:$HTTPS_PORT:127.0.0.1")
"${CURL[@]}" -D "$WORK/h" -o "$BODY" "$C/login" || fail "GET /login through Caddy"
grep -qi '^HTTP/[0-9.]* 200' "$WORK/h" || fail "console /login is not 200: $(head -n1 "$WORK/h")"
grep -i '^content-security-policy:' "$WORK/h" | grep -q "'nonce-" || fail "no CSP nonce on /login"
grep -qi '^strict-transport-security:' "$WORK/h" || fail "no HSTS from the console"
pass "console /login 200 over verified TLS; content-security-policy has nonce-; HSTS present"
code=$(curl -sS -o /dev/null -w '%{http_code}' --resolve "$DOMAIN:$HTTP_PORT:127.0.0.1" "http://$DOMAIN:$HTTP_PORT/login")
[[ "$code" == 308 || "$code" == 301 ]] || fail "http:// is not redirected to https (got $code)"
pass "http -> https redirect ($code)"

# ------------------------------------------------------------------ 5 API: users, device, ingest, note
STEP="5 api flows"
say "$STEP"
req() { # req METHOD URL TOKEN [JSON] [curl args...]
  local m=$1 u=$2 t=${3:-} d=${4:-}; shift 4 2>/dev/null || shift $#
  local args=(-o "$BODY" -w '%{http_code}' -X "$m" "$u")
  [[ -n "$t" ]] && args+=(-H "Authorization: Bearer $t")
  [[ -n "$d" ]] && args+=(-H 'Content-Type: application/json' --data-binary "$d")
  CODE=$("${CURL[@]}" "${args[@]}" "$@") || fail "curl $m $u"
}
expect() { local c; for c in "$@"; do [[ "$CODE" == "$c" ]] && return 0; done; fail "expected HTTP $* got $CODE"; }
j() { jq -er "$1" "$BODY" || fail "no $1 in response"; }
login() { req POST "$A/v1/auth/login" "" "$(jq -nc --arg e "$1" --arg p "$2" '{email:$e,password:$p}')"; expect 200; j .access_token; }

OWNER=$(login "$EMAIL" "$OWNER_PW")
ORG=$(jq -er .user.org_id "$BODY")
pass "owner signed in through api.$DOMAIN (org $ORG)"
CLIN_EMAIL="clinician.$RUN_ID@asd-smoke.test"
req POST "$A/v1/orgs/$ORG/users" "$OWNER" "$(jq -nc --arg e "$CLIN_EMAIL" --arg p "$PASSWORD_CLIN" \
  '{email:$e, role:"clinician", password:$p, display_name:"Dr Smoke", professional_registration:"GMC-0000"}')"
expect 201 200
CLIN=$(login "$CLIN_EMAIL" "$PASSWORD_CLIN")
pass "owner created a clinician; clinician signed in"
req POST "$A/v1/orgs/$ORG/devices/enrol-code" "$OWNER" '{"max_uses":1,"label":"smoke"}'; expect 201 200
ENROL=$(j .code)
req POST "$A/v1/devices/register" "" "$(jq -nc --arg c "$ENROL" '{enrol_code:$c, platform:"iOS", app_version:"1.0.0"}')"; expect 201 200
DEV=$(j .device_token)
SID="SMOKE-$RUN_ID"
jq --arg s "$SID" '.sessionId=$s | .rawPayload.sessionId=$s' "$FIXTURE" >"$WORK/session.json"
req POST "$A/v1/sessions" "$DEV" "@$WORK/session.json" -H "X-Schema-Version: 1.7" -H "X-Idempotency-Key: $SID"
expect 201
pass "device enrolled and posted session $SID (schema 1.7) through Caddy"
req GET "$A/v1/sessions/$SID" "$CLIN"; expect 200
PID=$(j .participant_id)
req POST "$A/v1/participants/$PID/notes" "$CLIN" "$(jq -nc --arg s "$SID" '{body:"Smoke test note.", session_id:$s}')"
expect 201 200
pass "clinician wrote a clinical note (encrypted with the KEK) as asd_app"
# A second participant for the erasure in step 8: one with a clinical note is held, not
# erased (jobs/erasure.rs, ADR-0008 Q2).
SID2="SMOKE-E-$RUN_ID"
jq --arg s "$SID2" --arg t "child-SMOKE-ERASE" \
  '.sessionId=$s | .rawPayload.sessionId=$s | .participantToken=$t | .rawPayload.participantId=$t' \
  "$FIXTURE" >"$WORK/session2.json"
req POST "$A/v1/sessions" "$DEV" "@$WORK/session2.json" -H "X-Schema-Version: 1.7" -H "X-Idempotency-Key: $SID2"
expect 201
req GET "$A/v1/sessions/$SID2" "$CLIN"; expect 200
EPID=$(j .participant_id)
[[ "$EPID" != "$PID" ]] || fail "the second session landed on the noted participant"
pass "second participant $EPID (no clinical records) for the erasure test"

# Console sign-in through Caddy (the login server action, with a cookie jar).
STEP="5b console login"
JAR="$WORK/jar"
"${CURL[@]}" -c "$JAR" -o "$BODY" "$C/login" || fail "GET /login"
# The action id is in the inline RSC payload as the server reference {"id":"<hex>",...}
# (escaped inside the HTML); fall back to the most frequent long hex strings.
ACTION=$( { grep -o 'id[\\]*"[[:space:]]*:[[:space:]]*[\\]*"[0-9a-f]\{40,42\}' "$BODY" | grep -o '[0-9a-f]\{40,42\}$'
  grep -o '[0-9a-f]\{40,42\}' "$BODY" | sort | uniq -c | sort -rn | awk '{print $2}'; } | awk '!seen[$0]++' | head -n 5)
signed_in=""
for id in $ACTION; do
  # React's encodeReply: part 0 is the argument list (prevState, FormData); "$K1" points at
  # FormData part 1, whose fields carry the prefix "_1_". Part 0 must come LAST: Next
  # decodes the multipart body as it streams (decodeReplyFromBusboy), so "$K1" only sees
  # the fields that arrived before it.
  # shellcheck disable=SC2016 # $K1 is React's FormData reference, not a shell variable
  code=$("${CURL[@]}" -b "$JAR" -c "$JAR" -o "$BODY" -w '%{http_code}' -X POST "$C/login" \
    -H "Next-Action: $id" -H "Origin: $C" -H 'Accept: text/x-component' \
    -F "_1_email=$EMAIL" -F "_1_password=$OWNER_PW" -F '0=[{"error":null,"step":"password"},"$K1"]' || true)
  if grep -q 'curv_access' "$JAR" 2>/dev/null; then signed_in=$code; break; fi
done
if [[ -n "$signed_in" ]]; then
  code=$("${CURL[@]}" -b "$JAR" -o "$BODY" -w '%{http_code}' "$C/$ORG/overview")
  [[ "$code" == 200 ]] || fail "console /$ORG/overview with the session cookies: HTTP $code"
  # Netscape jar: field 4 is the Secure flag; HttpOnly cookies are prefixed #HttpOnly_.
  awk -F'\t' '$6 == "curv_access" && $4 == "TRUE" && $1 ~ /^#HttpOnly_/ {ok=1} END {exit !ok}' "$JAR" \
    || fail "curv_access is not a Secure, HttpOnly cookie"
  pass "console sign-in through Caddy set curv_access (Secure, HttpOnly); /$ORG/overview 200"
else
  fail "console sign-in through Caddy did not set curv_access (server action id not found or refused)"
fi

# ------------------------------------------------------------------ 6 backup
STEP="6 backup"
say "$STEP"
bash "$HERE/backup.sh" || fail "backup.sh"
ARCHIVE=$(find "$ASD_STATE_DIR/backups" -maxdepth 1 -name 'asd-*.tar.age' | sort | tail -n 1)
[[ -n "$ARCHIVE" ]] || fail "no archive written"
pass "archive $(basename "$ARCHIVE")"
[[ ! -e "$ASD_STATE_DIR/backups/LAST_FAILED" ]] || fail "LAST_FAILED present after a good backup"
if tar -tf "$ARCHIVE" >/dev/null 2>&1; then fail "the archive is readable as a plain tar"; fi
head -c 22 "$ARCHIVE" | grep -q 'age-encryption.org/v1' || fail "the archive is not an age file"
other=$(docker run --rm --entrypoint age-keygen "$BIMG" 2>/dev/null | grep '^AGE-SECRET-KEY-1')
if docker run --rm -i -v "$(dirname "$ARCHIVE"):/b:ro" --entrypoint /bin/sh "$BIMG" -c \
    "cat >/tmp/k && age -d -i /tmp/k /b/$(basename "$ARCHIVE") >/dev/null 2>&1" <<<"$other"; then
  fail "the archive opened with a different key"
fi
pass "archive is age-encrypted: not a tar, and a wrong key cannot open it"
docker run --rm -i -v "$(dirname "$ARCHIVE"):/b:ro" --entrypoint /bin/sh "$BIMG" -c \
  "cat >/tmp/k && age -d -i /tmp/k /b/$(basename "$ARCHIVE") | tar -xO manifest.json" <<<"$BACKUP_KEY" >"$WORK/manifest.json" \
  || fail "cannot open the archive with the key-card key"
sql() { dc exec -T postgres psql -qAtX -U asd -d asd -c "$1"; }
for t in participants sessions; do
  [[ "$(jq -r ".counts.$t" "$WORK/manifest.json")" == "$(sql "SELECT count(*) FROM $t")" ]] || fail "manifest $t count differs from the database"
done
[[ "$(jq -r .counts.sessions "$WORK/manifest.json")" -ge 1 ]] || fail "manifest has no sessions"
jq -e '.bucket_objects >= 0 and (.sha256 | length == 3) and .asd_version != null' "$WORK/manifest.json" >/dev/null || fail "manifest incomplete"
pass "manifest counts match the database: $(jq -c .counts "$WORK/manifest.json"), objects $(jq .bucket_objects "$WORK/manifest.json")"

# ------------------------------------------------------------------ 7 restore drill
STEP="7 restore drill"
say "$STEP"
KEYFILE="$WORK/backup-key.txt"
( umask 077; printf '%s\n' "$BACKUP_KEY" >"$KEYFILE" )
bash "$HERE/restore-drill.sh" --key-file "$KEYFILE" || { cat "$ASD_STATE_DIR"/drills/*.log >&2; fail "drill on a good archive"; }
ls "$ASD_STATE_DIR"/drills/*.PASS >/dev/null 2>&1 || fail "no drills/<date>.PASS"
grep -q "verify SKIPPED (B.22 pending" "$ASD_STATE_DIR"/drills/*.log && skip "admin verify in the drill (B.22 pending)"
pass "restore-drill.sh PASS"
BAD="$WORK/corrupt/$(basename "$ARCHIVE")"
mkdir -p "$WORK/corrupt"; cp "$ARCHIVE" "$BAD"
size=$(wc -c <"$BAD" | tr -d ' ')
printf 'CORRUPTCORRUPT' | dd of="$BAD" bs=1 seek=$((size / 2)) conv=notrunc 2>/dev/null
if bash "$HERE/restore-drill.sh" --archive "$BAD" --key-file "$KEYFILE"; then fail "drill passed on a corrupted archive"; fi
ls "$ASD_STATE_DIR"/drills/*.FAIL >/dev/null 2>&1 || fail "no drills/<date>.FAIL"
if bash "$HERE/status.sh" >"$WORK/status.log" 2>&1; then fail "status.sh exit 0 after a failed drill"; fi
grep -q "restore drill FAILED" "$WORK/status.log" || fail "status.sh does not warn about the failed drill"
pass "corrupted archive: drill FAIL, status.sh warns"
docker ps -a --format '{{.Names}}' | grep -q "^$ASD_DRILL_PROJECT" && fail "drill containers left behind"
docker volume ls --format '{{.Name}}' | grep -q "^${ASD_DRILL_PROJECT}_" && fail "drill volumes left behind"
pass "drill project torn down (no containers, no volumes)"
bash "$HERE/restore-drill.sh" --key-file "$KEYFILE" >/dev/null || fail "drill re-run"

# ------------------------------------------------------------------ 8 restore + erasure replay
STEP="8 restore"
say "$STEP"
has_requeue=$(docker run --rm --entrypoint asd-backend "$BIMG" admin --help 2>&1 | grep -cE '^[[:space:]]*requeue-erasure([[:space:]]+--|[[:space:]]*$)' || true)
req POST "$A/v1/orgs/$ORG/participants/$EPID/erasure" "$OWNER" ""; expect 202 200
for _ in $(seq 1 60); do [[ "$(sql "SELECT erased_at IS NOT NULL FROM participants WHERE id='$EPID'")" == t ]] && break; sleep 1; done
[[ "$(sql "SELECT erased_at IS NOT NULL FROM participants WHERE id='$EPID'")" == t ]] || fail "participant not erased by the worker"
pass "participant $EPID erased after the backup"
if [[ "$has_requeue" -gt 0 ]]; then bash "$HERE/backup.sh" >/dev/null || fail "second backup (ledger)"; fi
bash "$HERE/restore.sh" "$ARCHIVE" --key-file "$KEYFILE" --yes | tee "$WORK/restore.log" || fail "restore.sh"
[[ ${PIPESTATUS[0]} -eq 0 ]] || fail "restore.sh failed"
grep -q "counts match the manifest" "$WORK/restore.log" || fail "restore counts differ from the manifest"
pass "restore.sh: counts match the manifest; stack ready again"
if [[ "$has_requeue" -gt 0 ]]; then
  for _ in $(seq 1 60); do [[ "$(sql "SELECT erased_at IS NOT NULL FROM participants WHERE id='$EPID'")" == t ]] && break; sleep 1; done
  [[ "$(sql "SELECT erased_at IS NOT NULL FROM participants WHERE id='$EPID'")" == t ]] || fail "erasure not replayed after restore"
  pass "erasure ledger replayed: the participant is erased again within a minute"
else
  grep -q "SKIPPED (B.22 pending" "$WORK/restore.log" || fail "restore did not report the missing ledger replay"
  skip "erasure ledger replay (B.22 pending)"
fi
login "$EMAIL" "$OWNER_PW" >/dev/null
pass "owner signs in after the restore"

# ------------------------------------------------------------------ 9 upgrade (same version) + status
STEP="9 upgrade"
say "$STEP"
V=$(env_get ASD_VERSION)
n_before=$(find "$ASD_STATE_DIR/backups" -name 'asd-*.tar.age' | wc -l)
bash "$HERE/upgrade.sh" "$V" | tee "$WORK/upgrade.log" || fail "upgrade.sh"
[[ ${PIPESTATUS[0]} -eq 0 ]] || fail "upgrade.sh failed"
[[ $(find "$ASD_STATE_DIR/backups" -name 'asd-*.tar.age' | wc -l) -gt $n_before ]] || fail "upgrade.sh made no backup"
grep -q "the API reports version" "$WORK/upgrade.log" || fail "upgrade.sh did not print the version"
pass "upgrade.sh $V (same version): backup, migrate no-op, restart, version printed"
STEP="9b status"
bash "$HERE/status.sh" | tee "$WORK/status.log" || fail "status.sh reports a problem"
pass "status.sh: healthy, backup fresh, last drill PASS"

say "COMPOSE SMOKE PASSED"
