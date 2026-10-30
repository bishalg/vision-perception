# ADR-0006: Rust is the single implementation of ASD core logic, contracts and backend

Status: Proposed, 2026-10-30 (owner decision recorded in `CLAUDE.md` the same day)
Spec ids: WS-*, CON-*, CORE-*, FFI-*, API-*, AUTH-*, ING-*, CFG-*, GATE-*, DEP-* in
`docs/spec/modules/asd-rust-core-and-backend/technical.md`

## Context

The ASD gaze product has three clients (Swift iOS app `apps/ios_asd_gaze_app` + kit
`domains/asd_screening/ios`, TypeScript console `apps/curv_console`, Kotlin Android
later) and needs a backend. Today the scoring mathematics lives only in Swift
(`PerVideoScoringEngine.swift`, `PerVideoMetrics.swift`, `AttentionFlag.swift`,
`SessionBiomarkerAggregator.swift`, `GazeQualityGate.swift`, `NormativeBaselineLibrary.swift`),
the norm aggregator and the synthetic generator are Python (`tools/asd_norm_aggregator.py`,
`tools/asd_synth/`), the wire contract is a Pydantic-exported JSON Schema
(`docs/spec/schemas/asd/session-v1.6.json`) whose generator no longer exists, and the
FastAPI backend (commit `34d44fc`) was deleted in `9655fcc`.

Three implementations of the same arithmetic (Swift on device, Python for norms, a
future server re-score) is how parity rots. The console (`src/lib/api/types.ts`) and
the iOS sync client (`SessionSyncClient.swift`, `DeviceRegistrationService.swift`) were
each written against the prose contract in `docs/roadmap_2026q4/02 §4` and disagree in
details (listed in the spec, CON-10..CON-20).

## Decisions

1. **One Cargo workspace at `rust/`** (`rust/Cargo.toml`, crates under `rust/crates/`),
   beside the CMake C++ tree and the pnpm apps; neither build system invokes the other.
   `rust/target/` is git-ignored. The C++ core (`core/`, palm, flood) is untouched.
2. **Crate graph (leaf first):** `asd-core` (domain types + all math, no I/O, no async)
   → `asd-contract` (wire DTOs, `schemars` export, Ed25519 bundle signing/verify, JCS
   canonicalisation) → `asd-sync` (device-plane protocol logic without HTTP) →
   `asd-ffi` (UniFFI, static lib for Apple/Android) and `asd-wasm` (web, Phase 2) and
   `asd-backend` (Axum binary) and `asd-cli` (dev/CI tool). Apps depend only on
   `asd-ffi`/`asd-wasm`; the backend depends on `asd-core` + `asd-contract`.
3. **Contract source of truth is Rust.** `asd-cli schema export` writes
   `docs/spec/schemas/asd/*.json`; CI fails if the committed files differ from the
   export (GATE-1). Session schema stays `1.6` with its current mixed casing (camelCase
   top level and `rawPayload`, snake_case `perVideo` rows); any field change is `1.7`.
4. **FFI boundary passes JSON documents, not mirrored structs.** Rust functions take and
   return UTF-8 JSON for documents (frames, baselines, catalog, score reports) and plain
   scalars otherwise. Swift keeps its existing `Codable` types; parity tests compare the
   JSON byte-for-byte after canonicalisation. UniFFI (MPL-2.0, Yellowlist) is accepted
   for the binding layer because it is consumed unmodified as a Cargo dependency in its
   own compilation unit, which is exactly the condition `docs/license_policy.md §2.2`
   sets; this ADR is the required architecture review. The C ABI (`cbindgen`) is the
   fallback if the owner rejects MPL in the app binary.
5. **Backend stack:** Axum + Tokio + tower-http, `sqlx` (Postgres, compile-time checked
   queries, embedded `sqlx::migrate!` migrations, `sqlx-cli` for authoring), `argon2`
   (Argon2id), `jsonwebtoken` (EdDSA access tokens, 15 min) + opaque rotating refresh
   tokens (hashed, 30 days, family revocation on reuse), `ed25519-dalek` for config
   bundles, `aws-sdk-s3` pre-signed PUTs (MinIO locally) with a proxy upload mode for
   the demo, `tracing` JSON logs with a redaction layer, `tower_governor` rate limits,
   `cargo-deny` licence gate. No ORM, no background-job framework: jobs are `tokio`
   tasks persisted in the `jobs` table and run by the same binary with `--worker`.
6. **Scoring moves to Rust in this order and nothing else moves first:** baseline model
   + `PerVideoScoringEngine` + `AttentionFlagAlgorithm` (CORE-1..4), then
   `PerVideoMetricsExtractor` + `SessionBiomarkerAggregator` (CORE-5..6), then
   `GazeQualityGate.evaluateSample` (CORE-7), then the norm aggregator and synthetic
   generator (CORE-8..9). Real-time filters (`OneEuroFilter`, `ScreenRaycaster`,
   calibration) stay in Swift for now (they are coupled to ARKit frame timing);
   `BiomarkerClassificationEngine` is Debug-only and is not ported.
7. **Swift consumes Rust through an xcframework** (`AsdCore.xcframework`: ios-arm64,
   ios-arm64-simulator, macos-arm64 so `swift test` on macOS still runs) produced by
   `scripts/build_asd_xcframework.sh`, declared as a local `binaryTarget` in
   `domains/asd_screening/ios/Package.swift`. The Swift engines become thin wrappers; the
   old Swift bodies survive for one release only inside the test target as the parity
   oracle, then are deleted.
8. **Python CI tooling is replaced, not wrapped:** `scripts/check_session_schema_parity.py`,
   `tools/asd_norm_aggregator.py`, `tools/asd_synth/`, `tools/asd_research_server.py` and
   their tests are deleted the moment the Rust equivalents pass (GATE-1..4).
9. **Deployment:** one multi-stage Docker image; `rust/docker-compose.yml` (api, worker,
   postgres:16, minio) for local and the demo (Cloudflare Tunnel in front); Phase 3 on
   AWS as ECS Fargate + RDS Postgres + S3 + Secrets Manager + ALB/ACM + WAF. The IaC
   language for `aws_infra/asd/` is an owner decision (H-11): TypeScript CDK recommended
   (pnpm exists; no new Python), extending the Python CDK app is the alternative.

## Alternatives rejected

- Root-level Cargo workspace: the root already carries `CMakeLists.txt`, `package.json`
  and `CMakePresets.json`; a root `Cargo.toml` makes `cargo` the apparent primary build
  and puts `target/` beside six `build-*` directories.
- One crate for everything: the backend would pull Tokio and sqlx into the mobile
  static library; the mobile library must stay small and `std`-only.
- Mirrored UniFFI records for every type: ~40 types duplicated in Swift and Kotlin,
  and every field change becomes a three-language edit. JSON at the boundary costs a
  few milliseconds per session and nothing in maintenance.
- `sea-orm`/`diesel`: sqlx's compile-time checked SQL and plain migrations are enough
  for ~20 tables; an ORM adds a second schema description to keep in sync.
- Keep Python for norms and synth "because they are tooling": they encode product
  maths (cell guards, direction inference, profile distributions) and would be a
  second implementation of what `asd-core` defines.
- Shim routes for the pre-WP-A.4 app build: WP-A.4 is done; nothing posts to
  `/api/v1/research/*` any more.

## Consequences

- The iOS kit gains a binary dependency and a build step; `scripts/ios_test.sh` must
  build the xcframework (or fetch a cached one) before `swift test`.
- `docs/spec/schemas/asd/session-v1.6.json` is regenerated by `schemars` and will differ
  textually from the Pydantic export; the acceptance set (iOS golden fixture + synth
  output) must validate against both before the file is replaced (CON-2).
- The console's zod enums widen slightly (`capture_source` gains `synthetic`, `battery`
  becomes the catalog protocol id, clip rows gain `rows`); listed in CON-10..20.
- `cargo-deny` needs an allow-list beyond `docs/license_policy.md`'s greenlist: ISC,
  Unicode-3.0 and the OpenSSL licence (for `aws-lc-sys`/`ring` TLS) are permissive but
  not listed; MPL-2.0 is allowed only for `uniffi*`. Owner to ratify (H-12).
- Android gets Kotlin bindings from the same UniFFI definitions for free when WP-A.12
  starts; nothing in this ADR has to change.
