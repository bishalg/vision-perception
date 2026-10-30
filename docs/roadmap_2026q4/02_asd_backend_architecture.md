# 02 — ASD Backend Architecture (replaces `tools/asd_research_server.py`)

> Scope: the server side of the ASD product — ingestion, storage, configuration delivery, admin API, jobs. The admin **UI** is in `03`. Work packages in `04` (WP-B.*, WP-R.*).
>
> **Revision 2026-10-30 (Rust-only core, owner decision in `CLAUDE.md`).** The Python stack in the original ADR-B1 was built (`34d44fc`) and removed (`9655fcc`). The binding design is now `docs/spec/adr/ADR-0006-rust-core-and-backend.md` and the module spec `docs/spec/modules/asd-rust-core-and-backend/` (testable ids `WS-*`, `CON-*`, `CORE-*`, `FFI-*`, `API-*`, `AUTH-*`, `ING-*`, `CFG-*`, `GATE-*`, `DEP-*`). Sections below were amended in place where the language changed; the data model (§3), auth planes (ADR-B4), config delivery (ADR-B6) and PHI posture (ADR-B8) are unchanged requirements. Where this file and the spec disagree, the spec wins.

## 1. Goals and non-goals

**Goals**
1. Receive sessions from the iOS app reliably (offline queue already exists on the client) with auth, idempotency and schema versioning.
2. Organise data by organisation → clinic/site → participant (pseudonymous) → session → per-clip scores, with full audit trail.
3. Serve **versioned configuration** to the app: stimulus catalog, normative baselines, scoring profile. This is the "training-level customisation": admins tune what the app measures and how it scores *without an app release*.
4. Run jobs: norm aggregation (port of `tools/asd_norm_aggregator.py`), dataset export for ML, PDF regeneration, retention/erasure.
5. Be deployable by one person, cheaply, in a week for the demo, and able to grow to a clinic pilot without a rewrite.

**Non-goals (this cycle)**
- Storing raw video (never). Storing identifiable child data (never; the app sends pseudonymous tokens; the *clinic* may hold a local mapping outside this system).
- Being a FHIR server. We export FHIR bundles; a FHIR façade is Phase 4 (WP-B.10).
- Training models server-side. Jobs *export datasets*; training stays in `tools/`.

## 2. Decisions (ADR-style)

### ADR-B1 (revised 2026-10-30): Language and framework — **Rust: Axum + Tokio + sqlx (Postgres) in the `rust/` Cargo workspace**
- Superseded text: "Python 3.12 + FastAPI + SQLAlchemy 2 + Alembic + Pydantic v2" (implemented in `34d44fc`, removed in `9655fcc`).
- Why: one implementation of scoring, contracts and server logic shared with the iOS kit (UniFFI), the console (WASM later) and Android; Rust is the owner-mandated core language (`CLAUDE.md`). Full reasoning and rejected alternatives: ADR-0006.
- Stack: `axum`, `tokio`, `tower-http`, `sqlx` (compile-time checked SQL, embedded migrations, `sqlx-cli` to author them), `argon2` (Argon2id), `jsonwebtoken` (EdDSA), `ed25519-dalek`, `serde_jcs` (canonical JSON for signatures), `aws-sdk-s3` (pre-signed PUTs; MinIO locally), `schemars` (JSON Schema export), `tracing` + JSON subscriber with a redaction layer, `tower_governor` (rate limits), `cargo-deny` (licence gate per `docs/license_policy.md`; allow-list additions pending H-12).
- Layout: `rust/crates/asd-core` (maths, no I/O) → `asd-contract` (DTOs, schema export, signing) → `asd-sync` (device protocol logic) → `asd-ffi` (UniFFI static lib), `asd-backend` (this service), `asd-cli` (dev/CI tool replacing the Python scripts). The C++ `core/` tree and pnpm apps are untouched; `rust/target/` is git-ignored.
- `tools/asd_research_server.py` and `tests/test_asd_research_server.py` are deleted in WP-D.7 (no shim routes; WP-A.4 already moved the app to `/v1`).

### ADR-B2: Database — **PostgreSQL 16** (SQLite only in unit tests via the same SQLAlchemy models)
- JSONB for session summaries and per-clip rows; relational tables for everything we filter or join on.
- `sqlx` migrations (`rust/crates/asd-backend/migrations/*.sql`) from day 1, run explicitly by `asd-backend migrate`; unit tests use the same Postgres (per-test schema via `sqlx::test`), not SQLite.

### ADR-B3: Blob storage — **S3-compatible** (MinIO in docker-compose, AWS S3 in Phase 3)
- The encrypted frame-level JSONL (`.jsonl.enc`) is uploaded as an opaque blob and stays encrypted with the **device** key. The server never needs to decrypt it for v1. Only the *summary* (what `GazeSessionRecord` v1.6 already contains) is stored in Postgres in clear. If a research partner needs frames, a key-escrow design is a separate ADR (not now).
- Bucket layout: `sessions/{org_id}/{participant_token}/{session_id}/frames.jsonl.enc`, `reports/.../report.pdf`, `exports/{job_id}/dataset.parquet`.

### ADR-B4: Auth — two planes
- **Device plane (app → API):** on first run the app calls `POST /v1/devices/register` with an Apple **App Attest** assertion (fallback: a one-time enrolment code issued by the clinic console for Phase 1 demo). Server returns a long-lived **device token** (opaque, hashed at rest) bound to `org_id`. All session uploads use `Authorization: Bearer <device token>`. Parent-mode devices register under a default "Public/Home" org.
- **User plane (console → API):** Phase 1: self-hosted email+password (Argon2id), short-lived JWT access + rotating refresh cookie, TOTP optional. Phase 3 (AWS move): swap to **Cognito** user pool (BAA-eligible) behind the same `AuthProvider` interface. H-5.
- **RBAC:** roles `owner`, `admin`, `clinician`, `researcher`, `reviewer`, `viewer` scoped to an org; `superadmin` global. Enforced by an Axum extractor `Require<Role>` driven by one matrix table (AUTH-4); another org's resource answers 404. Every mutating endpoint writes an audit row in the same transaction (AUTH-5).

### ADR-B5: Idempotency and versioning
- Client sends `X-Idempotency-Key` (= `sessionId` for sessions, UUID for others) and `X-Schema-Version` (= `GazeSessionRecord.schemaVersion`, currently `1.6`). Server stores key → response hash in `idempotency_keys` with 7-day TTL; replays return the original response. Use `INSERT … ON CONFLICT DO NOTHING` then read, never get-then-write (lesson from the flood Lambda).
- Payload schemas are Rust types in `asd-contract` exported with `schemars` by `asd-cli schema export` into `docs/spec/schemas/asd/` (`session-v1.6.json`, `config-bundle-v1.json`, `api-v1.json`); `asd-cli schema check` fails CI on drift (GATE-1). Swift and console fixtures validate against the same files.
- Replay semantics (CON-17): same key + same body hash → `200` with the original body and `duplicate: true`; same key + different body → `409 idempotency-key-reused`. Server never defaults a missing measurement (CON-14).

### ADR-B6: Configuration delivery (the "training-level customisation" channel)
- Three config documents, each versioned and signed: `stimulus_catalog`, `normative_baselines`, `scoring_profile`. Status lifecycle: `draft → candidate → published → retired`. Only `published` is served.
- `GET /v1/config/bundle?age_pack=…&lang=…&app_version=…` returns `{catalog, baselines, scoring_profile, bundle_version, signature}`; the app caches it and falls back to the bundled JSON when offline or on signature failure. Every session records the `bundle_version` that scored it, so results are reproducible.
- Signature: Ed25519, private key only on the server (KMS in Phase 3), public key embedded in the app. Prevents a MITM or a rogue admin build from changing scoring silently.
- Clip media files are **not** served by this API in Phase 1 (ODR stays). Phase 2 adds `StimulusCDNBaseURL` support (already an Info.plist hook) with signed URLs to S3/CloudFront.

### ADR-B7: Hosting — **Phase 1–2: single Docker Compose host; Phase 3: AWS (App Runner or ECS Fargate + RDS + S3 via CDK in `aws_infra/`)**
- Demo must be reachable at an https DNS name because the iOS client rejects IP/loopback URLs. Use **Cloudflare Tunnel** to a laptop/VPS for the demo (`cloudflared tunnel --url http://localhost:8000`), or Fly.io/Railway for a persistent demo.
- Reuse from `aws_infra/`: S3 bucket policy (SSE, block public, versioned), least-privilege IAM, SNS for alerts. Add: VPC, RDS Postgres (encrypted, no public IP), ECS Fargate service (api + worker from one image), Secrets Manager, CloudWatch logs with PHI-safe structured logging (`tracing` JSON, redaction layer), WAF rate limiting. IaC language for `aws_infra/asd/` is owner decision H-11 (TypeScript CDK recommended; no new Python).
- Local/demo: `rust/docker-compose.yml` (api, worker, postgres:16, minio, seed). iOS rejects IP/`localhost` hosts, so the demo runs `BLOB_MODE=proxy` (uploads via `/v1/sessions/{id}/blobs/*` under the device bearer) behind the tunnel; AWS uses `BLOB_MODE=presigned`.
- H-4.

### ADR-B8: PHI posture
- We treat session summaries as **sensitive** (child-related health observations) even though pseudonymous. Encryption in transit (TLS 1.2+), at rest (RDS/S3 encryption + column-level `pgcrypto` for free-text notes), access logging, data minimisation (no names, DOB → age bracket only), erasure endpoint, retention policy table per org (default 24 months), export-on-request.
- No BAA/HIPAA claims in marketing until an AWS BAA and a security review exist (WP-E.6).

## 3. Data model (Postgres)

```
organizations(id, name, kind[home|clinic|research], created_at, retention_months, settings jsonb)
users(id, org_id, email, pw_hash, totp_secret?, role, status, last_login_at)
devices(id, org_id, platform, app_version, attest_key_id?, token_hash, label, enrolled_by, enrolled_at, revoked_at?)
participants(id, org_id, token /* child-<id> from app */, age_bracket_at_enrol, language, consent_version, consent_at, mode[parent|clinic], withdrawn_at?)
sessions(id /* sessionId from app */, org_id, participant_id, device_id, capture_source[truedepth|simulated], started_at, ended_at, duration_sec,
         battery[express|comprehensive], age_pack, lang, bundle_version, schema_version,
         quality_verdict, valid_frame_ratio, calibrated_distance_cm,
         social_preference, eye_dwell, mouth_dwell, first_eye_latency_ms, affect jsonb,
         composite_score, composite_confidence, attention_flag[green|yellow|red|unreliable],
         summary jsonb /* full GazeSessionRecord */, frames_blob_key?, report_pdf_key?, received_at, idempotency_key)
session_clip_scores(session_id, clip_id, order, dwell_social, dwell_geometric, dwell_eye, dwell_mouth, orienting_latency_ms, gaze_cue_accuracy,
         fixation_count, mean_fixation_ms, status[onTarget|monitor|atypical], score, weight, baseline_ref, details jsonb)
care_events(id, org_id, participant_id, kind[task|medication|diary|milestone], occurred_at, payload jsonb, idempotency_key)   -- port of carekit.db
config_documents(id, kind[stimulus_catalog|normative_baselines|scoring_profile], version, status, body jsonb, signature, created_by, created_at, published_at?, notes)
config_bundles(id, catalog_id, baselines_id, scoring_id, bundle_version, signature, published_at)
stimulus_assets(id, clip_id, catalog_version, file_key, duration_sec_measured, checksum, licence, provenance, uploaded_by)
review_items(id, org_id, session_id, reason[low_quality|flag_red|random_audit|norm_candidate], status[open|in_review|done|dismissed], assigned_to?, decision jsonb, decided_by?, decided_at?)
labels(id, session_id, clip_id?, key, value jsonb, created_by, created_at)              -- human annotations for ML
norm_runs(id, baselines_source_id, params jsonb, cells jsonb, n_total, created_by, created_at, promoted_to_config_id?)
jobs(id, kind, status, params jsonb, result jsonb, started_at, finished_at, error?)
erasure_requests(id, org_id, participant_id, requested_by, requested_at, completed_at?, scope)
audit_log(id, ts, actor_type[user|device|system], actor_id, org_id, action, target_type, target_id, before jsonb?, after jsonb?, ip, user_agent)   -- append-only, no UPDATE/DELETE grants
idempotency_keys(key, scope, response_hash, status_code, created_at)  -- TTL 7 days
```

Indexes: `sessions(org_id, started_at desc)`, `sessions(participant_id, started_at)`, `review_items(org_id, status)`, `audit_log(org_id, ts)`, GIN on `sessions.summary`.

## 4. API (v1)

Base `/v1`. JSON. Errors RFC 7807 problem+json. All list endpoints paginate with `cursor`.

**Device plane**
- `POST /devices/register` {attest | enrol_code, platform, app_version} → {device_token, org_id, config_bundle_version}
- `GET  /config/bundle` → signed bundle (ADR-B6); `ETag`/`If-None-Match`
- `POST /sessions` (idempotent) body = `GazeSessionRecord` v1.6 + `per_video` rows + optional `frames_upload: true` → {session_id, frames_upload_url?, report_upload_url?} (pre-signed PUTs)
- `POST /care/events` (idempotent, batch) — port of `/carekit/sync` and `/batch/sync`
- `GET  /participants/{token}/summary` — port of `/carekit/summary`
- `POST /participants/{token}/erasure` — right to erasure from the device (consent withdrawal)

**User plane (console)**
- `POST /auth/login`, `/auth/refresh`, `/auth/logout`, `/auth/totp/*`
- `GET  /orgs/{id}`, `PATCH /orgs/{id}` (retention, settings)
- `GET/POST/PATCH /orgs/{id}/users`, `/orgs/{id}/devices` (+ `POST …/devices/enrol-code`)
- `GET  /orgs/{id}/participants`, `GET /participants/{id}`
- `GET  /orgs/{id}/sessions?flag=&quality=&from=&to=&bundle_version=`, `GET /sessions/{id}`, `GET /sessions/{id}/clips`, `GET /sessions/{id}/report.pdf` (regenerated server-side, WP-B.8), `GET /sessions/{id}/fhir` (bundle)
- `GET/POST /orgs/{id}/review`, `PATCH /review/{id}` (decision, labels)
- `GET/POST /config/{kind}`, `GET /config/{kind}/{version}`, `POST /config/{kind}/{version}/publish`, `POST /config/bundles` (compose + sign), `GET /config/bundles`
- `POST /jobs/norm-aggregate` {min_n=12, include_imputed=false, source_filter} → norm_run; `POST /norm-runs/{id}/promote` → draft baselines doc
- `POST /jobs/export-dataset` {filters, format=parquet|jsonl, include_labels} → job → signed download URL
- `GET  /orgs/{id}/audit?…`
- `GET  /orgs/{id}/metrics` (counts by flag/quality/week, bundle adoption, device app versions)
- `GET  /health`, `GET /health/ready`, `GET /metrics` (Prometheus)

No compatibility shims: WP-A.4 landed first, so nothing posts to `/api/v1/research/*` or `/api/v1/carekit/*` any more (CON-23).

**Contract reconciliation (2026-10-30).** The console (`apps/curv_console/src/lib/api/types.ts`) and the iOS client (`SessionSyncClient.swift`, `DeviceRegistrationService.swift`) were written against the prose above and diverged in 14 places; each is decided in the spec as `CON-10..CON-23`. Headlines: register response `{status, device_id, device_token, org_id, config_bundle_version}`; enrol codes get `max_uses`/`expires_at` and re-registration sends `device_id`; `battery` is the catalog protocol id (`express_30s`); `attentionFlag: null` is computed server-side with `asd-core` or stored as `unreliable`; `GET /sessions/{id}/clips` items carry `rows[]` of per-metric expected/recorded/deviation; `participant_id` is the UUID and `participant_token` is also returned; `captureSource` gains `synthetic` (dev only); `simulated` is always rejected. Also added: `PUT /sessions/{id}/blobs/{frames|report}` (proxy upload mode), `POST /devices/{id}/revoke`.

## 5. Jobs

Run by the same binary started as `asd-backend worker` (Tokio tasks polling the `jobs` table with `FOR UPDATE SKIP LOCKED`; Phase 3 may move to SQS, but a worker container is enough): norm aggregation (`asd-core::norms`), dataset export, PDF regeneration, retention sweep (nightly: delete frames blobs beyond retention, keep aggregate counts), erasure execution, config-bundle signing, metrics rollups.

## 6. Observability and ops

- `tracing` JSON logs with a redaction layer (drops `participant_token`, `email`, `notes`, `rawPayload`, e-mail patterns). No PHI in logs is a test (`asd-backend/tests/log_redaction.rs`, OBS-1).
- Sentry (self-hosted or SaaS with PII scrubbing) for the API; **not** for the iOS app until a privacy review (app currently declares "Data Not Collected").
- Prometheus `/metrics`; Grafana dashboard JSON committed under `rust/ops/`.
- `rust/docker-compose.yml` (api, worker, postgres, minio, seed); gates via `scripts/rust_test.sh` (no `Makefile`: the root `.gitignore` ignores them outside `services/**`).
- Seed uses `asd-cli synth` (deterministic, **synthetic** sessions only, `captureSource: "synthetic"`, accepted only with `ASD_ALLOW_SYNTHETIC=true`).

## 7. Security checklist for v1

- TLS everywhere; HSTS; CORS restricted to the console origin (not `*`).
- Device tokens hashed (SHA-256) at rest, revocable, per-org.
- Rate limits: `POST /sessions` 60/min/device; auth 10/min/IP → `429` + `Retry-After` (the client already honours backoff).
- Pre-signed PUT URLs expire in 15 min; max object size 50 MB.
- Config bundles signed (Ed25519); app verifies.
- Audit log append-only (DB role without UPDATE/DELETE).
- Secrets via env/Secrets Manager; `.env.example` committed, `.env` ignored (already in `.gitignore`).
- Dependency and licence scanning in CI (`cargo deny check`: advisories + licence allow-list), container image scanning.

## 8. Testing strategy

- Unit (`cargo test`): contract round-trips, scoring golden vectors shared with the Swift kit (FFI-4), signature, RBAC matrix, redaction.
- Integration (`sqlx::test` against the CI Postgres service, in-memory blob store): ingest → list → review → erasure; idempotent replay incl. 20 concurrent posts; config publish → bundle fetch → signature verify.
- Contract: `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json` and `asd-cli synth` output validate against the exported schemas; console zod schemas are checked against `api-v1.json` (WP-C.8).
- Load smoke: 200 concurrent uploads (`oha` or `k6`) once before the demo.
- Coverage gate 85 % for `asd-backend` and `asd-core` (`cargo llvm-cov`).

## 9. Migration from `tools/asd_research_server.py`

1. WP-R.0 lays the workspace and domain types; WP-B.1 scaffolds the Rust service (no shims).
2. WP-B.6 ports the care-event semantics of the deleted `shims.py` into `care_events` (owner may defer, H-15).
3. WP-R.4 ports `tools/asd_norm_aggregator.py` into `asd-core::norms` (n≥12 cell guard, imputed exclusion, zero-spread withholding) with the 11 Python test cases; `asd-cli norms aggregate` is the offline CLI; WP-B.5 wraps it as a job.
4. WP-D.7 deletes `tools/asd_research_server.py`, `tests/test_asd_research_server.py`, `tools/asd_synth/`, `tools/asd_norm_aggregator.py`, `scripts/check_session_schema_parity.py` once GATE-1..3 pass; updates `apps/ios_asd_gaze_app/README.md`, `domains/asd_screening/README.md`, `docs/roadmap_2026q4/README.md` step 4.
