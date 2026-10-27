# 04 — ASD Phased Work Packages (2026-10-27 → 2027-02-28)

> Status table is the live tracker. Update it when a WP changes state. Phases overlap on purpose: app, backend and console streams run in parallel when they share no files.
> Agent tiers: **A** = `architect` (Fable, design only), **S** = `senior-implementer` (Opus), **I** = `implementer` (Sonnet). Main session integrates, runs gates, commits.

## 0. Status table

| WP | Title | Phase | Tier | Status | Notes |
|---|---|---|---|---|---|
| WP-A.1 | Device checklist + demo-tools screen | 1 | I | done | DemoToolsView, DemoRelease config, build tests, symbols gate |
| WP-A.2 | Fix known bugs (double CareKit write, delete-all semantics, classifier double-run, elevatedRisk string) | 1 | S | done | RiskStratification scoped to ASD_RESEARCH_CLASSIFIER, duplicate writes eliminated, classifier passed to summary |
| WP-A.3 | Protected file-backed history store (old WP-4.3) | 1 | S | done | Protected file store (.completeFileProtectionUntilFirstUserAuthentication), defaults migration, dedupe preserved |
| WP-A.4 | Sync client: device registration, bearer token, idempotency + schema headers, new `/v1/sessions` payload, pre-signed uploads | 1 | S | todo | |
| WP-A.5 | `ScoringProfile` + remote config bundle consumption with Ed25519 verify and bundled fallback | 2 | S | todo | |
| WP-A.6 | Swift 6 language mode (old WP-4.2) | 2 | S | todo | |
| WP-A.7 | `Biomarker*`→`Engagement*` renames + claims-gate scanner covers model strings (old WP-4.4) | 2 | I | todo | |
| WP-A.8 | Stimulus CDN support with signed URLs; ODR fallback | 3 | S | todo | |
| WP-A.9 | Replay down-sample upload (for console heatmaps) | 3 | S | todo | |
| WP-A.10 | TestFlight beta pipeline (fastlane `beta`), crash reporting decision | 3 | I | todo | |
| WP-B.1 | Service scaffold: FastAPI, Postgres, Alembic, docker-compose, shim routes, health, CI | 1 | S | done | FastAPI + SQLAlchemy 2 + Alembic scaffold, models, shims, tests, CI |
| WP-B.2 | Care events port + tests migrated from `tools/asd_research_server.py` | 1 | I | done | Care events port with idempotency, summary metrics, 12 backend tests pass |
| WP-B.3 | Auth: device tokens + enrol codes; user login/JWT/refresh; RBAC dependency; audit log | 1 | S | todo | |
| WP-B.4 | `POST /v1/sessions` idempotent ingest, clip rows, pre-signed blob URLs, list/detail endpoints, review auto-rules | 1 | S | todo | |
| WP-B.5 | Config documents + bundles + Ed25519 signing + `GET /config/bundle` | 2 | S | todo | |
| WP-B.6 | Norm aggregation job (port of `tools/asd_norm_aggregator.py`) + promote to draft baselines | 2 | I | todo | |
| WP-B.7 | AWS deployment via CDK (App Runner/ECS, RDS, S3, Secrets, WAF, logs) | 3 | S | todo | H-4 |
| WP-B.8 | Server-side PDF regeneration + FHIR bundle endpoint (parity with iOS output, golden tests) | 3 | I | todo | |
| WP-B.9 | Dataset export job (parquet/jsonl + labels), retention sweep, erasure execution | 3 | S | todo | |
| WP-B.10 | FHIR façade + SMART-on-FHIR server validation (only if a clinic partner needs it) | 4 | S | backlog | |
| WP-C.1 | Console scaffold: Next.js, auth, layout, sessions table/detail, overview KPIs | 1 | S | todo | |
| WP-C.2 | Review queue + devices/enrol QR | 1 | I | todo | |
| WP-C.3 | Participants timeline, users/roles, audit page | 2 | I | todo | |
| WP-C.4 | Catalog editor (clips, AOI timeline, batteries) | 2 | S | todo | |
| WP-C.5 | Baselines editor + scoring profile editor + sandbox re-score + bundle publish with approval | 2 | S | todo | |
| WP-C.6 | Cohorts, norm-runs, exports, model registry | 3 | I | todo | |
| WP-C.7 | Replay heatmaps, hi/ne strings, superadmin | 4 | I | backlog | |
| WP-D.1 | Stimulus clips: rights or replacement; durations measured; catalog corrected | 1 | I + owner | done | All 10 clips measured at 8.0s via ffprobe, catalog aligned, NOTICE.md updated |
| WP-D.2 | Session JSON Schema export + Swift/Python parity gate in CI | 1 | I | done | session-v1.6.json exported from Pydantic, parity test in scripts/check_session_schema_parity.py, gated in CI |
| WP-D.3 | Age-band model: cleared dataset, train, validate (≥0.90 recall), CoreML, manifest + licence in `models/asd_age` | 2 | S | todo | |
| WP-D.4 | Synthetic session generator shared by seed script and tests | 1 | I | todo | |
| WP-D.5 | ML roadmap refresh: what an attention classifier would need (n, labels, validation plan) — doc only | 3 | A | backlog | |
| WP-D.6 | Decide/Link C++ `AsdBiomarkerPack` into kit via xcframework, or retire the claim | 4 | A | backlog | low value |
| WP-E.1 | Pitch/deck re-base to non-diagnostic reality; aspirational section | 1 | I | done | Re-based to observation diary, 6 metrics, 2027 aspirational roadmap |
| WP-E.2 | Org Apple account, bundle id, publisher domain, privacy policy + support pages live | 2 | owner + I | todo | H-6 |
| WP-E.3 | Docs drift sweep (iOS 18, Keychain not SE, names, screenshots) | 1 | I | todo | |
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

### WP-B.1 Service scaffold (S)
- Create `services/asd_backend/` per `02 §2 ADR-B1`; `docker-compose.yml`, `Makefile`, Alembic initial migration (tables in `02 §3`), `/health`, structlog + redaction, pytest + testcontainers, GitHub workflow `backend.yml` (lint ruff, mypy, pytest, pip-audit).
- Shim routes: `/api/v1/research/ingest|records|study-info`, `/api/v1/carekit/sync`, `/api/v1/batch/sync`, `/api/v1/carekit/summary/{id}` → mapped to new tables. `study-info` returns values from env, empty by default.
- Accept: `make up && make test` green locally and in CI; current app build can POST through the shim via a Cloudflare tunnel.

### WP-B.2 Care events port (I) — port `CareKitDatabase` semantics and all 10 server tests + 11 aggregator tests (aggregator moves in WP-B.6; for now keep CLI). Delete `tools/asd_research_server.py` at the end of Phase 1 once WP-A.4 and WP-B.4 are merged.

### WP-B.3 Auth + RBAC + audit (S) — per `02 §2 ADR-B4`. Accept: tests for role matrix (6 roles × key endpoints), token revocation, audit rows on every mutation, rate limiting 429.

### WP-B.4 Sessions ingest + read (S) — per `02 §4`. Review auto-rules (`quality_low`, `flag_red`, `random_audit`, `norm_candidate`). Accept: idempotent replay returns identical body; concurrent duplicate inserts produce one row (test with 20 threads); list filters; detail returns clip rows.

### WP-C.1 Console scaffold (S) — `apps/curv_console/`; login, org switcher, overview, sessions table/detail (Clips, Quality, Report tabs), reads from WP-B.4. Playwright smoke. Accept: demo storyline step 4 can be performed.

### WP-C.2 Review queue + devices (I).

### WP-D.1 Stimulus clips (I + owner) — measure real durations (`ffprobe`), fix `stimulus_catalog.json`, document licence per clip in `NOTICE.md`; if H-8 = replace, generate/commission clips and update AOIs. Accept: catalog durations == file durations (test), NOTICE has a licence line per clip.

### WP-D.2 Schema + parity gate (I) — Pydantic → JSON Schema in `docs/spec/schemas/asd/session-v1.6.json`; Swift test encodes a fixture and validates against it (via a tiny JSON-schema validator or by running `python -m jsonschema` in `scripts/ios_test.sh`). Accept: gate in `ios.yml` and `backend.yml`.

### WP-D.4 Synthetic generator (I) — one Python module used by backend seed + a Swift port already exists (`SummaryFixture`); keep parameters identical and documented.

### WP-E.1 Pitch re-base (I); WP-E.3 docs drift sweep (I).

## 2. Phase 2 — Configuration & customisation (2026-11-17 → 2026-12-14)

Goal: an admin can change what the app shows and how it scores, safely, without an app release.

- **WP-B.5** config documents, bundles, signing, `GET /config/bundle`, ETag; adoption telemetry (which device pulled which version).
- **WP-A.5** kit refactor: `StimulusCatalog`, `NormativeBaselineLibrary`, `PerVideoScoringEngine`, `AttentionFlag`, `GazeQualityGate` take parameters from a decoded `ConfigBundle`; Ed25519 verify (CryptoKit `Curve25519.Signing`); cache in Application Support; fallback to bundled JSON; session records `bundleVersion`. Accept: all kit tests pass with both bundled and fetched config; tampered signature → fallback + log.
- **WP-C.4** catalog editor, **WP-C.5** baselines/scoring editors + sandbox + publish-with-approval, **WP-C.3** participants/users/audit.
- **WP-B.6** norm aggregation job + promote.
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

Exit: a clinic (or friendly pilot site) can be onboarded end to end: org created, devices enrolled, sessions flowing, reviewers working the queue, researcher exports a labelled dataset.

## 4. Phase 4 — Pilot & store (2027-01-19 → 2027-02-28)

- **WP-E.7** App Store submission dry run; **WP-C.7** console polish; **WP-B.10** FHIR façade only if a partner needs it; **WP-D.5/D.6** ML roadmap doc and C++ decision.
- First own-norm candidate baselines from pilot data (if n≥12 per cell reached — unlikely this early; the point is the pipeline works).

## 5. Gates every WP must pass

- iOS: `scripts/ios_test.sh` (kit tests, TSan, strict concurrency, app tests, Release build, claims, privacy API, release symbols).
- Backend: `make test` (ruff, mypy, pytest with coverage ≥85 %, pip-audit).
- Console: `pnpm -C apps/curv_console lint test e2e`.
- Cross: schema parity gate (WP-D.2).
- No new tracked binaries > 1 MB; no model weights without manifest + licence; no personal data.

## 6. Suggested parallelisation for the first two weeks

| Week | Stream A (iOS) | Stream B (backend) | Stream C (console) | Stream D/E |
|---|---|---|---|---|
| 2026-10-27 | WP-A.1, WP-A.2 | WP-B.1 | — | U1–U5, WP-E.1, WP-D.1 |
| 2026-11-03 | WP-A.3, WP-A.4 | WP-B.2, WP-B.3, WP-B.4 | WP-C.1 | WP-D.2, WP-D.4, WP-E.3 |
| 2026-11-10 | demo hardening | tunnel/deploy, seed | WP-C.2 | rehearsal |

Each stream is one agent; streams share no files except `project.yml` (A only) and root `package.json` (C only).
