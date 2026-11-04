#!/usr/bin/env bash
# Live record-workspace + share-a-case e2e (WP-C.18 / WP-C.19 against WP-B.19 / WP-I.13): the console in live mode
# against the real asd-backend (serve + worker) on a scratch Postgres database. Runs record.live.ts and share.live.ts
# (`E2E_RECORD=1`); scripts/live_smoke.sh is unchanged and does not run them.
#
# The case-package ZIP is written by the worker and served by the API, two processes: the blob store must be shared.
# With MinIO/S3 configured (ASD_BLOB_BACKEND=s3 + S3_* / AWS_*), that is used; otherwise e2e/live/fake-s3.mjs (a
# tiny in-memory S3 stand-in, dev tooling only) is started.
#
# Usage (from anywhere):
#   PG_URL=postgres://postgres@127.0.0.1:5432 apps/curv_console/e2e/live/run-record-live.sh
#
# Environment (all optional except PG_URL):
#   PG_URL          server to create the scratch DB on (required; the DB is dropped at the end)
#   API_PORT        backend port      (default 18411)
#   CONSOLE_PORT    console port      (default 18511)
#   S3_PORT         fake S3 port      (default 18611)
#   SKIP_BUILD=1    use the binaries already in $CARGO_TARGET_DIR/debug (default rust/target/debug)
#   SERVE_ONLY=1    boot everything, print the e2e environment and wait (Ctrl-C to stop) instead of running Playwright
#   KEEP_LOGS=1     keep the temp dir with server logs after a pass
#   SUITE=research  run research.live.ts instead (researcher toolkit, phase 8): seed-research.sh adds a researcher,
#                   two superadmins, the published config, research-consented sessions and research frame logs
#                   (run-research-live.sh sets it)
#   LIVE_BIN=DIR    take asd-backend / asd-cli from DIR instead of $CARGO_TARGET_DIR/debug
set -euo pipefail

CONSOLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROOT="$(cd "$CONSOLE_DIR/../.." && pwd)"
: "${PG_URL:?set PG_URL to a Postgres server (postgres://user@host:port)}"
API_PORT="${API_PORT:-18411}"
CONSOLE_PORT="${CONSOLE_PORT:-18511}"
S3_PORT="${S3_PORT:-18611}"
API="http://127.0.0.1:${API_PORT}"
CONSOLE="http://127.0.0.1:${CONSOLE_PORT}"
BIN="${LIVE_BIN:-${CARGO_TARGET_DIR:-$ROOT/rust/target}/debug}"
SUITE="${SUITE:-record}"
FIXTURE="$ROOT/apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/record_live.XXXXXX")"
RUN_ID="$(date +%s)$RANDOM"
PASSWORD="record-live-passphrase-${RUN_ID}"
PIDS=()
CREATED_DB=""

say()  { printf '\n\033[1m== %s\033[0m\n' "$*"; }
fail() { printf '\n   FAIL  %s\n' "$*" >&2; [[ -s "${BODY:-/dev/null}" ]] && head -c 2000 "$BODY" >&2; echo >&2; exit 1; }

cleanup() {
  local rc=$?
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] || continue
    pkill -TERM -P "$pid" 2>/dev/null || true
    kill -TERM "$pid" 2>/dev/null || true
  done
  sleep 1
  for pid in "${PIDS[@]:-}"; do [[ -n "$pid" ]] && kill -KILL "$pid" 2>/dev/null || true; done
  if [[ -n "$CREATED_DB" ]]; then
    psql "$PG_URL/postgres" -qc "DROP DATABASE IF EXISTS \"$CREATED_DB\" WITH (FORCE)" >/dev/null 2>&1 || true
  fi
  if [[ $rc -eq 0 && -z "${KEEP_LOGS:-}" ]]; then rm -rf "$WORK"; else echo "logs kept in $WORK"; fi
  exit $rc
}
trap cleanup EXIT INT TERM

BODY="$WORK/body"
req() { # req METHOD PATH TOKEN [JSON] -> CODE, BODY
  local method=$1 path=$2 token=${3:-} data=${4:-}
  local args=(-sS -o "$BODY" -w '%{http_code}' -X "$method" "$API$path")
  [[ -n "$token" ]] && args+=(-H "Authorization: Bearer $token")
  [[ -n "$data" ]] && args+=(-H 'Content-Type: application/json' --data-binary "$data")
  CODE=$(curl "${args[@]}") || fail "curl $method $path"
}
expect() { local c; for c in "$@"; do [[ "$CODE" == "$c" ]] && return 0; done; fail "expected HTTP $* but got $CODE"; }
wait_for() { local i; for ((i = 0; i < $2; i++)); do curl -fsS -o /dev/null "$1" 2>/dev/null && return 0; sleep 1; done; fail "$3 not ready at $1"; }

for tool in curl jq openssl psql node npx; do command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 2; }; done

if [[ -z "${SKIP_BUILD:-}" ]]; then
  say "build asd-backend"
  (cd "$ROOT/rust" && CARGO_INCREMENTAL=0 SQLX_OFFLINE=true cargo build -q -p asd-backend --bins) || fail "cargo build"
fi
[[ -x "$BIN/asd-backend" ]] || fail "asd-backend missing in $BIN"

say "scratch database, blob store, backend (serve + worker)"
CREATED_DB="record_live_${RUN_ID}"
psql "$PG_URL/postgres" -qc "CREATE DATABASE \"$CREATED_DB\"" >/dev/null || fail "create database on $PG_URL"
export DATABASE_URL="$PG_URL/$CREATED_DB"
export ASD_BIND_ADDR="127.0.0.1:${API_PORT}"
ASD_JWT_SIGNING_KEY="$(openssl rand -base64 32)"; export ASD_JWT_SIGNING_KEY
ASD_DATA_KEK="$(openssl rand -base64 32)"; export ASD_DATA_KEK
export BLOB_MODE=proxy ASD_ALLOW_SYNTHETIC=true ASD_CONSOLE_ORIGIN="$CONSOLE" RUST_LOG="${RUST_LOG:-info,sqlx=warn}"
if [[ "${ASD_BLOB_BACKEND:-}" != s3 ]]; then
  node "$CONSOLE_DIR/e2e/live/fake-s3.mjs" "$S3_PORT" >"$WORK/s3.log" 2>&1 & PIDS+=($!)
  export ASD_BLOB_BACKEND=s3 S3_ENDPOINT_URL="http://127.0.0.1:${S3_PORT}" S3_FORCE_PATH_STYLE=true \
    S3_BUCKET=record-live AWS_REGION=us-east-1 AWS_ACCESS_KEY_ID=record-live AWS_SECRET_ACCESS_KEY=record-live \
    AWS_EC2_METADATA_DISABLED=true
fi
"$BIN/asd-backend" migrate >"$WORK/migrate.log" 2>&1 || { cat "$WORK/migrate.log" >&2; fail "migrate"; }
"$BIN/asd-backend" serve >"$WORK/serve.log" 2>&1 & PIDS+=($!)
"$BIN/asd-backend" worker >"$WORK/worker.log" 2>&1 & PIDS+=($!)
wait_for "$API/health/ready" 120 "asd-backend"

say "org, owner, clinician, device"
ORG=$("$BIN/asd-backend" admin create-org --name "Record Live $RUN_ID" --kind clinic) || fail "create-org"
OWNER_EMAIL="owner.$RUN_ID@example.org"; CLIN_EMAIL="clinician.$RUN_ID@example.org"
ASD_ADMIN_PASSWORD="$PASSWORD" "$BIN/asd-backend" admin create-user --email "$OWNER_EMAIL" --role owner --org "$ORG" >/dev/null \
  || fail "create-user owner"
req POST /v1/auth/login "" "$(jq -nc --arg e "$OWNER_EMAIL" --arg p "$PASSWORD" '{email:$e,password:$p}')"; expect 200
OWNER_TOKEN=$(jq -er .access_token "$BODY")
req POST "/v1/orgs/$ORG/users" "$OWNER_TOKEN" "$(jq -nc --arg e "$CLIN_EMAIL" --arg p "$PASSWORD" \
  '{email:$e, role:"clinician", password:$p, display_name:"Dr Record Live", professional_registration:"GMC-0001"}')"
expect 201 200
CLIN_ID=$(jq -er .id "$BODY")
req POST "/v1/orgs/$ORG/devices/enrol-code" "$OWNER_TOKEN" '{"max_uses":1,"label":"record live"}'; expect 201 200
req POST /v1/devices/register "" "$(jq -nc --arg c "$(jq -er .code "$BODY")" '{enrol_code:$c, platform:"iOS", app_version:"1.0.0"}')"
expect 201 200
DEVICE_TOKEN=$(jq -er .device_token "$BODY")

RESEARCH_ENV=()
# shellcheck source=seed-research.sh
[[ "$SUITE" == research ]] && source "$CONSOLE_DIR/e2e/live/seed-research.sh"

say "console (live, next dev) on $CONSOLE"
# `next dev` is started directly (no npm `predev`): copy the Rust core's wasm build into public/ (the workbench
# preview and the profile validation run it; without a build the script only warns).
(cd "$CONSOLE_DIR" && node scripts/prepare-asd-wasm.mjs) || fail "prepare asd-wasm"
(cd "$CONSOLE_DIR" && CURV_API_MODE=live CURV_API_BASE_URL="$API" CURV_COOKIE_SECURE=false NEXT_TELEMETRY_DISABLED=1 \
  exec npx next dev --port "$CONSOLE_PORT" --hostname 127.0.0.1) >"$WORK/console.log" 2>&1 & PIDS+=($!)
wait_for "$CONSOLE/login" 300 "console"

E2E_ENV=(E2E_RECORD=1 E2E_BASE_URL="$CONSOLE" E2E_API_URL="$API" E2E_ORG_ID="$ORG" E2E_PASSWORD="$PASSWORD"
  E2E_EMAIL="$CLIN_EMAIL" E2E_OWNER_EMAIL="$OWNER_EMAIL" E2E_CLINICIAN_ID="$CLIN_ID"
  E2E_DEVICE_TOKEN="$DEVICE_TOKEN" E2E_SESSION_FIXTURE="$FIXTURE" "${RESEARCH_ENV[@]}")
if [[ -n "${SERVE_ONLY:-}" ]]; then
  printf '%q ' "${E2E_ENV[@]}" >"$WORK/e2e.env"; echo
  say "serving; e2e environment in $WORK/e2e.env (logs in $WORK); Ctrl-C to stop"
  KEEP_LOGS=1
  wait
fi

SPECS=(record.live.ts share.live.ts)
[[ "$SUITE" == research ]] && SPECS=(research.live.ts)
say "playwright (${SPECS[*]})"
(cd "$CONSOLE_DIR" && env "${E2E_ENV[@]}" npx playwright test --config e2e/live/playwright.live.config.ts \
  "${SPECS[@]}") || fail "live Playwright specs failed (logs in $WORK)"
say "$(tr '[:lower:]' '[:upper:]' <<<"$SUITE") LIVE PASSED"
