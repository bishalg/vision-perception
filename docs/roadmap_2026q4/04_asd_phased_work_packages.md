# 04 — ASD Phased Work Packages (2026-10-27 → 2027-02-28)

> Status table is the live tracker. Update it when a WP changes state. Phases overlap on purpose: app, backend and console streams run in parallel when they share no files.
> Agent tiers: **A** = `architect` (Fable, design only), **S** = `senior-implementer` (Opus), **I** = `implementer` (Sonnet). Main session integrates, runs gates, commits.
> **2026-10-30:** backend and shared maths are Rust (`CLAUDE.md`, ADR-0006). New prefix **WP-R.n** = Rust core crates (`rust/`). WP-B.* were renumbered to the Rust plan; detail per package (files, tests, traps) is in `docs/spec/modules/asd-rust-core-and-backend/work-packages.md`, this table stays the status tracker.
> **2026-10-31:** new prefixes **WP-I.n** = interoperability and export bridge (`asd-export` crate, deliveries) and **WP-C.9+** = clinician workspace; detail in `docs/spec/modules/asd-clinician-and-interop/work-packages.md` (ADR-0008). **WP-G.n** = gaze-tracking robustness; detail in `docs/spec/modules/asd-gaze-robustness/work-packages.md` (ADR-0007).

## 0. Status table

| WP | Title | Phase | Tier | Status | Notes |
|---|---|---|---|---|---|
| WP-A.1 | Device checklist + demo-tools screen | 1 | I | done | DemoToolsView, DemoRelease config, build tests, symbols gate |
| WP-A.2 | Fix known bugs (double CareKit write, delete-all semantics, classifier double-run, elevatedRisk string) | 1 | S | done | RiskStratification scoped to ASD_RESEARCH_CLASSIFIER, duplicate writes eliminated, classifier passed to summary |
| WP-A.3 | Protected file-backed history store (old WP-4.3) | 1 | S | done | Protected file store (.completeFileProtectionUntilFirstUserAuthentication), defaults migration, dedupe preserved |
| WP-A.4 | Sync client: device registration, bearer token, idempotency + schema headers, new `/v1/sessions` payload, pre-signed uploads | 1 | S | done | Device registration, Keychain token, idempotent /v1/sessions client, pre-signed uploads; 338 app tests green; contract unverified against backend |
| WP-A.5 | `ScoringProfile` + remote config bundle consumption with Ed25519 verify and bundled fallback | 2 | S | todo | |
| WP-A.6 | Swift 6 language mode (old WP-4.2) | 2 | S | todo | |
| WP-A.7 | `Biomarker*`→`Engagement*` renames + claims-gate scanner covers model strings (old WP-4.4) | 2 | I | todo | |
| WP-A.8 | Stimulus CDN support with signed URLs; ODR fallback | 3 | S | todo | |
| WP-A.9 | Replay down-sample upload (for console heatmaps) | 3 | S | todo | |
| WP-A.10 | TestFlight beta pipeline (fastlane `beta`), crash reporting decision | 3 | I | todo | |
| WP-A.11 | iOS consumes the Rust core: xcframework `binaryTarget`, engines become wrappers, golden parity tests, `asd-sync` in the sync client, `device_id` on re-register | 1–2 | S | todo | FFI-3/4, CON-7/11; depends on WP-R.5, WP-B.2, WP-D.7 |
| WP-A.12 | Android Kotlin bindings from the same UniFFI definitions (`cargo-ndk`) | 4 | S | backlog | depends on WP-R.5 |
| WP-G.0 | Swift capture hygiene: record untracked frames, no cross-child calibration reuse, no fabricated calibration point, orientation lock | 2 | I | todo | TRK-1, CAL-1, CAL-3; lowers validRatio on track loss (H-G1); spec `docs/spec/modules/asd-gaze-robustness/` |
| WP-G.1 | Rust tracking types, raw log format, device geometry table, unclamped raycast, visual-angle maths | 2 | S | todo | GEO-1..6, PIPE-2, REP-1; ADR-0007 |
| WP-G.2 | Rust calibration models (screen/angular affine, homography, poly2), robust target aggregation, LOO-CV quality, selection | 2 | S | todo | CAL-2..8; depends on G.1 |
| WP-G.3 | Rust per-sample pipeline: track state, confidence, 1-Euro port, gap fill, AOI video→screen mapping | 2 | S | todo | TRK, CNF, GAP, PIPE-1/3/5, GEO-7; depends on G.1, G.2, G.4 |
| WP-G.4 | Rust validation, drift monitor, re-calibration advice, session TrackingQuality summary | 2 | S | todo | VAL, DRF, CAL-9, QS-1..4; depends on G.1 |
| WP-G.5 | Rust synthetic tracking generator, replay, `asd-cli gaze synth/replay/bench-report`, scenario goldens | 2 | I | todo | TV-1/2, BEN-5; depends on G.2–G.4 |
| WP-G.6 | Contract session v1.7 (`trackingQuality`), `ScoringProfile.trackingQuality` params, schema export | 2 | S | todo | SCH-1, SCH-3, QS-5; depends on G.1 |
| WP-G.7 | asd-ffi `GazePipelineHandle` (flat per-frame records) | 2 | S | todo | ADR-0007 D2; depends on WP-R.5, G.3, G.4 |
| WP-G.8 | iOS bench capture build (`ASD_BENCH_CAPTURE`, raw tracking JSONL, 13-point grid) | 2 | I | todo | BEN-2; depends on G.1, G.0 |
| WP-G.9 | Kit integration: GazeProcessingActor runs the Rust pipeline; session v1.7 writer; per-frame cost measured | 3 | S | todo | PIPE-4, QS-1, SCH-2; depends on G.7, WP-A.11, G.0, G.6 |
| WP-G.10 | App UI: child-friendly calibration, validation step, inter-clip drift checks, re-calibration prompts | 3 | I | todo | CAL-7..9, VAL-1, DRF-1; depends on G.9 |
| WP-G.11 | Backend accepts 1.6+1.7, tracking columns, `low_tracking_quality` review rule, API fields | 2 | S | todo | SCH-2, SCH-4; depends on G.6 |
| WP-G.12 | Console Quality tab tracking section, drift chart, loss bar, tracking filter | 2 | I | todo | CSL-1..3; depends on G.11 |
| WP-G.13 | Scoring uses tracking quality (accuracy cap, AOI resolution guard, I-VT in °/s), versioned | 3 | S | backlog | SCR-1..4; owner gate H-G2; depends on G.4, G.6 |
| WP-G.14 | Bench execution, device matrix, threshold and calibration-model decision | 3 | owner + I | todo | BEN-1..8, DEV-2/3; depends on G.8, G.5; H-G4, H-G5 |
| WP-R.0 | Cargo workspace skeleton at `rust/`, six crates, domain types ported 1:1 from Swift, `deny.toml`, `scripts/rust_test.sh`, `rust.yml` | 1 | I | done | 10 round-trip tests, fmt+clippy+deny clean; serial, run first |
| WP-R.1 | `asd-core` scoring engine, baselines validate, attention flag, quality gate + golden vectors | 1 | S | done | CORE-1..4, CORE-7 |
| WP-R.2 | `asd-core` metrics extraction (I-VT, AOI dwell, affect) + session aggregator + golden vectors | 1 | S | done | CORE-5..6 |
| WP-R.3 | `asd-contract` (session v1.6, config bundle, API DTOs, schemars export, Ed25519 + JCS signing), `asd-sync` protocol logic, `asd-cli schema|bundle`; regenerate `docs/spec/schemas/asd/*.json` | 1 | S | done | CON-2..7, CON-12; GATE-1 tool |
| WP-R.4 | `asd-core::norms` (port of `tools/asd_norm_aggregator.py`, 11 tests) + `asd-core::synth` (v2.0, ratios) + `asd-cli norms|synth` | 1 | I | done | CORE-8..9; GATE-2/3 tool |
| WP-R.5 | `asd-ffi` UniFFI crate + `scripts/build_asd_xcframework.sh` (ios, ios-sim, macOS slices) | 1–2 | S | todo | FFI-1/2; depends on R.1–R.3 |
| WP-R.6 | `asd-wasm` for console sandbox re-score (feeds WP-C.5) | 2 | S | backlog | depends on R.1, R.3 |
| WP-B.1 | Rust service scaffold: Axum, sqlx migrations for `02 §3` (+ enrol_codes, refresh_tokens, bundle_adoption), blob store trait (S3/memory), tracing redaction, Dockerfile, docker-compose, health | 1 | S | done | replaces the removed FastAPI scaffold; DEP-1/2, OBS-1 |
| WP-B.2 | Auth: Argon2id login, EdDSA JWT + rotating refresh families, device tokens, enrol codes (`max_uses`, `expires_at`, `device_id` rotation), RBAC matrix, audit rows + grants | 1 | S | done | AUTH-1..5, CON-10/11; depends on B.1, R.3 |
| WP-B.3 | `POST /v1/sessions` idempotent ingest (ON CONFLICT, body hash), derived flag via `asd-core`, clip rows, blob URLs (presigned/proxy), org-scoped list/detail/clips/metrics/review, `report.pdf` passthrough | 1 | S | done | ING-1..3, CON-13..21; depends on B.1, R.1, R.3 |
| WP-B.4 | Config documents lifecycle + bundles + signing + `GET /config/bundle` (ETag, adoption) + seed drafts from kit JSON | 2 | S | done | CFG-1/2; depends on B.2 |
| WP-B.5 | Norm aggregation job (worker loop, `asd-core::norms`) + promote to draft baselines | 2 | I | done | depends on B.3, B.4, R.4 |
| WP-B.6 | Care events (`/v1/care/events`), participant summary, erasure request (port of deleted `shims.py` semantics) | 2 | I | done | 3 device routes, migration 0006, per-event + per-batch idempotency, erasure request withdraws consent (deletion in B.9); 14 tests |
| WP-B.7 | AWS deployment (ECS Fargate, RDS, S3, Secrets, ALB/ACM, WAF, logs) under `aws_infra/asd/` | 3 | S | todo | H-4, H-11 (IaC language) |
| WP-B.8 | Server-side PDF regeneration + FHIR bundle endpoint (parity with iOS output, golden tests) | 3 | I | todo | |
| WP-B.9 | Dataset export job (parquet/jsonl + labels), retention sweep, erasure execution | 3 | S | todo | |
| WP-B.10 | FHIR façade + SMART-on-FHIR server validation (only if a clinic partner needs it) | 4 | S | backlog | |
| WP-C.1 | Console scaffold: Next.js, auth, layout, sessions table/detail, overview KPIs | 1 | S | done | Live mode vs Rust backend, org switcher, overview/sessions, Playwright 11/11 mock + live; 129 unit tests |
| WP-C.2 | Review queue + devices/enrol QR | 1 | I | done | Review triage (j/k, 1–4), devices revoke, enrol code + QR. Backend gaps: no `GET /v1/orgs`, review items lack session summary |
| WP-C.3 | Participants list, users/roles, audit page (participant detail/timeline moved to WP-C.14) | 2 | I | todo | |
| WP-C.4 | Catalog editor (clips, AOI timeline, batteries) | 2 | S | todo | |
| WP-C.5 | Baselines editor + scoring profile editor + sandbox re-score + bundle publish with approval | 2 | S | todo | |
| WP-C.6 | Cohorts, norm-runs, exports, model registry | 3 | I | todo | |
| WP-C.7 | Replay heatmaps, hi/ne strings, superadmin | 4 | I | backlog | |
| WP-C.8 | Console contract alignment to `api-v1.json`: `battery` string, `capture_source` + `synthetic`, clip `rows[]`, `participant_token`; zod-vs-schema test | 1 | I | done | API-3, CON-16/19/20; depends on R.3 |
| WP-C.9 | Clinical foundation: migration 0008, envelope encryption + MRN blind index, 14 RBAC actions (superadmin excluded from clinical content), `asd-contract::clinical`, user display name/registration, encrypted review notes | 3 | S | todo | ADR-0008; RBAC/KEY/MIG-2; needs B.6 merged |
| WP-C.10 | Worklist + clinical review sign/cosign/amend (hash-bound, step-up re-auth) | 3 | S | todo | CW-1/2, SIGN-*; needs C.9 |
| WP-C.11 | Participant detail + timeline, MRN mapping, encrypted clinical notes | 3 | I | todo | CW-3, MRN-*, NOTE-1/2; needs C.9, B.6 |
| WP-C.12 | Referral/summary letters + printable clinical summary endpoints | 3 | I | todo | NOTE-3..5; needs C.10, C.11, I.5, I.6 migration |
| WP-C.13 | Console: worklist, clinical review tab, sign/cosign dialogs, clinical settings | 3 | I | todo | UI-*; needs C.2, C.10 |
| WP-C.14 | Console: participant workspace (timeline, MRN, notes, letters, print) | 3 | I | todo | needs C.11, C.12, C.13 |
| WP-I.1 | `asd-export` crate foundation: model, de-id, consent policy, verified code table, watermark; fold B.8 PDF/FHIR code in | 3 | S | todo | XC-*; needs B.8 merged |
| WP-I.2 | CSV (canonical/Excel/Sheets) + XLSX encoders | 3 | I | todo | CSV-*, XLSX-*; needs I.1 |
| WP-I.3 | FHIR R4 clinical bundle (DiagnosticReport, DocumentReference, Practitioner), iOS id parity, code verification | 3 | S | todo | FHX-*; needs I.1 |
| WP-I.4 | HL7 v2.5.1 ORU^R01 encoder (file delivery) | 3 | I | todo | HL7-*; needs I.1 |
| WP-I.5 | PDF/A-2b clinical summary + letter renderer (A4, Devanagari) | 3 | S | todo | PDF-*; needs I.1, owner Q3 (OFL fonts) |
| WP-I.6 | Export jobs: migration 0009, runner run_after/heartbeat/context, consent filter, artifacts, signed download URLs, disclosures | 3 | S | todo | EXJ-*; needs I.1, I.2, C.9 |
| WP-I.7 | Delivery: integration settings (encrypted write-only secrets), signed webhooks, SFTP, SSRF guard, auto-deliver on final sign | 3 | S | todo | INTG-*, DLV-1..6; needs I.6, C.10 |
| WP-I.8 | Console: exports page + integration settings | 3 | I | todo | needs I.6, I.7, C.13 |
| WP-I.9 | FHIR push via SMART Backend Services (migration 0010) | 4 | S | backlog | DLV-7; relates to WP-B.10 |
| WP-I.10 | SMART EHR launch of the console | 4 | S | backlog | needs I.9, C.14 |
| WP-I.11 | HL7 MLLP/TLS sender + ACK handling | 4 | S | backlog | only if a partner needs real-time HL7 |
| WP-D.1 | Stimulus clips: rights or replacement; durations measured; catalog corrected | 1 | I + owner | done | All 10 clips measured at 8.0s via ffprobe, catalog aligned, NOTICE.md updated |
| WP-D.2 | Session JSON Schema export + Swift/Python parity gate in CI | 1 | I | done (to be replaced) | Pydantic export + `scripts/check_session_schema_parity.py`; superseded by WP-R.3 + WP-D.7 (Rust export, `asd-cli schema check`) |
| WP-D.3 | Age-band model: cleared dataset, train, validate (≥0.90 recall), CoreML, manifest + licence in `models/asd_age` | 2 | S | todo | |
| WP-D.4 | Synthetic session generator shared by seed script and tests | 1 | I | done (to be replaced) | `tools/asd_synth` v1.0 emits seconds/`offTarget`/`synthetic` which iOS and console reject (CON-13); superseded by WP-R.4 (v2.0) + WP-D.7 |
| WP-D.7 | Replace Python CI gates with Rust: `ios.yml`/`ios_test.sh` use `asd-cli schema check`; delete `check_session_schema_parity.py`, `asd_norm_aggregator.py`, `asd_synth/`, `asd_research_server.py` and their tests; README updates | 1 | I | done | GATE-1..4; depends on R.3, R.4 |
| WP-D.5 | ML roadmap refresh: what an attention classifier would need (n, labels, validation plan) — doc only | 3 | A | backlog | |
| WP-D.6 | Decide/Link C++ `AsdBiomarkerPack` into kit via xcframework, or retire the claim | 4 | A | backlog | low value |
| WP-E.1 | Pitch/deck re-base to non-diagnostic reality; aspirational section | 1 | I | done | Re-based to observation diary, 6 metrics, 2027 aspirational roadmap |
| WP-E.2 | Org Apple account, bundle id, publisher domain, privacy policy + support pages live | 2 | owner + I | todo | H-6 |
| WP-E.3 | Docs drift sweep (iOS 18, Keychain not SE, names, screenshots) | 1 | I | done | Keychain, iOS 18, names, links fixed; owner follow-ups listed in commit |
| WP-E.4 | M-CHAT licence outcome → keep gate or enable | 3 | owner | todo | H-7 |
| WP-E.5 | Native hi/ne review of store text | 3 | owner | todo | |
| WP-E.6 | Security/privacy review pack: threat model, data-flow diagram, DPIA-lite, retention policy, AWS BAA | 3 | A + I | todo | |
| WP-E.7 | App Store submission dry run (`appstore_precheck`, screenshots, age assurance flag) | 4 | I | backlog | |

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
