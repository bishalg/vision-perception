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

## Apple build (asd-ffi, FFI-2)

```bash
scripts/build_asd_xcframework.sh          # release; --debug for a faster dev build
```

Builds `asd-ffi` as a static library for `aarch64-apple-ios`, `aarch64-apple-ios-sim` and
`aarch64-apple-darwin` (adds the rustup targets if missing), generates the Swift bindings
with the crate's own `uniffi-bindgen` binary (`--features cli`, library mode) and writes,
git-ignored:

- `build/asd/AsdCore.xcframework`: `libasd_ffi.a` per slice, headers under
  `Headers/asd_coreFFI/` (C module `asd_coreFFI`);
- `build/asd/swift/asd_core.swift`: the Swift API (`coreVersion()`, `scoreSessionJson(...)`,
  `verifyBundle(...)`, `syncBuildSessionRequestJson(...)`, ...).

SwiftPM consumes it as `binaryTarget(name: "AsdCoreFFI", path: ...)` plus a wrapper target
that compiles `asd_core.swift` and depends on `AsdCoreFFI`. Slices are arm64 only: build
for a concrete simulator (or `ARCHS=arm64`), not `generic/platform=iOS Simulator`, which
also links x86_64. Documents cross the boundary as JSON in the Swift `Codable` format
(ADR-0006 §4); every fallible call throws `AsdError` (`InvalidJson`, `InvalidArgument`,
`NotRegistered`, `UploadRefused`, `InvalidUrl`). Bindings settings: `crates/asd-ffi/uniffi.toml`.

Android (WP-A.12, not wired yet): `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o <jniLibs>
build -p asd-ffi --release`, then `uniffi-bindgen generate --library <lib> --language kotlin`.

## First run (fresh database)

Nothing in the API can create the first org or user, so a new deployment is bootstrapped
with `asd-backend admin` (only `DATABASE_URL` is needed). Each command prints just the new
id on stdout and writes one `audit_log` row (actor `system`, `via: cli`). The password is
read from `ASD_ADMIN_PASSWORD` or the first line of stdin, never from an argument.

```bash
# 1. Schema (compose runs this as the `migrate` service).
asd-backend migrate

# 2. The organisation (kind: home | clinic | research; --retention-months defaults to 24).
ORG=$(asd-backend admin create-org --name "Example Clinic" --kind clinic)

# 3. Its first owner (password on stdin; or set ASD_ADMIN_PASSWORD).
read -rs PW && printf '%s\n' "$PW" | \
  asd-backend admin create-user --email owner@example.org --role owner --org "$ORG"
# Optional global operator (no org); a second one needs --force.
#   asd-backend admin create-user --email root@example.org --role superadmin

# 4. Sign in as the owner (the API needs ASD_JWT_SIGNING_KEY).
curl -s localhost:8000/v1/auth/login -H 'content-type: application/json' \
  -d '{"email":"owner@example.org","password":"..."}'          # -> access_token

# 5. First device enrol code (owner, admin or clinician); the code is shown once.
curl -s "localhost:8000/v1/orgs/$ORG/devices/enrol-code" -H "authorization: Bearer $ACCESS" \
  -H 'content-type: application/json' -d '{"max_uses": 1, "expires_in_days": 7}'
```

With compose, run the admin commands in a one-off container, e.g.
`docker compose run --rm -T api admin create-org --name "Example Clinic" --kind clinic`
(`-T` so stdin can carry the password). Further users and roles are created by the owner
through `POST /v1/orgs/{id}/users`.

Rules: an existing e-mail (case-insensitive) is refused and nothing changes, even with
`--force`; a second superadmin or a second org of the same name needs `--force`; an owner
needs an existing `--org`, a superadmin takes none; passwords follow the API policy (12+
characters). Exit codes: `0` ok, `2` usage or invalid input, `3` conflicts with existing
data, `1` other errors (e.g. database).

## Conventions

- Licence `Apache-2.0` on every crate; dependencies must pass `rust/deny.toml`.
- `asd-core` never depends on tokio, sqlx, reqwest, uniffi or other I/O crates.
- `serde_json` is used without `preserve_order`; contract bytes have sorted keys.
- Compare cross-language JSON through RFC 8785 (JCS), never raw bytes.
- Types never set `deny_unknown_fields` (Swift tolerates extra keys).
