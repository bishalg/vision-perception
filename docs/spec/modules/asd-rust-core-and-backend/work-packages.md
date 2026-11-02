# ASD Rust Core and Backend - Work Packages

Companion to `technical.md` (spec ids), `acceptance.md`, `qa-validation.md` and ADR-0006.
The live status table is `docs/roadmap_2026q4/04_asd_phased_work_packages.md §0`; this
file holds the detail. Inside a wave, packages share no files and run in parallel; they
still share one working tree, so repo-wide gates run in the main session after
integration. Build outputs go under `/private/tmp/asd-*` or `build/`, never into tracked
paths.

Tier legend: `S` = `senior-implementer` (Opus); `I` = `implementer` (Sonnet).
Every package: write the listed tests first, then the code; report what was skipped.

## Wave 0 (serial, one package, before anything else)

### WP-R.0 Workspace skeleton and domain types - `I`

Goal: WS-1..5, CON-1. Everything later compiles against these types; no logic here.

Files owned: `rust/Cargo.toml`, `rust/rust-toolchain.toml`, `rust/deny.toml`,
`rust/.cargo/config.toml`, `rust/README.md`, `rust/crates/{asd-core,asd-contract,asd-sync,asd-ffi,asd-backend,asd-cli}/Cargo.toml`
and each `src/lib.rs`/`src/main.rs` with the module tree from `technical.md` "Public
Rust API sketch" (every module present as an empty file with a `//! owner: WP-x` line),
`rust/crates/asd-core/src/types/{baselines,catalog,metrics,scoring,flag,frame,quality,capture}.rs`,
`rust/crates/asd-core/tests/types_roundtrip.rs`, `rust/crates/asd-core/tests/fixtures/`
(copies of `normative_baselines.json`, `stimulus_catalog.json`, `expected-session-v1.6.json`,
`expected-bundle.json`), `.gitignore` (WS-4), `scripts/rust_test.sh`,
`.github/workflows/rust.yml`.

Tests first: `types_roundtrip.rs` decodes each fixture into the typed struct, re-encodes,
and compares JCS-canonical bytes; one test per fixture; an unknown-field test proving
`deny_unknown_fields` is NOT set (Swift tolerates extra keys).
Commands: `scripts/rust_test.sh` (sqlx step skipped with `SQLX_OFFLINE=true` until B.1).
Depends on: nothing. Parallel with: nothing (serial by design, ~half a day).

## Wave 1 (parallel)

### WP-R.1 Scoring, baselines, flag, quality - `S`

Goal: CORE-1..4, CORE-7. Files: `asd-core/src/{scoring,baselines,flag,quality}.rs`,
`asd-core/tests/golden_{scoring,flag,quality}.rs`, `asd-core/tests/golden/{scoring,flag,quality}/*.json`.
Golden inputs: port every case of `PerVideoScoringEngineTests.swift`, `AttentionFlagTests.swift`,
`NormativeBaselineLibraryTests.swift`, `BlinkRejectionTests.swift` (quality) into JSON
`{input, expected}` files by reading the Swift expectations (the Swift tests are the
oracle until WP-A.11 generates them mechanically). Must not touch: `types/`, Swift files.
Depends on: R.0.

### WP-R.2 Metrics extraction and session aggregation - `S`

Goal: CORE-5, CORE-6. Files: `asd-core/src/{metrics,aggregate}.rs`,
`asd-core/tests/golden_{metrics,aggregate}.rs`, `asd-core/tests/golden/{metrics,aggregate}/*.json`.
Golden inputs from `PerVideoMetricsTests.swift`, `SessionBiomarkerAggregatorTests.swift`
and `Support/SummaryFixture.swift`. Traps: group order = first appearance (not sorted);
`dt <= 0` closes a fixation run and restarts; `observedDurationSec` prefers max
`clipMediaTimeSec > 0`. Depends on: R.0.

### WP-R.3 Contract crate, schema export, signing, sync protocol, CLI schema/bundle - `S`

Goal: CON-2..7, CON-12, GATE-1 (tool side). Files: `asd-contract/src/**`,
`asd-contract/tests/{session_v1_6,config_defaults,signing,api_v1_shapes}.rs`,
`asd-sync/src/**`, `asd-sync/tests/protocol.rs`, `asd-cli/src/cmd/{schema,bundle}.rs`,
`docs/spec/schemas/asd/{session-v1.6,config-bundle-v1,api-v1}.json` (regenerated, CON-4
proven in the same package). Must not touch: `scripts/ios_test.sh`, workflows (that is
D.7). Traps: `Option<Option<T>>` for explicit-null fields needs `#[serde(default, deserialize_with = ...)]`;
schemars output for them must be `anyOf [type, null]` like the Pydantic file;
`serde_json` must be built without `preserve_order` for sorted keys, and floats must
serialise like Swift (`1.0` not `1`) — assert in a test. Depends on: R.0.

### WP-R.4 Norm aggregator and synthetic generator - `I`

Goal: CORE-8, CORE-9, GATE-2, GATE-3 (tool side). Files: `asd-core/src/{norms,synth}.rs`,
`asd-core/tests/{norms,synth}.rs`, `asd-cli/src/cmd/{synth,norms}.rs`. Port the 11
cases of `tests/test_asd_norm_aggregator.py` and the 7 of `tools/asd_synth/test_asd_synth.py`
(adjusting units per CON-13). Do not delete the Python files (D.7 does). Depends on: R.0.
Note: `synth` needs `SessionIngestRequest` from R.3; until R.3 lands, build the payload as
`serde_json::Value` behind a `todo!()`-free adapter and switch to the typed struct in the
integration step (one-line change, main session).

### WP-B.1 Backend scaffold - `S`

Goal: WS-5 (sqlx step), DEP-1, DEP-2, OBS-1, CON-22, `02 §3` tables as migrations.
Files: `asd-backend/src/{main,app,config,db,error,problem,telemetry}.rs`,
`asd-backend/src/routes/health.rs`, `asd-backend/migrations/0001_initial.sql`
(all tables of `02 §3` with the additions: `enrol_codes(id, org_id, code_hash, max_uses, uses, expires_at, created_by, revoked_at)`,
`refresh_tokens(id, user_id, family_id, token_hash, expires_at, rotated_at?, revoked_at?)`,
`bundle_adoption(device_id, bundle_version, at)`, `sessions.participant_token` denormalised),
`asd-backend/tests/{health,log_redaction}.rs`, `asd-backend/tests/common/mod.rs`,
`rust/Dockerfile`, `rust/docker-compose.yml`, `rust/.env.example`, `rust/.sqlx/`.
Blob store trait `BlobStore { presign_put, put, get }` with `S3BlobStore` and `MemoryBlobStore`.
Depends on: R.0 (not on R.3; routes come in B.2/B.3).

## Wave 2 (parallel)

### WP-B.2 Auth, devices, enrol codes, RBAC, audit - `S`

Goal: AUTH-1..5, CON-10, CON-11, API-2 auth/org/user/device routes.
Files: `asd-backend/src/auth/{mod,password,jwt,refresh,device_token,rbac,extractors}.rs`,
`asd-backend/src/routes/{auth,orgs,users,devices}.rs`, `asd-backend/src/audit.rs`,
`asd-backend/migrations/0002_auth_grants.sql` (audit role grants),
`asd-backend/tests/{auth,rbac_matrix,audit,devices}.rs`. Depends on: B.1, R.3.

### WP-B.3 Sessions ingest and console reads - `S`

Goal: ING-1..3, CON-13..21, API-1 sessions/blobs, API-2 sessions/clips/metrics/review,
`report.pdf` passthrough. Files: `asd-backend/src/routes/{sessions,blobs,review,metrics,participants}.rs`,
`asd-backend/src/ingest/{mod,validate,derive,idempotency,auto_review}.rs`,
`asd-backend/src/read/{sessions,clips,cursor}.rs`, `asd-backend/tests/{sessions,blobs,console_read,review}.rs`.
Uses `asd-core::flag::assess` for CON-15. Depends on: B.1, R.1, R.3. Parallel with B.2
(B.3 stubs the device-token extractor with a test-only header until B.2 merges; the
main session swaps it).

### WP-R.5 FFI crate and xcframework script - `S`

Goal: FFI-1, FFI-2. Files: `asd-ffi/src/lib.rs`, `asd-ffi/uniffi.toml`,
`asd-ffi/build.rs`, `asd-ffi/tests/boundary.rs`, `scripts/build_asd_xcframework.sh`,
`rust/README.md` (build section). Targets: `aarch64-apple-ios`, `aarch64-apple-ios-sim`,
`aarch64-apple-darwin`; `cargo-ndk` wiring left as a documented TODO for WP-A.12.
Depends on: R.1, R.2, R.3. (`asd-sync` entry points included.)

### WP-D.7 Replace Python CI gates - `I`

Goal: GATE-1..4. Files: `scripts/ios_test.sh` (parity step → `asd-cli schema check` +
`schema validate`), `.github/workflows/ios.yml` (rustup + cache), deletions listed in
GATE-4, `docs/roadmap_2026q4/README.md` step 4, `apps/ios_asd_gaze_app/README.md`,
`domains/asd_screening/README.md`, `tests/README.md`. Depends on: R.3, R.4.

### WP-C.8 Console contract alignment - `I`

Goal: API-3, CON-16, CON-19, CON-20, `capture_source` gains `synthetic`, `unreliable` flag
already present. Files: `apps/curv_console/src/lib/api/types.ts`, `mock.ts`,
new `apps/curv_console/src/lib/api/contract.test.ts` (loads `docs/spec/schemas/asd/api-v1.json`
and asserts every zod schema accepts the schema's examples), the Clips tab component
rendering `rows`. Depends on: R.3.

## Wave 3 (parallel)

### WP-A.11 iOS consumes the Rust core - `S`

Goal: FFI-3, FFI-4, CON-7 (Swift side), CON-11 (`device_id` on re-register).
Files: `domains/asd_screening/ios/Package.swift`, the six Swift engine files listed in
FFI-3 (bodies become wrappers), `Tests/CurvAsdGazeKitTests/Support/LegacyReference/*.swift`
(moved bodies), `Tests/CurvAsdGazeKitTests/GoldenParityTests.swift` (+ `ASD_WRITE_GOLDEN`
writer), `apps/ios_asd_gaze_app/Sources/Models/{SessionSyncClient,DeviceRegistrationService}.swift`
(delegate header/URL/response logic to `asd-sync`; add `device_id`), their tests,
`scripts/ios_test.sh` (call `build_asd_xcframework.sh`). Must not touch Rust crates
except adding golden JSON files. Depends on: R.5, B.2 (for CON-11 end-to-end), D.7.

### WP-B.4 Config documents, bundles, signing, delivery - `S`

Goal: CFG-1, CFG-2, CON-6 server side. Files: `asd-backend/src/routes/config.rs`,
`asd-backend/src/config_docs/{mod,lifecycle,bundle,seed}.rs`, `asd-backend/tests/config.rs`.
Depends on: B.2.

### WP-B.5 Norm aggregation job and promote - `I`

Goal: `POST /jobs/norm-aggregate`, `POST /norm-runs/{id}/promote`, worker loop.
Files: `asd-backend/src/jobs/{mod,runner,norm_aggregate}.rs`, `asd-backend/src/routes/jobs.rs`,
`asd-backend/tests/jobs_norms.rs`. Uses `asd-core::norms`. Depends on: B.3, R.4, B.4
(promote writes a `draft` baselines document).

### WP-B.6 Care events and participant summary - `I` (owner H-15 may defer)

Goal: `POST /care/events`, `GET /participants/{token}/summary`, `POST /participants/{token}/erasure`
(request row only; execution is B.9). Files: `asd-backend/src/routes/care.rs`,
`asd-backend/src/care/*.rs`, `asd-backend/tests/care.rs`. Port the semantics of
`git show 34d44fc:services/asd_backend/app/api/v1/shims.py` (dedupe by idempotency key,
metrics aggregation). Depends on: B.2.

## Wave 4 (later phases; listed for dependency tracking)

- **WP-B.7** AWS deployment (DEP-3) - `S`, after H-11. Files: `aws_infra/asd/**`, `rust.yml` deploy job.
- **WP-B.8** Server PDF + FHIR parity - `I`; depends on B.3 and the iOS `DoctorReportGenerator` golden PDFs.
- **WP-B.9** Export, retention sweep, erasure execution - `S`; depends on B.5 runner.
- **WP-R.6** `asd-wasm` for the console sandbox re-score (WP-C.5) - `S`; depends on R.1, R.3.
- **WP-A.12** Android Kotlin bindings via `cargo-ndk` - `S`; depends on R.5.

## Recommended first parallel wave

Run WP-R.0 alone first (half a day, `implementer`). Then launch simultaneously:
WP-R.1 (`S`), WP-R.2 (`S`), WP-R.3 (`S`), WP-R.4 (`I`), WP-B.1 (`S`). They share only
`rust/Cargo.lock` (regenerated in the main session) and nothing else. Integration order
after the wave: R.3 → R.4 adapter swap → `cargo test --workspace` → commit.

## Traps for implementers

1. `.gitignore` ignores every `Makefile` outside `services/**` and every `*.a`, `*.pdf`.
   Do not add a `rust/Makefile`; use `scripts/rust_test.sh`. Golden PDFs for B.8 need an
   explicit `!` rule.
2. Swift encodes `Double` whole numbers as `1` in some paths and `1.0` in others
   (`JSONEncoder` emits `1` for `1.0`). Canonicalise with JCS before comparing; never
   compare raw bytes across languages.
3. `SessionIngestPayload.encode(to:)` writes `attentionFlag`/`battery` as explicit `null`
   but omits `eyeDwellRatio` etc. when nil. The Rust type needs both behaviours on the
   same struct (`Option<Option<T>>` with a custom deserialiser for the first group,
   `skip_serializing_if` for the second).
4. `perVideo` rows are snake_case; `rawPayload.perVideo[].rows[]` are camelCase
   (`PerVideoScoreRow`). Two different structs, do not reuse one.
5. `PerVideoScoringEngine.score` groups frames by `stimulusId ?? "session_untagged"`;
   the untagged id must match exactly.
6. `AttentionFlagAlgorithm.assess` returns yellow (not `unreliable`) for "no scored
   clips"; `unreliable` exists only in the backend-derived column (CON-15).
7. Idempotency: insert the `idempotency_keys` row first inside the transaction and treat
   `ON CONFLICT DO NOTHING` returning zero rows as "replay"; then compare the stored body
   hash to decide 200 vs 409. Never `SELECT` first (02 ADR-B5, flood Lambda lesson).
8. `sqlx::query!` macros need either a live `DATABASE_URL` at compile time or committed
   `rust/.sqlx/` data; run `cargo sqlx prepare --workspace` after every query change or CI
   fails with an opaque error.
9. The kit's tests run on macOS; forgetting the `aarch64-apple-darwin` slice in the
   xcframework breaks `swift test` with a linker error, not a clear message.
10. UniFFI's generated Swift module name must not collide with the SwiftPM target name
    (`AsdCoreFFI` for the binary target, `asd_core` for the generated module).
11. `jsonwebtoken` EdDSA needs the key in PKCS#8 DER; generate with `asd-cli keygen`,
    not with `openssl genpkey` output pasted raw.
12. `tower_governor` keys on peer IP; behind Cloudflare Tunnel or an ALB use the
    `CF-Connecting-IP`/`X-Forwarded-For` extractor or every device shares one bucket.
13. The console sends `Authorization` from a server component; CORS is irrelevant for it
    but still restrict `Access-Control-Allow-Origin` to the console origin for browsers.
14. iOS rejects IP-literal and `localhost` hosts for every URL including upload URLs;
    with MinIO in compose, use `BLOB_MODE=proxy` for device testing.

## Phase 5 wave (2026-11-01, ADR-0009) — security operations

Batch of nine packages (tracker `04 §7`): WP-B.11..B.15, WP-C.15, WP-C.16, WP-A.13 here,
WP-G.8 in `docs/spec/modules/asd-gaze-robustness/work-packages.md`. Migration numbers are
assigned here: **0020** = WP-B.11, **0022** = WP-B.14 (0021 went to the WP-B.12
ingest-hash and housekeeping indexes). Every backend package that adds or
changes a `sqlx::query!` must run `cargo sqlx prepare --workspace` against a live database
and commit `rust/.sqlx/` (gate `sqlx prepare --check`).

### Wave 1 (parallel, no shared files)

#### WP-B.11 TOTP enrolment API - `S`

Goal: AUTH-6, AUTH-7, AUTH-8, AUTH-10 (AUTH-9 `reset-totp` ships with B.14, which owns
`admin.rs`/`main.rs` in its wave; B.11 exposes `totp::reset(db, user_id)` for it).
Files: `asd-backend/migrations/0020_totp_enrolment.sql` (three nullable columns on
`users`, check `(totp_pending_ct IS NULL) = (totp_pending_kek_id IS NULL)`),
`asd-backend/src/auth/totp.rs` (`TOTP_ISSUER`, `pending_aad`, `start_enrolment`,
`confirm_enrolment`, `disable`, `otpauth_url`), new `asd-backend/src/routes/totp.rs`
(router merged in `routes/mod.rs`, its own `RateLimiter<Uuid>` 5/min), `routes/users.rs`
(`totp_pending` on `/users/me` only), `asd-contract/src/api_v1.rs` (DTOs
`TotpEnrolRequest`, `TotpEnrolResponse`, `TotpConfirmRequest`, `TotpDisableRequest`,
`TotpStatus`; `UserMe.totp_pending`), `asd-contract/src/problem.rs` (three slugs),
`docs/spec/schemas/asd/api-v1.json` regenerated with `asd-cli schema`, `rust/.sqlx/`,
tests `asd-backend/tests/totp_enrol.rs`.
Not to touch: `routes/auth.rs` login flow, `clinical/reauth.rs`, `crypto/keyring.rs`,
`config.rs`, `admin.rs`, `main.rs`.
Tests first: enrol → confirm → login needs code; wrong code 401 and audited; expired
pending 410; enrol while enrolled 409; disable without code 401, with password + code 200
and `last_totp_step` NULL; pending seed ignored by login and by step-up; 6th enrol in a
minute 429; log capture (pattern from `tests/log_redaction.rs`) contains neither the seed
nor the code; `otpauth_url` percent-encodes `+` in e-mails; `GET /users/me` shows
`totp_pending`.
Commands: `scripts/rust_test.sh`, `asd-cli schema check`.
Depends on: nothing. Parallel with: B.12, B.13, B.15, C.16, A.13, G.8.

#### WP-B.12 Housekeeping sweep - `I`

Goal: HK-1..HK-5.
Files: new `asd-backend/src/jobs/housekeeping.rs`, `jobs/mod.rs` (`KIND_HOUSEKEEPING`,
`KINDS`), `jobs/runner.rs` (`run_phased` arm, `schedule_daily` call next to
`retention::schedule_nightly`), `routes/jobs.rs` (`POST /jobs/housekeeping`, superadmin),
`asd-contract/src/api_v1.rs` only if a new DTO is needed (prefer reusing the job
response), `rust/.sqlx/`, tests `asd-backend/tests/housekeeping.rs`.
Not to touch: `retention.rs`, `erasure.rs`, `export.rs`, `ingest/idempotency.rs`.
Tests first: the seeded-pairs scenario (acceptance 20) including the FK-referenced job
and the device-referenced enrol code kept; replay after sweep is `200 duplicate:true`
(ingest fixture from `tests/ingest_support`); rotation with a swept-eligible but
referenced code still works; dry run deletes nothing and writes `housekeeping.sweep_dry_run`;
two workers calling `schedule_daily` concurrently queue one job; a scheduled run that
deletes nothing writes no audit row.
Commands: `scripts/rust_test.sh`.
Depends on: nothing. Parallel with: B.11, B.13, B.15, C.16, A.13, G.8.

#### WP-B.13 Prometheus listener and API HSTS - `I`

Goal: OBS-2, OBS-3, OBS-4.
Files: `asd-backend/Cargo.toml` (+ `rust/Cargo.lock`; a Prometheus client crate whose
licence `deny.toml` already allows, no `unsafe`, e.g. `prometheus-client`), new
`asd-backend/src/observability.rs` (registry, middleware layer, exposition handler, jobs
gauge query with 10 s cache), `src/app.rs` (metrics layer + `SetResponseHeaderLayer` for
HSTS), `src/config.rs` (`metrics_bind_addr: Option<SocketAddr>`), `src/main.rs`
(`serve` spawns the second listener), `src/auth/mod.rs` (`RateLimiter` increments
`auth_rate_limited_total` on reject, one line), `rust/.env.example`, tests
`asd-backend/tests/metrics_endpoint.rs`.
Not to touch: `routes/metrics.rs` (dashboard), `telemetry.rs` log format.
Tests first: listener off by default (connect refused); `/metrics` on the API port 404;
route label equals the template after `GET /v1/sessions/{id}`; label-safety grep over
the exposition for every id/e-mail the test used; histogram has the configured buckets;
HSTS present on `/health` and on a 404; scrape of `/metrics` produces no request-log line.
Commands: `scripts/rust_test.sh` (incl. `cargo deny check`).
Depends on: nothing. Conflicts (`main.rs`, `config.rs`) with B.15 → **wave 2** if B.15 is
in wave 1; the tracker places it in wave 2.

#### WP-B.15 KEK rotation - `S`

Goal: KEY-8, KEY-9, KEY-10.
Files: `asd-backend/src/crypto/keyring.rs` (`previous: Vec<Kek>`, selection by `kek_id`
in `unwrap_key` and `open_user_secret`, `Keyring::with_previous`), new
`asd-backend/src/crypto/rotate.rs` (the rewrite loops; reads `totp::secret_aad` and, once
B.11 lands, `totp::pending_aad`: B.14 adds the pending column to the loop), `src/config.rs`
(`ASD_DATA_KEK_PREVIOUS`, validation), `src/admin.rs` + `src/main.rs` (`rotate-kek`),
`src/app.rs` (keyring construction passes previous keys), `rust/.env.example`,
`rust/README.md` runbook, tests `asd-backend/tests/kek_rotation.rs`.
Not to touch: `auth/totp.rs` (read only), `interop/destination.rs` (secrets are under
DEKs; nothing to do), `clinical/`.
Tests first: write under A; start keyring with active B + previous A → reads work, new
writes carry B's id; `rotate-kek` → `remaining 0`, every row has B's id, decrypted
values equal the originals, blind index unchanged (`mrn_blind_index` same bytes); rerun
→ zero changes; keyring with only B after rotation works, with only B before rotation
→ `KeyUnwrap`; a row rewritten under B by a concurrent writer between select and
update is not overwritten (CAS); unknown `kek_id` row → `remaining 1`, exit 3; config
refuses `PREVIOUS` equal to the active key and non-32-byte entries.
Commands: `scripts/rust_test.sh`.
Depends on: nothing. Parallel with: B.11, B.12, C.16, A.13, G.8.

#### WP-C.16 Console CSP and HSTS - `I`

Goal: CSP-1..CSP-4.
Files: `apps/curv_console/src/proxy.ts` (nonce, CSP header, `x-nonce` request header,
matcher review), `next.config.ts` (HSTS), `src/app/layout.tsx` only if Next needs the
nonce read explicitly (check `node_modules/next/dist/docs/` for the current CSP guide
before writing: the console's `AGENTS.md` warns the API differs from training data),
`e2e/helpers.ts` (violation listener), new `e2e/csp.spec.ts`, new `tests/csp.test.ts`.
Not to touch: `src/lib/api/**`, settings pages (C.15 owns them).
Tests first: unit test of the policy builder (nonce differs per call, dev adds
`unsafe-eval`, production does not, no `unsafe-inline` in `script-src`); e2e: headers on
`/login`, the scoring-profile editor loads `asd_wasm` with no violation, enrol-code QR
renders, participant print view renders; negative: a test page or fixture with an
un-nonced inline script triggers the listener (proves the listener works).
Commands: `npm test`, `npm run lint`, `npm run typecheck`, `CURV_API_MODE=mock npm run
build`, `npm run e2e`.
Depends on: nothing. Parallel with every backend package and A.13, G.8.

#### WP-A.13 `ScoringProfile.trackingQuality` into the gaze pipeline through Rust - `S`

Goal: CFG-3 (closes the WP-A.5 note "trackingQuality not yet wired").
Files: `asd-ffi/src/gaze.rs` (`gaze_config_json_for_profile`, next to
`gaze_default_config_json`), the UniFFI definition it is declared in, `asd-ffi/tests/`
(new `gaze_config_profile.rs`), kit `GazePipelineSupport.swift`
(`GazePipelineDocuments.config(modelIdentifier:orientation:scoringProfileJSON:)`),
`GazeProcessingActor.swift` (init takes `scoringProfileJSON: String`, `makePipeline`
calls the new function; `CoreQualityGate` JSON assembly removed from the pipeline path),
`GazeSessionController.swift` (passes `ScoringConfiguration.active.scoringProfileJSON`),
kit tests `GazePipelineDocumentsTests.swift`.
Not to touch: `ARSessionBridge.swift`, `SettingsView.swift`, `project.yml` (G.8 owns
them); `asd-core` scoring (G.13); `ScoringConfiguration.qualityGate` stays for the
per-sample Swift gate callers.
Tests first (Rust): profile 1.1.0 with a changed `drift` threshold appears in the config;
1.0.0 profile (no `trackingQuality`) yields JSON identical to `gaze_default_config_json`;
`qualityGate` comes from the profile; malformed profile → `AsdError::InvalidJson`.
Tests first (kit): four-point `layout5` → `GazeTargetSchedule.calibrationLayout.count == 4`
and a scripted session records four calibration targets; `GoldenParityTests` unchanged.
Commands: `scripts/rust_test.sh`, `scripts/ios_test.sh` (rebuilds the xcframework).
Depends on: nothing. Parallel with: G.8 (disjoint kit files), all backend packages.

### Wave 2 (after wave 1 is integrated)

#### WP-C.15 Console TOTP enrolment page - `I`

Goal: AUTH-6..8 as seen from the console; nudge for privileged roles.
Files: new `src/app/[org]/settings/security/page.tsx` + `actions.ts` (server actions:
enrol, confirm, disable; password and code read from `FormData`, never React state, as
in `components/clinical/SignDialog`), new `src/components/security/TotpSetup.tsx`
(`QrCode.tsx` reused with the `otpauth` URL, seed shown as text, confirm form, disable
form), `src/lib/api/{types,live,mock,index}.ts` (three calls + `UserMe.totp_pending`;
mock: `MOCK_TOTP_EMAIL` enrolled, others enrol with the mock code), `src/lib/nav.ts`
(settings entry), `src/messages/en.ts`, layout banner for owner/admin/clinician/superadmin
without TOTP (`settings/security` link), tests `tests/security-settings.test.tsx`,
`src/lib/api/contract.test.ts` (zod vs regenerated `api-v1.json`), `e2e/totp.spec.ts`.
Not to touch: `proxy.ts`, `next.config.ts`, `e2e/helpers.ts` (C.16).
Tests first: component renders QR + seed from the response, hides both after confirm;
wrong code keeps the page and shows the problem detail; disable form requires both
fields; banner shown for an admin without TOTP, hidden for a viewer; contract test
passes against the new schema; e2e enrol/confirm/disable in mock, enrol/confirm in live
(code computed from the returned seed in the test, RFC 6238 in `e2e/helpers.ts`).
Commands: console gates.
Depends on: B.11 (schema). Parallel with: B.13, B.14.

#### WP-B.14 Drop plaintext columns, migrate ordering - `S`

Goal: KEY-11, KEY-12, AUTH-9 (`admin reset-totp`); extends B.15's rotation to the pending column.
Files: `asd-backend/migrations/0022_drop_plaintext_columns.sql`, `src/db.rs`
(`migrate` pre-step; `backfill` keeps running after migrations for the 0016 case),
`src/main.rs` (`migrate()` calls the pre-step), `src/auth/totp.rs` (delete the legacy
branch of `load_secret`; `legacy_secret_count` and `encrypt_legacy_secrets` stay because
step (a) needs them, but query `users.totp_secret` through a runtime `sqlx::query`
guarded by an `information_schema` check since the column no longer exists at compile
time), `src/routes/users.rs` (`totp_enrolled` = `totp_secret_ct IS NOT NULL`),
`src/admin.rs` + `src/main.rs` (remove `encrypt-review-notes`, add `reset-totp`), the review-notes
back-fill module it calls, `src/crypto/rotate.rs` (pending seed column), `rust/.sqlx/`,
`rust/README.md`, tests `asd-backend/tests/migrations.rs`, `tests/review_note_encryption.rs`
(drop the back-fill cases), `tests/kek_rotation.rs` (pending seed), `tests/admin.rs` (`reset-totp`).
Not to touch: 0016 and 0008 SQL files (history), `routes/totp.rs`.
Tests first: acceptance 19 scenarios (fresh DB; DB at 0019 with a plaintext seed, with
and without the KEK; DB with a plaintext note); after 0022 `information_schema.columns`
has neither column; `tests/migrations.rs` still applies every migration on a fresh DB in
order; rotation re-seals a pending seed.
Commands: `scripts/rust_test.sh`.
Depends on: B.11 (0020, pending columns), B.15 (`rotate.rs`). Runs after both; parallel
with C.15 only.

#### WP-B.13 — see wave 1 entry; scheduled in wave 2 because it shares `main.rs` and
`config.rs` with B.15. Parallel with C.15; **not** with B.14 (`main.rs`).

### Integration order

Wave 1: B.11, B.12, B.15, C.16, A.13, G.8 → main session: `cargo sqlx prepare`, regenerate
`api-v1.json`, full gates, commit. Wave 2: C.15 ∥ B.13 → gates, commit. Wave 3: B.14 →
gates, commit. The tracker's migration rule becomes "≥0023".

### Traps for implementers (phase 5)

1. `sqlx::query!` is compile-time checked against `rust/.sqlx/`. Any changed query needs
   `cargo sqlx prepare --workspace` against a migrated database before `rust_test.sh`
   passes; after 0022 nothing may name `users.totp_secret` or `review_items.note` in a
   macro query.
2. `Keyring::open_user_secret` compares `kek_id` to the active key and errors with
   `KeyUnwrap` *before* trying to decrypt; B.15 must change that comparison to a lookup
   across active + previous, or previous keys never get used.
3. Unwrapped DEKs are cached per process (`Keyring.keys`); tests of rotation must build a
   fresh `Keyring` to prove the database state, not reuse the one that wrote the data.
4. `jobs` has four incoming foreign keys with no `ON DELETE`; delete only unreferenced rows
   (HK-2). `devices.enrol_code_id` likewise.
5. Enrol-code rotation (CON-11) accepts expired codes on purpose; sweeping a referenced
   code breaks a device's recovery path (HK-5).
6. `retention::schedule_nightly` dedupes on `params->>'scheduled' = 'true'` and
   `created_at > now() - 1 day`; copy the pattern with a different advisory-lock key
   (`SCHEDULE_LOCK` is `0x0B09_5EED`), or two daily jobs block each other.
7. The telemetry layer drops fields whose name contains `secret`, `code` is **not**
   redacted (`totpcode` is, `enrolcode` is): name the confirm payload field `totp_code`
   or log nothing from it.
8. `routes/metrics.rs` already exists (dashboard). Name the Prometheus module
   `observability`, never mount it under `/v1`.
9. `tower_http::set_header::SetResponseHeaderLayer` needs the `set-header` feature;
   `Cargo.toml` enables only `cors, request-id, timeout, trace, util` today.
10. The console loads `asd_wasm.js` with a dynamic `import()` of a URL under `/asd-wasm/`;
    `'strict-dynamic'` lets nonced scripts load it, `'wasm-unsafe-eval'` lets it
    instantiate. Without both the config editors fall back to "core unavailable" silently
    (`src/lib/asd/browser.ts`), which the existing tests treat as a pass: add the explicit
    violation listener first (CSP-3) or the regression is invisible.
11. Next.js reads the nonce from the request's CSP header or `x-nonce` depending on the
    version; the console's `AGENTS.md` says the installed Next differs from training data:
    read `node_modules/next/dist/docs/` on CSP before writing `proxy.ts`.
12. The `proxy.ts` matcher excludes `_next/static`, `_next/image`, `favicon.ico`; HTML
    responses all pass through it, so headers set there reach every page. Do not move the
    CSP to `next.config.ts` `headers()`: it cannot vary per request, so no nonce.
13. `GazePipelineDocuments.config` sorts keys (`.sortedKeys`) before handing JSON to Rust;
    the new Rust function makes that moot, but `GazeTargetSchedule(configJSON:)` parses the
    pipeline's `configJson()` output, so the FFI must keep emitting the same camelCase keys.
14. `GazeProcessingActor.init` is `nonisolated`-sensitive (strict concurrency complete):
    pass the profile JSON as a `String` (Sendable), not `ScoringConfiguration`.
15. `ScoringConfiguration.active` is the delivered or bundled configuration; sessions
    already record `rawPayload.bundleVersion`, so no new field is needed to know which
    params ran.
16. The 0020 migration adds columns to `users`; `tests/migrations.rs` compares applied
    versions with the embedded list, so add the file and nothing else breaks, but the
    `CHECK` must allow all-NULL (users without a pending seed).
