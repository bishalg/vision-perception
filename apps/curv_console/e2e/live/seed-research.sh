# Sourced by run-record-live.sh when SUITE=research (do not run on its own): seeds what research.live.ts needs on top
# of the record suite's org, owner, clinician and device, and sets RESEARCH_ENV (appended to the e2e environment).
#
# - a researcher (no TOTP: not a required role) and two superadmins (the config four-eyes rule: one approves, another
#   publishes); both superadmins sign in inside their 14-day TOTP grace;
# - the seeded draft config documents published (superadmin A approves, B publishes; seeded documents are exempt
#   from `config-unanalysed`), so experiments start from a published profile and baselines;
# - RESEARCH_N research-consented sessions at 30 months and 3 at 54 months (a band below small_cell_k = 12, so it is
#   suppressed), plus 2 sessions without research consent; observed values vary per session so the distributions
#   have a spread;
# - a research frame log (`asd-cli gaze synth --scenario ideal`, header clips + session id added as
#   asd-cli/tests/research.rs builds them) uploaded through `PUT /v1/sessions/{id}/blobs/research_frames` for
#   FRAMES_N of the 30-month sessions, so the workbench and runs reach the `frames` level.

RESEARCH_N="${RESEARCH_N:-14}"
FRAMES_N="${FRAMES_N:-14}"
CLI="$BIN/asd-cli"
if [[ -z "${SKIP_BUILD:-}" ]]; then
  (cd "$ROOT/rust" && CARGO_INCREMENTAL=0 SQLX_OFFLINE=true cargo build -q -p asd-cli) || fail "cargo build asd-cli"
fi
[[ -x "$CLI" ]] || fail "asd-cli missing in $BIN"

say "research: researcher, two superadmins, published config"
RESEARCHER_EMAIL="researcher.$RUN_ID@example.org"
SA1_EMAIL="root1.$RUN_ID@example.org"; SA2_EMAIL="root2.$RUN_ID@example.org"
req POST "/v1/orgs/$ORG/users" "$OWNER_TOKEN" "$(jq -nc --arg e "$RESEARCHER_EMAIL" --arg p "$PASSWORD" \
  '{email:$e, role:"researcher", password:$p, display_name:"Research Live"}')"
expect 201 200
ASD_ADMIN_PASSWORD="$PASSWORD" "$BIN/asd-backend" admin create-user --email "$SA1_EMAIL" --role superadmin --force >/dev/null \
  || fail "create-user superadmin 1"
ASD_ADMIN_PASSWORD="$PASSWORD" "$BIN/asd-backend" admin create-user --email "$SA2_EMAIL" --role superadmin --force >/dev/null \
  || fail "create-user superadmin 2"
login() { req POST /v1/auth/login "" "$(jq -nc --arg e "$1" --arg p "$PASSWORD" '{email:$e,password:$p}')"; expect 200; jq -er .access_token "$BODY"; }
SA1_TOKEN=$(login "$SA1_EMAIL"); SA2_TOKEN=$(login "$SA2_EMAIL")
for kind in stimulus_catalog normative_baselines scoring_profile; do
  req GET "/v1/config/$kind?status=draft&limit=1" "$SA1_TOKEN"; expect 200
  version=$(jq -er '.items[0].version' "$BODY") || fail "no seeded $kind draft"
  req POST "/v1/config/$kind/$version/approve" "$SA1_TOKEN"; expect 200
  req POST "/v1/config/$kind/$version/publish" "$SA2_TOKEN"; expect 200
done

say "research: consented sessions and research frame logs"
SYNTH="$WORK/ideal.jsonl"
"$CLI" gaze synth --scenario ideal --seed 7 --out "$SYNTH" >/dev/null || fail "asd-cli gaze synth"
ingest_research() { # ingest_research INDEX AGE_MONTHS CONSENTED -> SESSION_ID
  local i=$1 age=$2 consented=$3 sid="LIVE-RES-${RUN_ID}-$1" token="child-$(openssl rand -hex 10)"
  jq -c --arg sid "$sid" --arg tok "$token" --argjson age "$age" --argjson c "$consented" \
    --argjson f "$(awk -v i="$i" 'BEGIN { printf "%.3f", 0.82 + (i % 9) * 0.045 }')" '
    def scale: if type == "number" then ([. * $f, 1] | min | . * 1000 | round / 1000) else . end;
    .sessionId = $sid | .participantToken = $tok | .rawPayload.sessionId = $sid | .rawPayload.participantId = $tok
    | .rawPayload.ageMonths = $age
    | (if $c then .rawPayload.normCollection = {consented: true} else . end)
    | .perVideo |= map(.details.rows |= map(.observed |= scale))
    | .rawPayload.perVideo |= map(.rows |= map(.observed |= scale))
    | .socialPreferenceScore |= scale | .eyeDwellRatio |= scale' "$FIXTURE" >"$WORK/session.json"
  CODE=$(curl -sS -o "$BODY" -w '%{http_code}' -X POST "$API/v1/sessions" -H "Authorization: Bearer $DEVICE_TOKEN" \
    -H 'Content-Type: application/json' -H 'X-Schema-Version: 1.7' -H "X-Idempotency-Key: $sid" \
    --data-binary @"$WORK/session.json") || fail "ingest $sid"
  expect 201
  echo "$sid"
}
# The synth log names its clips synth_clip_1/2; the published catalog has no such clips, so a frames-level evaluation
# would score nothing. Rename them to two catalog clips and put those clips' areas in the header (the device writes
# the screen-space AOIs it hit-tested against; the synth screen is filled edge to edge, so the catalog boxes are it).
req GET "/v1/orgs/$ORG/config/effective" "$OWNER_TOKEN"; expect 200
CLIP1=stim_05_geopref_split_10s; CLIP2=stim_01_social_peekaboo_8s
CLIPS=$(jq -c --arg a "$CLIP1" --arg b "$CLIP2" '.stimulus_catalog.body.clips as $c | [$a, $b] | to_entries
  | map(.value as $id | {clipId: $id, order: .key, aois: [$c[] | select(.id == $id) | .areas[]
    | {name, startSec, endSec, rect: {xMin, yMin, xMax, yMax}, roles: (.roles // [])}]})' "$BODY")
[[ "$(jq '[.[].aois | length] | min' <<<"$CLIPS")" -gt 0 ]] || fail "catalog clips $CLIP1 / $CLIP2 have no areas"
TEMPLATE="$WORK/research-template.jsonl"
{ head -n 1 "$SYNTH" | jq -c --argjson clips "$CLIPS" '.clips = $clips'
  tail -n +2 "$SYNTH" | sed "s/\"synth_clip_1\"/\"$CLIP1\"/; s/\"synth_clip_2\"/\"$CLIP2\"/"; } >"$TEMPLATE"
upload_frames() { # upload_frames SESSION_ID
  local sid=$1 log="$WORK/research-$1.jsonl" digest
  { head -n 1 "$TEMPLATE" | jq -c --arg sid "$sid" '.sessionId = $sid'; tail -n +2 "$TEMPLATE"; } >"$log"
  digest=$(openssl dgst -sha256 -binary "$log" | openssl base64 -A)
  CODE=$(curl -sS -o "$BODY" -w '%{http_code}' -X PUT "$API/v1/sessions/$sid/blobs/research_frames" \
    -H "Authorization: Bearer $DEVICE_TOKEN" -H 'Content-Type: application/x-ndjson' \
    -H "Content-Digest: sha-256=:$digest:" --data-binary @"$log") || fail "research frames $sid"
  expect 204 200 201
}
FRAMES_SESSION=""
for ((i = 1; i <= RESEARCH_N; i++)); do
  sid=$(ingest_research "$i" 30 true)
  if ((i <= FRAMES_N)); then upload_frames "$sid"; [[ -n "$FRAMES_SESSION" ]] || FRAMES_SESSION=$sid; fi
done
for i in 101 102 103; do ingest_research "$i" 54 true >/dev/null; done
for i in 201 202; do ingest_research "$i" 30 false >/dev/null; done
req GET "/v1/sessions/$FRAMES_SESSION" "$OWNER_TOKEN"; expect 200
jq -e '.research_frames.available == true' "$BODY" >/dev/null || fail "research frames not available on $FRAMES_SESSION"

RESEARCH_ENV=(E2E_RESEARCH=1 E2E_ORG="$ORG" E2E_RESEARCHER_EMAIL="$RESEARCHER_EMAIL" E2E_SUPERADMIN_EMAIL="$SA1_EMAIL"
  E2E_SUPERADMIN2_EMAIL="$SA2_EMAIL" E2E_FRAMES_SESSION="$FRAMES_SESSION")
