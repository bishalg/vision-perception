# ASD Rust Core and Backend - Technical Spec

Status: Proposed (2026-10-30). Snapshot reviewed: `dev` at `9655fcc`.
Decision record: `docs/spec/adr/ADR-0006-rust-core-and-backend.md`.
Supersedes the stack choice in `docs/roadmap_2026q4/02_asd_backend_architecture.md` ADR-B1
and the Python-specific parts of ADR-B5/B7; the data model (`02 §3`), auth planes (ADR-B4),
config delivery (ADR-B6) and PHI posture (ADR-B8) there remain the requirements and are
cited, not repeated.

## Purpose and scope

One Rust implementation of the ASD scoring mathematics, wire contracts, device-sync
protocol logic and the HTTP backend, consumed by Swift (UniFFI), TypeScript (HTTP now,
WASM later) and Kotlin (later). In scope: the `rust/` workspace, the backend service, the
Swift integration of the scoring core, the replacement of the Python CI gates, local and
AWS deployment shape. Out of scope (deliberately): porting real-time gaze filtering and
calibration (`OneEuroFilter`, `ScreenRaycaster`, `GazeCalibrationProfile`), the Debug-only
`BiomarkerClassificationEngine`, CoreML age estimation, the C++ core, FHIR façade
(WP-B.10), server PDF generation (WP-B.8, later phase), Android bindings (WP-A.12).

## Verified current state this spec builds on

| # | Finding | Evidence |
| --- | --- | --- |
| F1 | Scoring maths exists only in Swift: status bands and piecewise score (`evaluate`, `piecewise`), composite weighting, bracket clamping, latency imputation at clip duration. | `domains/asd_screening/ios/Sources/CurvAsdGazeKit/PerVideoScoringEngine.swift` |
| F2 | Metric extraction (AOI dwell ratios, first-entry latency, GeoPref preference, eye/mouth share, I-VT fixations with 5.0 units/s threshold and 80 ms minimum, affect means, smile reactivity with 500 ms baseline, `flatAffect = 1 - expressivity*50`) is Swift only. | `PerVideoMetrics.swift`, `SessionBiomarkerAggregator.swift` |
| F3 | Traffic-light flag thresholds 0.65/0.35, low-confidence share 0.50, atypical share 0.40; iOS flag has 3 values, console expects 4 (`unreliable`). | `AttentionFlag.swift`; `apps/curv_console/src/lib/api/types.ts:7` |
| F4 | iOS ingest payload is camelCase at top level and inside `rawPayload`, snake_case inside `perVideo` rows; dwell values are ratios (`details.dwellUnits = "ratio"`); unmeasured metrics are absent keys; `attentionFlag`/`battery` are explicit `null` when unknown. | `apps/ios_asd_gaze_app/Sources/Models/SessionSyncClient.swift` |
| F5 | The Python synth generator and the parity script emit dwell values in seconds, status `offTarget` and `captureSource: "synthetic"`, none of which the iOS kit or the console accept. | `tools/asd_synth/generator.py:62-86`; `scripts/check_session_schema_parity.py:62-76` |
| F6 | iOS treats HTTP 409 as a successful duplicate and 401/403 as "re-register once with the stored enrol code"; `02` describes enrol codes as one-time. No device identifier is sent on re-registration. | `SessionSyncClient.swift:postSession`, `DeviceRegistrationService.swift:reRegister` |
| F7 | iOS accepts relative upload URLs (resolved against the API base, bearer attached) and absolute https URLs on other hosts (no bearer); rejects IP-literal and `localhost` hosts. | `SessionSyncClient.swift:resolveUploadURL`, `ResearchStudyConfiguration.swift:163` |
| F8 | Console expects `POST /auth/refresh` and `/auth/logout` with `{refresh_token}` in the body, `TokenPair` with `expires_in` and `user`; `GET /sessions/{id}/clips` returning `{items}` with per-clip `metric/expected/recorded/deviation` columns; `battery` enum `express|comprehensive`; `capture_source` enum `truedepth|simulated`. | `apps/curv_console/src/lib/api/{types,live,report}.ts` |
| F9 | The deleted FastAPI backend defaulted missing measurements (`validRatio=0.95`, `attentionFlag="green"`, `battery="express_30s"`, `durationSec=30`), returned relative upload paths and used `(id == sessionId) OR (idempotency_key == key)` get-then-write for idempotency. | `git show 34d44fc:services/asd_backend/app/{schemas/session.py,api/v1/sessions.py}` |
| F10 | `tools/asd_research_server.py` and `tests/test_asd_research_server.py` still exist although `02 §9` scheduled their deletion after WP-A.4/B.4. | tree at `9655fcc` |
| F11 | `.gitignore` ignores `Makefile` (except `services/**/Makefile`), `*.a`, `*.pdf`; nothing ignores `rust/target/`. | `.gitignore:26,58` |
| F12 | Kit tests run on macOS via `swift test` (`Package.swift` declares `.macOS(.v14)`); any binary dependency must ship a macOS slice. | `domains/asd_screening/ios/Package.swift`; `qa-validation.md` of the store-readiness module |
| F13 | The iOS norm-collection export reads `per_video` blocks with `ageMonths`, `videos[].stimulusId`, `rows[].{biomarker,observed,imputed}`; the aggregator excludes imputed rows and withholds cells with n < 12 or zero spread. | `tools/asd_norm_aggregator.py:70-180` |

## 1. Workspace (WS)

- **WS-1** The workspace MUST be `rust/Cargo.toml` with members `crates/asd-core`,
  `crates/asd-contract`, `crates/asd-sync`, `crates/asd-ffi`, `crates/asd-backend`,
  `crates/asd-cli` (and `crates/asd-wasm` when WP-R.6 lands). `rust/rust-toolchain.toml`
  pins a stable channel; `rust/deny.toml` configures `cargo-deny`.
- **WS-2** Dependency direction MUST be acyclic as drawn in ADR-0006 D2. `asd-core` MUST
  NOT depend on tokio, sqlx, reqwest, uniffi or any I/O crate; it is `std`-only with
  `serde`, `serde_json`, `thiserror`, `rand_chacha` (synth, feature `synth`).
- **WS-3** Every crate MUST declare `license = "Apache-2.0"` and `cargo deny check` MUST
  pass with the allow-list: Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib,
  CC0-1.0, BSL-1.0, Unicode-3.0; `OpenSSL` only for `aws-lc-sys`/`ring`; `MPL-2.0` only
  for `uniffi*` crates. Any other licence fails the build (GATE-5).
- **WS-4** `.gitignore` MUST gain `rust/target/`, `**/*.xcframework/` and
  `rust/.sqlx/` MUST be committed (offline query data).
- **WS-5** `scripts/rust_test.sh` MUST run, in order: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `cargo deny check`, `cargo sqlx prepare --check --workspace` (needs `DATABASE_URL`
  or `SQLX_OFFLINE=true`), and `asd-cli schema check`. `.github/workflows/rust.yml` runs
  the same script with a `postgres:16` service container.

## 2. Domain types and contract (CON)

- **CON-1** Serialisable domain types live in `asd-core::types` and are 1:1 ports of the
  Swift `Codable` structs with identical JSON key names: `NormativeBaselineSet`,
  `NormativeVideo`, `NormativeSegment`, `NormativeExpectation`, `NormativeRange`,
  `NormativeDirection` (`higherBetter|lowerBetter|band`), `AgeBracket`, `StimulusCatalog`
  (+ `Clip`, `Area`, `Membership`, `AgePack`), `PerVideoMetricSet`, `PerVideoScoreRow`,
  `PerVideoScoreReport`, `PerVideoSessionScore`, `ExpectationStatus`
  (`onTarget|monitor|atypical`), `AttentionFlag` (`green|yellow|red`),
  `AttentionFlagAssessment`, `GazeFrame` (only the fields the extractors read:
  `stimulusId`, `activeAOINames`, `qualityVerdict`, `gaze.{isValid,xNorm,yNorm,videoTimeSec,clipMediaTimeSec}`,
  `affect.{socialSmileIntensity,expressivityIndex,surpriseScore}`), `GazeTelemetrySample`,
  `GazeQualityGate::Verdict` (`optimal|acceptable|degraded|rejected`), `CaptureSource`
  (`truedepth|simulated|synthetic`). Optional Swift fields MUST be `Option<T>` with
  `skip_serializing_if = "Option::is_none"` where Swift uses `encodeIfPresent`.
- **CON-2** `asd-contract::session_v1_6` defines `SessionIngestRequest`,
  `SessionPerVideoRow` (+ `Details`), `SessionIngestResponse` and `GazeSessionRecord`
  exactly as `SessionSyncClient.swift` encodes them (F4). Required fields: `sessionId`,
  `participantToken`, `captureSource`, `durationSec`, `validRatio`,
  `socialPreferenceScore`, `qualityVerdict`, `schemaVersion`, `rawPayload`. `attentionFlag`
  and `battery` are `Option<Option<String>>`-style nullable (explicit `null` allowed).
  No field has a server-side default value (F9 is a defect, not a contract).
- **CON-3** `asd-cli schema export --out docs/spec/schemas/asd/` MUST write
  `session-v1.6.json` (`$id https://schemas.curv.org/asd/session-v1.6.json`, title
  `SessionIngestRequest`), `config-bundle-v1.json`, `api-v1.json` (one document with
  `$defs` for every `/v1` DTO). `asd-cli schema check` MUST exit non-zero when the
  committed files differ from a fresh export after JSON canonicalisation.
- **CON-4** Before the regenerated `session-v1.6.json` replaces the Pydantic one, both
  `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json` and one output of
  `asd-cli synth` MUST validate against the new schema (`asd-cli schema validate`), and
  a payload without `sessionId` MUST fail.
- **CON-5** Config documents: `StimulusCatalog` and `NormativeBaselineSet` (CON-1),
  `ScoringProfile` (new: `schemaVersion`, `flag: {greenFloor, yellowFloor, lowConfidenceShare, atypicalShareForRed}`,
  `lowConfidenceValidRatio`, `qualityGate: {minDistanceCm, maxDistanceCm, maxYawRad, maxPitchRad, minConfidence, blinkThreshold}`),
  and `ConfigBundle { bundle_version, catalog, baselines, scoring_profile, feature_flags, signed_at, signature }`.
  Bundled defaults MUST equal today's Swift constants (F3, `GazeQualityGate.init`).
- **CON-6** Bundle signature = Ed25519 over the JCS canonical form (RFC 8785, crate
  `serde_jcs`) of the bundle with `signature` removed; encoded base64url without padding,
  prefixed `ed25519:`. `asd-contract::verify_bundle(bundle_json, public_key)` MUST be the
  only verifier used by apps and backend. Key ids: `key_id` field, so keys can rotate.
- **CON-7** `asd-sync` MUST provide, without performing I/O: `build_session_request`
  (method, path `v1/sessions`, headers `Authorization`, `X-Idempotency-Key = sessionId`,
  `X-Schema-Version`, `User-Agent`, `Content-Type`, `Accept`, body bytes with sorted keys
  and RFC 3339 UTC millisecond dates), `classify_response(status, headers, body) ->
  Accepted{response} | Duplicate | Reauthenticate | RetryAfter(secs ≤ 3600) | Fatal`,
  `validate_upload_url(url, api_host) -> Ok{attach_bearer: bool} | Err` implementing F7,
  and `backoff_schedule(attempt)`. The Swift `SessionSyncClient` MUST call these instead of
  its own copies by WP-A.11 (its tests become the parity oracle).

### Contract mismatches found and decided

| Id | Mismatch | Decision |
| --- | --- | --- |
| CON-10 | iOS expects `device_token`, `org_id`, optional `device_id`, `config_bundle_version`; Python added `status`. | Response is `{status:"registered", device_id, device_token, org_id, config_bundle_version: string\|null}`. |
| CON-11 | Enrol codes "one-time" vs iOS re-registration with the stored code (F6). | Codes have `max_uses` (default 1) and `expires_at` (default 7 days). Request gains optional `device_id`; when present and the device was enrolled by that code and is not revoked, the server rotates that device's token without consuming a use (`status:"rotated"`). Without `device_id` a consumed code → `410 Gone` (problem type `enrol-code-exhausted`). iOS sends `device_id` in WP-A.11. |
| CON-12 | Mixed casing (F4). | Kept as is in 1.6; serde `rename` per field. 1.7 is the earliest point to normalise. |
| CON-13 | Units/enums in synth and parity fixtures (F5). | Contract: `dwell_*` are ratios in [0,1]; `status ∈ onTarget\|monitor\|atypical`; `captureSource ∈ truedepth\|simulated\|synthetic`. Backend rejects `simulated` always (SIM-5) and `synthetic` unless `ASD_ALLOW_SYNTHETIC=true`. Rust synth emits ratios and the 3 statuses. |
| CON-14 | Server defaults for measurements (F9). | None. Missing required field → 400 problem `validation-failed` with `invalid_params`. |
| CON-15 | Flag: iOS 3 values, console 4. | `attentionFlag: null` from the device → server computes with `asd-core::flag::assess` over `rawPayload.perVideo` when `validRatio ≥ 0.60` and ≥1 scored clip, else stores `unreliable`. iOS enum unchanged. |
| CON-16 | `battery` enum `express\|comprehensive` (console) vs protocol ids `express_30s\|comprehensive_60s` (iOS, catalog). | Wire value is the catalog protocol id. Console schema becomes `z.string().nullable()` (WP-C.8). |
| CON-17 | 409 semantics (F6, F9). | Same key + same body hash → `200` with the original body and `duplicate: true`. Same key + different body → `409` problem `idempotency-key-reused`. iOS treats 409 as accepted today; acceptable because `sessionId` is globally unique, documented as a known iOS limitation. |
| CON-18 | Upload URLs relative (Python) vs pre-signed absolute (02 ADR-B3). | `BLOB_MODE=presigned` returns absolute S3 URLs (15 min, PUT, `Content-Type` bound, ≤ 50 MB). `BLOB_MODE=proxy` (demo, no DNS for MinIO) returns relative `/v1/sessions/{id}/blobs/frames` and `/blobs/report` served under the device bearer. Both satisfy F7. |
| CON-19 | Console clips columns (F8) vs iOS multi-row `details.rows`. | `GET /sessions/{id}/clips` item = per-clip row from `session_clip_scores` plus `rows: MetricRow[]` (`segment_id, metric, target_aoi, direction, expected, acceptable_low, acceptable_high, recorded, deviation = recorded − expected, status, score, imputed, source`). The flat `metric/expected/recorded/deviation/baseline_ref` are filled from the worst-status row (ties: largest \|deviation\|). |
| CON-20 | Console `participant_id` ambiguity. | `participant_id` = `participants.id` (UUID); list and detail also carry `participant_token`. |
| CON-21 | `X-Schema-Version` vs body `schemaVersion`. | Must be equal and supported (`1.6`), else `400` problem `unsupported-schema-version`. |
| CON-22 | Health paths. | `GET /health` (liveness), `GET /health/ready` (DB + blob store). No `/v1/health`. |
| CON-23 | Legacy shims `/api/v1/research/*`, `/api/v1/carekit/*`. | Not implemented. `tools/asd_research_server.py` and its test are deleted (GATE-4). |

## 3. Core maths (CORE)

Each item names the Swift source of truth at `9655fcc`; the Rust result MUST equal it on
the golden vectors to 1e-9 for floats and exactly for strings and enums.

- **CORE-1** `scoring::evaluate(observed, direction, range) -> (ExpectationStatus, f64)`
  and `scoring::piecewise` = `PerVideoScoringEngine.evaluate/piecewise` including the
  degenerate `band` branch (`high <= low` → `onTarget`/`monitor`, 1.0/0.5).
- **CORE-2** `scoring::score_video(metric_set, video, bracket_id, clip_frames, clip_duration_ms)`
  = `scoreVideo`: first-entry latency imputed at `max(observedDurationSec, 0.5) * 1000`
  and marked `imputed`; non-imputable missing metrics skipped; segment windows use
  `clipMediaTimeSec ?? videoTimeSec` in `[startSec, endSec)` and fall back to the whole-clip
  metric set when empty.
- **CORE-3** `scoring::score_session(metric_sets, baseline, age_months, frames, scored_at)`
  = `score` (composite = Σ w·subscore / Σ w over clips with a baseline; `0` when none;
  `resolvedBracketId` clamps to the nearest bracket, `"default"` when no brackets).
  `baselines::validate(set, required_video_ids) -> Vec<String>` = `NormativeBaselineLibrary.validate`
  with identical message text.
- **CORE-4** `flag::assess(score, profile) -> AttentionFlagAssessment` =
  `AttentionFlagAlgorithm.assess` with thresholds from `ScoringProfile` (defaults F3)
  and identical headline/detail strings.
- **CORE-5** `metrics::extract(frames) -> Vec<PerVideoMetricSet>` and `extract_clip` =
  `PerVideoMetricsExtractor` (valid frame = `gaze.isValid && qualityVerdict != rejected`;
  group order = first appearance; I-VT with velocity 5.0 units/s, min 80 ms, 1/60 s
  fallback interval; dt ≤ 0 closes the run). The `AOIRole` name sets MUST be data in the
  catalog `roles` first and the hard-coded sets second (same precedence as Swift).
- **CORE-6** `aggregate::session(valid_frames, duration_sec)` =
  `SessionBiomarkerAggregator.aggregate` (`None` latency when no person/eye hit, never a
  default).
- **CORE-7** `quality::evaluate_sample(sample, gate) -> Evaluation` = `GazeQualityGate.evaluateSample`
  including the reason strings (they are shown in the UI).
- **CORE-8** `norms::aggregate(records, params{min_n=12, include_imputed=false, brackets})
  -> NormativeBaselineSet` = `tools/asd_norm_aggregator.py` (F13): median expected, P10
  `acceptableLow` for higher-is-better, P90 `acceptableHigh` for `first_entry_latency_ms`
  and `flat_affect_score`, cells withheld (`null`) when n < min_n or spread = 0,
  `schemaVersion "1.0.0-candidate"`, `status "candidate-collected"`, linear-interpolated
  percentile. Input accepts vault JSONL lines (`type: per_video_score`) and single-object
  exports with `per_video`.
- **CORE-9** `synth::generate_session(seed, index, profile) -> SessionIngestRequest`
  (feature `synth`): deterministic from `(seed, index, profile)` via ChaCha20 seeded with
  `blake3`/`sha2` of `"asd_synth:2.0:{seed}:{index}"`, profiles `typical|reduced_social|low_quality|short`,
  bands and clips as in `tools/asd_synth/generator.py`, but ratios and 3 statuses (CON-13),
  `rawPayload` populated with a consistent `GazeSessionRecord`. `GENERATOR_VERSION = "2.0"`;
  outputs are not byte-compatible with the Python 1.0 generator and MUST NOT claim to be.

## 4. FFI and Swift integration (FFI)

- **FFI-1** `asd-ffi` exposes via UniFFI proc-macros: `score_session_json(metric_sets_json,
  baseline_json, age_months, frames_json, scored_at_rfc3339) -> String`,
  `assess_flag_json(score_json, profile_json) -> String`, `extract_metrics_json(frames_json) -> String`,
  `aggregate_session_json(frames_json, duration_sec) -> String`,
  `evaluate_quality_json(sample_json, gate_json) -> String`, `validate_baselines_json(set_json, required_ids) -> Vec<String>`,
  `verify_bundle(bundle_json, public_key_b64) -> bool`, `sync_build_session_request_json(...)`,
  `sync_classify_response(...)`, `core_version() -> String`. Errors are a single
  `AsdError { kind, message }` enum; malformed JSON never panics across the boundary
  (`catch_unwind` is not relied on; every entry point returns `Result`).
- **FFI-2** `scripts/build_asd_xcframework.sh` builds `aarch64-apple-ios`,
  `aarch64-apple-ios-sim`, `aarch64-apple-darwin` static libs, runs `uniffi-bindgen`
  (Swift), and assembles `build/asd/AsdCore.xcframework` + `AsdCoreFFI` Swift sources.
  Output is never committed (WS-4). `scripts/ios_test.sh` calls it first.
- **FFI-3** `domains/asd_screening/ios/Package.swift` adds `binaryTarget(name: "AsdCoreFFI", path: ...)`
  and a wrapper target; `PerVideoScoringEngine.score`, `AttentionFlagAlgorithm.assess`,
  `PerVideoMetricsExtractor.extract`, `SessionBiomarkerAggregator.aggregate`,
  `GazeQualityGate.evaluateSample`, `NormativeBaselineLibrary.validate` keep their public
  signatures and delegate to Rust. The former Swift bodies move to
  `Tests/CurvAsdGazeKitTests/Support/LegacyReference/` as the parity oracle for one release.
- **FFI-4** Parity: `rust/crates/asd-core/tests/golden/{scoring,flag,metrics,aggregate,quality}/*.json`
  hold `{input, expected}` pairs generated once from the Swift oracle (via a Swift test
  that writes them when `ASD_WRITE_GOLDEN=1`). `cargo test` and the kit's
  `GoldenParityTests` MUST both pass on the same files. A change to a golden file requires
  a new file with a new name, never an edit.

## 5. Backend API (API, AUTH, ING, CFG)

Base `/v1`, JSON, RFC 7807 `application/problem+json` for every error with `type`
`https://schemas.curv.org/asd/problems/<slug>`. List endpoints: `cursor` (opaque
base64url of `(started_at, id)`), `limit ≤ 100`, response `{items, next_cursor, total?}`.

- **API-1** Routes (device plane, bearer = device token): `POST /devices/register`
  (CON-10/11), `GET /config/bundle` (ETag/If-None-Match → 304), `POST /sessions` (ING-*),
  `PUT /sessions/{id}/blobs/{frames|report}` (CON-18 proxy mode only),
  `POST /care/events` (idempotent batch), `GET /participants/{token}/summary`,
  `POST /participants/{token}/erasure`.
- **API-2** Routes (user plane, bearer = JWT): `POST /auth/login|refresh|logout`,
  `GET /orgs/{id}`, `PATCH /orgs/{id}`, `GET/POST/PATCH /orgs/{id}/users`,
  `GET /orgs/{id}/devices`, `POST /orgs/{id}/devices/enrol-code`, `POST /devices/{id}/revoke`,
  `GET /orgs/{id}/participants`, `GET /orgs/{id}/sessions?flag=&quality=&from=&to=&bundle_version=&reviewed=`,
  `GET /sessions/{id}`, `GET /sessions/{id}/clips` (CON-19), `GET /sessions/{id}/report.pdf`
  (device-uploaded blob or 404 until WP-B.8), `GET /orgs/{id}/metrics` (console shape F8),
  `GET/POST /orgs/{id}/review`, `PATCH /review/{id}`, config routes (CFG-*),
  `GET /orgs/{id}/audit`, `GET /health`, `GET /health/ready`, `GET /metrics` (Prometheus).
- **API-3** Response bodies MUST be the `asd-contract::api_v1` types; the console's zod
  schemas are checked against `api-v1.json` in `apps/curv_console` tests (WP-C.8).
- **AUTH-1** Passwords: Argon2id, m = 19456 KiB, t = 2, p = 1, 16-byte salt; login
  failure is constant-time against a dummy hash; `10/min/IP` → 429 + `Retry-After`.
- **AUTH-2** Access token: JWT EdDSA (Ed25519, separate key from CFG signing), 15 min,
  claims `sub, org, role, jti, exp, iat`. Refresh token: 32 random bytes base64url, stored
  SHA-256 with `family_id`, 30 days, rotated on every `/auth/refresh`; presenting a
  rotated-out token revokes the whole family. `TokenPair = {access_token, refresh_token,
  expires_in, user{id,email,role,org_id}}`.
- **AUTH-3** Device tokens: `dvc_` + 32 random bytes base64url, SHA-256 at rest, bound to
  `(device_id, org_id)`, revocable; `POST /sessions` is `60/min/device`.
- **AUTH-4** RBAC roles `owner, admin, clinician, researcher, reviewer, viewer` org-scoped
  and global `superadmin`; an extractor `Require<Role>` applied per route from a single
  matrix table in `auth/rbac.rs`; a request for another org's resource → 404, not 403.
- **AUTH-5** Every mutating handler writes one `audit_log` row in the same transaction;
  the migration grants the application role no `UPDATE`/`DELETE` on `audit_log`.
- **ING-1** `POST /sessions` MUST validate against `SessionIngestRequest` (CON-2,
  CON-13, CON-21), resolve `org_id` from the device token, upsert the participant by
  `(org_id, token)`, and insert `sessions` + `session_clip_scores` + `idempotency_keys` in
  one transaction using `INSERT ... ON CONFLICT (key) DO NOTHING` followed by a read of
  the stored response (CON-17). 20 concurrent identical posts MUST yield one row.
- **ING-2** Derived columns: `attention_flag` (CON-15), `composite_score =
  rawPayload.perVideoComposite`, `composite_confidence = 1 − share of lowConfidence clips`,
  `battery` from the payload, `bundle_version` from `rawPayload.bundleVersion` when
  present else `null`, `received_at = now()`, `summary = rawPayload`.
- **ING-3** Review auto-rules in the same transaction: `low_quality` when `validRatio < 0.80`,
  `flag_red` when the stored flag is `red`, `random_audit` with probability `org.settings.audit_rate`
  (default 0.05, deterministic from `sha256(session_id)`), `norm_candidate` when
  `rawPayload.normCollection.consented == true`.
- **CFG-1** `config_documents` kinds `stimulus_catalog | normative_baselines | scoring_profile`,
  lifecycle `draft → candidate → published → retired`; only `published` is bundled.
  `POST /config/bundles` composes the three published documents, signs (CON-6) with the
  key in `ASD_BUNDLE_SIGNING_KEY` (PEM/base64; KMS in Phase 3), stores and serves it.
  `GET /config/bundle` returns the latest bundle for the device's org (or the global
  default), records `(device_id, bundle_version, at)` for adoption metrics.
- **CFG-2** On first boot with no documents, the service MUST seed `draft` documents from
  the files under `domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/` plus the
  default `ScoringProfile`, never auto-publish.
- **OBS-1** Logs are `tracing` JSON; a redaction layer drops fields named
  `participant_token`, `email`, `notes`, `rawPayload` and anything matching an e-mail
  regex. `tests/log_redaction.rs` MUST prove no participant token reaches a log line.

## 6. CI gates replacing Python (GATE)

- **GATE-1** `asd-cli schema check` replaces `scripts/check_session_schema_parity.py` in
  `scripts/ios_test.sh` and `.github/workflows/ios.yml` (install Rust via `rustup` on the
  macOS runner, cache `rust/target`).
- **GATE-2** `asd-cli synth --seed N --count M --out dir|-` replaces `python -m tools.asd_synth`;
  `rust/docker-compose.yml` seed service uses it. Tests: determinism, schema validity, all
  profiles covered for `count ≥ 5`, no `simulated` capture source.
- **GATE-3** `asd-cli norms aggregate --input dir --output file [--min-n 12]` replaces
  `tools/asd_norm_aggregator.py`; the 11 cases in `tests/test_asd_norm_aggregator.py` are
  ported one-for-one into `asd-core/tests/norms.rs` before the Python file is deleted.
- **GATE-4** Deleted together, in one package after GATE-1..3 pass:
  `scripts/check_session_schema_parity.py`, `tools/asd_norm_aggregator.py`, `tools/asd_synth/`,
  `tests/test_asd_norm_aggregator.py`, `tools/asd_research_server.py`,
  `tests/test_asd_research_server.py`; `docs/roadmap_2026q4/README.md` step 4 and
  `apps/ios_asd_gaze_app/README.md`, `domains/asd_screening/README.md` updated.
- **GATE-5** `cargo deny check` (WS-3) runs in `rust.yml` and in `scripts/rust_test.sh`.

## 7. Deployment (DEP)

- **DEP-1** `rust/Dockerfile`: multi-stage (`rust:1-bookworm` builder with cargo-chef
  layer caching → `gcr.io/distroless/cc-debian12`), one image, entrypoints
  `asd-backend serve` and `asd-backend worker`; migrations run with `asd-backend migrate`
  as a compose `init` step, never implicitly on boot in production.
- **DEP-2** `rust/docker-compose.yml`: `api`, `worker`, `postgres:16`, `minio`, `seed`
  (runs `asd-cli synth` + posts through the API with a seeded enrol code). `.env.example`
  committed; `.env` ignored. The demo exposes `api` through Cloudflare Tunnel (iOS needs
  https + DNS, F7); `BLOB_MODE=proxy` for the demo.
- **DEP-3** AWS (Phase 3, WP-B.7): ECS Fargate service (api + worker), RDS Postgres 16
  (encrypted, private subnets), S3 (SSE-S3, versioned, block public), Secrets Manager
  (DB URL, JWT key, bundle signing key), ALB + ACM + WAF rate rules, CloudWatch logs.
  IaC language: owner decision H-11 (ADR-0006 D9).

## Public Rust API sketch (load-bearing names only)

```rust
// asd-core
pub mod types;      // CON-1
pub mod scoring;    // evaluate, piecewise, score_video, score_session, resolved_bracket_id
pub mod baselines;  // validate, bracket_id_for
pub mod flag;       // assess
pub mod metrics;    // extract, extract_clip, fixation_stats, AoiRole
pub mod aggregate;  // session
pub mod quality;    // evaluate_sample
pub mod norms;      // aggregate, percentile
#[cfg(feature = "synth")] pub mod synth;

// asd-contract
pub mod session_v1_6; pub mod config; pub mod api_v1; pub mod problem;
pub mod schema;     // export_all(out_dir), check(out_dir) -> Result<(), Vec<Drift>>
pub mod signing;    // sign_bundle, verify_bundle, canonical_bytes (JCS)

// asd-sync
pub fn build_session_request(..) -> SessionRequest;
pub fn classify_response(status: u16, headers: &[(String,String)], body: &[u8]) -> SyncOutcome;
pub fn validate_upload_url(url: &str, api_host: &str) -> Result<UploadTarget, SyncError>;
```
