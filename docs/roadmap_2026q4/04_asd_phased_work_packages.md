# 04 — ASD Phased Work Packages (2026-10-27 → 2027-02-28)

> Status table is the live tracker. Update it when a WP changes state. Phases overlap on purpose: app, backend and console streams run in parallel when they share no files.
> Agent tiers: **A** = `architect` (Fable, design only), **S** = `senior-implementer` (Opus), **I** = `implementer` (Sonnet). Main session integrates, runs gates, commits.
> **2026-10-30:** backend and shared maths are Rust (`CLAUDE.md`, ADR-0006). New prefix **WP-R.n** = Rust core crates (`rust/`). WP-B.* were renumbered to the Rust plan; detail per package (files, tests, traps) is in `docs/spec/modules/asd-rust-core-and-backend/work-packages.md`, this table stays the status tracker.
> **2026-11-01 (phase 5):** security-operations batch designed in ADR-0009; MUST ids in `docs/spec/modules/asd-rust-core-and-backend/technical.md §8`, package detail in that module's `work-packages.md` ("Phase 5 wave"); plan summary in §7 below.
> **2026-11-02 (phase 6):** Swift 6 mode, `Biomarker*` renames + wider claims gate, mandatory TOTP per role designed in ADR-0010; MUST ids CONC-5, CI-3, CLM-7/8 (`asd-gaze-app-store-readiness/technical.md`) and AUTH-11..14 (`asd-rust-core-and-backend/technical.md §9`); package detail in both modules' `work-packages.md` ("Phase 6 wave"); plan summary in §8 below.
> **2026-10-31:** new prefixes **WP-I.n** = interoperability and export bridge (`asd-export` crate, deliveries) and **WP-C.9+** = clinician workspace; detail in `docs/spec/modules/asd-clinician-and-interop/work-packages.md` (ADR-0008). **WP-G.n** = gaze-tracking robustness; detail in `docs/spec/modules/asd-gaze-robustness/work-packages.md` (ADR-0007).
> **2026-11-02 (phase 7, vision):** the owner's product vision (`docs/product/VISION.md`: EMR-lite for clinics with no record system, share-a-case to a US doctor/researcher, offline-first, one-command deployment, Rust core on every OS) is designed in ADR-0011; MUST ids REC/SHR/SYNC/DPL/HIP/TRS/AND in `docs/spec/modules/asd-emr-lite-and-sharing/technical.md`, package detail in that module's `work-packages.md`; plan summary in §9 below. The hospital-push half of ADR-0008 (WP-I.9, I.10, I.11, B.10) is **deferred**; WP-B.7 (AWS) is superseded by WP-B.18; WP-B.17 is promoted to wave 1; WP-C.12 is re-scoped to the clinical summary PDF. Migration numbers: 0024 B.19, 0025 I.13, 0026 B.21.
> **2026-11-03 (phase 8, researcher toolkit + three-audience platform):** the owner's requests ("admin portal as a data-scientist toolkit: tune the eye-movement algorithm's ranges dynamically, age-driven analysis"; "one tool for researchers, doctors and families alike, mobile and web"; "nothing hard-coded, every string localised, tunable at run time") are designed in ADR-0012 (toolkit) and ADR-0013 (platform, principles, gap list); MUST ids PRM/INV/RFL/RPX/EXP/COH/RUN/SWP/AGE/PRV/GOV/RUI in `docs/spec/modules/asd-research-toolkit/technical.md`, packages **WP-X.n** in that module's `work-packages.md`; localisation **WP-L.n** and mobile/family **WP-M.n** rows below with one-line scope (detail when scheduled); plan summary in §10. WP-C.6 is folded into phase 8. Migration **0026** = WP-X.0; **WP-B.21 moves to 0027**.
> **2026-11-03 (phase 7, wave 2 step 0 done):** shared seams committed (REC-8 RBAC rows, stub routers `encounters`/`attachments`/`follow_ups`/`share`, `ExportFormat::{case_package,audit_csv}` + `ExportRequest.case` + audit filters + `Export.{case_participant_id,share_links_n}`, timeline kinds `encounter`/`attachment`, `ErasureOutcome.attachments_deleted`, console `emr`/`share` API stubs and permissions) with the wave 1 defect fixes 1–4 (HIP-2 on client navigation and server actions, requester on `export.file_served`, one shared step-up limiter, contract `sanitize_file_name` in the case package); wave 2a can start.

> **2026-11-08 (store build):** the App Store build is an observation-capture tool with no on-device verdict (owner decision); design record `docs/spec/modules/asd-gaze-app-store-readiness/store-collection-build.md`, packages **WP-S.n** below. Still human: H-6 account/domain, H-8 clip licence, U7 device checklist, store screenshots from the store build.

## 0. Status table

| WP | Title | Phase | Tier | Status | Notes |
|---|---|---|---|---|---|
| WP-A.1 | Device checklist + demo-tools screen | 1 | I | done | DemoToolsView, DemoRelease config, build tests, symbols gate |
| WP-A.2 | Fix known bugs (double CareKit write, delete-all semantics, classifier double-run, elevatedRisk string) | 1 | S | done | RiskStratification scoped to ASD_RESEARCH_CLASSIFIER, duplicate writes eliminated, classifier passed to summary |
| WP-A.3 | Protected file-backed history store (old WP-4.3) | 1 | S | done | Protected file store (.completeFileProtectionUntilFirstUserAuthentication), defaults migration, dedupe preserved |
| WP-A.4 | Sync client: device registration, bearer token, idempotency + schema headers, new `/v1/sessions` payload, pre-signed uploads | 1 | S | done | Device registration, Keychain token, idempotent /v1/sessions client, pre-signed uploads; 338 app tests green; contract unverified against backend |
| WP-A.5 | `ScoringProfile` + remote config bundle consumption with Ed25519 verify and bundled fallback | 2 | S | done | 2026-11-01: signature check, rollback refusal and expiry in Rust (`asd-sync::config_bundle`, `asd-ffi` `config_*`); Swift `ScoringConfiguration` + `ConfigBundleService` (Application Support cache, bundled fallback). Trusted-keys list in Info.plist ships empty: no remote bundle is trusted until the owner adds a real key. `ScoringProfile.trackingQuality` wiring → WP-A.13 (phase 5) |
| WP-A.6 | Swift 6 language mode (old WP-4.2): A.6a kit `swift-tools-version: 6.0` (I), A.6b app sources + `project.yml` `SWIFT_VERSION: "6.0"` (S), A.6c app tests (I) | 6 | I/S/I | done | ADR-0010 d1–d2, CONC-5, CI-3; phase 6 wave 1, the three run in parallel (disjoint files). Survey 2026-11-02 at `03d84a3`: kit 0 errors in mode 6, app 8 errors in 7 files, ~70 test-target diagnostics in 13 files; detail in `asd-gaze-app-store-readiness/work-packages.md` |
| WP-A.7 | `Biomarker*`→`Engagement*` renames + claims gate covers prose literals and the kit (old WP-4.4): A.7a scanner `--prose` + selftest (I, wave 1), A.7b renames + triage of ~24 findings + gate wiring (I, wave 2, alone among iOS packages) | 6 | I | done | ADR-0010 d3–d4, CLM-7/8; 2026-11-02 wave 2: 11 files renamed, 23 prose findings triaged (18 reworded, 5 `claims-ok`), `ios_test.sh` runs `--prose`, `WireNamePinningTests` pins the `biomarker` JSON keys; Swift identifiers only: the JSON property `biomarker` (session 1.6–1.8, config bundle, api-v1), Rust fields, fixtures, `"DigitalBiomarker"` stay. Q5: extend the Python scanner now; port to `asd-cli` = backlog WP-D.8 |
| WP-A.8 | Stimulus CDN support with signed URLs; ODR fallback | 3 | S | todo | |
| WP-A.9 | Replay down-sample upload (for console heatmaps) | 3 | S | todo | |
| WP-A.10 | TestFlight beta pipeline (fastlane `beta`), crash reporting decision | 3 | I | todo | |
| WP-A.11 | iOS consumes the Rust core: xcframework `binaryTarget`, engines become wrappers, golden parity tests, `asd-sync` in the sync client, `device_id` on re-register | 1–2 | S | done | 6 kit engines are FFI wrappers; GoldenParity + LegacyParity tests; sync via asd-sync, device_id on re-register; ios_test.sh builds xcframework. Follow-ups: asd-ffi format_scored_at ms rounding, dwell/affect/bracket maths still in Swift |
| WP-A.12 | Android Kotlin bindings from the same UniFFI definitions (`cargo-ndk`), `cdylib`, `scripts/build_asd_android.sh`, Gradle AAR `apps/android_asd_core/` with golden parity test, CI job | 7 | S | done | AND-1..3; phase 7 wave 1 (ADR-0011 D10); independent of the capture-device decision; 2026-11-02 wave 1: AAR + 13 JVM parity tests; uniffi Kotlin rename `message`→`detail`; `android-bindings` CI job |
| WP-A.14 | Android front-camera gaze feasibility spike (ARCore Augmented Faces vs MediaPipe, two cheap phones, Rust raw logs replayed by `asd-cli gaze replay`) → number sheet for the owner | 7 | S | in progress | 2026-11-03: spike app + protocol committed (7e926f2); accuracy column via WP-R.8 convert-android; device runs pending (owner); AND-4; phase 7 wave 2, after A.12; throw-away app, no product code; feeds ADR-0011 Q1 (capture device) |
| WP-R.8 | Rust: MediaPipe landmarks → ARKit-equivalent gaze inputs (`asd-core::gaze::mediapipe`), `TrackingSource::mediapipeFace/arcoreFace`, `asd-cli gaze convert-android` | 7 | S | done | 2026-11-03 wave 2a; 1 px iris error ≈ 5.5° at 50 cm/640×480: raise spike analysis resolution before device runs; FFI export later |
| WP-A.15 | iOS offline-first sync hardening (flush on enqueue/launch/foreground/timer, background `URLSession` blob uploads, Rust backoff, dead-letter cycles, 409 slug classification in `asd-sync`, `Content-Digest`, clinic sync status) + fault-injection tests + `scripts/field_sync_test.sh` (toxiproxy) | 7 | S | done | SYNC-2..10; phase 7 wave 1; 2026-11-02 wave 1: flush triggers, background blob uploads, `Content-Digest`, `asd-sync` `classify_response_with_body` (+ FFI), dead-letter cycles, sync status line (en/ne/hi), 18 fault-injection scenarios. `scripts/field_sync_test.sh` (toxiproxy) not yet run locally; `field-sync` CI job is `workflow_dispatch` only |
| WP-A.16a | iOS: stable participant token per child profile (today `child-<sessionId prefix>` = a new participant per clinic session) | 7 | I | done | REC-11; phase 7 wave 1; file-disjoint from A.15; 2026-11-02 wave 1: token stable per child, rotates on rename, forgotten on delete-all. Follow-up: `GazeSessionResultView.swift:63` still builds `child-<session prefix>` for the local FHIR/PDF export; switch to the profile token |
| WP-A.16b | iOS clinic mode: scan the console's participant QR / pick recent; `parse_qr_payload` via FFI | 7 | I | todo | REC-12; phase 7 wave 3, after B.19 |
| WP-A.13 | `ScoringProfile.trackingQuality` into the gaze pipeline via `asd-ffi gaze_config_json_for_profile` (CFG-3) | 5 | S | done | phase 5 wave 1; closes the A.5 gap; H-G2 gates only G.13, not this; 2026-11-01: `gaze_config_json_for_profile` builds the pipeline config from the profile and refuses invalid values (`TrackingQualityParams::validate`, also run by `profile_checks` at publish and by `accept_bundle` on device); the kit takes its gate from that one config |
| WP-G.0 | Swift capture hygiene: record untracked frames, no cross-child calibration reuse, no fabricated calibration point, orientation lock | 2 | I | done | TRK-1, CAL-1, CAL-3; lowers validRatio on track loss (H-G1); spec `docs/spec/modules/asd-gaze-robustness/` | done 2026-11-01: untracked frames buffered, `BoundCalibrationStore` (child key + session id), `CalibrationAccumulator` (no fabricated point), `CaptureOrientationLock`; device check of rotation/overlay pending |
| WP-G.1 | Rust tracking types, raw log format, device geometry table, unclamped raycast, visual-angle maths | 2 | S | done | `asd-core::gaze` geometry/raycast/raw_log + tracking types; 42 device entries (community id list, M4 iPad camera offset + iPad16,5/6 to verify in G.14); ARKit camera-axis mirroring to confirm in G.8 |
| WP-G.2 | Rust calibration models (screen/angular affine, homography, poly2), robust target aggregation, LOO-CV quality, selection | 2 | S | done | 4 models + MAD aggregation + retry-once + LOO bands + auto-select + CalibrationSession; 49 tests; angular-affine holds <0.3° across a 10 cm head move vs >1° for screen-affine (synthetic) |
| WP-G.3 | Rust per-sample pipeline: track state, confidence, 1-Euro port, gap fill, AOI video→screen mapping | 2 | S | done | 1-Euro matches Swift to 1e-12, confidence =0.5 at gate limits, gap fill never touches dwell/validRatio, AOI aspect-fill mapping, GazePipeline; host p99 0.0013 ms/frame. G.9 must call set_recording(true) around clips. 2026-11-01: per-clip `ClipTracking` producer (`begin_clip_json`/`end_clip`/`clip_tracking_json`, read at session end) |
| WP-G.4 | Rust validation, drift monitor, re-calibration advice, session TrackingQuality summary | 2 | S | done | validation/drift/summary in `asd-core::gaze`, 52 tests + 9 hand-built goldens; G.3 must supply session data loss + validRatio to `SummaryInput` |
| WP-G.5 | Rust synthetic tracking generator, replay, `asd-cli gaze synth/replay/bench-report`, scenario goldens | 2 | I | done | 15 seeded scenarios + goldens, `asd-cli gaze synth/replay/bench-report` (model ranking, light-vs-error Spearman, per-log SHA-256) |
| WP-G.6 | Contract session v1.7 (`trackingQuality`), `ScoringProfile.trackingQuality` params, schema export | 2 | S | done | session-v1.7.json alongside byte-identical 1.6; all 1.7 fields optional; ScoringProfile 1.1.0 with trackingQuality defaults from asd-core; backend/iOS still 1.6 until G.11/G.9 |
| WP-G.7 | asd-ffi `GazePipelineHandle` (flat per-frame records) | 2 | S | done | `GazePipelineHandle` + `GazeTelemetryWriterHandle` over UniFFI, byte-identical to asd-core on scripted + 36k-frame sessions; host p99 0.002 ms/frame; also fixed `format_scored_at` to match Foundation exactly (35,191 edge values) |
| WP-G.8 | iOS bench capture build (`ASD_BENCH_CAPTURE`, raw tracking JSONL, 13-point grid) | 2 | I | done | Debug-only flag (app + kit), `BenchCaptureView`/`BenchLogWriter`, `ARSessionBridge` tap, `check_release_symbols.sh [Release\|DemoRelease]`; on-device 10 s log + `asd-cli gaze replay --check` and the free-viewing clip still open (owner, G.14); `ios_test.sh` builds DemoRelease and runs the gate on it (2026-11-01) |
| WP-G.9 | Kit integration: GazeProcessingActor runs the Rust pipeline; session v1.7 writer; per-frame cost measured | 3 | S | done | 2026-11-01: actor pushes every frame through GazePipelineHandle, sessions posted as 1.7 with trackingQuality + per-clip ClipTracking (asd-sync gains the 1.7 request path); Swift raycast/1-Euro moved to test-only legacy refs; push p99 0.026 ms sim (budget 0.2 ms, device number pending). Until G.10 every live session verdict is `unusable` (calibration recorded as skipped). Scoring fallback (socialPreference 0 / flatAffect 1) still present: owner chose session 1.8 with `scoringStatus` + optional score fields |
| WP-G.10 | App UI: child-friendly calibration, validation step, inter-clip drift checks, re-calibration prompts | 3 | I | done | 2026-11-01: calibration + validation run through the Rust pipeline in one session per wizard run (Swift fit removed from the live path), balloon calibration, star validation, ladybug drift check after each clip, re-calibration offered only after a clip's check; good synthetic session → verdict `good`. Not done: CNF-2 glasses/light hint, Maestro/UI tests; child/device tuning (sounds 1103/1104, timing) pending |
| WP-G.11 | Backend accepts 1.6+1.7, tracking columns, `low_tracking_quality` review rule, API fields | 2 | S | done | 1.6 + 1.7 ingest (1.6 upgraded losslessly), migration 0013 tracking columns, `low_tracking_quality` review rule, list filter + detail `tracking_quality`; distance NULL when unmeasured (no more fabricated 35 cm for 1.7). `report.rs` decodes by schema_version (1.7 without calibratedDistanceCm no longer 500s; report builders never read the distance) |
| WP-G.12 | Console Quality tab tracking section, drift chart, loss bar, tracking filter | 2 | I | done | Quality tab tracking section (verdict, accuracy, calibration, loss bar, drift chart, conditions), Tracking column + filter, 'Not measured' for nulls/1.6; Playwright 16/16. Gap: drift warn/hard lines need thresholds in the API payload |
| WP-G.13 | Scoring uses tracking quality (accuracy cap, AOI resolution guard, I-VT in °/s), versioned | 3 | S | backlog | SCR-1..4; owner gate H-G2; depends on G.4, G.6 |
| WP-G.14 | Bench execution, device matrix, threshold and calibration-model decision | 3 | owner + I | todo | BEN-1..8, DEV-2/3; depends on G.8, G.5; H-G4, H-G5 |
| WP-R.0 | Cargo workspace skeleton at `rust/`, six crates, domain types ported 1:1 from Swift, `deny.toml`, `scripts/rust_test.sh`, `rust.yml` | 1 | I | done | 10 round-trip tests, fmt+clippy+deny clean; serial, run first |
| WP-R.1 | `asd-core` scoring engine, baselines validate, attention flag, quality gate + golden vectors | 1 | S | done | CORE-1..4, CORE-7 |
| WP-R.2 | `asd-core` metrics extraction (I-VT, AOI dwell, affect) + session aggregator + golden vectors | 1 | S | done | CORE-5..6 |
| WP-R.3 | `asd-contract` (session v1.6, config bundle, API DTOs, schemars export, Ed25519 + JCS signing), `asd-sync` protocol logic, `asd-cli schema|bundle`; regenerate `docs/spec/schemas/asd/*.json` | 1 | S | done | CON-2..7, CON-12; GATE-1 tool |
| WP-R.4 | `asd-core::norms` (port of `tools/asd_norm_aggregator.py`, 11 tests) + `asd-core::synth` (v2.0, ratios) + `asd-cli norms|synth` | 1 | I | done | CORE-8..9; GATE-2/3 tool |
| WP-R.5 | `asd-ffi` UniFFI crate + `scripts/build_asd_xcframework.sh` (ios, ios-sim, macOS slices) | 1–2 | S | done | UniFFI 0.32, FFI-1 surface + full asd-sync, 28 boundary tests; 3-slice xcframework (arm64 only) verified from SwiftPM on macOS, iOS sim and device build; deny.toml: MPL-2.0 exception `uniffi_pipeline`, RUSTSEC-2026-0374 ignored (bindgen-only) — add to H-12 |
| WP-R.6 | `asd-wasm` for console sandbox re-score (feeds WP-C.5) | 2 | S | done | 2026-11-01: `rescoreSession`, scoring primitives, `validateBaselines`, `contractInfo`; JSON envelopes; native == wasm32 parity on golden vectors; `scripts/build_asd_wasm.sh`. Open: scoring-profile range checks live in asd-backend (move to asd-contract), `lowConfidenceValidRatio` not applied by core scorer, backend-only `unreliable` flag |
| WP-R.7 | `asd-contract::emr`: encounter, attachment, follow-up, participant create/search, `ParticipantIdentity` (frozen for wave 3), case-share and share-link DTOs, QR payload build/parse, problem slugs, `$defs` in `api-v1.json` | 7 | S | done | REC-9; phase 7 wave 1; consumed by B.19, I.13, C.18, C.19, A.16b; 2026-11-02 wave 1: `emr.rs` DTOs + QR payload, 10 slugs, `$defs`; share-link defaults set at integration to 72 h (`expires_in_days` 3) and 1 download (ADR-0011 decision 5 recommendation; owner may still change) |
| WP-B.1 | Rust service scaffold: Axum, sqlx migrations for `02 §3` (+ enrol_codes, refresh_tokens, bundle_adoption), blob store trait (S3/memory), tracing redaction, Dockerfile, docker-compose, health | 1 | S | done | replaces the removed FastAPI scaffold; DEP-1/2, OBS-1 |
| WP-B.2 | Auth: Argon2id login, EdDSA JWT + rotating refresh families, device tokens, enrol codes (`max_uses`, `expires_at`, `device_id` rotation), RBAC matrix, audit rows + grants | 1 | S | done | AUTH-1..5, CON-10/11; depends on B.1, R.3 |
| WP-B.3 | `POST /v1/sessions` idempotent ingest (ON CONFLICT, body hash), derived flag via `asd-core`, clip rows, blob URLs (presigned/proxy), org-scoped list/detail/clips/metrics/review, `report.pdf` passthrough | 1 | S | done | ING-1..3, CON-13..21; depends on B.1, R.1, R.3 |
| WP-B.4 | Config documents lifecycle + bundles + signing + `GET /config/bundle` (ETag, adoption) + seed drafts from kit JSON | 2 | S | done | CFG-1/2; depends on B.2 |
| WP-B.5 | Norm aggregation job (worker loop, `asd-core::norms`) + promote to draft baselines | 2 | I | done | depends on B.3, B.4, R.4 |
| WP-B.6 | Care events (`/v1/care/events`), participant summary, erasure request (port of deleted `shims.py` semantics) | 2 | I | done | 3 device routes, migration 0006, per-event + per-batch idempotency, erasure request withdraws consent (deletion in B.9); 14 tests |
| WP-B.7 | AWS deployment (ECS Fargate, RDS, S3, Secrets, ALB/ACM, WAF, logs) under `aws_infra/asd/` | 3 | S | superseded | 2026-11-02 ADR-0011 D8: replaced by WP-B.18 (one-server Compose; the same stack runs on a cloud VM as a runbook). H-11 answered: Compose + shell + Caddy, no CDK; `aws_infra/` stays flood-only |
| WP-B.8 | Server-side PDF regeneration + FHIR bundle endpoint (parity with iOS output, golden tests) | 3 | I | done | New `asd-report` crate (FHIR R4 + PDF), byte goldens + iOS bundle parity; `report.pdf` now server-rendered (`?source=device` = uploaded copy); synthetic → 409. Gaps: M-CHAT/CareKit not on server, age bracket never set at ingest, iOS `_ms` shown as % bug |
| WP-B.9 | Dataset export job (parquet/jsonl + labels), retention sweep, erasure execution | 3 | S | done | 2026-11-01 (0017): dataset_jsonl/parquet export formats (always de-identified, research purpose), nightly retention sweep (frames blobs only, per 02 §5) with dry run, erasure job with tombstone; participants with signed clinical records are `held` (ADR-0008 Q2 open); console erasure/retention pages |
| WP-B.10 | FHIR façade + SMART-on-FHIR server validation (only if a clinic partner needs it) | 4 | S | deferred | 2026-11-02 ADR-0011 D1: no hospital systems at the target clinics; share-a-case (WP-I.12/I.13) is the way out |
| WP-B.11 | TOTP enrolment API: `/v1/users/me/totp/{enrol,confirm}`, `DELETE /v1/users/me/totp`, pending seed (migration 0020), DTOs + `api-v1.json` | 5 | S | done | AUTH-6..8, AUTH-10; phase 5 wave 1; 2026-11-01: enrol/confirm/disable routes, pending seed (0020), 4 slugs incl. `invalid-credentials`; confirm on an enrolled user is 409 `totp-already-enrolled`, expired pending is 410 and cleared; limiter is per process |
| WP-B.12 | `housekeeping_sweep` job: idempotency keys 7 d, expired refresh tokens, unreferenced expired enrol codes 90 d, unreferenced finished jobs 30 d | 5 | I | done | HK-1..5; phase 5 wave 1; reuses the runner and the nightly-scheduling pattern; 2026-11-01: `jobs/housekeeping.rs`, also clears expired pending TOTP seeds; indexes in 0021; `HousekeepingRequest/Result` in `asd-contract` |
| WP-B.13 | Prometheus `/metrics` on a separate listener (`ASD_METRICS_BIND_ADDR`), route-template labels, API HSTS | 5 | I | done | OBS-2..4; 2026-11-02: `observability.rs` (`prometheus` 0.14 text 0.0.4, process-wide registry), second `serve` listener only with `ASD_METRICS_BIND_ADDR`, HSTS layer in `app::router`; tests `metrics_endpoint.rs` |
| WP-B.14 | Migration 0022 drops `users.totp_secret` + `review_items.note` behind a guard; `migrate` runs the 0016 back-fill before new migrations; `admin reset-totp`; remove `encrypt-review-notes` | 5 | S | done | KEY-11, KEY-12, AUTH-9; phase 5 wave 3, after B.11 + B.15; 2026-11-02: `0022_drop_plaintext_columns.sql` (guard, rolled back on failure), `db::migrate_with_backfill` seals seeds before pending migrations, `admin reset-totp`; `encrypt-review-notes` removed (run it with the previous release before upgrading, `rust/README.md` "Upgrading") |
| WP-B.15 | KEK rotation: `ASD_DATA_KEK_PREVIOUS` read-only keys in `Keyring`, `admin rotate-kek` (re-wrap DEKs, re-seal TOTP seeds, CAS, audit), runbook | 5 | S | done | KEY-8..10; phase 5 wave 1. Integration secrets are under org DEKs and need no re-seal (security pack corrected); 2026-11-01: `crypto/rotate.rs` (CAS on old KEK id, `SKIP LOCKED`, exit 3 on remaining), covers `wrapped_dek`, `totp_secret_ct` and `totp_pending_ct` |
| WP-B.16 | Mandatory TOTP per role (O-10): migration 0023 `users.totp_required_by`, `asd-contract::auth_policy` (roles, 14-day grace, `TotpEnforcement`), per-request `403 totp-enrolment-required` outside the enrolment routes, deadline set at login/create/role change, `/users/me` fields, audit | 6 | S | done | AUTH-11..14 (ADR-0010 d5–d6); phase 6 wave 1; owner to confirm roles [owner, admin, clinician, superadmin] and grace [14 d]; no iOS impact (device tokens) |
| WP-B.17 | Org-admin TOTP reset endpoint (`POST /v1/orgs/{id}/users/{uid}/totp/reset`, owner/admin, caller step-up, revokes the target's refresh families, audited; shared `reset_for_user` with `admin reset-totp`) | 7 | S | done | TRS-1/2; **promoted 2026-11-02** (ADR-0011 D11: staff lose phones, clinics have no engineer); phase 7 wave 1; console action in C.18; 2026-11-02 wave 1: endpoint + `totp-reset-forbidden`; `admin reset-totp` now also revokes refresh tokens |
| WP-B.18 | One-command clinic deployment: `deploy/clinic-server/` (Caddy TLS, api/worker as `asd_app`, postgres/minio unpublished, console image), `install.sh` with key card, nightly `age`-encrypted backups + erasure ledger, `restore.sh`, monthly `restore-drill.sh`, `upgrade.sh`, CI smoke `deploy.yml` | 7 | S | done | DPL-1..11, HIP-6/7; phase 7 wave 1; answers H-11, supersedes B.7; 2026-11-02 wave 1: `deploy/clinic-server/test/compose_smoke.sh` passes locally (install, TLS via Caddy, console login, backup with erasure ledger, restore + `admin verify`, ledger replay, drill, status) in ~95 s; images built from host binaries because Docker Desktop's 7.8 GB disk cannot hold the `rust/Dockerfile` build, so that build runs in CI (`deploy.yml`) only. MinIO images are `bitnamilegacy/minio` + `minio-client` 2024.10 (`minio/minio` no longer on Docker Hub; frozen, no security updates: owner decision on a maintained S3 image). Not yet tested: `mc mirror` with real objects; `docker-image://` backend context under CI's classic image store |
| WP-B.19 | EMR-lite backend: migration 0024 (encounters, attachments, encounter links), `POST /orgs/{id}/participants` + QR payload, participant search, encounters CRUD + session links, follow-ups due, attachments encrypted under the org DEK, RBAC actions, timeline kinds | 7 | S | done | 2026-11-03 wave 2a: 632 backend tests; attachments list route + contract AttachmentPage; encounter with clinician summary holds erasure; REC-1..8, REC-10; phase 7 wave 2, after R.7 + B.20 |
| WP-B.20 | HIPAA audit gaps: audited reads (`session.read`, `participant.read`, `participant.timeline_read`, `clinical_review.read`, `note.read`, `disclosures.read`, `worklist.read`, `export.file_served`) via `audit::record_read` (failure = 500), blob PUT `Content-Digest` verification | 7 | I | done | HIP-1, HIP-4, SYNC-6 (backend); phase 7 wave 1; 2026-11-02 wave 1: `audit::record_read` on the 8 read paths (fail closed), blob `Content-Digest` → `digest-mismatch`; table in `rust/README.md` |
| WP-B.21 | Participant identity (`identity.patient_ref_mode = local_record`): migration **0027** (was 0026; moved 2026-11-03, phase 8 took 0026), encrypted name/sex/birth-month/guardian phone + name blind index, identity routes, search, erasure, de-id exclusion | 7 | S | blocked | ADR-0011 Q3 (ADR-0008 Q1 reopened); phase 7 wave 3 |
| WP-B.22 | `asd-backend admin requeue-erasure --ledger`, `erasure-ledger`, `verify` (restore helpers; factor `enqueue_erasure` out of the privacy route) | 7 | I | done | DPL-7/8; phase 7 wave 1; consumed by B.18; 2026-11-02 wave 1: `erasure-ledger`, `requeue-erasure --ledger`, `verify` (`tests/admin_restore.rs`) |
| WP-B.23 | Audit log export as a normal export job (`audit_csv`, owner/admin, canonical CSV via `asd-export::tabular`) + console button | 7 | I | done | HIP-3; 2026-11-03 wave 2b: `audit_csv` (de-identified, `quality_improvement` only, filters from/to/actor_id/action_prefix, download only, no destinations); `AuditExport` checked on create and on download URL; 9 columns, ids and SHA-256 of `before`/`after` only, formula-guarded, 200,000-row cap; console "Export CSV" on the audit page (hidden from superadmin, which lacks `ExportDeidentified`) |
| WP-B.24 | `asd-backend admin reset-password` (server shell emergency access: one-time temporary password, revokes refresh tokens, audited, TOTP untouched) | 7 | I | done | 2026-11-03 wave 2a; no forced change at next sign-in yet (password-change package proposed, owner to schedule) |
| WP-C.1 | Console scaffold: Next.js, auth, layout, sessions table/detail, overview KPIs | 1 | S | done | Live mode vs Rust backend, org switcher, overview/sessions, Playwright 11/11 mock + live; 129 unit tests |
| WP-C.2 | Review queue + devices/enrol QR | 1 | I | done | Review triage (j/k, 1–4), devices revoke, enrol code + QR. Backend gaps: no `GET /v1/orgs`, review items lack session summary |
| WP-C.3 | Participants list, users/roles, audit page (participant detail/timeline moved to WP-C.14) | 2 | I | done | 2026-11-01; backend gaps: no invite/password reset, no audit filters, users/audit `total: null` |
| WP-C.4 | Catalog editor (clips, AOI timeline, batteries) | 2 | S | done | 2026-11-01; backend gaps: catalog body checked for shape + non-empty clips only (no AOI range, time, duplicate-id or battery checks), body errors carry no JSON path, config problem slugs not in ProblemSlug, no media ref/upload, no org-role drafts (ConfigWrite unused) |
| WP-C.5 | Baselines editor + scoring profile editor + sandbox re-score + bundle publish with approval | 2 | S | done | 2026-11-01: profile range checks moved to asd-contract `profile_checks` (backend + wasm share them); console baselines/scoring editors validated live by asd-wasm, sandbox draft-vs-published re-score, bundle compose/sign. Limits: age typed in (raw payload endpoint added in phase 4), 'sandbox before publish' + changelog enforced in console only |
| WP-C.6 | Cohorts, norm-runs, exports, model registry | 3 | I | folded | 2026-11-03: cohorts → WP-X.2/X.7, analysis exports → WP-X.8, norm-run console view → WP-X.7 (runs page lists `norm_runs` too); model registry has no consumer (WP-D.3 todo) and stays backlog under WP-D.3 |
| WP-C.7 | Replay heatmaps, hi/ne strings, superadmin | 4 | I | backlog | |
| WP-C.8 | Console contract alignment to `api-v1.json`: `battery` string, `capture_source` + `synthetic`, clip `rows[]`, `participant_token`; zod-vs-schema test | 1 | I | done | API-3, CON-16/19/20; depends on R.3 |
| WP-C.9 | Clinical foundation: migration 0008, envelope encryption + MRN blind index, 14 RBAC actions (superadmin excluded from clinical content), `asd-contract::clinical`, user display name/registration, encrypted review notes | 3 | S | done | Migration 0008, AES-256-GCM envelope (aws-lc-rs) + MRN blind index, superadmin carve-out in `rbac::allows`, signer profile `/v1/users/me`, encrypted review notes + `admin encrypt-review-notes` back-fill, org-settings audit diff only. New optional env `ASD_DATA_KEK` (clinical endpoints 503 without it) |
| WP-C.10 | Worklist + clinical review sign/cosign/amend (hash-bound, step-up re-auth) | 3 | S | done | worklist + claim/draft/sign/cosign/amend, JCS content hash, step-up password+TOTP (no replay guard yet), 20-way concurrent sign = 1 signature; finalize hook stub for I.7. Open: disposition visible without content-read |
| WP-C.11 | Participant detail + timeline, MRN mapping, encrypted clinical notes | 3 | I | done | participant detail + merged timeline (headers only), MRN put/get/delete + POST search (audited, 409 when mode=none), encrypted notes with append-only corrections |
| WP-C.12 | Printable clinical summary endpoint (`GET /sessions/{id}/clinical-summary.pdf`, NOTE-5, EXJ-7); referral letters (NOTE-3/4) deferred | 3 | I | todo | 2026-11-02 re-scoped by ADR-0011 D1 (letters wait for a clinic that asks); still needs I.5 for PDF/A + Devanagari, works with the built-in-font PDF until then |
| WP-C.13 | Console: worklist, clinical review tab, sign/cosign dialogs, clinical settings | 3 | I | done | worklist, Clinical review tab, sign/cosign/amend dialogs (password/TOTP never in React state), clinical settings + signer profile; Playwright 15/15. Backend gaps: org settings not readable, no TOTP-enrolled flag, no user id in session |
| WP-C.14 | Console: participant workspace (timeline, MRN, notes, letters, print) | 3 | I | done | 2026-11-01: workspace tabs timeline/notes (append-only corrections)/MRN (reveal audited)/disclosures/print (HTML @media print); letters placeholder until C.12. Backend gaps closed in phase 4 (timeline filters, session-by-participant filter, console participant summary) |
| WP-C.15 | Console security settings: TOTP enrol (QR + seed), confirm, disable; nudge banner for privileged roles; mock + live | 5 | I | done | phase 5 wave 2, after B.11 (schema); 2026-11-01: `[org]/settings/security` (QR, seed shown once, confirm, disable), `TotpNudge` for owner/admin/clinician/superadmin; mock + live; e2e `security.spec.ts` |
| WP-C.16 | Console CSP (per-request nonce in `proxy.ts`, `wasm-unsafe-eval`) + HSTS; Playwright CSP-violation listener | 5 | I | done | CSP-1..4; phase 5 wave 1; 2026-11-01: per-request nonce on every response incl. redirects, all routes dynamic (`connection()`), postbuild strips/guards static HTML, zod jitless via `@/lib/zod` (lint-enforced), HSTS in production; Playwright `prod` project runs against `next start` |
| WP-C.17 | Console: TOTP nudge shows the deadline, `[org]/layout.tsx` redirects `required` users to the security page (`x-pathname` from `proxy.ts`), required-state panel, mock users for the three states, e2e | 6 | I | done | AUTH-11..13 console side (ADR-0010 d7); 2026-11-02: layout redirect on document loads only (`Sec-Fetch-Dest`) plus a page-level redirect in `loaders.ts` (a layout redirect on RSC requests looped); per-user mock state (`required@`/`grace@example.org`); live e2e grace assertion not yet run against a backend; phase 6 wave 2, after B.16's `api-v1.json`; parallel with A.7b |
| WP-C.18 | Console record workspace: create participant (token + QR), Visits and Attachments tabs, follow-ups page, participant search (POST form), TOTP reset action for owner/admin | 7 | I | done | 2026-11-03 wave 2a: mock-first; Files tab lists via timeline (switch to the attachments route); live pass pending; REC-1..7 UI, TRS-3; phase 7 wave 2, after B.19 (+ B.17 DTO) |
| WP-C.19 | Console share-a-case dialog, share-link panel (passcode shown once), public `/share/[id]` page with passcode form and streamed download | 7 | I | done | 2026-11-03 wave 2a: mock-first; /share anonymous in guard.ts; live pass pending; SHR-10, HIP-9; phase 7 wave 2, after I.13 |
| WP-C.20 | Console identity panel + name search (only when the org mode is `local_record`) | 7 | I | blocked | with B.21 (ADR-0011 Q3); phase 7 wave 3 |
| WP-C.21 | Console automatic logoff: idle 30 min / absolute 12 h as constants (`session-policy.ts`), enforced in `proxy.ts` on document loads, `/login?reason=idle|expired` | 7 | I | done | HIP-2 (O-10 second half; owner may change the constants before start); phase 7 wave 1; 2026-11-02 wave 1: `session-policy.ts` (idle 30 min / absolute 12 h), `last_seen` refreshed on non-prefetch authenticated requests, redirect on document loads only |
| WP-I.1 | `asd-export` crate foundation: model, de-id, consent policy, verified code table, watermark; fold B.8 PDF/FHIR code in | 3 | S | done | `asd-report` folded into `asd-export` (goldens byte-identical); ExportRecord v1, de-id, consent policy (24-row table), watermark, code table; session bundle keeps unverified `93044-6` + XFDF code as documented iOS-parity exception (Q6) |
| WP-I.2 | CSV (canonical/Excel/Sheets) + XLSX encoders | 3 | I | done | CSV canonical/Excel(BOM+CRLF)/Sheets + 5-file ZIP with manifest hashes, XLSX with README/watermark header+footer, formula-injection escaping, de-id applied inside encoder, 200k-row XLSX cap |
| WP-I.3 | FHIR R4 clinical bundle (DiagnosticReport, DocumentReference, Practitioner), iOS id parity, code verification | 3 | S | done | DiagnosticReport/Observation/DocumentReference/Practitioner/Provenance + transaction bundle; unverified codes dropped by default. FOUND: iOS LOINC 93044-6 = 'Level of evidence', all 23 M-CHAT LOINC codes are toxicology tests, SNOMED 713444003 does not exist — iOS + session bundle must change (Q6) |
| WP-I.4 | HL7 v2.5.1 ORU^R01 encoder (file delivery) | 3 | I | done | ORU^R01 encoder, P/F/C goldens, 300-case escaping property test, +0545 timestamps, refuses de-identified/no-MRN. Follow-up: verify `MR`/UCUM entries in codes.rs; `Hl7Profile` to be mapped from I.6 integration settings |
| WP-I.5 | PDF/A-2b clinical summary + letter renderer (A4, Devanagari) | 3 | S | todo | PDF-*; needs I.1, owner Q3 (OFL fonts) |
| WP-I.6 | Export jobs: migration 0009, runner run_after/heartbeat/context, consent filter, artifacts, signed download URLs, disclosures | 3 | S | done | migration 0009, export jobs (CSV/XLSX/FHIR/NDJSON/HL7; PDF pending I.5), runner run_after + heartbeat + fencing, consent filter with exclusion counts, signed 5-min download URLs (superadmin refused), disclosures, expiry sweep. Gap: no zip-scan test of de-identified CSV in backend |
| WP-I.7 | Delivery: integration settings (encrypted write-only secrets), signed webhooks, SFTP, SSRF guard, auto-deliver on final sign | 3 | S | done | migration 0014; integration routes (write-only encrypted webhook secret, server-generated Ed25519 SFTP key, pinned host key, connection test), HMAC-signed webhooks (hyper+rustls, no redirects), SFTP drop (russh, no RSA), SSRF guard (resolve once, refuse private/loopback/mapped, connect to checked address), outbox with retries, auto-deliver on final sign (`review.finalized`) |
| WP-I.8 | Console: exports page + integration settings | 3 | I | done | 2026-11-01; backend gaps: integrations list not paginated, exporters cannot list destinations, no destination delete |
| WP-I.9 | FHIR push via SMART Backend Services (new migration ≥0014; see migration rule) | 4 | S | deferred | 2026-11-02 ADR-0011 D1: no hospital systems at the target clinics (Nepal); design in ADR-0008 stays valid if a hospital appears |
| WP-I.10 | SMART EHR launch of the console | 4 | S | deferred | same reason; needs I.9, C.14 |
| WP-I.11 | HL7 MLLP/TLS sender + ACK handling | 4 | S | deferred | same reason; the file-based ORU^R01 (I.4/I.7) stays as built, not extended |
| WP-I.12 | `asd-export::case_package`: deterministic ZIP (manifest, README, multi-session FHIR collection, case PDF, de-identified JSONL + CSV; notes/MRN/attachments in identified mode), one pseudonym salt per package, `case-package-v1.json` schema via `asd-cli schema export` | 7 | S | done | SHR-1..6; phase 7 wave 1; 2026-11-02 wave 1: `case_package` + `case-package-v1.json`, 14 tests |
| WP-I.13 | Case-package export format + share links (backend): migration 0025, `ExportFormat::case_package`, recipient/consent fields, job arm, `POST /exports/{id}/share-link` (passcode once), public `POST /share/{id}/open` + `GET /share/{id}/file` (rate-limited, audited with IP), erasure revokes | 7 | S | done | 2026-11-03 wave 2a: Argon2id passcode, 5-min HMAC download token, atomic download count, 10/link 30/IP per hour; 2026-11-03 wave 2b (I.13b): identified packages carry encounters (summaries only with clinical content) and, with `include_attachments`, decrypted attachments; de-identified carry no encounter text or attachments; erasure also expires packages found through `case_participant_id` and revokes their share links (audit `share_link.revoke`, reason erasure); no per-attachment `attachment.read` audit (export audit covers it); SHR-7..9, SHR-11, HIP-8/9; phase 7 wave 2, after R.7 + I.12 + B.20 |
| WP-D.1 | Stimulus clips: rights or replacement; durations measured; catalog corrected | 1 | I + owner | done | All 10 clips measured at 8.0s via ffprobe, catalog aligned, NOTICE.md updated |
| WP-D.2 | Session JSON Schema export + Swift/Python parity gate in CI | 1 | I | done (to be replaced) | Pydantic export + `scripts/check_session_schema_parity.py`; superseded by WP-R.3 + WP-D.7 (Rust export, `asd-cli schema check`) |
| WP-D.3 | Age-band model: cleared dataset, train, validate (≥0.90 recall), CoreML, manifest + licence in `models/asd_age` | 2 | S | todo | |
| WP-D.4 | Synthetic session generator shared by seed script and tests | 1 | I | done (to be replaced) | `tools/asd_synth` v1.0 emits seconds/`offTarget`/`synthetic` which iOS and console reject (CON-13); superseded by WP-R.4 (v2.0) + WP-D.7 |
| WP-D.7 | Replace Python CI gates with Rust: `ios.yml`/`ios_test.sh` use `asd-cli schema check`; delete `check_session_schema_parity.py`, `asd_norm_aggregator.py`, `asd_synth/`, `asd_research_server.py` and their tests; README updates | 1 | I | done | GATE-1..4; depends on R.3, R.4 |
| WP-D.8 | Port `scripts/check_asd_claims.py` to `asd-cli claims check` (metadata limits, denylist incl. Devanagari, Swift UI + prose literals, secrets, selftest), single call site in `ios_test.sh` | 6+ | I | backlog | ADR-0010 d4 / ADR-0009 Q5: do it the next time the scanner needs a structural change; about one day; regex dialect re-verified by the selftest |
| WP-D.9 | Tauri 2 desktop shell `apps/curv_desktop/` loading the clinic server's console URL in remote mode (no bundled console: every route is dynamic since WP-C.16) | 7 | I | done | ADR-0011 D10 (desktop = console in a browser/PWA; Tauri only when a binary is needed); 2026-11-02: `apps/curv_desktop/` (remote mode, URL policy in `src/policy.rs`) landed in 17fbf42 from a separate session; its gates were not re-run by the wave-1 integration |
| WP-D.5 | ML roadmap refresh: what an attention classifier would need (n, labels, validation plan) — doc only | 3 | A | backlog | |
| WP-D.6 | Decide/Link C++ `AsdBiomarkerPack` into kit via xcframework, or retire the claim | 4 | A | backlog | low value |
| WP-E.1 | Pitch/deck re-base to non-diagnostic reality; aspirational section | 1 | I | done | Re-based to observation diary, 6 metrics, 2027 aspirational roadmap |
| WP-E.2 | Org Apple account, bundle id, publisher domain, privacy policy + support pages live | 2 | owner + I | todo | H-6 |
| WP-E.3 | Docs drift sweep (iOS 18, Keychain not SE, names, screenshots) | 1 | I | done | Keychain, iOS 18, names, links fixed; owner follow-ups listed in commit |
| WP-E.4 | M-CHAT licence outcome → keep gate or enable | 3 | owner | todo | H-7 |
| WP-E.5 | Native hi/ne review of store text | 3 | owner | todo | |
| WP-E.6 | Security/privacy review pack: threat model, data-flow diagram, DPIA-lite, retention policy, AWS BAA | 3 | A + I | done | 2026-11-01: `docs/security/` (threat model, data flow, DPIA-lite, retention policy, AWS BAA checklist); owner decisions O-1..O-15 open |
| WP-E.7 | App Store submission dry run (`appstore_precheck`, screenshots, age assurance flag) | 4 | I | backlog | |
| WP-E.8 | HIPAA / Privacy Act 2018 administrative pack (docs only): risk analysis, policies, emergency-access procedure, BAA/DPA templates for US recipients, breach process, cross-border consent design, threat-model rows for share links, DPIA update | 7 | A + owner | drafts | ADR-0011 D9; 2026-11-02: drafts landed in 55c1cb2 (`docs/security/hipaa-safeguards.md`, `policies/`, `templates/`); final versions still need O-2, O-6, O-11, O-12, O-15 |
| WP-S.1 | Store build flag `ASD_STORE_BUILD` (Release only): Capture/Sessions/Settings tabs, result view reduced to recording quality, verdict surfaces hidden, legal links in About | 1 | S | done | 2026-11-08 landed; Release+DemoRelease build, 467 app tests, symbols ok; design `asd-gaze-app-store-readiness/store-collection-build.md` (S-1..S-9); Debug/DemoRelease unchanged |
| WP-S.2 | Branding and copy: no "ASD" in UI, neutral taglines, verdict-like lines removed, consent sentence, README | 1 | I | done | 2026-11-08 landed |
| WP-S.3 | Store listing rewrite (en/hi/ne/android) as observation capture; beta changelog; Snapfile | 1 | I | done | 2026-11-08 landed; `validate_metadata` ok; hi/ne native review still WP-E.5 |
| WP-S.5 | Consent flow (ResearchKit-style, SwiftUI), permission primers, per-feature opt-ins, Privacy & Your Data screen, pre-session disclaimer, re-consent gate | 1 | S | done | 2026-11-08 landed; decisions S-10..S-13; WP-S.5b (opt-in gating) open |
| WP-S.5b | Opt-in gating: clinic delivery, reminders, HealthKit, dictation, photo attachments actually gated by `FeatureOptIns`; store build shows only the opt-ins whose feature exists (age range, clinic delivery); clinic delivery authorised by guardian consent + opt-in (+ research consent outside the store build) | 1 | S | done | 2026-11-09 landed; decisions S-15..S-21 in `store-collection-build.md`; new `ClinicDeliveryGate.swift`, `OptInGatingTests`; `DeviceEnrolmentRow` through `Localizer.tr`, `NormCollectionToggleRow` hidden in the store build; 18 catalogue keys (hi/ne `needs_review`); Maestro flow 04 asserts the two-toggle section; 485 app tests. Debug/DemoRelease now also need the Clinic delivery toggle on before sessions upload. Facial-expression per-session drop split out as WP-S.5c |
| WP-S.5c | Facial-expression metrics droppable per session in the kit (`FacialAffectMetrics`, `smileReactivityIndex` and siblings in `GazeSessionRecord` and the ingest payload; console schema impact), then re-offered as an opt-in | 2 | S | todo | after submission; S-21; consent already lists the measurements under "What is recorded" |
| WP-W.1 | Public site alignment (repo `bishalg/open-spectrum-tracking`, `apps/web/landing`): landing copy without research/detection/HIPAA claims, pages `/earlygaze`, `/privacy/earlygaze`, `/support/earlygaze`; then point the listing URLs and in-app links at them | 1 | I | awaiting merge | 2026-11-09 landed on landing `dev` (d2541e0, b0a35cb); owner opens/merges dev->main for Vercel production; listing URLs and in-app links already point at the vercel.app paths (`~/Documents/WORK/Open Source/open-spectrum-tracking`, branch `dev`); `/privacy/earlygaze` and `/support/earlygaze` ship `noindex` while `[TODO]` placeholders remain; owner/legal review of the policy; H-6 publisher name |
| WP-S.7 | App Store screenshots from the store build: `StoreSim` configuration (store flag + simulation), Maestro flows `maestro/flows/store/` mirrored by `Tests/bdd/features/store_*.feature`, runner `scripts/run_store_screenshots.sh`, real captures replace the Sept 15 composites in `fastlane/screenshots/` | 1 | I | done | 2026-11-09 landed; 16 en-US captures (iPhone 18 Pro Max 1320x2868, iPad Pro 13-inch (M5) 2064x2752), PNGs git-ignored and regenerated before upload; hi/ne-NP via `--lang` after H-10. Polish follow-up WP-S.7b done in WP-S.9 (2026-11-09) |
| WP-S.8 | App Review notes and gate hygiene: `fastlane/review_notes.txt` sent as `app_review_information[:notes]` by `update_metadata` (contacts still env-only), length check in `validate_metadata`, notes scanned by `check_asd_claims.py`; claims script flags made additive (`--swift --prose --secrets` previously ran the secrets scan only, so the `ios_test.sh` claims gate had never scanned claims); README drift sweep; HANDOFF rewritten to the 2026-11-09 state | 1 | I | done | 2026-11-09 landed |
| WP-S.9 | Submission hardening (S-22..S-27): Release/StoreSim sign with `AsdGazeCollector-Store.entitlements` (Declared Age Range only; `scripts/check_store_entitlements.sh` in `ios_test.sh`), HealthKit untouched in the store build; plain-language Settings rows (WP-S.7b); one-step delete-all incl. consent, opt-ins and profile in the store build; 640 pt readable column on consent/primer/privacy/onboarding (iPad); neutral sync status text; `fastlane ios release_check` lane | 1 | S | done | 2026-11-09 landed; 487 app tests; screenshots regenerated on both devices; closes WP-S.7b |
| WP-LN.1 | i18n catalogues: `Localizable.xcstrings` + `InfoPlist.xcstrings` replace the in-code dictionary; `Localizer.tr` wrapper; `knownRegions`/`CFBundleLocalizations` en/hi/ne; Settings Language row; key-parity tests | 1 | I | done | 2026-11-09 landed: `Localizable.xcstrings` (163 keys en/hi/ne, hi/ne `needs_review`), `InfoPlist.xcstrings` (7 usage strings + display name), `CFBundleLocalizations` en/hi/ne, Settings Language row, About "translation under review" note, `asd-cli i18n check --app-xcstrings` in `ios_test.sh`; design `i18n-readiness.md` I-1..I-9 |
| WP-LN.2 | Literal baseline burn-down for store-build files (SettingsView, OnboardingFlowView, GazeSessionResultView, StoreSessionsView, wizard, consent views) to zero; store-file baseline check in `ios_test.sh` | 1 | I | todo | needs WP-I.1 |
| WP-LN.3 | Locale-aware formatting: 82 `String(format:)` and 22 `DateFormatter` uses classified; user-visible ones to `formatted()`; machine ones explicit POSIX; lint rule | 1 | I | done | 2026-11-09 landed: 74 user-visible `String(format:)` → `locale: .current`, 8 machine → `FormatLocale.posix`, 2 fixed-pattern dates → `formatted(.dateTime…)`, lint rule `swift-format-locale` in `asd-cli i18n lint`, `LocaleFormattingTests` |
| WP-LN.4 | Pseudo-locale gate: `run_store_screenshots.sh --pseudo` (double-length strings) on both devices; fix clipping | 1 | I | todo | needs WP-I.1, WP-S.7 |
| WP-LN.5 | Widget and care-alert text through the localizer | 2 | I | todo | after submission |
| WP-S.6 | Shared consent/primer/opt-in text in the catalogue; `consent_document_text/sha256` in `asd-contract`; session schema v1.9 `consent`; iOS reads via `asd-ffi` | 1 | S | todo | parity prerequisite for Android; Tier A text per `i18n-readiness.md` I-1/I-4 (consent hash per language, schema v1.9 `consent.language`); `android-parity-plan.md` |
| WP-AN.0 | Android capture app parity: requirements ANDP-1..15, architecture, packages WP-AN.1..7 | 9+ | A | done | 2026-11-08 `asd-gaze-app-store-readiness/android-parity-plan.md`; no code; needs AN-D1..D4; supersedes the scope line of WP-M.5 |
| WP-S.4 | Entitlements/plist hygiene: declared-age-range entitlement, drop unused HealthKit background delivery, widget version keys | 1 | I | done | 2026-11-08 landed; declared-age-range entitlement must also be enabled on the App ID (H-6) |
| WP-X.0 | Phase 8 step 0: `asd-contract::params` registry + `validate`, scoring profile 1.2.0 (`metrics`, `aoi`, `ageBands`, `norms`), `research.rs` DTOs + slugs, `ExportFormat::analysis_results`, raw-log header/sample extension, FFI log wrappers, RBAC rows (`AnalysisRead/Run`, `ExperimentWrite/Promote`, `FramesRead`), migration 0026, `analysis_run` job kind + stub routers, `param-registry-v1.json`, console API/mock/permissions/nav stubs | 8 | S | done | 2026-11-03: registry 88 rows (completeness test both ways), profile 1.2.0 (goldens unchanged), RFL-1 log fields + FFI writers, migration 0026, GOV-1 RBAC (vendor research exclusions as a separate list), `analysis_run` stub kind, `JobContext::progress`/`cancelled`, 501 stubs, `BlobKind::ResearchFrames`, CLI params, console research client/mock/permissions/nav/messages; schemas + `.sqlx` regenerated; 1,572 Rust tests, 754 console tests. Open: `ExperimentPromote` for admin (Q5 vs GOV-1, owner); `asd_sync::accept_bundle` still on `scoring_profile_problems` (unowned switch to `params::validate`); admin may create a researcher who can read frames; PRM-1..4, PRM-7, RFL-1, GOV-1; ADR-0012 |
| WP-X.1 | Core: constants → params (`metrics::extract_with`, margin, replay gate, norms from profile), `gaze::reprocess` (raw log + catalog → frames → metrics), `analysis::{evaluate, Aggregator, suppress}`; goldens byte-identical | 8 | S | done | 2026-11-03 (`c5e8e40`): `asd-contract` depends on `asd-core`, so the core holds twins (`analysis::AnalysisProfile` + sections, `RunSummary`, `Dist`, …) with JSON parity tested against the contract (asd-contract as dev-dependency); constants moved: I-VT velocity/min fixation/frame interval/smile window/flat-affect gain → `metrics.*`, min-n + age brackets → `norms`/`ageBands`, low-confidence ratio, replay gate → `ReplayOptions.quality_gate`, AOI margin → `aoi.marginNorm`, review ratio → `review.lowQualityValidRatio`; `gaze::reprocess`, `analysis::{evaluate, Aggregator (exact quantiles, seeded bootstrap), suppress}`; 20 new tests, goldens + 16 scenario replays byte-identical. Gaps: `aoi.dwellCapMs` read by nothing in core (cap lives in the kit); `ivtUnit = deg` awaits G.13; `from_reports` copied not moved (X.4 switches asd-wasm); `aggregate.rs` still uses `FRAME_INTERVAL_SEC` + inline gain (INV-1 gap, X.4 or X.9); RPX-3 `param_not_applied` is the caller's diff (`changed_params`, X.3/X.4); PRM-9, INV-1 core rows, RPX-1..3/6, PRV-1; wave 1 |
| WP-X.2 | Backend: research-frames proxy PUT sealed under the org DEK, `GET /sessions/{id}/research-frames`, retention/erasure of the log, `age_months` at ingest, experiments + cohorts CRUD + preview with suppression, promote → draft via `ConfigWrite`, `config-unanalysed` publish rule, `config/effective`, review thresholds and norm bands from the published profile | 8 | S | done | 2026-11-03 (`9fc2cfc`): experiments/cohorts CRUD + preview (`ExperimentWrite`/`AnalysisRead`, `If-Match`), promote (`ExperimentPromote`, 201 `ConfigDocument`), `config/effective` (`ConfigRead`), `GET /sessions/{id}/research-frames` (`FramesRead`, `record_read`), proxy-only `PUT blobs/research_frames` sealed under the org DEK (key id + AES-GCM, AAD `sessions:research_frames:{org}:{session}`), retention purge on `research_frames_retention_months` (fallback `retention_months`), erasure deletes the log even when held, `age_months` at ingest, flag/review thresholds + norm bands from the published profile, `POST /config/{kind}` now `ConfigWrite`, GOV-3 `config-unanalysed` (evidence: succeeded run over body hash or baselines version), `asd-sync::accept_bundle` on `profile_problems`; 758 tests. Deviations: AGE-2 `norms.minN` is a floor under the request; COH-1 `sex` ignored; GOV-2 opens drafts for every kind incl. catalog. Contract follow-ups done in WP-X.0b (`bb85fbd`, 2026-11-03) except the promote response type (still 201 `ConfigDocument`, open). X.3 must store `analysis_runs.profile_hash = lifecycle::body_hash(profile)`; RFL-3..6, EXP, COH, GOV-2/3/6, INV-1 backend rows; wave 1 |
| WP-X.5 | Console: `/[org]/research` experiments + workbench (registry-driven `RegistryForm`, session picker, live preview with scanpath, single-session sweep, run dialog), `ScoringProfileEditor` switched to the registry form, `en`+`ne` messages | 8 | I | done (mock) | 2026-11-03 (`ed2b358`): `/[org]/research`, `/research/experiments{,/new,/[id]}` workbench; `RegistryForm`/`ParamControl` render every row by `kind` from the bundled `param-registry-v1.json` (test diffs it against the docs copy), capture_only disabled in preview, default/published markers, diff count; `ScoringProfileEditor` on the registry form (hand-written fields gone); all 88 labels + help in `en`/`ne` (ne machine draft), `messages.test.ts` fails on `TODO`; `AsdCore.supports()` so missing wasm exports return `unavailable`; 801 console tests, research + config + prod CSP e2e. Remaining: RFL-4 client method + real `loadResearchFramesAction` (now 501 → score-level fallback), AOI rects on the scanpath (the 5b fix parses `Evaluation.trace`/`fixations`, passes `traceHz` = preview-only `PREVIEW_TRACE_HZ`, and the page reads the list's `research_frames_available`; WP-X.5b), sweep score-level only, cohorts/runs pages (X.7; "Run on cohort" links to a 404 until then), mock has no frames session, mock `createExperiment` ignores `from_version`, flag labels research wording not FlagBadge; live pass after X.2/X.4; PRM-6, RUI-1..4, RUI-7; wave 1 mock-first |
| WP-X.6 | iOS: research frame log writer (raw samples + affect + clips, consent-gated, 16 MiB cap) and third blob upload with `Content-Digest` | 8 | I | done | 2026-11-03 (`b6f850a`): `ResearchLogWriter` armed only when `NormCollectionGate.isActive` and capture is TrueDepth; lines via `gaze_research_log_sample_line`, header via `gaze_research_log_header_line` at finalize; 16 MiB cap drops the log and sets `normCollection.research_log_dropped`; third blob `research_frames` enqueued only with consent, deleted after `done`/no URL/quarantine/no consent; relaunch resumes without a second POST; one settings string en/ne/hi; iOS gate green (kit 267, app 430, TSAN, claims). Follow-ups: `asd-sync::IngestAck` + FFI `AsdIngestAck` gained `research_frames_upload_url` in WP-X.0b (`bb85fbd`); the Swift raw-ack read is gone, `research_frames_upload_url` comes from the typed `AsdIngestAck` (WP-X.6b, 2026-11-03); no FFI log parse (fixed-point test + optional `asd-cli replay --check`); header `profileVersion`/`clips[].catalogVersion` carry the delivered `bundleVersion` (bundle has no separate profile/catalog versions; nil for the bundled default) (WP-X.6b); log kept on dead-letter until retry; RFL-2; wave 1 |
| WP-X.3 | Backend: `analysis_run` job (`cohort_rescore`: membership snapshot, levels, unseal, evaluate ×2, incremental aggregates, sealed result blob, summary, progress, cancel, budget, quota), runs routes (create/list/get/sessions/cancel/rerun), `POST /jobs/{id}/cancel` | 8 | S | done | 2026-11-03 (`c5fda5e`): `POST /orgs/{id}/analysis-runs` (202; `cohort-empty`/`cohort-too-large`/`sweep-too-large`/`analysis-level-unavailable`/`config-invalid`, 429 `analysis-quota-exceeded`), list/get/sessions (409 without rows)/cancel/rerun (same seed + definition, `membership_drift`), `POST /jobs/{id}/cancel` + `JobView.progress` for every job; one analysis per worker + per-org running cap in the claim SQL; rows JSONL `AnalysisSessionRow` sealed at `analysis/{org}/{run}/sessions.jsonl.sealed` (AAD `analysis_runs/result`), `summary` = PRV-1 `RunSummary`, `profile_hash = body_hash(profile)` (verified through X.8's `load_run`/`load_rows`); audits `analysis.run_enqueue/run_complete/run_cancel/read/sessions_read`, `job.cancel`; `ASD_ANALYSIS_MAX_RUNNING_PER_ORG=1, _MAX_QUEUED_PER_ORG=5, _MAX_SESSIONS=2000, _MAX_FRAME_SESSIONS=500, _MAX_EVALUATIONS=200000, _MAX_FRAME_EVALUATIONS=20000, _MAX_MINUTES=30, _MAX_RESULT_MIB=64, _PAGE_SIZE=200`; 13 integration + 4 unit tests, 725 backend tests. Deviations: GOV-5 proven with a read-only Postgres role, not a statement log; `capture_only` knobs → `param_not_applied` warnings; exclusions only `no_age`/`no_frame_log`/`frames_swept`/`frame_log_unreadable`/`session_gone`; published baselines read at job time (no column), `bundle_version` NULL, `contract_version` literal (no contract constant); rerun drift from member ids in `jobs.result.membership` (visible to JobRun); per-org cap racy across workers; RUN-4 1 MiB summary cap not enforced; `param_sweep`/`age_profile` budget-checked then 400 until X.9. WP-X.3b (`b091e70`): `GET /analysis-runs/{id}` writes `analysis.read` only when the body carries `summary`/`membership_drift`; progress polls are not audited (GOV-4 sentence added); RUN-1..6/8/9, GOV-4/5; wave 2 |
| WP-X.4 | asd-wasm: `paramRegistry`, `reprocessSession`, `rescoreSession` honours `lowConfidenceValidRatio` + `metrics` (closes the R.6 open item); native == wasm32 parity | 8 | S | done | 2026-11-03 (`5b446ff`): `paramRegistry()` = `param-registry-v1.json`; `reprocessSession(log, catalog|null, profile, baselines, {ageMonths, scoredAtUtc?, model?, glasses?, traceHz?})` = `analysis::evaluate(Frames)` returning the core `Evaluation`; `rescoreSession`/`rescoreApiSession` via `evaluate(Score)` so `lowConfidenceValidRatio` + `metrics` apply (old golden unchanged); `metric_sets_from_reports` deleted; 26 native + 21 wasm32 tests, console 801 against the real build. Deviation: frames-level `tracking` (`sdDeg`, `offsetDeg`) differs native vs wasm32 by ~1e-18 (platform vs Rust libm `atan2`/`acos`), compared to 1e-12; byte-exact would need one maths library in asd-core (owner call). Follow-up: X.5's `PreviewPanel.parseTrace` expects `trace: {points, fixations}` but `Evaluation` has `trace: [points]` + top-level `fixations`, and it never passes `traceHz`, so the scanpath does not draw (fixed in WP-X.5b); RPX-4; wave 2 |
| WP-X.7 | Console: cohorts pages, runs list/detail (progress, transition matrices, per-band histograms with baseline range, delta bars with CI, provenance, export pre-fill, re-run/cancel/promote), `useJobPoll`, drift lines from `config/effective` | 8 | I | done (mock) | 2026-11-03: `research/cohorts{,/new,/[id]}`, `research/runs{,/[id],/[id]/sessions}` with `RunStatus`, `TransitionMatrix` (n < k, never 0), `Histogram` (baseline range from the bundle's `expectations[].byAge`, shades nothing when ambiguous), `DeltaBars` (composite per band), `ProvenanceBlock` (every RUN-5 field), `RunSessionsTable`; `useJobPoll` (RetentionPanel keeps `pollMs`); export pre-fill `?format=analysis_results&run=` with layout + encoding fields; drift lines only when the profile has `trackingQuality.drift.*`; mock run progression (queued → running from 2nd poll → succeeded from 5th; seeded running stays cancellable, seeded failed `budget_exceeded`); 836 console tests, 20 e2e (research/exports/privacy/sessions). Open: run page polls `getAnalysisRun` 1/s (backend audits `analysis.read` per poll: X.3 to exempt polls or add `getJob`); cohort create/archive gated on `analysis.run` (no cohort permission); sweep/age-profile views (X.9); per-metric deltas; live e2e unrun at the time; 2026-11-04 live pass: `e2e/live/run-research-live.sh` (seed-research.sh: researcher, 2 superadmins, published config, 17 consented sessions + 14 research frame logs) runs `research.live.ts` 9/9 against serve + worker (experiment, score+frames preview, cohort suppression, rescore/sweep/age_profile runs, analysis_results export download, promote + `config-unanalysed`, GOV-1, 60 s watch without `analysis.read`, cancel); console fixes: 64-bit `seed` kept exact, research frames loaded (`getResearchFrames`), promote returns the draft `ConfigDocument`, cancel of a running run keeps polling, `/switch-org` relative redirect, `config-unanalysed` message; `prod` CSP project was red on the login page in the agent's loaded run (verified by the main session, see wave-2 integration note); RUI-5/6, INV-2; wave 2 |
| WP-X.8 | `asd-export::analysis` + backend arm: `analysis_results` export (aggregates CSV/JSONL, sessions JSONL/Parquet through `deid.rs`, provenance in the manifest) | 8 | I | done | 2026-11-03 (`8cf3bdb`): `aggregates` (JSONL or CSV ZIP, RUN-7 columns) and `sessions` (JSONL, CSV ZIP, Parquet with `curv.run_id`/`curv.definition_hash` footer) layouts; `S-` pseudonyms (fresh salt), `D-` site pseudonym, `session_month`, no `age_months`/`device_id`; suppressed cells carry no numbers; manifest `provenance` = run record; needs `AnalysisRead`; sessions joined at export time (withdrawn since the run → `excluded.withdrawn`, erased → `session_gone`); PRV-5 test enumerates `ExportFormat::ALL`; 10 tests. Assumed run format (X.3 told to match): rows JSONL of `AnalysisSessionRow` at `analysis_runs.result_blob_key`, sealed `key_id || AEAD`, AAD `Aad::new("analysis_runs","result",org,run)`; `summary` = `RunSummary`. Deviations: no `P-` column (rows carry no participant id); run-not-succeeded → 400 `validation-failed` (no slug); the request's `analysis` block is stored in `exports.filters.analysis` (no column/DTO field: contract follow-up); RUN-7, PRV-3/5; wave 2 |
| WP-X.10 | `asd-cli research reprocess|evaluate`, `params list|check` polish | 8 | I | done | 2026-11-03 (`c304cce`): `research reprocess LOG --catalog --baselines --age-months [--profile] [--level auto|frames] [--trace-hz]` and `research evaluate SESSION.json --baselines [--profile] [--age-months] [--level auto|score]`, output byte-equal to `asd_core::analysis::evaluate`; exit 0/1 input error/2 not processable at the level; profile checked against the registry first; 6 tests. Deviations: `--profile` optional (default profile); exit codes per the brief, not "like `gaze replay`"; RPX-5; wave 2 |
| WP-X.9 | Sweeps (1–2 axes, ≤ 625 cells, budgets) and `age_profile` runs (shares vs baseline ranges, draft baselines), console curves and age tables, owner-gated knobs sweepable | 8 | S | done | 2026-11-03 (`b4c454a`): `asd_core::analysis::{sweep, age_profile}`; `summary.sweep = {axes, published_values, cells[{values, n, flag_rate, quality_rate, metrics, composite, changed_vs_published}]}` (row-major, published point appended when off-grid, so ≤ 626 cells; cells summarise `options.metrics`, composite by default); `summary.age_profile` per band × expectation with published/draft sides (`dist`, baseline `range`, shares on_target/monitor/atypical, `expected_percentile`), suppression `max(k, norms.minN)`; SWP-1 validation against the registry (1–2 Number/Integer ids, ≤ 24 steps/axis, frames knobs only at `level: frames`, capture_only never, H-G2 knobs allowed per Q9), SWP-2 budgets (≤ 625 cells, evaluations within `ASD_ANALYSIS_MAX_[FRAME_]EVALUATIONS`, new `ASD_ANALYSIS_MAX_SUMMARY_KIB=1024`), RUN-4 1 MiB cap enforced (`result-too-large`); contract `SweepSummary.published_values`; console `SweepPanel` (curves / two-axis heatmap) + `AgeProfileTable` on the run page; 11 core + 4 backend + 7 console unit + 3 e2e tests. WP-X.9b (`945fbb3`): `RunDialog` kind picker (`cohort_rescore`/`param_sweep`/`age_profile`) + axis editor bounded by the registry (Number/Integer, no capture_only, frames knobs only at `level: frames`, ≤ 24 steps/axis, ≤ 2 axes, ≤ 625 cells, published value always included), metrics multi-select, server `invalid_params` routed per field; 16 unit + 1 e2e tests, 861 console tests (`METRIC_KEYS` duplicates Rust `PerVideoMetricKey::ALL`: contract follow-up). Deviations: frames knobs refused at `level: auto`; one extra cell for the published value, not a row/column; summary pre-check covers the sweep block only; age profiles not in the X.8 export, no Parquet polish; SWP-1..3, AGE-1; wave 3 (phase B) |
| WP-X.0c | Contract addition for WP-M.1: `ScoringProfile` 1.3.0 `display` (14 rows: bands for social preference, eye dwell, mouth dwell, first-eye latency, smile intensity, smile reactivity, surprise, flat affect; `scanpathMaxGapMs`) and `sessionQuality` (2 rows), `ageBands[].representativeMonths`, `asd_core::display::band` + `quality::session_verdict`, FFI/wasm wrappers, `params::validate_for_device` (device ignores unknown keys; publish stays strict), schemas + catalogue keys | 8 | S | done | 2026-11-04: `SCORING_PROFILE_SCHEMA_VERSION = "1.3.0"`; contract `DisplayParams` (`display.bands.<metric>.<bound>` + `scanpathMaxGapMs`) and `SessionQualityParams`, both `serde(default)` (a 1.2.0 body decodes to the DSP-1 defaults), converted field by field into the flat core twins; registry 88 + 16 = 104 rows (`REGISTRY_VERSION` stays 1.0.0), 6 cross-field rules in `profile_checks`; `AgeBracket.representativeMonths` (`Option`, default profile 14/20/28/40/54, `params` checks `minMonths ≤ r ≤ maxMonths`, baselines/norm candidates never carry it); `asd_core::display::{DisplayMetric (10), BandStatus (4), Comparator, BandTarget, band}` and `quality::session_verdict`, boundary tables against a Swift-operator oracle; `params::validate_for_device` + `unknown_parameters` (device ignores unknown members, still checks known ranges; `asd_sync::accept_bundle` validates the raw delivered profile with it, `ignored_profile_members` for the log); publish (`validate`) stays strict; FFI `display_params_from_profile`, `display_band`, `session_quality_verdict`, `age_bands_from_profile` (UniFFI records, missing/undecodable sections → defaults); wasm `displayBand`, `sessionQualityVerdict`; catalogue +48 keys (en 464 / ne 464 draft), schemas (`api-v1`, `config-bundle-v1`, `param-registry-v1`, `i18n-v1`) and console `domain.ts` regenerated; 19 new Rust tests; goldens untouched. Deviations: `Comparator` has a fourth variant `Below` (strict `<` for latency, so targets read "< 550 ms"); `BandTarget.unit` is the registry unit wire string; wasm takes the profile JSON (not a params record); unknown-key log is `ignored_profile_members`, not text in the verdict message; console contract mirrors also touched (zod `display`/`sessionQuality`/`representativeMonths`, mock profile 1.3.0, bundled `param-registry-v1.json` copy, `research.ts` group labels) or its contract tests fail. Open: console's default-profile draft (wasm default) is now 1.3.0, so no publish of it before X.0c is on pilot devices (Q17); `scripts/build_asd_android.sh --test` not run (disk); dev's `rbac_matrix::org_patch_merges_settings_and_validates` fails since L.1e (`settings.locale` now 400), not X.0c | DSP-1..7; detail `docs/spec/modules/asd-localisation/work-packages.md` step 0; after L.0, parallel with L.1.0; goldens unchanged; no 1.3.0 publish before an app with X.0c is on pilot devices (ADR-0013 Q17) |
| WP-L.0 | Step 0 of localisation: `asd-contract::i18n` catalogue (`i18n/domain.{en,ne}.json`, optional partial `hi`; enum labels, `flag_assessment.*`, registry labels moved from `research.ts`, units, age-band formats, `problem.<slug>`), `FlagCase` in asd-core + wasm `flagCase`, `claims::DENY_TERMS`, `asd-cli i18n export --target console|xcstrings|android`, `i18n check`, `i18n lint --swift --kotlin` (baseline then fail, `scripts/i18n/literal-baseline.txt`), `i18n-v1.json` + negotiation fixture, contract `UserProfile.locale/effective_locale`, `Org(Patch).default_locale`, `i18n::negotiate` (user > org > Accept-Language > en), **migration 0028** `users.locale` + `organizations.default_locale`, generated console `domain.ts` | 8 | S | done | 2026-11-04 (`6cd8345`): `asd_contract::i18n` catalogue en 416 / ne 416 (all draft) / hi 11; required keys computed from Rust enums, registry, units, 65 problem slugs (spec said 67); `claims::DENY_TERMS` mirrors the Python list (9 keys exempt via `CLAIMS_OK`); `asd-core::FlagCase` + wasm `flagCase`; `asd-cli i18n export|check|fmt|lint` (lint baseline 411 Swift entries, may only shrink; CI in rust.yml + ios_test.sh); `i18n::negotiate` + 29-case fixture; migration 0028 (`users.locale`, `organizations.default_locale`, `settings.locale` moved); `tests/migrations.rs` no longer assumes contiguous numbers; console `domain.ts` generated, registry labels moved out of `research.ts`. **WP-B.21 takes 0029** (0027 stays unused). | ADR-0013 D7; I18N-1..8, I18N-10..12; detail `docs/spec/modules/asd-localisation/work-packages.md` step 0; after X.0; unblocks L.1.0, X.0c, L.1a–e, L.2, L.4, L.5; 0027 stays B.21 (B.21 → 0029 if L.0 lands first) |
| WP-L.1 | Console fully in `ne`: L.1.0 seams (`src/lib/i18n` resolve/provider/`getText`/`useText`, message files as `{en, ne}` typed by `en`, `<html lang>`, locale-aware `format.ts`, `problemText`, self-hosted Noto Sans Devanagari, ESLint `react/jsx-no-literals` + `curv/no-literal-ui-props` at warn) then wave L.1a shell/sessions/review/devices/audit, L.1b record/clinical, L.1c exports/privacy/security/settings (language selectors)/share, L.1d users/catalog/config/research, L.1e backend `/users/me` + `/orgs/{id}` locale; integration flips the gate to error | 8 | I | todo | ADR-0013 D7; I18N-9, I18N-13..23; detail `docs/spec/modules/asd-localisation/work-packages.md` (L.1.0 `I` S; L.1a–c `I` I; L.1d `I` S; L.1e `I` S); after L.0; console packages run alone among console packages (not with C.22) 2026-11-04 L.1.0 done (`b6931a0`): `src/lib/i18n` (resolve via `/users/me` `effective_locale`, else Accept-Language, else en; 29 fixture cases), `getText`/`useText`, `<html lang>`, 9 message files reshaped `{en, ne}` with `ne = en // ne: TODO` (`NE_PENDING`), research params via `paramText`, `problemText`, Noto Sans Devanagari self-hosted (OFL-1.1), `react/jsx-no-literals` + `curv/no-literal-ui-props` at warn (53 warnings), `LITERAL_GATE_ERROR_DIRS = []`, `e2e/locale.spec.ts` (+ acceptance-9 crawl as fixme); 927 console tests. Wave next: L.1a–e, M.1. 2026-11-04 L.1e done: `GET/PATCH /users/me` locale + negotiated `effective_locale`, `GET/PATCH /orgs/{id}` `default_locale`, `settings.locale` rejected, 7 tests in `tests/locale.rs`. |
| WP-L.2 | iOS String Catalog (`Localizable.xcstrings` en/ne/hi), ~360 literal call sites migrated, `AppLocalizer` folded, in-app language override, generated `Domain.xcstrings`, `i18n lint --swift` flipped to fail per directory | 8+ | I | todo | ADR-0013 D7; after L.0; largest localisation package |
| WP-L.3 | Nepali review of both catalogues by a native-speaking clinician (owner-arranged); fixes applied | 8+ | owner + I | todo | ADR-0013 Q6; after L.1, L.2 |
| WP-L.4 | Rust-rendered text: `ExportRequest.locale`, keys + params for `ProfileProblem`/`Problem`/flag texts, README and manifest per locale, CSV data dictionary; Devanagari PDFs stay with WP-I.5 | 8+ | I | todo | ADR-0013 D7; after L.0 |
| WP-L.5 | Android `res/values(-ne,-hi)/strings.xml` generated from the catalogue + app chrome strings | 9 | I | todo | with WP-M.5 |
| WP-M.0 | Org-user login on the device (`asd-sync::user_session`: login, TOTP, refresh rotation, step-up; Keychain/Keystore; device token unchanged) | 9 | S | todo | ADR-0013 D4; iOS first, Kotlin with M.5 |
| WP-M.1 | iOS reads display bands, session-quality cut-offs, `aoi.dwellCapMs` and `ageBands` (with `representativeMonths`) from the installed bundle through `asd-ffi` (`display_band`, `session_quality_verdict`, `age_bands_from_profile`; bundled default as fallback); composite/per-clip rings and the PDF status take `AttentionFlag`/`ExpectationStatus` from Rust; `PediatricAgeBracket` → `ageBandId` with a legacy decoder (same `ageMonths`); ODR fallback split and the dashboard duration tag deleted; gate `scripts/check_ios_constants.sh` in `ios_test.sh` | 8 | I | todo | INV-1 iOS rows, AGE-2, DSP-8..11; inventory 34 sites (28 product, 6 exempt) in `asd-localisation/technical.md` §5.1; detail `docs/spec/modules/asd-localisation/work-packages.md`; after X.0c, L.0; before L.2 |
| WP-M.2 | Native clinician companion screens: offline visit entry and follow-up completion queued through `asd-sync` (`build_encounter_request`), worklist/follow-ups read-out; role-gated | 9 | S | todo | ADR-0013 D1(b); after M.0, C.22 |
| WP-M.3 | Family view of the own record: device-plane `GET /v1/participants/me/{follow-ups,visits}` scoped to the device's tokens, audited `participant.self_read`, caregiver screen with non-diagnostic copy, threat-model row | 9 | S | todo | ADR-0013 D1(c)/D5, Q3; after B.19 |
| WP-M.4 | Follow-up reminders as local notifications from synced follow-ups (no messaging) | 9 | I | todo | ADR-0013 Q2; after M.3 |
| WP-M.5 | Android caregiver app on the AAR: capture (per A.14 outcome), calibration/validation flow, scoring, offline queue via `asd-sync`, results, consent; en/ne/hi | 9+ | S | blocked | ADR-0011 Q1 (A.14 number sheet) |
| WP-M.6 | Android clinician companion screens in the same app | 9+ | S | blocked | after M.5, M.0 |
| WP-M.7 | Family account (any phone, two parents, recovery) | 9+ | A + S | blocked | ADR-0013 Q1, ADR-0011 Q3, O-6 |
| WP-C.22 | Console responsive layout + PWA manifest/icons/install (clinician companion on the clinic iPad): worklist, follow-ups, search, visit entry, case read-out at tablet width; same RBAC and audits | 8 | I | todo | ADR-0013 D1(b), ADR-0011 Q10 reopened |
| WP-C.23 | Console navigation and route groups: five top divisions (Clinic, Research, Configuration, Data, Administration) + left sidebar + account menu; route folders and URLs regrouped (`clinic/`, `data/`, `admin/`, `account/`), 308 redirects from old paths, `src/lib/paths.ts` as the single path source, drawer below 1024 px | 8 | S | done | 2026-11-09 landed: `src/lib/paths.ts` (single path source, `LEGACY_REDIRECTS`), `navTree`, `components/shell/` (AppShell, TopBar, SideNav, AccountMenu, Breadcrumb; one props contract, swappable for Carbon per ADR-0014 O-14), 15 folders `git mv`-ed, 308 redirects in `next.config.ts`, e2e `navigation.spec.ts`; 1025 unit tests, build + CSP ok. Deviations: breadcrumb above the h1 (N-9), "operator" role does not exist. Known e2e flakes in `research.spec.ts` (:221 DeltaBars hydration, :237 shared mock state) predate this package. Spec §3a N-1..N-9; groundwork for WP-C.22 |
| WP-O.1 | Open-source files: licence alignment (Apache-2.0 code, CC-BY-4.0 docs), `THIRD_PARTY_NOTICES.md`, SECURITY/CONTRIBUTING/CODE_OF_CONDUCT/CITATION/CODEOWNERS/GOVERNANCE, templates, README standards section | 9 | I | todo | ADR-0014 O-1, O-3; needs owner O-1 |
| WP-O.2 | Supply chain: Dependabot, cargo audit, npm audit, gitleaks, CodeQL, CycloneDX SBOM, cosign + attestations, REUSE lint | 9 | I | todo | ADR-0014 O-4; can start now |
| WP-O.3 | Public-push runbook: history rewrite or fresh-history repo, rotation check | 9 | owner + I | todo | ADR-0014 O-2 |
| WP-O.4 | FHIR R4 `Consent` and `AuditEvent` exports + contract v1.9 `consent` | 9 | S | todo | ADR-0014 O-6/O-7; with WP-S.6 |
| WP-O.5 | Terminology licence flags (SNOMED per org), OpenAPI from `asd-contract` | 9 | I | todo | ADR-0014 O-8/O-9; needs owner O-8 |
| WP-O.6 | Carbon Design System spike: UI Shell replaces `components/shell`, one table page, bundle/CSP check | 9 | S | todo | ADR-0014 O-14; after WP-C.23; needs owner O-14 |
| WP-O.7 | Accessibility automation: axe in e2e, `docs/accessibility.md`, WCAG 2.2 AA fixes | 9 | I | todo | ADR-0014 O-15; after WP-C.23 |
| WP-O.8 | OIDC login option (PKCE) + Keycloak reference service | 9 | S | todo | ADR-0014 O-11 |
| WP-O.9 | ASVS L2 self-assessment, OTel exporter, regulatory position statement, DPIA GDPR/Nepal mapping | 9 | I | todo | ADR-0014 O-12/O-13/O-5 |
| WP-W.2 | Website screenshots: 16 iOS store captures and 19 console captures produced as local files (never committed); owner uploads to a CDN; then replace the verdict-era images on the landing site | 1 | I | done (capture) | 2026-11-09 delivered; console set regenerated after WP-C.23; CDN wiring waits on owner URLs |
| WP-X.11 | Dataset entity (freeze, hash, exclusions, labels, splits, lineage) + console pages | 10 | S | done | ADR-0015 R-7..R-9; 2026-11-09: design note `docs/spec/modules/asd-research-toolkit/datasets.md`; `asd_contract::datasets` (DTOs, `ExclusionRule` with the V-5 kinds, `content_hash` = sha256(JCS) over sorted members + exclusions + rules + split + label kind + lineage, `assign_split` seeded and stratified by age band, largest remainder), run DTOs `dataset_id`/`dataset_hash`/`definition.dataset`, domain keys `dataset_status`/`dataset_split`/`label_source_kind`/`exclusion_rule_kind`/`dataset_column` (ne draft); migration **0029** (`datasets`, `dataset_members`, freeze triggers, `analysis_runs.dataset_id/dataset_hash`); routes `POST/GET /v1/research/datasets`, `GET /{id}`, `GET /{id}/members` (+`SessionRead`, audited per page), `POST /{id}/archive`; labels = signed clinical review disposition with `R-` pseudonyms; erasure marks members `erased` (hash kept); runs accept `dataset_id` (snapshot membership, `session_gone` for members erased since); console `research/datasets{,/new,/[id]}`, run dialog dataset source (`?dataset=` pre-fill), provenance shows dataset fields, "Datasets" in the sidebar, mock + e2e `datasets.spec.ts`. Deviations: split/hash in `asd-contract` (asd-core was owned by WP-X.19 in parallel; move later + WASM verify), `tukey`/`mad_z` and `labels: import` refused until X.22/X.16, exports page does not read the dataset pre-fill yet (X.15); also fixed the `<title>` hydration mismatch in `DeltaBars`/`SweepCurve` that made the sweep e2e flaky |
| WP-X.12 | `asd-core::stats` (ECDF, KDE, QQ, Welch, Mann–Whitney, Hedges g, Cliff's delta, Holm, Bland–Altman, ICC, BCa bootstrap) + run kinds `group_compare`, `agreement` | 10 | S | todo | ADR-0015 R-2, R-4; can start now |
| WP-X.13 | LMS age percentile curves with bootstrap bands; promote to draft baselines | 10 | S | todo | ADR-0015 R-3; after X.12; owner min-n |
| WP-X.14 | Vega-Lite chart specs from Rust (`asd-core::viz`) + `vega-embed`; SVG/PNG/spec downloads; chart table views | 10 | S | todo | ADR-0015 R-14/R-15; needs owner R-14 |
| WP-X.15 | Export package: Frictionless Data Package, Croissant, DataCite, BIDS eye-tracking derivatives, README/CITATION | 10 | I | todo | ADR-0015 R-10/R-11; after X.11 |
| WP-X.16 | Import wizard: CSV column mapping, BIDS import, participants/labels CSV, de-id at the door | 10 | S | todo | ADR-0015 R-12; after X.11; owner scope |
| WP-X.17 | Classifier evaluation on labelled datasets (ROC/PR/calibration with bands) under governance | 10 | S | todo | ADR-0015 R-5; after X.11, X.12 |
| WP-X.18 | Gaze helpers: I-DT, time-to-first-fixation and dwell curves, cohort heatmaps | 10 | S | todo | ADR-0015 R-6; after X.14 |
| WP-X.19 | `asd-core::replay` + WASM: fixation/saccade events with profile params, AOI-hit ribbon, windowed metric traces with expected bands, KDE heatmap grids, outlier rules | 10 | S | done | 2026-11-09 landed: `asd_core::replay` {events (I-VT reusing `metrics::ivt_runs`, I-DT option), aoi_ribbon, windowed_trace, expected_bands (baseline range + scoring atypical boundary), heatmap_grid/cohort_grid/difference, outliers::screen}; WASM `replayEvents/Ribbon/Trace/Heatmap`, `expectedBands`, `heatmapCohort/Difference`, `outlierScreen`; 27 core + 5 wasm tests, native==wasm32 parity; workspace 1063 tests. New `metrics` knobs (`fixationAlgorithm`, `idtDispersionNorm`, `blinkMinSec`, `blinkMaxSec`, skipped while default) need contract/registry rows + `replay.band.*` i18n keys: follow-up WP-X.19b after X.11 |
| WP-X.19b | Contract follow-up of X.19: replay-only `metrics` knobs in `ScoringProfile` + registry, `replay.band.*` catalogue keys | 10 | S | done | 2026-11-09: `asd_contract::config::MetricsParams` gains `fixationAlgorithm` (`ivt`/`idt`), `idtDispersionNorm` 0.05, `blinkMinSec` 0.05, `blinkMaxSec` 0.5 (profile version stays 1.3.0: additive, defaulted, no stored value depends on them); registry rows (Fixation stage, `frames` reprocess, since 1.3.0, not owner-gated) with a new optional `ParamSpec.affects: ["replay"]` (`params::REPLAY_ONLY`); cross rule `blinkMinSec <= blinkMaxSec`; the four `skip_serializing_if` in `asd-core` profile removed, so defaults now serialise (goldens `scoring_profile_is_camel_case`, `config_defaults`, `replay::events` updated to that rule); `replay.band.{expected,atypicalBelow,atypicalAbove}` required from `asd_core::replay::BAND_LABELS` (en + ne draft) plus `param.metrics.*` label/help; schemas, console registry copy and `messages/domain.ts` regenerated |
| WP-X.20 | Console Replay tab + timeline strip + expected-versus-actual clip charts + heatmap pair + tuning side-by-side | 10 | S | done | 2026-11-09: spec `asd-research-toolkit/replay-console.md`. `lib/asd/replay.ts` (zod mirrors + wrappers of the 7 replay exports), `components/replay/` {ReplayCanvas, TimelineStrip, TraceChart (band, mean, labelled thresholds, Table, CSV), ClipList (recorded vs expected + scoring status word), HeatmapPair, SessionReplay, TuningReplay, drawModel, traceData, replayStore}; session `?tab=replay`; Quality tab reads the playhead (text + drift-chart line); workbench "Replay" toggle (published vs draft, 150 ms debounce); role colour tokens; mock frames in `raw_payload.frames` (+ `ageMonths`) and replay clips/expectations in the mock effective config; tests `replay-draw`, `replay-components`, `replay-wasm`, e2e `replay.spec.ts`. Open: live records carry no `frames` (replay says so; a core export turning the research frame log into frames is needed), no session age in the API (record `ageMonths` or the age field), cohort heatmap needs a session-to-run link (hidden with a sentence), no Web Worker for >20k frames, axe pending WP-O.7 |
| WP-X.21 | Clip media upload (sealed, sha256, licence ack) and audited serving for replay | 10 | S | todo | ADR-0015 V-8; needs owner H-8 position |
| WP-X.22 | Outlier and filter tools on datasets with what-if aggregates | 10 | S | todo | ADR-0015 V-5; after X.11, X.19 |
| WP-X.23 | Centile strip component on session detail and review | 10 | I | todo | ADR-0015 V-7; after X.19 |

**Migration rule (2026-10-31 audit):** backend migrations are applied in order and numbers 0007 and 0010–0012 were never used. A new migration must take the next number above the highest existing one (≥0024 once WP-B.16 lands: 0020 is WP-B.11 pending TOTP, 0021 is ingest `request_hash` + housekeeping indexes, 0022 is WP-B.14 plaintext-column drop, 0023 is reserved for WP-B.16 `users.totp_required_by`), never a gap, or it will be applied out of order on databases that already ran later migrations. **2026-11-02 (verified: highest existing is 0023):** phase 7 reserves **0024** WP-B.19 (EMR-lite tables), **0025** WP-I.13 (share links, case-share columns), **0026** WP-B.21 (identity); wave 1 adds no migration; anything else takes ≥0027. **2026-11-03 (phase 8; verified: highest existing is 0025):** **0026** is taken by WP-X.0 (research toolkit: experiments, cohorts, analysis runs, research-frame columns, `jobs.progress`/`cancel_requested_at`, `sessions.age_months`, export format) because WP-B.21 is blocked on an owner decision and a reserved-but-unapplied number followed by a higher applied one is exactly the out-of-order case; **WP-B.21 moves to 0027**; anything else takes ≥0028 (WP-L.1's `users.locale` is the next candidate). **2026-11-09 (verified: highest existing is 0028):** **0029** is WP-X.11 (research datasets); WP-B.21 (still unapplied) takes the next free number above the highest when it lands.

**Audit 2026-10-31 (independent backend-security and gaze/contract reviews, fresh-DB gates, live smoke `scripts/live_smoke.sh`).** Fixed: superadmin/admin clinical-role provisioning bypass, global config publish by org roles, export reclaim overwrite, list indexes, ingest 500s, worklist MRN flag, TOTP replay (0015); gaze recalibration after pose shift, replay retries, failed-recal isolation, empty validation (asd-core); 1.7 report decode; console report link; export missing-artifact 410. Open (owner or follow-up):
- ~~WP-G.3 gap: per-clip ClipTracking producer~~ done 2026-11-01.
- ~~Swift aggregate fallback writes socialPreference 0 / flatAffect 1 when scoring fails~~ done 2026-11-01 (1.8 `scoringStatus: unscorable`).
- Gaze L2 deferred: `calibrated_median_norm` absent for angularAffine.
- ~~Login does not ask for TOTP; `totp_secret` stored in clear.~~ done 2026-11-01. Enrol-code rotation accepts expired codes (CON-11 as specified).
- ~~Norm-run promote lets org roles create a draft global baselines document~~ done 2026-11-01.
- Smoke export download + sha256 needs `ASD_BLOB_BACKEND=s3` (MinIO); the memory store is per process.
- ~~Flaky timing test `tests/auth.rs`~~ done 2026-11-01 (injectable clock).
- 2026-11-01: login now requires TOTP for enrolled users (`totp-required`/`totp-invalid`, console code step); seeds encrypted under `ASD_DATA_KEK` (0016, backfilled by `asd-backend migrate`); norm-run promote is superadmin-only. Follow-ups: KEK rotation must re-encrypt seeds (no tool yet) → WP-B.15; drop plaintext `users.totp_secret` column in a later migration → WP-B.14; no TOTP enrolment flow yet → WP-B.11 + WP-C.15 (all phase 5, ADR-0009).
- 2026-11-01 session schema 1.8 (SCH-5, migration 0019): required `scoringStatus` (`scored`|`unscorable` + reason), the five score/affect fields optional; backend accepts 1.6/1.7/1.8, unscorable → flag `unreliable`, review reason `unscorable`, excluded from norms; console/exports show "Not scored". Follow-up: iOS writer to 1.8 and remove the SessionBiomarkerAggregator placeholder scores.
- 2026-11-01 phase 4: iOS writes session 1.8; the SessionBiomarkerAggregator placeholder scores are gone, failed scoring is recorded as `scoringStatus: unscorable` (fixtures `expected-session-v1.8*.json`, checked by `ios_test.sh`). iOS sends `rawPayload.normCollection` only with research consent plus the norm-collection opt-in (open: whether research consent without the opt-in counts as `research_consented` in exports; 2026-11-01 ADR-0009 Q4 recommends keeping the conservative reading, owner decision under DPIA O-6).
- 2026-11-01 catalog checks: `asd-contract::catalog_checks` shared by backend (errors carry `body.<path>`), `asd-wasm` `validateCatalog` and the console catalog editor; config problem slugs added to the contract. Catalog AOI `endSec` 10.0/9.0 trimmed to 8.0 in both catalog copies (all ten videos measure 8.0 s; the `_10s`/`_3s` ids are legacy names).
- 2026-11-01 C.14/C.5 backend gaps closed: timeline `kind`/`from`/`to` filters, `?participant_id=` on the session list, `GET /v1/participants/{id}/summary` for console users, `GET /v1/sessions/{id}/raw` (sandbox re-score uses the real payload); `asd-wasm` decodes 1.8.
- 2026-11-01 console gaps closed (0018): GET org user, audit filters, integrations paging, `GET /v1/orgs/{id}/export-destinations` for exporters, `DELETE /v1/integrations/{id}` archives (409 while deliveries in flight).
- 2026-11-03 live console↔backend pass (record workspace + share-a-case, `apps/curv_console/e2e/live/run-record-live.sh` with an in-memory S3 stand-in): four console fixes (clear follow-up date sends `""`, Files tab on the real attachments list, timeline keys, `ApiError` across module copies). Backend fixes: a visit posted by a `clinician` defaults `clinician_id` to the caller (REC-4); a case can be shared with visits/files but no session (SHR-7, `sessions_n: 0`). Open (owner): share-link opens record the console's IP, not the recipient's (needs console → backend IP forwarding behind `ASD_TRUST_PROXY_HEADERS`); research packages of a sessionless case are refused by the job, not the route.

## 1. Phase 1 — Demo-ready vertical slice (2026-10-27 → 2026-11-16)

Goal: a real session captured on an iPhone lands in a cloud backend and is visible in a console, with no over-claims. Exit = all D1–D9 in `01 §5`.

### WP-A.1 Device checklist + demo tools (I)
- Touch: `apps/ios_asd_gaze_app/Sources/Views/DemoToolsView.swift` (new), `project.yml` (`ASD_DEMO_TOOLS` flag, Debug + a new `DemoRelease` configuration), `scripts/check_release_symbols.sh` (assert flag absent in Release).
- Must not: change capture pipeline, claims copy, store metadata.
- Accept: checklist table in `01 §6` filled; demo tools present in `DemoRelease`, absent in `Release` (test in `BuildConfigurationTests`).

### WP-A.2 Known bug fixes (S)
- Touch: `MainGazeScannerView.swift`, `DataDeletionService.swift`, `BiomarkerClassificationEngine` call site, `RiskStratification`.
- Accept: regression tests for each; app suite green via `scripts/ios_test.sh`.

### WP-A.3 Protected history store (S)
- Replace UserDefaults in `GazeSessionHistoryStore` with a file store under Application Support with `.completeUntilFirstUserAuthentication`, migration from defaults, dedupe by sessionId preserved.
- Accept: migration test; existing `testGazeSessionPersistence` passes unchanged.

### WP-A.4 Sync client v2 (S)
- Touch: `ResearchKitSyncService.swift`, `ResearchStudyConfiguration.swift` (add `apiBaseURL`, keep https/DNS rule), new `DeviceRegistrationService.swift` (enrol code path first; App Attest behind a flag), Keychain storage of device token.
- Headers: `Authorization`, `X-Idempotency-Key`, `X-Schema-Version`, `User-Agent`. Payload: `GazeSessionRecord` + `perVideo` rows (already in record). Pre-signed PUT for `frames.jsonl.enc` and `report.pdf` when server returns URLs.
- Accept: existing queue/backoff/dead-letter tests still pass; new tests for headers, 401 → re-register once, 409 idempotent replay treated as success; contract test against JSON Schema from WP-D.2.

### Rust core and backend (WP-R.*, WP-B.*, WP-D.7, WP-C.8, WP-A.11) — detail in `docs/spec/modules/asd-rust-core-and-backend/work-packages.md`
- **Wave 0 (serial, first):** WP-R.0 workspace + domain types (I, half a day).
- **Wave 1 (parallel, no shared files):** WP-R.1 scoring/flag/quality (S), WP-R.2 metrics/aggregate (S), WP-R.3 contract/schema/signing/sync/CLI (S), WP-R.4 norms/synth (I), WP-B.1 service scaffold (S).
- **Wave 2:** WP-B.2 auth (S), WP-B.3 ingest + console reads (S), WP-R.5 FFI + xcframework (S), WP-D.7 Python gate replacement (I), WP-C.8 console alignment (I).
- **Wave 3:** WP-A.11 iOS consumes Rust (S), WP-B.4 config bundles (S), WP-B.5 norm job (I), WP-B.6 care events (I, H-15).
- Accept (Phase 1 exit for this stream): acceptance scenarios 1–9 and 13–16 in the module's `acceptance.md`; D5 in `01 §5` is met by WP-B.1–B.3 behind a Cloudflare Tunnel with `BLOB_MODE=proxy`.
- Key decisions already made (do not re-litigate in packages): JSON at the FFI boundary; no server defaults for measurements; `200 duplicate:true` vs `409` on body mismatch; enrol codes with `max_uses` and `device_id` rotation; no legacy shims; Python tooling deleted, not wrapped.

### WP-C.1 Console scaffold (S) — `apps/curv_console/`; login, org switcher, overview, sessions table/detail (Clips, Quality, Report tabs), reads from WP-B.3 (contract fixes in WP-C.8). Playwright smoke. Accept: demo storyline step 4 can be performed.

### WP-C.2 Review queue + devices (I).

### WP-D.1 Stimulus clips (I + owner) — measure real durations (`ffprobe`), fix `stimulus_catalog.json`, document licence per clip in `NOTICE.md`; if H-8 = replace, generate/commission clips and update AOIs. Accept: catalog durations == file durations (test), NOTICE has a licence line per clip.

### WP-D.2 Schema + parity gate (I) — done with Pydantic; the generator no longer exists. Replaced by WP-R.3 (schemars export) and WP-D.7 (`asd-cli schema check` in `ios.yml` and `rust.yml`).

### WP-D.4 Synthetic generator (I) — done in Python (v1.0); replaced by WP-R.4 (`asd-core::synth` v2.0, ratios and the three statuses per CON-13) and deleted in WP-D.7. The Swift `SummaryFixture` stays as a kit test helper only.

### WP-E.1 Pitch re-base (I); WP-E.3 docs drift sweep (I).

## 2. Phase 2 — Configuration & customisation (2026-11-17 → 2026-12-14)

Goal: an admin can change what the app shows and how it scores, safely, without an app release.

- **WP-B.4** config documents, bundles, signing, `GET /config/bundle`, ETag; adoption telemetry (which device pulled which version).
- **WP-A.5** kit refactor: `StimulusCatalog`, `NormativeBaselineLibrary`, `PerVideoScoringEngine`, `AttentionFlag`, `GazeQualityGate` take parameters from a decoded `ConfigBundle` (the `ScoringProfile` type is `asd-contract`'s, CON-5); signature verified through `asd-ffi verify_bundle` (CON-6), not a second CryptoKit implementation; cache in Application Support; fallback to bundled JSON; session records `bundleVersion`. Depends on WP-A.11. Accept: all kit tests pass with both bundled and fetched config; tampered signature → fallback + log.
- **WP-C.4** catalog editor, **WP-C.5** baselines/scoring editors + sandbox + publish-with-approval, **WP-C.3** participants/users/audit.
- **WP-B.5** norm aggregation job + promote; **WP-B.6** care events (if H-15 says Phase 2); **WP-R.6** WASM for the sandbox re-score in WP-C.5.
- **WP-A.6** Swift 6, **WP-A.7** renames + scanner fix.
- **WP-D.3** age-band model on a cleared dataset (H: pick dataset with a licence that allows shipping; UTKFace is research-only). Register in model registry (WP-C.6 later; for now the `models/asd_age/README.md` manifest).
- **WP-E.2** org account, domains, privacy policy page.

Exit: a scoring-profile change published from the console changes a re-scored session on a device within one app launch; audit shows who/when; sandbox shows the impact before publish.

## 3. Phase 3 — Data platform & pilot readiness (2026-12-15 → 2027-01-18)

- **WP-B.7** AWS deployment (reuse `aws_infra/` CDK patterns; new app `aws_infra/asd/`); staging + prod; backups; alarms.
- **WP-B.8** server PDF/FHIR parity; **WP-B.9** exports, retention, erasure.
- **WP-C.6** cohorts, norm-runs, exports, model registry.
- **WP-A.8** stimulus CDN; **WP-A.9** replay down-sample upload; **WP-A.10** TestFlight beta.
- **WP-E.4** M-CHAT decision; **WP-E.5** native locale review; **WP-E.6** security/privacy pack.
- **Clinician workspace + interop bridge** (ADR-0008, `docs/spec/modules/asd-clinician-and-interop/`): Wave 1 WP-C.9 + WP-I.1; Wave 2 WP-C.10, C.11, I.2, I.3, I.4, I.5; Wave 3 WP-I.6, C.12, C.13; Wave 4 WP-I.7, C.14, I.8. Exit: a clinician signs a session and an HL7 ORU^R01 file lands on the hospital's SFTP, while a researcher downloads a de-identified Sheets CSV with no identifiers.

Exit: a clinic (or friendly pilot site) can be onboarded end to end: org created, devices enrolled, sessions flowing, reviewers working the queue, researcher exports a labelled dataset.

## 4. Phase 4 — Pilot & store (2027-01-19 → 2027-02-28)

- **WP-E.7** App Store submission dry run; **WP-C.7** console polish; **WP-B.10** FHIR façade only if a partner needs it; **WP-D.5/D.6** ML roadmap doc and C++ decision.
- First own-norm candidate baselines from pilot data (if n≥12 per cell reached — unlikely this early; the point is the pipeline works).

## 5. Gates every WP must pass

- iOS: `scripts/ios_test.sh` (kit tests, TSan, strict concurrency, app tests, Release build, claims, privacy API, release symbols).
- Rust (core + backend + CLI): `scripts/rust_test.sh` (fmt, clippy `-D warnings`, `cargo test --workspace` with the Postgres service, `cargo deny check`, `sqlx prepare --check`, `asd-cli schema check`); coverage ≥85 % on `asd-core` and `asd-backend`.
- Console: `pnpm -C apps/curv_console lint test e2e`.
- Cross: `asd-cli schema check` + `schema validate` on the iOS fixtures (GATE-1, replaces WP-D.2's Python script); golden parity vectors pass in both `cargo test -p asd-core` and the kit's `GoldenParityTests` (FFI-4).
- No new tracked binaries > 1 MB; no model weights without manifest + licence; no personal data.

## 6. Suggested parallelisation for the first two weeks

| Week | Stream A (iOS) | Stream B (backend) | Stream C (console) | Stream D/E |
|---|---|---|---|---|
| 2026-10-27 | WP-A.1, WP-A.2 | WP-B.1 | — | U1–U5, WP-E.1, WP-D.1 |
| 2026-11-03 | WP-A.3, WP-A.4 | WP-B.2, WP-B.3, WP-B.4 | WP-C.1 | WP-D.2, WP-D.4, WP-E.3 |
| 2026-11-10 | demo hardening | tunnel/deploy, seed | WP-C.2 | rehearsal |

Each stream is one agent; streams share no files except `project.yml` (A only) and root `package.json` (C only).

### Rust stream from 2026-10-31 (replaces the Stream B column above)

| Step | Packages in parallel | Tier | Shared files |
|---|---|---|---|
| 0 (serial) | WP-R.0 | I | creates everything under `rust/`, `.gitignore`, `scripts/rust_test.sh`, `.github/workflows/rust.yml` |
| 1 | WP-R.1, WP-R.2, WP-R.3, WP-R.4, WP-B.1 | S, S, S, I, S | only `rust/Cargo.lock` (main session regenerates) |
| 2 | WP-B.2, WP-B.3, WP-R.5, WP-D.7, WP-C.8 | S, S, S, I, I | none (B.3 stubs the device-token extractor until B.2 merges) |
| 3 | WP-A.11, WP-B.4, WP-B.5, WP-B.6 | S, S, I, I | none |

## 7. Phase 5 batch — security operations (designed 2026-11-01, ADR-0009)

Implementation batch, not a calendar phase. Nine packages; the detail (files, tests
first, not-to-touch, traps) is in `docs/spec/modules/asd-rust-core-and-backend/work-packages.md`
("Phase 5 wave") and, for WP-G.8, `docs/spec/modules/asd-gaze-robustness/work-packages.md`.
MUST statements: `asd-rust-core-and-backend/technical.md §8`; acceptance scenarios 17–24.

| WP | One line | Tier | Wave | Shares files with |
|---|---|---|---|---|
| WP-B.11 | TOTP enrol/confirm/disable API, pending seed columns (0020), DTOs, schema regen | S | 1 | — |
| WP-B.12 | Daily `housekeeping_sweep` job (idempotency keys, refresh tokens, enrol codes, finished jobs) | I | 1 | — |
| WP-B.15 | KEK rotation: previous-KEK reads, `admin rotate-kek`, runbook | S | 1 | B.13, B.14 (`main.rs`, `config.rs`) |
| WP-C.16 | Console CSP (nonce) + HSTS, e2e violation listener | I | 1 | — |
| WP-A.13 | `trackingQuality` → pipeline config built in `asd-ffi`; kit stops assembling gate JSON | S | 1 | — (kit files disjoint from G.8) |
| WP-G.8 | iOS bench capture build (`ASD_BENCH_CAPTURE`, raw JSONL, 13-point grid) | I | 1 | — |
| WP-C.15 | Console security settings page (QR, confirm, disable, nudge) | I | 2 | needs B.11's `api-v1.json` |
| WP-B.13 | Prometheus listener + API HSTS | I | 2 | B.15 (wave 1), B.14 (`main.rs`) |
| WP-B.14 | Drop plaintext columns (0022) with guard, `migrate` pre-back-fill, `reset-totp`, remove `encrypt-review-notes` | S | 3 | B.11, B.15, B.13 |

Parallel groups: **wave 1** = {B.11, B.12, B.15, C.16, A.13, G.8}; **wave 2** = {C.15,
B.13}; **wave 3** = {B.14}. After each wave the main session runs `cargo sqlx prepare`,
regenerates `docs/spec/schemas/asd/api-v1.json`, runs every gate and commits.

Deliberately out of this batch: WP-A.6 Swift 6 and WP-A.7 renames (tree-wide, phase 6);
mandatory TOTP per role (O-10); DEK rotation (F2); console idle timeout; worker metrics;
HSTS `preload`; org-admin TOTP reset; the `research_consented` semantics (ADR-0009 Q4).
(2026-11-02: A.6, A.7 and O-10 are designed in §8 / ADR-0010; org-admin TOTP reset is
backlog WP-B.17; the rest is unchanged.)

Still blocked on the owner, unchanged: fonts for I.5/C.12 (Q3); IaC tool for B.7 (H-11);
catalog rules (org-role drafts, editable drafts, media/licence fields); clinical-record
erasure (ADR-0008 Q2 / O-1); G.14 device bench (H-G4/H-G5; G.8 only makes it possible);
DPIA decisions O-1..O-15. Dependencies of this batch on those: B.13's private interface
and the KEK-in-KMS custody (O-9) are wired by B.7; nothing in the batch waits for them.

Owner decisions for this batch (recommendation in brackets, none blocks wave 1):
1. O-10 roles that must have TOTP and the grace period [owner, admin, clinician,
   superadmin; enforce in phase 6 after a 14-day nudge].
2. Finished-job retention [30 days, unreferenced rows only].
3. HSTS `preload` [no, until WP-E.2 fixes the domain].
4. `research_consented` without the norm opt-in [keep excluded; explicit consent object
   in session 1.9 if the broader reading is wanted; record under O-6].
5. Whether `scripts/check_asd_claims.py` may be extended for WP-A.7 or must be ported to
   `asd-cli` first [small extension acceptable; it is dev tooling, not product logic].

Gates at the end of the batch: `scripts/rust_test.sh` with a UTF-8 Postgres
(`initdb -E UTF8 --locale=C -k ''` on a free port, `ASD_REQUIRE_ALL=1`),
`scripts/build_asd_wasm.sh --test`, `scripts/ios_test.sh`, and in `apps/curv_console`
`npm test`, `npm run lint`, `npm run typecheck`, `CURV_API_MODE=mock npm run build`,
`npm run e2e`.

## 8. Phase 6 batch — Swift 6 mode, renames and claims gate, mandatory TOTP (designed 2026-11-02, ADR-0010)

Implementation batch, not a calendar phase. Seven packages; detail (files, tests first,
not-to-touch, traps) in `docs/spec/modules/asd-gaze-app-store-readiness/work-packages.md`
("Phase 6 wave", iOS) and `docs/spec/modules/asd-rust-core-and-backend/work-packages.md`
("Phase 6 wave", backend and console). MUST statements: CONC-5, CI-3 (amended), CLM-7,
CLM-8; AUTH-11..14. Acceptance: store-readiness scenarios 28–30, Rust module 25–26.

Survey at `03d84a3` (Xcode 27.2, Swift 6.4.2, scratch builds, nothing committed): kit
compiles in Swift 6 mode with 0 errors (debug, tests, release; UniFFI bindings included);
app 8 errors in 7 source files (non-`Sendable` `static let`s, main-actor statics read
from nonisolated code, two region-isolation "sending" errors), widget and
`CareActivityKit` clean, ~10 SDK-callback warnings survive; app test target ~70
diagnostics in 13 files (48 in `DataDeletionServiceTests` + `CareKitClinicalStoreTests`).
`biomarker` is a wire name (session 1.6–1.8, config bundle, api-v1, vault JSON, bundled
baselines), so the rename is Swift identifiers only. A prose-literal claims scan finds 24
hits in 15 files today. The iOS app has no user login, so O-10 has no iOS surface.

| WP | One line | Tier | Wave | Shares files with |
|---|---|---|---|---|
| WP-A.6a | Kit `// swift-tools-version: 6.0`; `ios_test.sh` kit step drops `-strict-concurrency` (implied) | I | 1 | — |
| WP-A.6b | App `project.yml` `SWIFT_VERSION: "6.0"` (all targets) + the 8 source errors, no `@unchecked Sendable` | S | 1 | — (Sources only) |
| WP-A.6c | App test target: `@MainActor` test classes, `Box<T>` for captured results, same assertions | I | 1 | — (Tests only) |
| WP-A.7a | Claims scanner `--prose` mode + kit path + selftest (flag off in the gate until A.7b) | I | 1 | — (Python + denylist doc) |
| WP-B.16 | Mandatory TOTP policy: 0023, `auth_policy` constants, per-request `403 totp-enrolment-required`, deadline at login/create/role change, `/users/me` fields, audit, `api-v1.json` | S | 1 | — |
| WP-A.7b | `Biomarker*`→`Engagement*` renames (Swift + docs; wire names untouched), triage of the 24 findings, `ios_test.sh` runs `--prose` | I | 2 | A.6b/A.6c files (hence wave 2); alone among iOS packages |
| WP-C.17 | Console: nudge with deadline, layout redirect for `required`, required panel, mock states, e2e | I | 2 | needs B.16's schema; shares nothing with A.7b |

Parallel groups: **wave 1** = {A.6a, A.6b, A.6c, A.7a, B.16}; **wave 2** = {A.7b, C.17}.
ADR-0009 D10 ("A.7 runs alone last") is kept for the iOS tree and relaxed for the console:
A.7b and C.17 share no files. After wave 1 the main session runs `xcodegen generate`,
`cargo sqlx prepare`, regenerates `api-v1.json`, runs every gate and commits; after wave
2 the same, with the claims gate now in `--prose` mode.

Deliberately out of this batch: console idle timeout and absolute session length (O-10
second half; cookie `max-age` + refresh TTL once the owner confirms numbers); org-admin
TOTP reset (WP-B.17 backlog); the claims-scanner port to `asd-cli` (WP-D.8 backlog); DEK
rotation (F2); worker metrics; HSTS `preload` (WP-E.2); `research_consented` (ADR-0009
Q4); `SWIFT_TREAT_WARNINGS_AS_ERRORS` for the app (the surviving warnings are
`@preconcurrency` SDK callbacks).

Owner decisions for this batch (recommendation in brackets; none blocks wave 1 because
the values are constants the package can change before it starts):
1. O-10 roles that must have TOTP [owner, admin, clinician, superadmin].
2. O-10 grace period [14 days from the user's first login after the release, set once].
3. Q5 `check_asd_claims.py` [extend now (A.7a); port later as WP-D.8].
4. Console session policy (idle / absolute) [30 min / 12 h; separate package after the
   answer].

Gates per wave: `scripts/rust_test.sh` with a UTF-8 Postgres (`initdb -E UTF8 --locale=C
-k ''` on a free port, `ASD_REQUIRE_ALL=1`), `scripts/build_asd_wasm.sh --test` (the
contract crate changes in B.16), `scripts/ios_test.sh` (both waves; wave 2 with
`--prose`), and in `apps/curv_console` `npm test`, `npm run lint`, `npm run typecheck`,
`CURV_API_MODE=mock npm run build`, `npm run e2e`.

## 9. Phase 7 batch — EMR-lite record, share-a-case, offline-first, one-command deployment (designed 2026-11-02, ADR-0011)

Implementation batch driven by the owner's product vision (`docs/product/VISION.md`).
Twenty packages in three waves; detail (files, tests first, not-to-touch, traps) in
`docs/spec/modules/asd-emr-lite-and-sharing/work-packages.md`. MUST statements: REC-1..12,
SHR-1..11, SYNC-1..10, DPL-1..11, HIP-1..9, TRS-1..3, AND-1..4 in that module's
`technical.md`; acceptance scenarios 1–25 in its `acceptance.md`.

What changed in scope: the hospital-push half of ADR-0008 (WP-I.9, I.10, I.11, B.10) is
deferred (no hospital systems at the target clinics); the record-keeping half becomes
core (participants, encounters, attachments, follow-ups, search); a consented, audited
"share a case" package with a passcode share link replaces the hospital bridge as the way
out; WP-B.7 (AWS) is superseded by a one-server Compose stack (answers H-11: Compose +
shell + Caddy, no CDK, `aws_infra/` stays flood-only); WP-B.17 is promoted. Verified
blocker for any longitudinal record: the iOS app mints `child-<sessionId prefix>` per
session in clinic mode (`GazeSessionRecord.swift:215-219`), so every clinic session is a
new participant on the server today (WP-A.16a fixes it).

| WP | One line | Tier | Size | Wave | Shares files with |
|---|---|---|---|---|---|
| WP-B.17 | Org-admin TOTP reset with caller step-up; shared `reset_for_user` | S | S | 1 | R.7 (`problem.rs`, two lines; main session resolves) |
| WP-R.7 | `asd-contract::emr` DTOs, QR payload, slugs, `$defs` | S | S | 1 | B.17 (`problem.rs`) |
| WP-I.12 | `asd-export::case_package` + `case-package-v1.json` | S | I | 1 | — |
| WP-B.18 | `deploy/clinic-server/`: Caddy TLS, `install.sh`, encrypted backups, restore, drill, upgrade, console Dockerfile, CI smoke | S | I | 1 | — (consumes B.22 when merged) |
| WP-B.22 | `admin requeue-erasure`, `erasure-ledger`, `verify` | I | S | 1 | — |
| WP-B.20 | Read audits (`audit::record_read`, failure = 500), blob `Content-Digest` | I | I | 1 | — |
| WP-A.12 | Android Kotlin bindings, `cdylib`, build script, AAR + golden parity, CI job | S | I | 1 | A.15 (`rust.yml`, separate blocks) |
| WP-A.15 | iOS offline-first sync hardening, `asd-sync` 409 slug classifier, fault injector, field script | S | I | 1 | A.12 (`rust.yml`) |
| WP-A.16a | iOS stable participant token per child profile | I | S | 1 | — |
| WP-C.21 | Console idle/absolute logoff (constants 30 min / 12 h) | I | S | 1 | — |
| WP-B.19 | EMR-lite backend: 0024, participants create/search, encounters, attachments, follow-ups, RBAC | S | I | 2 | I.13 (`jobs/erasure.rs`, additive hunks) |
| WP-I.13 | Case-package export + share links (backend): 0025, public share routes | S | I | 2 | B.19 (`jobs/erasure.rs`), B.23 (`ExportFormat`) |
| WP-C.18 | Console record workspace, create participant + QR, follow-ups, search, TOTP reset action | I | I | 2 | — |
| WP-C.19 | Console share-a-case dialog, share-link panel, public `/share/[id]` | I | S | 2 | — |
| WP-A.14 | Android capture feasibility spike → number sheet (owner decision) | S | I | 2 | — |
| WP-B.23 | Audit CSV export job + console button | I | S | 2 | I.13 (`ExportFormat`) |
| WP-B.21 | Participant identity `local_record` (0026) | S | I | 3 | blocked: ADR-0011 Q3 |
| WP-C.20 | Console identity panel + name search | I | S | 3 | blocked with B.21 |
| WP-A.16b | iOS clinic mode: scan participant QR | I | S | 3 | after B.19 |
| WP-E.8 | HIPAA / Privacy Act administrative pack (docs) | A + owner | I | 3 | drafts landed; final blocked: O-2, O-6, O-11, O-12, O-15 |

Parallel groups: **wave 1** = {B.17, R.7, I.12, B.18, B.22, B.20, A.12, A.15, A.16a, C.21}
(ten packages, none waits on an owner decision; `rust/README.md` is the one deliberately
shared file: each appends its own section); **wave 2** = {B.19, I.13, C.18, C.19, A.14,
B.23}; **wave 3** = {B.21, C.20, A.16b, E.8}. After each wave the main session runs `cargo
sqlx prepare`, regenerates `api-v1.json` (and `case-package-v1.json` after wave 1),
merges the named overlaps, runs every gate and commits.

Deliberately out of this batch: referral letters (C.12 re-scoped to the clinical summary
PDF), PDF/A fonts (I.5, owner Q3), DEK rotation (F2), a `frontdesk` role (ADR-0011 Q2),
ZIP encryption of the case package (Q4), PWA manifest for the console (Q10), any Rust
change for Android capture (the spike reports gaps; a package follows), `aws_infra/asd/`.

Owner decisions for this batch (recommendation in brackets; none blocks wave 1):
1. Capture device: iPad with TrueDepth now vs Android trial [pilot on iPads; run A.14 in
   parallel; decide on its number sheet].
2. Front-desk role [reuse `admin` in v1].
3. Identity in the record, ADR-0008 Q1 reopened [allow `local_record` opt-in per org;
   default stays `none`; gates wave 3].
4. Encrypt the case ZIP at rest [no in v1].
5. Share-link limits [72 h expiry, max 7 d; 1 download, max 5; 12-char passcode; 10
   opens/link/h, 30/IP/h].
6. Console session policy, O-10 second half [idle 30 min, absolute 12 h; constants in
   C.21].
7. TLS mode for the first pilot [`letsencrypt` with a public DNS name, else `tunnel`].
8. Backup retention and off-site target, O-12/O-7 [14 daily + 8 weekly on the box;
   encrypted off-site copies to a bucket in a region the owner chooses].
9. Cross-border consent text and evidence, O-6/O-2/O-15 [the share request makes a
   consent reference mandatory; the text and its storage are the owner's].
10. PWA install of the console [not for the pilot].

Still blocked on the owner, unchanged: fonts for I.5/C.12 (Q3); clinical-record erasure
(ADR-0008 Q2 / O-1); G.14 device bench (H-G4/H-G5); DPIA decisions O-1..O-15.

Gates per wave: `scripts/rust_test.sh` with a UTF-8 Postgres (`initdb -E UTF8 --locale=C
-k ''` on a free port, `ASD_REQUIRE_ALL=1`), `scripts/build_asd_wasm.sh --test` (the
contract crate changes), `scripts/build_asd_android.sh --test` (from wave 1),
`scripts/ios_test.sh` (`--prose`), in `apps/curv_console` `npm test`, `npm run lint`,
`npm run typecheck`, `CURV_API_MODE=mock npm run build`, `npm run e2e`, and
`bash deploy/clinic-server/test/compose_smoke.sh` (CI `deploy.yml`). Optional, numbers
reported: `ASD_FIELD_TEST=1 scripts/field_sync_test.sh`.

## 10. Phase 8 batch — researcher toolkit and the three-audience platform (designed 2026-11-03, ADR-0012, ADR-0013)

Implementation batch driven by the owner's three requests of 2026-11-03 (data-scientist
toolkit with dynamic ranges and age-driven analysis; one tool for researchers, doctors and
families on mobile and web; nothing hard-coded, everything localised, tunable at run time).
Toolkit detail (files, tests first, not-to-touch, traps) in
`docs/spec/modules/asd-research-toolkit/work-packages.md`; MUST statements PRM-1..9,
INV-1/2, RFL-1..8, RPX-1..6, EXP-1/2, COH-1..3, RUN-1..9, SWP-1..4, AGE-1..3, PRV-1..6,
GOV-1..6, RUI-1..7 in that module's `technical.md`; acceptance scenarios 1–22 in its
`acceptance.md`. Platform gaps (localisation WP-L.*, mobile/family WP-M.*, WP-C.22) are
rows in §0 with one-line scope and ADR-0013 §C as their priority order; each gets its
module spec when scheduled.

Verified facts the design rests on: the server cannot read `frames.jsonl.enc` (device
vault key; DPIA O-13), so frame-level reprocessing needs a consent-gated research frame
log (raw samples, sealed by the server under the org DEK); I-VT, affect and
low-confidence constants are hard-coded in `asd-core` and the wasm rescore ignores the
profile's `lowConfidenceValidRatio`; config documents are global and superadmin-only with
four-eyes; norms already use five age bands with min-n 12; the runner has no cancel,
progress or quota; the console is English-only with ~1,420 centralised strings and no
literal gate; iOS has 143 localised keys and ~360 literal call sites; ~20 traffic-light
cut-offs live in iOS views.

| WP | One line | Tier | Size | Wave | Shares files with |
| --- | --- | --- | --- | --- | --- |
| WP-X.0 | Step 0 seams: registry, profile 1.2.0, research DTOs, log format, RBAC, migration 0026, job kind, stubs, console stubs, schemas | S | I | 0 | everything (serial) |
| WP-X.1 | Core: constants → params, `reprocess`, `evaluate`, `Aggregator`, `suppress` | S | L | 1 | — |
| WP-X.2 | Backend: research frames, cohorts, experiments, promote, publish rule, `config/effective`, thresholds from profile | S | L | 1 | — |
| WP-X.5 | Console: research area, `RegistryForm`, workbench, live preview, single-session sweep | I | L | 1 | — (mock-first) |
| WP-X.6 | iOS: research frame log writer + third blob | I | I | 1 | — |
| WP-X.3 | Backend: `analysis_run` job, runs routes, cancel/progress/quota | S | L | 2 | — |
| WP-X.4 | wasm: `paramRegistry`, `reprocessSession`, honest `rescoreSession` | S | S | 2 | — |
| WP-X.7 | Console: cohorts, runs pages, charts, `useJobPoll`, drift lines | I | L | 2 | — |
| WP-X.8 | Export `analysis_results` (crate + job arm) | I | I | 2 | — |
| WP-X.10 | CLI `research reprocess|evaluate` | I | S | 2 | — |
| WP-X.9 | Sweeps + age profiles (phase B) | S | L | 3 | alone |
| WP-L.0 | Shared string catalogue + `asd-cli i18n` export/check/lint + locale contract + migration 0028 (localisation step 0) | S | I | after X.0 | — |
| WP-X.0c | Profile 1.3.0 `display`/`sessionQuality` rows, band + verdict in Rust, FFI, tolerant device acceptance | S | I | after L.0 (∥ L.1.0) | `asd-contract` (serial with L.0) |
| WP-M.1 | iOS display bands / age labels / constants from the bundle + grep gate (`asd-localisation` work-packages) | I | I | after X.0c | iOS-wide (before L.2) |
| WP-C.22 | Console responsive + PWA (clinician companion on the iPad) | I | I | any | console layout files (not with X.5/X.7 in the same wave) |
| WP-L.1 | Console `ne` + locale selection + literal gate: L.1.0 seams, then L.1a–d (console, directory-disjoint) ∥ L.1e (backend) ∥ M.1 (`asd-localisation` work-packages) | I | L | after L.0 | console-wide (runs alone among console packages) |
| WP-L.2 | iOS String Catalog migration | I | L | after L.0 | iOS-wide (runs alone among iOS packages) |

Parallel groups: **step 0** = {X.0}; **wave 1** = {X.1, X.2, X.5, X.6}; **wave 2** = {X.3,
X.4, X.7, X.8, X.10}; **wave 3** = {X.9}; the platform packages slot in as their
dependencies land and their "runs alone" notes allow (L.0 and M.1 fit wave 2; C.22 fits
wave 3; L.1 and L.2 after their trees are quiet). After each wave the main session rebuilds
the xcframework and `public/asd-wasm`, runs `cargo sqlx prepare`, regenerates
`api-v1.json`, `config-bundle-v1.json`, `param-registry-v1.json`, runs every gate and
commits.

Phase A (what the owner gets first, waves 0–2): tunable gaze-processing and scoring
parameters with validated ranges; a research frame log so a session can be fully
reprocessed from raw samples; a live single-session preview in the browser (sliders →
scanpath, dwell, subscores, flag) with a one-knob sweep; named experiments; cohorts by age
band, site, date, quality and consent; cohort re-score jobs with per-band distributions,
per-metric deltas, flag and quality transition counts, small-cell suppression, a
reproducibility record and re-open; de-identified JSONL/CSV/Parquet exports; promote to a
draft profile with the evidence attached; publish blocked without a run. Phase B (wave
3): two-axis sweeps and age profiles against published or draft baselines.

Deliberately out of this batch: new metrics; Android frame logging (WP-M.5 first);
differential privacy; publishing from the workbench; any Python; a chart library; sex in
the session schema (ADR-0012 Q8); a family account (WP-M.7, ADR-0013 Q1); messaging
(ADR-0013 Q2).

Owner decisions for this batch (recommendation in brackets; none blocks step 0 or wave 1):
1. Research frame log retention [= `retention_months`; separate setting exists].
2. Small-cell `k` [12; org setting 5..50].
3. Publish without an analysis run: refuse or warn [refuse].
4. O-13 stop uploading the device-sealed frames blob [stop with the WP-X.6 app release].
5. Who may promote an experiment to a draft [owner, admin, researcher; approve/publish
   unchanged].
6. Clinic server without a vendor account: owner may approve/publish under four-eyes?
   [keep vendor for the pilot; `org.local_publish` later].
7. Analysis budgets [ADR-0012 D6 values; `ASD_ANALYSIS_*` env].
8. `sex` in session schema 1.9 [yes, optional, with WP-X.6].
9. Owner-gated knobs (`trackingQuality.scoring.*`, H-G2) sweepable in analysis [yes].
10. Family account [no in phases 8–9; QR join]. 11. Messaging [out; local reminders].
12. What a family sees [follow-ups, visit headers, clinic line, own summary]. 13. Locale
selection [org default + user override; device OS language + in-app override]. 14. Bundle
staleness [refresh when > 1 h at launch/foreground; ≤ 24 h online; offline until
`expires_at` 90 d]. 15. Nepali reviewer [a Nepali-reading clinician before the pilot].
16. Devanagari fonts (ADR-0008 Q3) [ratify OFL]. 17. Clinician companion device [iPad PWA
first]. 18. Android go/no-go [A.14 sheet]. 19. Literal-string gate strictness [warn +
baseline → fail per directory].

Still blocked on the owner, unchanged: fonts for I.5/C.12 (Q3); clinical-record erasure
(ADR-0008 Q2 / O-1); G.14 device bench (H-G4/H-G5); DPIA decisions O-1..O-15; identity
(ADR-0011 Q3 → B.21 at 0027).

Gates per wave: `scripts/rust_test.sh` with a UTF-8 Postgres (`initdb -E UTF8 --locale=C
-k ''` on a free port, `ASD_REQUIRE_ALL=1`), `scripts/build_asd_wasm.sh --test`,
`scripts/build_asd_android.sh --test`, `scripts/ios_test.sh` (`--prose`), in
`apps/curv_console` `npm test`, `npm run lint`, `npm run typecheck`, `CURV_API_MODE=mock
npm run build`, `npm run e2e`, `npm run e2e -- --project=prod`, and
`bash deploy/clinic-server/test/compose_smoke.sh` (CI `deploy.yml`). From WP-L.0:
`cargo run -p asd-cli -- i18n check` and `i18n lint` (warn) in `rust.yml`.
