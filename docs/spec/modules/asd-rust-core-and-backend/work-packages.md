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
