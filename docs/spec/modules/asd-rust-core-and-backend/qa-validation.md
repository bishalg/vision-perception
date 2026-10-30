# ASD Rust Core and Backend - QA Validation

## Layers

1. Rust unit and golden tests (`cargo test --workspace`), pure, no network.
2. Rust integration tests for the backend (`rust/crates/asd-backend/tests/`), each test
   creates its own schema in the Postgres given by `DATABASE_URL` (`sqlx::test` with
   migrations), MinIO replaced by an in-memory blob store behind the `BlobStore` trait.
3. Cross-language parity: Swift `GoldenParityTests` and console contract tests read the
   same committed JSON files as the Rust tests.
4. Static gates: `cargo fmt`, `clippy -D warnings`, `cargo deny`, `sqlx prepare --check`,
   `asd-cli schema check`.

## Commands (build outside the repository where a tool allows it)

```bash
cd ~/Documents/WORK/Code/vision-perception
scripts/rust_test.sh                                   # all Rust gates (WS-5)
CARGO_TARGET_DIR=/private/tmp/asd-rust cargo test --manifest-path rust/Cargo.toml -p asd-core
DATABASE_URL=postgres://asd:asd@localhost:5432/asd_test cargo test --manifest-path rust/Cargo.toml -p asd-backend
cargo run --manifest-path rust/Cargo.toml -p asd-cli -- schema check
scripts/build_asd_xcframework.sh                       # FFI-2, writes build/asd/
scripts/ios_test.sh                                    # kit + app + parity + static gates
pnpm -C apps/curv_console test                         # includes api-v1.json contract test (WP-C.8)
docker compose -f rust/docker-compose.yml up -d && curl -fsS localhost:8000/health/ready
```

## MUST → proof

| Spec id | Test or command |
| --- | --- |
| WS-1..3, GATE-5 | `scripts/rust_test.sh`; `rust/deny.toml` reviewed in WP-R.0 |
| WS-4 | `git check-ignore rust/target build/asd/AsdCore.xcframework` |
| CON-1 | `asd-core/tests/types_roundtrip.rs`: decode `normative_baselines.json`, `stimulus_catalog.json`, kit golden fixtures; re-encode; JCS-equal |
| CON-2, CON-12, CON-14 | `asd-contract/tests/session_v1_6.rs`: encode/decode `expected-session-v1.6.json` byte-equal after JCS; missing required field rejected |
| CON-3, GATE-1 | `asd-cli schema check` in `rust.yml` and `ios.yml` |
| CON-4 | `asd-cli schema validate` on the two fixtures (acceptance 3) |
| CON-5 | `asd-contract/tests/config_defaults.rs`: default `ScoringProfile` equals Swift constants |
| CON-6 | `asd-contract/tests/signing.rs`: sign/verify, tamper, key rotation, JCS vectors from RFC 8785 §3 |
| CON-7 | `asd-sync/tests/protocol.rs`: ports of `SessionSyncClientTests` cases (headers, 401 → reauth, 409 → duplicate, Retry-After cap, URL rules) |
| CON-10, CON-11 | `asd-backend/tests/devices.rs` |
| CON-13, CON-15, CON-17, CON-21, ING-1..3 | `asd-backend/tests/sessions.rs` (incl. 20-task concurrency test) |
| CON-16, CON-19, CON-20, API-3 | `asd-backend/tests/console_read.rs` + `apps/curv_console/src/lib/api/contract.test.ts` |
| CON-18 | `asd-backend/tests/blobs.rs` (both modes) |
| CORE-1..7, FFI-4 | `asd-core/tests/golden_*.rs` and kit `GoldenParityTests.swift` |
| CORE-8, GATE-3 | `asd-core/tests/norms.rs` (11 ported cases + percentile vectors) |
| CORE-9, GATE-2 | `asd-core/tests/synth.rs` |
| FFI-1 | `asd-ffi/tests/boundary.rs`: malformed JSON returns `AsdError`, never panics |
| FFI-2, FFI-3 | `scripts/ios_test.sh` (builds xcframework, runs kit tests on macOS slice) |
| AUTH-1..5 | `asd-backend/tests/auth.rs`, `rbac_matrix.rs` (7 roles × key routes table-driven), `audit.rs` |
| CFG-1, CFG-2 | `asd-backend/tests/config.rs` |
| OBS-1 | `asd-backend/tests/log_redaction.rs` |
| DEP-1, DEP-2 | `docker compose up` + `curl /health/ready` in `rust.yml` (smoke job) |
