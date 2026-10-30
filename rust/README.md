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

## Conventions

- Licence `Apache-2.0` on every crate; dependencies must pass `rust/deny.toml`.
- `asd-core` never depends on tokio, sqlx, reqwest, uniffi or other I/O crates.
- `serde_json` is used without `preserve_order`; contract bytes have sorted keys.
- Compare cross-language JSON through RFC 8785 (JCS), never raw bytes.
- Types never set `deny_unknown_fields` (Swift tolerates extra keys).
