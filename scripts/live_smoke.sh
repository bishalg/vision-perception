#!/usr/bin/env bash
# Live end-to-end smoke test: asd-backend (serve + worker) on a fresh Postgres database,
# driven over HTTP with curl + jq, then the clinician console in live mode with a small
# Playwright spec. No mocks. Fails fast on the first broken step and exits non-zero.
#
# Usage (from anywhere):
#   scripts/live_smoke.sh
#
# Environment (all optional):
#   PG_URL          server to create the scratch DB on   (default postgres://postgres@127.0.0.1:55499)
#   DATABASE_URL    use this existing database instead of creating/dropping one
#   API_PORT        backend port                          (default 18010)
#   CONSOLE_PORT    console port                          (default 18100)
#   SKIP_BUILD=1    use the binaries already in rust/target/debug
#   SKIP_CONSOLE=1  stop after the API steps (no Next.js / Playwright)
#   KEEP_LOGS=1     keep the temp dir with server logs after a pass
#
# Needs: cargo, psql/createdb/dropdb, curl, jq, openssl, unzip, shasum, node/npx.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PG_URL="${PG_URL:-postgres://postgres@127.0.0.1:55499}"
API_PORT="${API_PORT:-18010}"
CONSOLE_PORT="${CONSOLE_PORT:-18100}"
API="http://127.0.0.1:${API_PORT}"
CONSOLE="http://127.0.0.1:${CONSOLE_PORT}"
BIN="$ROOT/rust/target/debug"
FIXTURES="$ROOT/apps/ios_asd_gaze_app/Tests/Fixtures"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/live_smoke.XXXXXX")"
RUN_ID="$(date +%s)$RANDOM"
PASSWORD="live-smoke-passphrase-${RUN_ID}"   # >= 12 chars (AUTH-1 policy)
PIDS=()
CREATED_DB=""
STEP="init"

say()  { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()   { printf '   PASS  %s\n' "$*"; }
fail() {
  printf '\n   FAIL  [%s] %s\n' "$STEP" "$*" >&2
  [[ -s "${BODY:-/dev/null}" ]] && { echo "   last response body:" >&2; head -c 2000 "$BODY" >&2; echo >&2; }
  exit 1
}

# ---- teardown (step 11): kill servers, drop the scratch DB --------------------------------
cleanup() {
  local rc=$?
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] || continue
    pkill -TERM -P "$pid" 2>/dev/null || true   # children (next dev spawns workers)
    kill -TERM "$pid" 2>/dev/null || true
  done
  sleep 1
  for pid in "${PIDS[@]:-}"; do [[ -n "$pid" ]] && kill -KILL "$pid" 2>/dev/null || true; done
  if [[ -n "$CREATED_DB" ]]; then
    dropdb --if-exists --force -d "$PG_URL/postgres" "$CREATED_DB" 2>/dev/null \
      || psql "$PG_URL/postgres" -qc "DROP DATABASE IF EXISTS \"$CREATED_DB\" WITH (FORCE)" >/dev/null 2>&1 || true
  fi
  if [[ $rc -eq 0 && -z "${KEEP_LOGS:-}" ]]; then rm -rf "$WORK"; else echo "logs kept in $WORK"; fi
  exit $rc
}
trap cleanup EXIT INT TERM

# ---- HTTP helper: req METHOD PATH TOKEN [JSON] [extra curl args...] --------------------------
# Sets CODE (HTTP status) and BODY (file holding the response body).
BODY="$WORK/body"
req() {
  local method=$1 path=$2 token=${3:-} data=${4:-}
  shift 4 2>/dev/null || shift $#
  local args=(-sS -o "$BODY" -w '%{http_code}' -X "$method" "$API$path")
  [[ -n "$token" ]] && args+=(-H "Authorization: Bearer $token")
  [[ -n "$data" ]] && args+=(-H 'Content-Type: application/json' --data-binary "$data")
  CODE=$(curl "${args[@]}" "$@") || fail "curl $method $path failed"
}
expect() { # expect CODE... (any of them)
  local c; for c in "$@"; do [[ "$CODE" == "$c" ]] && return 0; done
  fail "expected HTTP $* but got $CODE"
}
j() { jq -er "$1" "$BODY" || fail "response has no $1"; }

wait_for() { # wait_for URL SECONDS NAME
  local i
  for ((i = 0; i < $2; i++)); do
    curl -fsS -o /dev/null "$1" 2>/dev/null && return 0
    sleep 1
  done
  fail "$3 not ready at $1 after $2 s"
}

for tool in curl jq openssl unzip shasum psql createdb; do
  command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 2; }
done

# ---- build ------------------------------------------------------------------------------------
STEP="build"
if [[ -z "${SKIP_BUILD:-}" ]]; then
  say "build asd-backend + asd-cli (rust/target, may wait for the cargo lock)"
  (cd "$ROOT/rust" && CARGO_INCREMENTAL=0 SQLX_OFFLINE=true cargo build -q -p asd-backend -p asd-cli --bins) \
    || fail "cargo build failed"
fi
[[ -x "$BIN/asd-backend" && -x "$BIN/asd-cli" ]] || fail "binaries missing in $BIN"

# ---- step 1: fresh DB, migrate, serve + worker, health ----------------------------------------
STEP="1 start backend"
say "$STEP"
if [[ -z "${DATABASE_URL:-}" ]]; then
  CREATED_DB="live_smoke_${RUN_ID}"
  createdb -d "$PG_URL/postgres" "$CREATED_DB" 2>/dev/null \
    || psql "$PG_URL/postgres" -qc "CREATE DATABASE \"$CREATED_DB\"" >/dev/null \
    || fail "cannot create database $CREATED_DB on $PG_URL"
  export DATABASE_URL="$PG_URL/$CREATED_DB"
fi
export ASD_BIND_ADDR="127.0.0.1:${API_PORT}"
ASD_JWT_SIGNING_KEY="$(openssl rand -base64 32)"; export ASD_JWT_SIGNING_KEY   # base64 32-byte Ed25519 seed
ASD_DATA_KEK="$(openssl rand -base64 32)"; export ASD_DATA_KEK                 # clinical routes 503 without it
export ASD_BLOB_BACKEND="${ASD_BLOB_BACKEND:-memory}" BLOB_MODE=proxy   # s3: pass S3_*/AWS_* too
export ASD_ALLOW_SYNTHETIC=true                           # for the asd-cli synth session
export ASD_CONSOLE_ORIGIN="$CONSOLE"
export RUST_LOG="${RUST_LOG:-info,sqlx=warn}"

# `serve` never migrates (DEP-1); `migrate` is the documented init step.
"$BIN/asd-backend" migrate >"$WORK/migrate.log" 2>&1 || { cat "$WORK/migrate.log" >&2; fail "migrate failed"; }
ok "migrations applied"
"$BIN/asd-backend" serve  >"$WORK/serve.log"  2>&1 & PIDS+=($!)
# Export jobs run in the separate worker process (jobs::runner), not in the API.
"$BIN/asd-backend" worker >"$WORK/worker.log" 2>&1 & PIDS+=($!)
wait_for "$API/health/ready" 60 "asd-backend"
ok "GET /health/ready"

# ---- step 2: bootstrap org + owner via the admin CLI, log in ----------------------------------
STEP="2 bootstrap + login"
say "$STEP"
ORG=$("$BIN/asd-backend" admin create-org --name "Live Smoke Clinic $RUN_ID" --kind clinic) || fail "create-org"
OWNER_EMAIL="owner.$RUN_ID@example.org"
SUPER_EMAIL="super.$RUN_ID@example.org"
CLIN_EMAIL="clinician.$RUN_ID@example.org"
ASD_ADMIN_PASSWORD="$PASSWORD" "$BIN/asd-backend" admin create-user --email "$OWNER_EMAIL" --role owner --org "$ORG" >/dev/null \
  || fail "create-user owner"
ASD_ADMIN_PASSWORD="$PASSWORD" "$BIN/asd-backend" admin create-user --email "$SUPER_EMAIL" --role superadmin --force >/dev/null \
  || fail "create-user superadmin"
login() { # login EMAIL -> prints access token
  req POST /v1/auth/login "" "$(jq -nc --arg e "$1" --arg p "$PASSWORD" '{email:$e,password:$p}')"
  expect 200
  j .access_token
}
OWNER_TOKEN=$(login "$OWNER_EMAIL")
SUPER_TOKEN=$(login "$SUPER_EMAIL")
ok "org $ORG, owner + superadmin logged in"

# ---- step 3: owner creates a clinician; superadmin may not (security audit finding 1) -----------
STEP="3 user provisioning"
say "$STEP"
new_clinician() { jq -nc --arg e "$1" --arg p "$PASSWORD" \
  '{email:$e, role:"clinician", password:$p, display_name:"Dr Live Smoke", professional_registration:"GMC-0000"}'; }
req POST "/v1/orgs/$ORG/users" "$SUPER_TOKEN" "$(new_clinician "nope.$RUN_ID@example.org")"
expect 403
ok "superadmin creating a clinician -> 403 ($(jq -r '.type // .title // ""' "$BODY"))"
req POST "/v1/orgs/$ORG/users" "$OWNER_TOKEN" "$(new_clinician "$CLIN_EMAIL")"
expect 201 200
CLIN_TOKEN=$(login "$CLIN_EMAIL")
ok "owner created clinician $CLIN_EMAIL and it logged in"

# ---- step 4: enrol code -> device registration -> device token ---------------------------------
STEP="4 device enrolment"
say "$STEP"
req POST "/v1/orgs/$ORG/devices/enrol-code" "$OWNER_TOKEN" '{"max_uses":1,"label":"live smoke"}'
expect 201 200
ENROL_CODE=$(j .code)
req POST /v1/devices/register "" "$(jq -nc --arg c "$ENROL_CODE" '{enrol_code:$c, platform:"iOS", app_version:"1.0.0"}')"
expect 201 200
DEVICE_TOKEN=$(j .device_token)
ok "device $(j .device_id) enrolled"

# ---- step 5: ingest 1.6 + 1.7 fixtures and an asd-cli synth session; replay is idempotent -----
STEP="5 ingest"
say "$STEP"
ingest() { # ingest FILE VERSION -> sets CODE/BODY
  local sid; sid=$(jq -r .sessionId "$1")
  req POST /v1/sessions "$DEVICE_TOKEN" "@$1" -H "X-Schema-Version: $2" -H "X-Idempotency-Key: $sid"
}
fixture() { # fixture SRC VERSION OUT: unique sessionId, same participant token
  jq --arg s "$3" '.sessionId=$s | .rawPayload.sessionId=$s' "$1" >"$WORK/$3.json"
}
S16="LIVE-$RUN_ID-v16"; S17="LIVE-$RUN_ID-v17"
fixture "$FIXTURES/expected-session-v1.6.json" 1.6 "$S16"
fixture "$FIXTURES/expected-session-v1.7.json" 1.7 "$S17"
for pair in "$S16:1.6" "$S17:1.7"; do
  sid=${pair%%:*}; ver=${pair##*:}
  ingest "$WORK/$sid.json" "$ver"; expect 201
  ingest "$WORK/$sid.json" "$ver"; expect 200
  [[ "$(jq -r .duplicate "$BODY")" == "true" ]] || fail "replay of $sid did not report duplicate:true"
  ok "schema $ver session $sid: 201, replay 200 duplicate"
done
"$BIN/asd-cli" synth --seed 7 --count 1 --out - >"$WORK/synth.jsonl" || fail "asd-cli synth"
head -n1 "$WORK/synth.jsonl" >"$WORK/synth.json"
SSYN=$(jq -r .sessionId "$WORK/synth.json")
ingest "$WORK/synth.json" "$(jq -r .schemaVersion "$WORK/synth.json")"; expect 201
ok "asd-cli synth session $SSYN: 201"

# ---- step 6: reads: list, detail with tracking_quality, report PDF + FHIR for the 1.7 session ---
STEP="6 session reads"
say "$STEP"
req GET "/v1/orgs/$ORG/sessions" "$OWNER_TOKEN"; expect 200
for sid in "$S16" "$S17" "$SSYN"; do
  jq -e --arg s "$sid" '.items | any(.id == $s)' "$BODY" >/dev/null || fail "session list misses $sid"
done
ok "session list holds all 3 sessions"
req GET "/v1/sessions/$S17" "$CLIN_TOKEN"; expect 200
jq -e '.tracking_quality | type == "object"' "$BODY" >/dev/null || fail "1.7 detail has no tracking_quality object"
ok "1.7 detail has tracking_quality ($(jq -c '.tracking_quality | keys' "$BODY"))"
req GET "/v1/sessions/$S16" "$CLIN_TOKEN"; expect 200
jq -e '.tracking_quality == null' "$BODY" >/dev/null || fail "1.6 detail should have tracking_quality null"
ok "1.6 detail has tracking_quality null"
req GET "/v1/sessions/$S17/report.pdf" "$OWNER_TOKEN"; expect 200
[[ "$(head -c 4 "$BODY")" == "%PDF" ]] || fail "report.pdf does not start with %PDF"
ok "1.7 report.pdf ($(wc -c <"$BODY" | tr -d ' ') bytes, %PDF)"
req GET "/v1/sessions/$S17/fhir" "$OWNER_TOKEN"; expect 200
[[ "$(jq -r .resourceType "$BODY")" == "Bundle" ]] || fail "fhir is not a Bundle"
ok "1.7 FHIR bundle (resourceType Bundle, $(jq '.entry | length' "$BODY") entries)"
req GET "/v1/sessions/$S16/report.pdf" "$OWNER_TOKEN"; expect 200
[[ "$(head -c 4 "$BODY")" == "%PDF" ]] || fail "1.6 report.pdf does not start with %PDF"
ok "1.6 report.pdf"
req GET "/v1/sessions/$SSYN/report.pdf" "$OWNER_TOKEN"; expect 409
ok "synthetic session report.pdf refused with 409 (SIM-5)"

# ---- step 7: clinical review: worklist -> claim -> draft -> step-up sign ----------------------
STEP="7 clinical review"
say "$STEP"
req GET "/v1/orgs/$ORG/worklist" "$CLIN_TOKEN"; expect 200
jq -e --arg s "$S17" '.items | any(.id == $s)' "$BODY" >/dev/null || fail "worklist misses $S17"
ok "worklist lists $S17"
req POST "/v1/sessions/$S17/clinical-review" "$CLIN_TOKEN" '{}'; expect 201 200
REVIEW=$(j .id); ROWV=$(j .row_version)
ok "claimed review $REVIEW (state $(j .state))"
req PATCH "/v1/clinical-reviews/$REVIEW" "$CLIN_TOKEN" \
  '{"disposition":"monitor","impression":"Live smoke impression.","recommendations":"Recheck in 6 months."}' \
  -H "If-Match: \"$ROWV\""
expect 200
HASH=$(j .content_hash)
ok "draft saved (row_version $(j .row_version), content_hash $HASH)"
# No TOTP enrolled -> step-up needs only the password (clinical/reauth.rs).
req POST "/v1/clinical-reviews/$REVIEW/sign" "$CLIN_TOKEN" \
  "$(jq -nc --arg h "$HASH" '{content_hash:$h, password:"wrong-password-xyz", meaning_ack:true}')"
expect 401
ok "sign with a wrong password -> 401"
req POST "/v1/clinical-reviews/$REVIEW/sign" "$CLIN_TOKEN" \
  "$(jq -nc --arg h "$HASH" --arg p "$PASSWORD" '{content_hash:$h, password:$p, meaning_ack:true}')"
expect 200 201
[[ "$(j .state)" == "final" ]] || fail "review state is $(j .state), expected final"
[[ "$(j .current.status)" == "signed" ]] || fail "current version is $(j .current.status), expected signed"
SIG_HASH=$(j '.signatures[0].content_hash')
[[ "$SIG_HASH" =~ ^[0-9a-f]{64}$ ]] || fail "signature content_hash missing or malformed: $SIG_HASH"
ok "signed: state final, signature content_hash $SIG_HASH"

# ---- step 8: export jobs (CSV de-identified + FHIR NDJSON), worker runs them -----------------
STEP="8 exports"
say "$STEP"
# The artifact lives in the blob store. With ASD_BLOB_BACKEND=memory the worker and the API
# are separate processes with separate in-memory stores, so the API cannot serve what the
# worker uploaded (500: artifact missing; never 410, which means expired). Run with ASD_BLOB_BACKEND=s3 (+ S3_* / AWS_* for MinIO) for the
# full download + sha256 + content check; under memory the download is checked as far as
# the signed-URL handling goes and the content check is skipped with a warning.
DOWNLOADED=()
run_export() { # run_export FORMAT -> on success, the artifact is in $WORK/export.<format>
  local fmt=$1 id status i url
  req POST "/v1/orgs/$ORG/exports" "$OWNER_TOKEN" "$(jq -nc --arg f "$fmt" \
    '{format:$f, mode:"deidentified", purpose:"quality_improvement", filters:{}, reason:"Live smoke test export run"}')"
  expect 201 202 200
  id=$(j .id)
  for ((i = 0; i < 60; i++)); do
    req GET "/v1/exports/$id" "$OWNER_TOKEN"; expect 200
    status=$(j .status)
    [[ "$status" == succeeded ]] && break
    [[ "$status" == failed ]] && fail "export $fmt failed: $(jq -r .error "$BODY")"
    sleep 1
  done
  [[ "$status" == succeeded ]] || fail "export $fmt still $status after 60 s (is the worker running? see $WORK/worker.log)"
  local want rows excluded
  want=$(j .sha256); rows=$(j .row_count); excluded=$(jq -c .excluded "$BODY")
  [[ "$want" =~ ^[0-9a-f]{64}$ ]] || fail "export $fmt has no sha256"
  [[ "$rows" -ge 1 ]] || fail "export $fmt has row_count $rows"
  ok "$fmt export $id succeeded: row_count $rows, excluded $excluded"
  req POST "/v1/exports/$id/download-url" "$OWNER_TOKEN" '{}'; expect 200 201
  url=$(j .url); [[ "$url" == http* ]] || url="$API$url"
  # A tampered token is refused; the signed URL itself needs no bearer.
  CODE=$(curl -sS -o "$BODY" -w '%{http_code}' "${url%?}x"); expect 403
  CODE=$(curl -sS -o "$WORK/export.$fmt" -w '%{http_code}' "$url")
  if [[ "$CODE" == 500 && "$ASD_BLOB_BACKEND" == memory ]]; then
    printf '   WARN  %s download: 500 under ASD_BLOB_BACKEND=memory (worker blob store is not the API one); content not checked\n' "$fmt"
    return 0
  fi
  [[ "$CODE" == 200 ]] || { cp "$WORK/export.$fmt" "$BODY"; fail "download $fmt from signed URL: HTTP $CODE"; }
  local got; got=$(shasum -a 256 "$WORK/export.$fmt" | cut -d' ' -f1)
  [[ "$got" == "$want" ]] || fail "$fmt sha256 mismatch: recorded $want, downloaded $got"
  ok "$fmt downloaded via signed URL (tampered token 403), sha256 matches"
  DOWNLOADED+=("$fmt")
}
has() { [[ " ${DOWNLOADED[*]:-} " == *" $1 "* ]]; }
run_export csv
if has csv; then
  # CSV is a ZIP of the four tables + manifest.json (CSV-1).
  mkdir -p "$WORK/csv"; unzip -qo "$WORK/export.csv" -d "$WORK/csv" || fail "csv export is not a zip"
  SESS_CSV=$(find "$WORK/csv" -name 'sessions*.csv' | head -n1)
  [[ -n "$SESS_CSV" ]] || fail "no sessions*.csv in the zip: $(ls "$WORK/csv")"
  [[ $(wc -l <"$SESS_CSV") -ge 2 ]] || fail "$(basename "$SESS_CSV") has no data row"
  ok "zip holds $(cd "$WORK/csv" && echo *); $(basename "$SESS_CSV"): header + $(($(wc -l <"$SESS_CSV") - 1)) rows"
fi
run_export fhir_ndjson
if has fhir_ndjson; then
  head -n1 "$WORK/export.fhir_ndjson" | jq -e '.resourceType == "Bundle"' >/dev/null || fail "ndjson line 1 is not a Bundle"
  ok "fhir_ndjson first line is a Bundle"
fi

# ---- step 9: webhook delivery: skipped ---------------------------------------------------------
# The SSRF guard (interop/netguard.rs) refuses loopback/private targets, so a local receiver
# cannot be used without weakening production config. Covered by tests/webhook.rs instead.
say "9 webhook delivery: SKIPPED (SSRF guard refuses loopback receivers)"

# ---- step 10: console in live mode + Playwright ------------------------------------------------
if [[ -n "${SKIP_CONSOLE:-}" ]]; then
  say "10 console: SKIPPED (SKIP_CONSOLE set)"
else
  STEP="10 console live"
  say "$STEP"
  command -v npx >/dev/null || fail "npx not found"
  (cd "$ROOT/apps/curv_console" && CURV_API_MODE=live CURV_API_BASE_URL="$API" CURV_COOKIE_SECURE=false \
    NEXT_TELEMETRY_DISABLED=1 exec npx next dev --port "$CONSOLE_PORT" --hostname 127.0.0.1) \
    >"$WORK/console.log" 2>&1 & PIDS+=($!)
  wait_for "$CONSOLE/login" 180 "console"
  ok "console up on $CONSOLE"
  (cd "$ROOT/apps/curv_console" && E2E_BASE_URL="$CONSOLE" E2E_EMAIL="$CLIN_EMAIL" E2E_PASSWORD="$PASSWORD" \
    E2E_ORG_ID="$ORG" E2E_SESSION_17="$S17" E2E_SESSION_16="$S16" \
    npx playwright test --config e2e/live/playwright.live.config.ts) || fail "live Playwright spec failed"
  ok "live Playwright spec passed"
fi

STEP="done"
say "LIVE SMOKE PASSED"
