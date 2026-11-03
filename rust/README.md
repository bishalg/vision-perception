# ASD Rust workspace

One implementation of the ASD scoring maths, wire contracts, sync protocol and backend.
Spec: `docs/spec/modules/asd-rust-core-and-backend/`, decision: `docs/spec/adr/ADR-0006-rust-core-and-backend.md`.

| Crate | Role |
| --- | --- |
| `asd-core` | Domain types (`types`), scoring, flag, metrics, quality, norms, synth. Pure, no I/O. |
| `asd-contract` | Session/config/API wire types, JSON Schema export, bundle signing. |
| `asd-sync` | Device-sync protocol logic without I/O. |
| `asd-ffi` | UniFFI surface for Swift/Kotlin. |
| `asd-backend` | Axum + sqlx + Postgres service. |
| `asd-cli` | `schema`, `bundle`, `synth`, `norms` commands. |

Every source file starts with `//! owner: WP-x`, the work package that fills it.

## Gates

```bash
scripts/rust_test.sh                      # fmt, clippy -D warnings, test, deny, sqlx, schema check
CARGO_TARGET_DIR=/private/tmp/asd-rust cargo test --manifest-path rust/Cargo.toml -p asd-core
```

Do not add a `rust/Makefile` (the repo `.gitignore` ignores every `Makefile` outside `services/**`).
Tools: `cargo install cargo-deny sqlx-cli --locked`.

## Apple build (asd-ffi, FFI-2)

```bash
scripts/build_asd_xcframework.sh          # release; --debug for a faster dev build
```

Builds `asd-ffi` as a static library for `aarch64-apple-ios`, `aarch64-apple-ios-sim` and
`aarch64-apple-darwin` (adds the rustup targets if missing), generates the Swift bindings
with the crate's own `uniffi-bindgen` binary (`--features cli`, library mode) and writes,
git-ignored:

- `build/asd/AsdCore.xcframework`: `libasd_ffi.a` per slice, headers under
  `Headers/asd_coreFFI/` (C module `asd_coreFFI`);
- `build/asd/swift/asd_core.swift`: the Swift API (`coreVersion()`, `scoreSessionJson(...)`,
  `verifyBundle(...)`, `syncBuildSessionRequestJson(...)`, ...).

SwiftPM consumes it as `binaryTarget(name: "AsdCoreFFI", path: ...)` plus a wrapper target
that compiles `asd_core.swift` and depends on `AsdCoreFFI`. Slices are arm64 only: build
for a concrete simulator (or `ARCHS=arm64`), not `generic/platform=iOS Simulator`, which
also links x86_64. Documents cross the boundary as JSON in the Swift `Codable` format
(ADR-0006 §4); every fallible call throws `AsdError` (`InvalidJson`, `InvalidArgument`,
`NotRegistered`, `UploadRefused`, `InvalidUrl`). Bindings settings: `crates/asd-ffi/uniffi.toml`.

Android: see the next section.

## Android build (asd-ffi, AND-1..3)

```bash
scripts/build_asd_android.sh              # .so files, Kotlin bindings and the AAR
scripts/build_asd_android.sh --test       # ... plus the JVM golden parity test
scripts/build_asd_android.sh --host --test  # no NDK: host library, bindings, JVM test only
```

Needs `cargo-ndk` (`cargo install cargo-ndk --locked`), the Android NDK r27+
(`ANDROID_NDK_HOME`, or the newest under `$ANDROID_HOME/ndk`), the SDK (`ANDROID_HOME`),
JDK 17 and Gradle 8.14+ (`gradle` on `PATH` or `GRADLE=`). The same `asd-ffi` crate
(crate type `cdylib`) is built with `cargo ndk --platform 26` for `arm64-v8a`,
`armeabi-v7a` and `x86_64`; the Kotlin bindings come from the crate's `uniffi-bindgen`
(`--language kotlin`, library mode, package `org.curv.asd.core` from
`crates/asd-ffi/uniffi.toml`). Output, git-ignored:

- `build/asd/android/jniLibs/<abi>/libasd_ffi.so`;
- `build/asd/android/kotlin/org/curv/asd/core/asd_core.kt`: the Kotlin API (`coreVersion()`,
  `scoreSessionJson(...)`, `syncBuildSessionRequestJson(...)`, ...); fallible calls throw
  `AsdException` (`InvalidJson`, `InvalidArgument`, ...);
- `build/asd/android/host/libasd_ffi.{dylib,so}`: the host library for the JVM test;
- `apps/android_asd_core/core/build/outputs/aar/core-release.aar`: the AAR
  `org.curv.asd:core` (minSdk 26, depends on `net.java.dev.jna:jna:5.17.0@aar`).

`apps/android_asd_core/` packages the bindings and holds no logic. Its JVM test
(`GoldenParityTest`) runs the asd-core golden vectors through JNA and checks the
`expected-session-v1.8.json` request body byte for byte against its JCS golden. CI: the
`android-bindings` job in `.github/workflows/rust.yml`.

## First run (fresh database)

Nothing in the API can create the first org or user, so a new deployment is bootstrapped
with `asd-backend admin` (only `DATABASE_URL` is needed). Each command prints just the new
id on stdout and writes one `audit_log` row (actor `system`, `via: cli`). The password is
read from `ASD_ADMIN_PASSWORD` or the first line of stdin, never from an argument.

```bash
# 1. Schema (compose runs this as the `migrate` service).
asd-backend migrate

# 2. The organisation (kind: home | clinic | research; --retention-months defaults to 24).
ORG=$(asd-backend admin create-org --name "Example Clinic" --kind clinic)

# 3. Its first owner (password on stdin; or set ASD_ADMIN_PASSWORD).
read -rs PW && printf '%s\n' "$PW" | \
  asd-backend admin create-user --email owner@example.org --role owner --org "$ORG"
# Optional global operator (no org); a second one needs --force.
#   asd-backend admin create-user --email root@example.org --role superadmin

# 4. Sign in as the owner (the API needs ASD_JWT_SIGNING_KEY).
curl -s localhost:8000/v1/auth/login -H 'content-type: application/json' \
  -d '{"email":"owner@example.org","password":"..."}'          # -> access_token

# 5. First device enrol code (owner, admin or clinician); the code is shown once.
curl -s "localhost:8000/v1/orgs/$ORG/devices/enrol-code" -H "authorization: Bearer $ACCESS" \
  -H 'content-type: application/json' -d '{"max_uses": 1, "expires_in_days": 7}'
```

With compose, run the admin commands in a one-off container, e.g.
`docker compose run --rm -T api admin create-org --name "Example Clinic" --kind clinic`
(`-T` so stdin can carry the password). Further users and roles are created by the owner
through `POST /v1/orgs/{id}/users`.

A user who lost their authenticator app is reset by an operator (AUTH-9): this clears the
TOTP seed and any pending enrolment, writes audit `user.totp_reset` (`actor: operator`)
and prints the user id; the user then signs in with the password and enrols again from the
console. No KEK is needed. No HTTP endpoint changes another user's TOTP.

```bash
asd-backend admin reset-totp --user "$USER_ID"     # unknown id: exit 2, nothing changed
```

Rules: an existing e-mail (case-insensitive) is refused and nothing changes, even with
`--force`; a second superadmin or a second org of the same name needs `--force`; an owner
needs an existing `--org`, a superadmin takes none; passwords follow the API policy (12+
characters). Exit codes: `0` ok, `2` usage or invalid input, `3` conflicts with existing
data, `1` other errors (e.g. database).

## Mandatory two-step sign-in (AUTH-11..14, ADR-0010)

Users in the roles `owner`, `admin`, `clinician` and `superadmin` must enrol a TOTP
authenticator (`POST /v1/users/me/totp/enrol`, then `confirm`). Both the role list and the
14-day grace period are owner decisions held as constants in
`asd-contract::auth_policy` (`TOTP_REQUIRED_ROLES`, `TOTP_GRACE_DAYS`), not settings.

- **Grace.** `users.totp_required_by` (migration 0023) is set to now + 14 days the first
  time such a user without TOTP is seen: at login, when an owner creates them, or when
  their role changes into a required one. There is no back-fill: existing users start
  their 14 days at their first login after the upgrade. The deadline is never moved,
  cleared, or extended. `GET /v1/users/me` reports `totp_enforcement` (`not_required`,
  `grace`, `required`) and `totp_required_by`; the console nudges with the date.
- **After the deadline.** Login still succeeds (audit `auth.login_totp_enrolment_required`),
  but every user-plane route except `GET /v1/users/me`, `POST /v1/users/me/totp/enrol`,
  `POST /v1/users/me/totp/confirm` and `GET /v1/orgs/{id}` answers `403
  totp-enrolment-required`. The request after a successful `confirm` is unrestricted with
  the same access token. Device tokens are unaffected.
- **Replacing an authenticator.** `DELETE /v1/users/me/totp` (password and code) and
  `asd-backend admin reset-totp` both remove the seed; if the deadline has passed, the
  user is restricted to enrolment again at their next request. `reset-totp` does **not**
  touch `totp_required_by`, so it is not a way to grant a new grace period.
- Moving a user out of a required role makes them `not_required` at once; the stored
  deadline stays and applies again if they are moved back.

## Upgrading (`asd-backend migrate`, KEY-11, KEY-12)

`asd-backend migrate` runs, in order: (a) when migration 0016 is applied and plaintext
TOTP seeds remain in `users.totp_secret`, it seals them under `ASD_DATA_KEK` **before**
applying anything (without the KEK it stops, names the variable and applies nothing);
(b) the pending migrations (a database below 0016 is first taken to 0016 and sealed, so one
run reaches the end); (c) the same back-fill again (a no-op after 0022). Each migration is
its own transaction, so after a failure `migrate` can simply be run again once the cause is
fixed; no manual SQL.

Migration **0022** drops the plaintext columns `users.totp_secret` and `review_items.note`.
It refuses (and drops nothing) while either still holds a value:

- `plaintext TOTP seeds remain: run "asd-backend migrate" with ASD_DATA_KEK set`: set the
  KEK and run `migrate` again (only seen without the KEK or when the migrations are applied
  by another tool, since `migrate` seals the seeds first).
- `plaintext review notes remain: run "asd-backend admin encrypt-review-notes" with the
  previous release ...`: that command was removed together with 0022. Before upgrading a
  deployment that ran WP-C.9 (0008) on existing review notes, run
  `asd-backend admin encrypt-review-notes` (needs `ASD_DATA_KEK`) with the release you are
  upgrading from; if the upgrade already stopped at 0022, the new API still serves (it no
  longer reads the column), so run the old binary's command against the same database and
  then the new `migrate` again.

`asd-backend admin encrypt-review-notes` no longer exists (exit 2 with this hint).

## Rotating the data KEK (`ASD_DATA_KEK`, KEY-8..10)

`ASD_DATA_KEK` wraps each org's data key (`org_data_keys`) and seals per-user secrets (the
TOTP seed). Rotation re-wraps and re-seals those rows; it does not change the data keys,
so clinical ciphertexts, integration-destination secrets (sealed with the org data key)
and MRN blind indexes stay as they are and are not rewritten.

```sh
NEW=$(openssl rand -base64 32)        # store it in the secret store next to the old key

# 1. API and worker: the new key active, the old one readable. Restart both.
#      ASD_DATA_KEK=$NEW
#      ASD_DATA_KEK_PREVIOUS=$OLD
#    Reads work for rows under either key; every new write uses $NEW.

# 2. Optional check, writes nothing: every row under $OLD is opened.
asd-backend admin rotate-kek --dry-run
# {"rewrapped_deks":12,"resealed_users":4,"remaining":0,"dry_run":true}

# 3. Rewrite (batches of 500 rows per transaction; --batch N to change). Same environment
#    as step 1. Safe to run while the API serves traffic, and safe to run again.
asd-backend admin rotate-kek
# {"rewrapped_deks":12,"resealed_users":4,"remaining":0,"dry_run":false}

# 4. Only when remaining is 0: remove ASD_DATA_KEK_PREVIOUS from API and worker, restart.
#    Keep $OLD in the secret store's history until backups made before step 3 have expired.
```

- `remaining > 0` exits `3` (the report is still printed): some rows are under a KEK that
  is not configured, could not be opened, or were locked by a concurrent writer. Run it
  again; if the count stays, find the stored fingerprint
  (`SELECT DISTINCT kek_id FROM org_data_keys` and `users.totp_secret_kek_id`), add the
  matching old key to `ASD_DATA_KEK_PREVIOUS` and rerun. The service logs the stored and
  configured fingerprints (`kek-` + 16 hex digits) when it meets such a row.
- A process started with only the new key before step 3 finished answers
  `503 clinical-crypto-unavailable` (and refuses TOTP logins) for rows still under the old
  key, by design. Put `ASD_DATA_KEK_PREVIOUS` back and restart to recover.
- Each run writes one audit row `crypto.kek_rotated` (no org) with the counts and the key
  fingerprints, never key bytes. `--dry-run` writes nothing.
- The configuration refuses a `PREVIOUS` entry equal to `ASD_DATA_KEK`, duplicates, and
  entries that are not base64 of exactly 32 bytes (exit `2`).

## Metrics and HSTS (`ASD_METRICS_BIND_ADDR`, OBS-2..4)

- Set `ASD_METRICS_BIND_ADDR` (for example `127.0.0.1:9100`) on `asd-backend serve` to start a
  second listener that serves only `GET /metrics` (Prometheus text 0.0.4; no auth, no CORS, not
  request-logged). Unset means no listener. The API port never serves `/metrics`
  (`/v1/orgs/{id}/metrics` is the unrelated dashboard). Keep the listener on a private interface.
- Series: `http_requests_total{route,method,status}`, `http_request_duration_seconds{route,method}`
  (buckets 5 ms to 10 s), `auth_rate_limited_total{limiter}`, `jobs_rows{kind,status}` (one query,
  cached 10 s) and `asd_backend_build_info{version,git_sha}` (`git_sha` comes from the
  `ASD_GIT_SHA` build-time variable, `unknown` otherwise). `route` is the matched path template
  (`/v1/sessions/{id}`) or `unmatched`; no id, e-mail or token ever becomes a label value.
- Every API response carries `Strict-Transport-Security: max-age=63072000; includeSubDomains`.

## What is audited (HIP-1, HIP-4)

Every mutation writes one `audit_log` row in its own transaction. Since phase 7 every **read** of
clinical content or participant detail does too, through `audit::record_read`, before the response
body is built. The write is never best-effort: if the row cannot be written the request answers
`500` and no data is sent. `after` holds ids or a part name only, never clinical text.

| Endpoint | `action` | `target_type` / `target_id` | `after` |
|---|---|---|---|
| `GET /v1/sessions/{id}`, `/clips`, `/report.pdf`, `/fhir` | `session.read` | `session` / session id | `{"part":"detail"\|"clips"\|"report"\|"fhir"}` |
| `GET /v1/sessions/{id}/raw` | `session.raw_read` | `session` / session id | none |
| `GET /v1/participants/{id}` | `participant.read` | `participant` / participant id | none |
| `GET /v1/participants/{id}/timeline` | `participant.timeline_read` | `participant` / participant id | none |
| `GET /v1/sessions/{id}/clinical-review`, `GET /v1/clinical-reviews/{id}` | `clinical_review.read` | `clinical_review` / review id | none |
| `GET /v1/notes/{id}` | `note.read` | `clinical_note` / note id | none |
| `GET /v1/participants/{id}/notes` (one row per page) | `note.read` | `participant` / participant id | `{"note_ids":[...]}` |
| `GET /v1/participants/{id}/disclosures` | `disclosures.read` | `participant` / participant id | none |
| `GET /v1/orgs/{id}/worklist` (one row per page) | `worklist.read` | `organization` / org id | `{"session_ids":[...]}` |
| `GET /v1/exports/{id}/file` (proxy mode; actor `system`, the signed token is the credential) | `export.file_served` | `export` / export id | none |

Session, participant and export list endpoints carry no clinical text and are not audited per row.
A superadmin's reads of the data it may see are audited the same way (org = the resource's org).

Blob uploads (`PUT /v1/sessions/{id}/blobs/{kind}`) verify an RFC 9530 `Content-Digest:
sha-256=:<base64>:` when sent (`blob::verify_content_digest`): a mismatch is `400 digest-mismatch`
and nothing is stored, a malformed header or one without `sha-256` is `400 validation-failed`, an
absent header is accepted (older apps).

## Resetting a user's two-step sign-in from the console (TRS-1, TRS-2, WP-B.17)

When a user loses their authenticator, an `owner` or `admin` of the org resets it with
`POST /v1/orgs/{id}/users/{user_id}/totp/reset`, body `{"password", "totp_code"?, "reason"}`:
the caller's own password (and current code when the caller has TOTP), plus a reason of
10..200 characters. It answers `204`; in one transaction the target's TOTP seed, spent step
and any pending enrolment are cleared and every refresh token of the target is revoked (the
next refresh fails; the current access token runs out within 15 minutes). The target then
signs in with the password alone and enrols again; `totp_required_by` is not changed, so a
passed deadline still sends them straight to enrolment. Audit: `user.totp_reset_by_admin`
with the target id and the reason; a wrong password is `401 reauthentication-failed`,
audited as `user.totp_reset_failed`, and changes nothing. An `admin` cannot reset an
`owner`, and nobody resets themselves (`DELETE /v1/users/me/totp`): `403
totp-reset-forbidden`. A superadmin, or an org with no owner/admin able to sign in, still
uses the operator command `asd-backend admin reset-totp --user <uuid>`, which runs the same
reset (`auth::totp::reset_for_user`) and now also revokes the user's refresh tokens.

## Restore helpers (DPL-6..8, WP-B.22)

Three `asd-backend admin` commands used by the clinic-server backup, restore and drill
scripts (only `DATABASE_URL`, plus `ASD_DATA_KEK` for `verify`):

- `erasure-ledger` prints one JSON line per erasure request,
  `{"participant_id","org_id","requested_at","status"}` (ids only, oldest first; nothing
  when there are none). `backup.sh` appends it to `erasure-ledger.jsonl`, so erasures are
  known even if the database is restored from an older dump.
- `requeue-erasure --ledger <file>` replays that file after a restore: for every distinct
  participant in it whose restored row is not erased (`erased_at IS NULL`), the open
  pending request gets a job (unless one is queued or running) or a new request is filed
  (`requested_by_type = system`) and queued, with audit `participant.erasure_requeued`
  (`via: cli`). Erased participants are skipped; held requests (clinical record) are left
  held; ids not in the database are printed on stderr and listed in the report, and do not
  fail the run; a malformed line exits 2 before anything changes. Prints
  `{"entries","participants","requeued","already_erased","in_progress","held","unknown"}`.
  It only queues: the worker must be running to erase, so wait for the jobs before
  comparing counts. Running it twice is safe.
- `verify` runs `SELECT 1` against every table and unwraps every `org_data_keys` row with
  `ASD_DATA_KEK` (and `ASD_DATA_KEK_PREVIOUS`); prints `{"tables","data_keys","kek_id"}`
  (`kek_id` is the fingerprint, never the key). Exits 3 when any data key cannot be
  unwrapped (the key card does not match this database), 2 without `ASD_DATA_KEK`.

## Emergency password reset (WP-B.24, server shell only)

`asd-backend admin reset-password --user <uuid>` sets a new password for an active user, revokes all
their refresh tokens and writes audit `user.password_reset`. It prints a generated 20-character
temporary password once on stdout; with `ASD_ADMIN_PASSWORD` set it uses that value instead and prints
the user id, so the owner can type their own. It does not change two-step sign-in (use `reset-totp`),
access tokens already issued stay valid up to 15 minutes, and there is no forced change at next
sign-in yet. Use it with a second person present (`docs/security/policies/emergency-access.md`).

## EMR-lite record (WP-B.19, REC-1..8)

Migration 0024 adds `encounters` and `attachments`. Encounter summaries are encrypted with the org
data key (AAD `encounters:summary_ct:{org}:{id}`); attachment bytes are encrypted before they reach the
blob store (AAD `attachments:blob:{org}:{id}`, key `attachments/{org}/{participant}/{id}`).

- `POST /v1/orgs/{id}/participants` (token + QR payload), `POST /v1/orgs/{id}/participants/search`
- `/v1/participants/{id}/encounters`, `/v1/encounters/{id}` (PATCH with `If-Match`),
  `/v1/encounters/{id}/sessions`
- `GET /v1/orgs/{id}/follow-ups`
- `/v1/participants/{id}/attachments`, `PUT /v1/attachments/{id}/content` (at most 10 MiB, optional
  `Content-Digest: sha-256=:<b64>:`, type checked by magic number, written once),
  `GET`/`DELETE /v1/attachments/{id}`

Reads of summaries and attachments are audited (`encounter.read`, `attachment.read`). Erasure deletes
attachments, their blobs and encounters unless the participant is held (a signed note or an encounter
with a clinician summary holds it).

## Share a case (WP-I.13, SHR-7..9)

`POST /v1/orgs/{id}/exports` with `format: case_package` and `case {participant_id, session_ids?,
recipient, consent_reference?, consent_recorded_at?, include_attachments}` queues a ZIP of one
participant's case (identified needs consent; research is de-identified and consent-filtered). On a
ready case export, `POST /v1/exports/{id}/share-link {expires_in_days? (1..7, default 3),
max_downloads? (1..5, default 1)}` returns the link and a 12-character passcode once;
`GET /v1/exports/{id}/share-links` lists links; `DELETE /v1/share-links/{id}` revokes.

Public, no bearer: `POST /v1/share/{id}/open {passcode}` returns a 5-minute download token, then
`GET /v1/share/{id}/file?t=`. Limits: 10 opens per link and 30 per IP per hour (per process).
Responses are `no-store`, `noindex` and `no-referrer`; open, failed open and download are audited with
the client IP. Passcodes are Argon2id-hashed and never logged. Links never outlive the artifact
(EXJ-4). Migration 0025.

Share a case works before any gaze session: a child's visits and documents can be packaged alone
(`sessions_n: 0`, FHIR bundle of `Patient` + `Organization` only, `data/sessions.jsonl` header only).
A participant with no session, no visit and no uploaded file is refused (`400 validation-failed`,
"the participant has nothing to share"). A visit posted by a `clinician` without `clinician_id`
records the caller as its clinician, so `follow-ups?assigned=me` lists it (REC-4).

## Researcher toolkit seams (phase 8, WP-X.0)

`asd_contract::params` is the parameter registry: one row per leaf of the 1.2.0 `ScoringProfile`
(`metrics`, `aoi`, `ageBands`, `norms`, `review` are new and default to today's constants).
`asd-cli schema export|check` covers `param-registry-v1.json`; `asd-cli params list [--stage S]
[--reprocess R]` prints it and `asd-cli params check FILE` validates a profile (exit 0/1/2).
Migration 0026 adds `experiments`, `cohorts`, `analysis_runs`, `sessions.age_months` (back-filled)
and the research-frame columns, `jobs.progress` / `cancel_requested_at`, and the `analysis_results`
export format. Job kind `analysis_run` and the `/experiments`, `/cohorts`, `/analysis-runs`,
`/orgs/{id}/config/effective` routes are mounted and answer `501 not-implemented` until WP-X.2/X.3.
`ASD_ANALYSIS_*` settings arrive with WP-X.3.

**WP-X.10 (asd-cli).** `research reprocess LOG --catalog F --baselines F --age-months N [--profile F]
[--level auto|frames] [--trace-hz N]` re-runs a research frame log through
`asd_core::analysis::evaluate` at frames level; `research evaluate SESSION.json --baselines F
[--profile F] [--age-months N] [--level auto|score]` re-scores a session at score level. Output is
the `Evaluation` JSON, byte-equal to the core. Exit 0 ok, 1 input error, 2 not processable at the
requested level (e.g. a log header without clips).

**WP-X.8 (`analysis_results` export).** De-identified export of a succeeded analysis run, purpose
research or quality_improvement, `AnalysisRead` required. Layout `aggregates` (JSONL, or a CSV ZIP;
RUN-7 columns) or `sessions` (JSONL, CSV ZIP or Parquet with `curv.*` footer metadata incl.
`run_id` and `definition_hash`). Sessions are joined at export time (withdrawn since the run →
`excluded.withdrawn`, erased → `session_gone`). Suppressed cells carry no numbers; the manifest
`provenance` block equals the run record. The research frame log has no encoder (PRV-5). The
request's `analysis` block is stored in `exports.filters.analysis` until the contract gains a field.

**WP-X.0b (contract catch-up)** types the X.2/X.6 additions: `SessionIngestResponse` /
`asd_sync::IngestAck` / `AsdIngestAck.research_frames_upload_url`, `Org` / `OrgPatch.
research_frames_retention_months` (patch: absent = unchanged, `null` = follow `retention_months`,
1..=240 = own horizon), `RetentionOrgResult.research_frames_purged`, slug `cohort-name-exists`, and
`SessionPage.items` as `SessionListItem` (summary + `research_frames_available`, one extra column in
the list query). The iOS raw-ack read stays until the xcframework is rebuilt.

**WP-X.4 (asd-wasm).** Exports `paramRegistry()` (the `param-registry-v1.json` document) and
`reprocessSession(log, catalog|null, profile, baselines, {ageMonths, scoredAtUtc?, model?, glasses?,
traceHz?})`, which is `analysis::evaluate` at `frames` level and returns the core `Evaluation`.
`rescoreSession`/`rescoreApiSession` now go through `evaluate(Score)`, so the profile's
`lowConfidenceValidRatio` and `metrics` apply (`contractInfo.lowConfidenceValidRatio` is the
default). `tests/research.rs` checks native == wasm32 on the synth scenarios; the replay's
`tracking` record is compared to 1e-12 because of last-bit maths-library differences.

**WP-X.3** fills `jobs/analysis.rs` and `routes/analysis_runs.rs`. A run snapshots the experiment
profile, cohort, level, options, seed and every RUN-5 version, then queues job `analysis_run`. The
job resolves membership in a read-only snapshot, evaluates each session twice (published and
experiment) at `frames` level where a research log exists and `score` otherwise, adding
`param_not_applied` where needed. It checks cancel and the deadline before every page, seals the
rows under the org key at `analysis/{org}/{run}/sessions.jsonl.sealed` and stores a PRV-1 `summary`.
`POST /v1/jobs/{id}/cancel` and `GET /v1/jobs/{id}` `progress` work for every job. Limits are the
`ASD_ANALYSIS_*` settings in `.env.example` (one running analysis per org by default).

**WP-X.9.** `asd_core::analysis::{sweep, age_profile}`: sweep grids (the published value is always a
cell; the cell nearest the experiment is the draft side), per-cell rates, means and medians
(`summary.sweep`, plus `published_values`), and per band × expectation shares on-target / monitor /
atypical and the expected-value percentile under published and draft baselines
(`summary.age_profile`). `suppress` hides cells under k, and age cells under `max(k, norms.minN)`.
The job runs `param_sweep` (cells evaluated off the async workers, cancel and deadline checked per
session) and `age_profile`. The route validates SWP-1/2 against the registry and level; owner-gated
knobs are sweepable. The new limit `ASD_ANALYSIS_MAX_SUMMARY_KIB` (1024) enforces RUN-4. The console
run page gains `SweepPanel` (curves, or two-axis heatmaps up to 625 cells) and `AgeProfileTable`.

**WP-X.1.** `asd_core::analysis` holds the core twins of the 1.2.0 profile sections
(`AnalysisProfile`, which decodes from any `ScoringProfile` body) and of the run summary types,
`evaluate` (score/frames/auto, RPX-2/3), `Aggregator` (exact quantiles, seeded bootstrap CIs, RUN-4)
and `suppress` (PRV-1). `asd_core::gaze::reprocess` replays a research frame log with a profile's
gate, tracking, metrics and AOI sections and builds frames the way the device does (RPX-1).
`metrics::extract_with` reads every `metrics.*` knob; `extract`/`extract_with_roles` keep their
signatures and the goldens are byte-identical. The contract↔core JSON parity is tested in
`asd-core/tests/analysis.rs` (asd-contract is a dev-dependency of asd-core).

**WP-X.2** fills `routes/research.rs`. Experiments are an org's draft `ScoringProfile`, validated by
`params::validate` and edited with `If-Match`. Promote files a draft config document whose notes
start with `experiment:<id> run:<run>`; publishing a human-authored scoring profile or baselines
candidate then needs a succeeded `analysis_runs` row over its body hash (or its version), else
`409 config-unanalysed`. Cohorts are saved `CohortDefinition`s; the preview is one SQL count,
research consent and withdrawal are always applied, and small-cell suppression uses
`settings.research.small_cell_k` (default 12, 5..=50). `GET /orgs/{id}/config/effective` returns
the org's bundle, else the global published documents, else the bundled defaults. The research
frame log is uploaded through the proxy only (`PUT /sessions/{id}/blobs/research_frames`, digest
required), sealed under the org data key (16-byte key id + AES-GCM), read by owners and researchers
with a `session.frames_read` audit row, purged on `research_frames_retention_months` (falling back
to `retention_months`), and deleted by erasure. Ingest stores `age_months` and takes its review and
flag thresholds from the published profile. `POST /v1/config/{kind}` (draft) now needs `ConfigWrite`.

## Conventions

- Licence `Apache-2.0` on every crate; dependencies must pass `rust/deny.toml`.
- `asd-core` never depends on tokio, sqlx, reqwest, uniffi or other I/O crates.
- `serde_json` is used without `preserve_order`; contract bytes have sorted keys.
- Compare cross-language JSON through RFC 8785 (JCS), never raw bytes.
- Types never set `deny_unknown_fields` (Swift tolerates extra keys).
