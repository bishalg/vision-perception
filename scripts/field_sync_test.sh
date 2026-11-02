#!/usr/bin/env bash
# SYNC-10 field harness (stack side): the compose stack behind toxiproxy, hammered the way a
# clinic iPad on a bad link would. Opt-in: does nothing unless ASD_FIELD_TEST=1.
#
# Usage: ASD_FIELD_TEST=1 scripts/field_sync_test.sh
#
# What it does, on a throwaway compose project (own volumes, nothing published but the proxy):
#   1. builds and starts rust/docker-compose.yml (postgres, minio, migrate, api, worker) plus
#      toxiproxy (ghcr.io/shopify/toxiproxy) in front of `api`;
#   2. bootstraps an org and owner (admin CLI), gets an enrol code, registers a device;
#   3. adds toxics: latency 2000 +/- 1500 ms, bandwidth 64 KB/s both ways, `timeout` on 20 %
#      of connections, `slicer`;
#   4. posts 50 `asd-cli synth` sessions, each 3 times concurrently with the device headers
#      (bearer, X-Idempotency-Key, X-Schema-Version, User-Agent), retrying like the app, then
#      PUTs a 32 KiB blob per session in proxy mode with `Content-Digest` (RFC 9530);
#   5. passes when `sessions` has exactly 50 rows, every `frames_blob_key` is set,
#      `audit_log` has 50 `session.ingest` rows and the API logged no 5xx.
#
# Environment (optional):
#   FIELD_PROXY_PORT     host port of the proxied API (default 18474)
#   FIELD_TOXI_API_PORT  host port of the toxiproxy API (default 18475)
#   FIELD_SESSIONS       sessions to post (default 50)
#   FIELD_PARALLEL       sessions in flight at once (default 10)
#   KEEP_STACK=1         leave the project running (and its volumes) after the run
#
# Needs: docker with the compose plugin (or docker-compose), curl, jq, openssl.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SELF="$ROOT/scripts/field_sync_test.sh"

# ---- one session, run by xargs in a child process: --session-job INDEX ----------------------
if [[ "${1:-}" == "--session-job" ]]; then
  i=$2
  file="$FIELD_WORK/session_$i.json"
  blob="$FIELD_WORK/blob_$i.bin"
  sid=$(jq -r .sessionId "$file")
  ver=$(jq -r .schemaVersion "$file")
  ua="EyeGaze-iOS/1.0.0 (1; iOS 18.0)"
  post_once() { # post_once COPY: retries with the app's backoff until the server accepts
    local copy=$1 attempt code delay
    for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
      code=$(curl -sS -o "$FIELD_WORK/post_${i}_$copy.body" -w '%{http_code}' --max-time 30 \
        -X POST "$FIELD_API/v1/sessions" \
        -H "Authorization: Bearer $FIELD_DEVICE_TOKEN" \
        -H "X-Idempotency-Key: $sid" -H "X-Schema-Version: $ver" -H "User-Agent: $ua" \
        -H 'Content-Type: application/json' -H 'Accept: application/json' \
        --data-binary "@$file" 2>/dev/null) || code=000
      echo "POST $sid copy=$copy attempt=$attempt status=$code" >>"$FIELD_WORK/requests_$i.log"
      case "$code" in
        200|201) return 0 ;;
        409) return 0 ;;   # an older server's replay answer
        4??) [[ "$code" == 408 || "$code" == 429 ]] || return 1 ;;
      esac
      delay=$((attempt < 4 ? 1 << (attempt - 1) : 8))
      sleep "$delay"
    done
    return 1
  }
  post_once 1 & p1=$!
  post_once 2 & p2=$!
  post_once 3 & p3=$!
  ok=0
  for p in $p1 $p2 $p3; do wait "$p" && ok=1; done
  [[ $ok -eq 1 ]] || { echo "FAIL $sid: no POST accepted" >>"$FIELD_WORK/failures.log"; exit 1; }

  body=""
  for c in 1 2 3; do
    candidate="$FIELD_WORK/post_${i}_$c.body"
    if [[ -s "$candidate" ]] && jq -e '.frames_upload_url | type == "string"' "$candidate" >/dev/null 2>&1; then
      body=$candidate; break
    fi
  done
  [[ -n "$body" ]] || { echo "FAIL $sid: no frames_upload_url" >>"$FIELD_WORK/failures.log"; exit 1; }
  url=$(jq -r .frames_upload_url "$body")
  [[ "$url" == http* ]] || url="$FIELD_API$url"
  digest="sha-256=:$(openssl dgst -sha256 -binary "$blob" | base64 | tr -d '\n'):"
  for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
    code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 60 -X PUT "$url" \
      -H "Authorization: Bearer $FIELD_DEVICE_TOKEN" -H "User-Agent: $ua" \
      -H 'Content-Type: application/octet-stream' -H "Content-Digest: $digest" \
      --data-binary "@$blob" 2>/dev/null) || code=000
    echo "PUT $sid attempt=$attempt status=$code" >>"$FIELD_WORK/requests_$i.log"
    case "$code" in
      200|201|204) exit 0 ;;
      4??) [[ "$code" == 408 || "$code" == 429 ]] || { echo "FAIL $sid: PUT $code" >>"$FIELD_WORK/failures.log"; exit 1; } ;;
    esac
    sleep $((attempt < 4 ? 1 << (attempt - 1) : 8))
  done
  echo "FAIL $sid: PUT never accepted" >>"$FIELD_WORK/failures.log"
  exit 1
fi

if [[ "${ASD_FIELD_TEST:-}" != "1" ]]; then
  echo "field_sync_test.sh: skipped (set ASD_FIELD_TEST=1 to run the SYNC-10 field harness)"
  exit 0
fi

for tool in docker curl jq openssl; do
  command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 2; }
done
if docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null; then
  COMPOSE=(docker-compose)
else
  echo "docker compose is not available" >&2; exit 2
fi
docker info >/dev/null 2>&1 || { echo "the docker daemon is not reachable" >&2; exit 2; }

RUN_ID="$(date +%s)$RANDOM"
PROJECT="asd-field-$RUN_ID"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/field_sync.XXXXXX")"
PROXY_PORT="${FIELD_PROXY_PORT:-18474}"
TOXI_PORT="${FIELD_TOXI_API_PORT:-18475}"
COUNT="${FIELD_SESSIONS:-50}"
PARALLEL="${FIELD_PARALLEL:-10}"
API="http://127.0.0.1:$PROXY_PORT"
TOXI="http://127.0.0.1:$TOXI_PORT"
PASSWORD="field-sync-passphrase-$RUN_ID"
STEP="init"

say()  { printf '\n\033[1m== %s\033[0m\n' "$*"; }
ok()   { printf '   PASS  %s\n' "$*"; }
fail() { printf '\n   FAIL  [%s] %s\n' "$STEP" "$*" >&2; exit 1; }

# A throwaway project: no host ports but the proxy's, keys for login and clinical routes,
# synthetic sessions allowed, toxiproxy on the compose network.
export FIELD_JWT_KEY FIELD_DATA_KEK
FIELD_JWT_KEY="$(openssl rand -base64 32)"
FIELD_DATA_KEK="$(openssl rand -base64 32)"
cat >"$WORK/override.yml" <<YAML
services:
  postgres:
    ports: !reset []
  minio:
    ports: !reset []
  migrate:
    environment:
      ASD_JWT_SIGNING_KEY: \${FIELD_JWT_KEY}
      ASD_DATA_KEK: \${FIELD_DATA_KEK}
  api:
    ports: !reset []
    environment:
      ASD_JWT_SIGNING_KEY: \${FIELD_JWT_KEY}
      ASD_DATA_KEK: \${FIELD_DATA_KEK}
      ASD_ALLOW_SYNTHETIC: "true"
      BLOB_MODE: proxy
  worker:
    environment:
      ASD_JWT_SIGNING_KEY: \${FIELD_JWT_KEY}
      ASD_DATA_KEK: \${FIELD_DATA_KEK}
  toxiproxy:
    image: ghcr.io/shopify/toxiproxy:2.9.0
    depends_on:
      - api
    ports:
      - "127.0.0.1:$PROXY_PORT:18000"
      - "127.0.0.1:$TOXI_PORT:8474"
YAML
DC=("${COMPOSE[@]}" -p "$PROJECT" -f "$ROOT/rust/docker-compose.yml" -f "$WORK/override.yml")

cleanup() {
  local rc=$?
  if [[ $rc -ne 0 ]]; then
    "${DC[@]}" logs --no-color api >"$WORK/api.log" 2>&1 || true
    echo "logs and request traces kept in $WORK"
  fi
  if [[ -z "${KEEP_STACK:-}" ]]; then
    "${DC[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
  else
    echo "stack kept: ${DC[*]}"
  fi
  [[ $rc -eq 0 && -z "${KEEP_STACK:-}" ]] && rm -rf "$WORK"
  exit $rc
}
trap cleanup EXIT INT TERM

# ---- 1. stack + proxy ---------------------------------------------------------------------------
STEP="1 stack"
say "$STEP ($PROJECT)"
"${DC[@]}" up -d --build postgres minio minio-init migrate api worker toxiproxy >"$WORK/up.log" 2>&1 \
  || { cat "$WORK/up.log" >&2; fail "compose up failed"; }
for _ in $(seq 1 60); do curl -fsS -o /dev/null "$TOXI/version" 2>/dev/null && break; sleep 2; done
curl -fsS -o /dev/null "$TOXI/version" || fail "toxiproxy API not reachable on $TOXI"
curl -fsS -o /dev/null -X POST "$TOXI/proxies" \
  -d '{"name":"api","listen":"0.0.0.0:18000","upstream":"api:8000","enabled":true}' \
  || fail "cannot create the toxiproxy proxy"
for _ in $(seq 1 90); do curl -fsS -o /dev/null "$API/health/ready" 2>/dev/null && break; sleep 2; done
curl -fsS -o /dev/null "$API/health/ready" || fail "API not ready behind the proxy"
ok "stack up; API ready through toxiproxy at $API"

# ---- 2. org, owner, enrol code, device ----------------------------------------------------------
STEP="2 enrolment"
say "$STEP"
ORG=$("${DC[@]}" exec -T api /usr/local/bin/asd-backend admin create-org --name "Field $RUN_ID" --kind clinic) \
  || fail "create-org"
ORG=$(echo "$ORG" | tail -n1 | tr -d '\r')
OWNER="owner.$RUN_ID@example.org"
"${DC[@]}" exec -T -e ASD_ADMIN_PASSWORD="$PASSWORD" api /usr/local/bin/asd-backend \
  admin create-user --email "$OWNER" --role owner --org "$ORG" >/dev/null || fail "create-user"
TOKEN=$(curl -fsS -X POST "$API/v1/auth/login" -H 'Content-Type: application/json' \
  -d "$(jq -nc --arg e "$OWNER" --arg p "$PASSWORD" '{email:$e,password:$p}')" | jq -er .access_token) \
  || fail "owner login"
CODE=$(curl -fsS -X POST "$API/v1/orgs/$ORG/devices/enrol-code" -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"max_uses":1,"label":"field sync"}' | jq -er .code) \
  || fail "enrol code"
DEVICE_TOKEN=$(curl -fsS -X POST "$API/v1/devices/register" -H 'Content-Type: application/json' \
  -d "$(jq -nc --arg c "$CODE" '{enrol_code:$c, platform:"iOS", app_version:"1.0.0"}')" | jq -er .device_token) \
  || fail "device registration"
ok "org $ORG, device enrolled"

# ---- 3. sessions and blobs ----------------------------------------------------------------------
STEP="3 payloads"
"${DC[@]}" run --rm --no-deps -T --entrypoint /usr/local/bin/asd-cli api \
  synth --seed 17 --count "$COUNT" --out - >"$WORK/synth.jsonl" || fail "asd-cli synth"
[[ $(wc -l <"$WORK/synth.jsonl" | tr -d ' ') -eq $COUNT ]] || fail "synth wrote $(wc -l <"$WORK/synth.jsonl") sessions"
n=0
while IFS= read -r line; do
  n=$((n + 1))
  printf '%s\n' "$line" >"$WORK/session_$n.json"
  openssl rand 32768 >"$WORK/blob_$n.bin"
done <"$WORK/synth.jsonl"

# ---- 4. toxics ---------------------------------------------------------------------------------
STEP="4 toxics"
say "$STEP"
toxic() {
  curl -fsS -o /dev/null -X POST "$TOXI/proxies/api/toxics" -H 'Content-Type: application/json' -d "$1" \
    || fail "toxic $1"
}
toxic '{"name":"latency","type":"latency","stream":"downstream","toxicity":1,"attributes":{"latency":2000,"jitter":1500}}'
toxic '{"name":"bw_down","type":"bandwidth","stream":"downstream","toxicity":1,"attributes":{"rate":64}}'
toxic '{"name":"bw_up","type":"bandwidth","stream":"upstream","toxicity":1,"attributes":{"rate":64}}'
toxic '{"name":"timeout","type":"timeout","stream":"upstream","toxicity":0.2,"attributes":{"timeout":3000}}'
toxic '{"name":"slicer","type":"slicer","stream":"downstream","toxicity":1,"attributes":{"average_size":256,"size_variation":128,"delay":5}}'
ok "latency 2000+/-1500 ms, 64 KB/s, timeout on 20 %, slicer"

# ---- 5. post (3x each, concurrently) and upload ------------------------------------------------
STEP="5 traffic"
say "$STEP: $COUNT sessions x 3 posts + $COUNT blob PUTs, $PARALLEL at a time"
export FIELD_WORK="$WORK" FIELD_API="$API" FIELD_DEVICE_TOKEN="$DEVICE_TOKEN"
started=$(date +%s)
seq 1 "$COUNT" | xargs -P "$PARALLEL" -I{} bash "$SELF" --session-job {} || true
elapsed=$(( $(date +%s) - started ))
cat "$WORK"/requests_*.log >"$WORK/requests.log" 2>/dev/null || true
posts=$(grep -c '^POST' "$WORK/requests.log" || true)
puts=$(grep -c '^PUT' "$WORK/requests.log" || true)
dropped=$(grep -c 'status=000' "$WORK/requests.log" || true)
client5xx=$(grep -cE 'status=5[0-9][0-9]' "$WORK/requests.log" || true)
echo "   $posts POST and $puts PUT attempts in ${elapsed}s; $dropped transport failures; $client5xx 5xx seen by clients"
[[ ! -s "$WORK/failures.log" ]] || { cat "$WORK/failures.log" >&2; fail "some sessions never completed"; }

# ---- 6. verdict --------------------------------------------------------------------------------
STEP="6 verify"
say "$STEP"
sql() { "${DC[@]}" exec -T postgres psql -U asd -d asd -tAc "$1" | tr -d '[:space:]'; }
rows=$(sql "SELECT count(*) FROM sessions")
blobs=$(sql "SELECT count(*) FROM sessions WHERE frames_blob_key IS NOT NULL")
audits=$(sql "SELECT count(*) FROM audit_log WHERE action = 'session.ingest'")
"${DC[@]}" logs --no-color api >"$WORK/api.log" 2>&1 || true
server5xx=$(grep -cE 'status[=: ]+"?5[0-9]{2}' "$WORK/api.log" || true)
echo "   sessions=$rows frames_blob_key=$blobs session.ingest=$audits api_5xx=$server5xx"
[[ "$rows" == "$COUNT" ]] || fail "sessions has $rows rows, expected $COUNT"
[[ "$blobs" == "$COUNT" ]] || fail "$blobs sessions have frames_blob_key, expected $COUNT"
[[ "$audits" == "$COUNT" ]] || fail "audit_log has $audits session.ingest rows, expected $COUNT"
[[ "$server5xx" == 0 ]] || fail "the API logged $server5xx 5xx responses"
ok "exactly $COUNT sessions, $COUNT blobs, $COUNT session.ingest audit rows, no 5xx"
say "FIELD SYNC TEST PASSED"
