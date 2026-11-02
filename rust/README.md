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

A user who lost their authenticator app is reset by an operator (AUTH-9): this clears the
TOTP seed and any pending enrolment, writes audit `user.totp_reset` (`actor: operator`)
and prints the user id; the user then signs in with the password and enrols again from the
console. No KEK is needed. No HTTP endpoint changes another user's TOTP.

```bash
asd-backend admin reset-totp --user "$USER_ID"     # unknown id: exit 2, nothing changed
```

Rules: an existing e-mail (case-insensitive) is refused and nothing changes, even with
`--force`; a second superadmin or a second org of the same name needs `--force`; an owner
needs an existing `--org`, a superadmin takes none; passwords follow the API policy (12+
characters). Exit codes: `0` ok, `2` usage or invalid input, `3` conflicts with existing
data, `1` other errors (e.g. database).

## Upgrading (`asd-backend migrate`, KEY-11, KEY-12)

`asd-backend migrate` runs, in order: (a) when migration 0016 is applied and plaintext
TOTP seeds remain in `users.totp_secret`, it seals them under `ASD_DATA_KEK` **before**
applying anything (without the KEK it stops, names the variable and applies nothing);
(b) the pending migrations (a database below 0016 is first taken to 0016 and sealed, so one
run reaches the end); (c) the same back-fill again (a no-op after 0022). Each migration is
its own transaction, so after a failure `migrate` can simply be run again once the cause is
fixed; no manual SQL.

Migration **0022** drops the plaintext columns `users.totp_secret` and `review_items.note`.
It refuses (and drops nothing) while either still holds a value:

- `plaintext TOTP seeds remain: run "asd-backend migrate" with ASD_DATA_KEK set`: set the
  KEK and run `migrate` again (only seen without the KEK or when the migrations are applied
  by another tool, since `migrate` seals the seeds first).
- `plaintext review notes remain: run "asd-backend admin encrypt-review-notes" with the
  previous release ...`: that command was removed together with 0022. Before upgrading a
  deployment that ran WP-C.9 (0008) on existing review notes, run
  `asd-backend admin encrypt-review-notes` (needs `ASD_DATA_KEK`) with the release you are
  upgrading from; if the upgrade already stopped at 0022, the new API still serves (it no
  longer reads the column), so run the old binary's command against the same database and
  then the new `migrate` again.

`asd-backend admin encrypt-review-notes` no longer exists (exit 2 with this hint).

## Rotating the data KEK (`ASD_DATA_KEK`, KEY-8..10)

`ASD_DATA_KEK` wraps each org's data key (`org_data_keys`) and seals per-user secrets (the
TOTP seed). Rotation re-wraps and re-seals those rows; it does not change the data keys,
so clinical ciphertexts, integration-destination secrets (sealed with the org data key)
and MRN blind indexes stay as they are and are not rewritten.

```sh
NEW=$(openssl rand -base64 32)        # store it in the secret store next to the old key

# 1. API and worker: the new key active, the old one readable. Restart both.
#      ASD_DATA_KEK=$NEW
#      ASD_DATA_KEK_PREVIOUS=$OLD
#    Reads work for rows under either key; every new write uses $NEW.

# 2. Optional check, writes nothing: every row under $OLD is opened.
asd-backend admin rotate-kek --dry-run
# {"rewrapped_deks":12,"resealed_users":4,"remaining":0,"dry_run":true}

# 3. Rewrite (batches of 500 rows per transaction; --batch N to change). Same environment
#    as step 1. Safe to run while the API serves traffic, and safe to run again.
asd-backend admin rotate-kek
# {"rewrapped_deks":12,"resealed_users":4,"remaining":0,"dry_run":false}

# 4. Only when remaining is 0: remove ASD_DATA_KEK_PREVIOUS from API and worker, restart.
#    Keep $OLD in the secret store's history until backups made before step 3 have expired.
```

- `remaining > 0` exits `3` (the report is still printed): some rows are under a KEK that
  is not configured, could not be opened, or were locked by a concurrent writer. Run it
  again; if the count stays, find the stored fingerprint
  (`SELECT DISTINCT kek_id FROM org_data_keys` and `users.totp_secret_kek_id`), add the
  matching old key to `ASD_DATA_KEK_PREVIOUS` and rerun. The service logs the stored and
  configured fingerprints (`kek-` + 16 hex digits) when it meets such a row.
- A process started with only the new key before step 3 finished answers
  `503 clinical-crypto-unavailable` (and refuses TOTP logins) for rows still under the old
  key, by design. Put `ASD_DATA_KEK_PREVIOUS` back and restart to recover.
- Each run writes one audit row `crypto.kek_rotated` (no org) with the counts and the key
  fingerprints, never key bytes. `--dry-run` writes nothing.
- The configuration refuses a `PREVIOUS` entry equal to `ASD_DATA_KEK`, duplicates, and
  entries that are not base64 of exactly 32 bytes (exit `2`).

## Metrics and HSTS (`ASD_METRICS_BIND_ADDR`, OBS-2..4)

- Set `ASD_METRICS_BIND_ADDR` (for example `127.0.0.1:9100`) on `asd-backend serve` to start a
  second listener that serves only `GET /metrics` (Prometheus text 0.0.4; no auth, no CORS, not
  request-logged). Unset means no listener. The API port never serves `/metrics`
  (`/v1/orgs/{id}/metrics` is the unrelated dashboard). Keep the listener on a private interface.
- Series: `http_requests_total{route,method,status}`, `http_request_duration_seconds{route,method}`
  (buckets 5 ms to 10 s), `auth_rate_limited_total{limiter}`, `jobs_rows{kind,status}` (one query,
  cached 10 s) and `asd_backend_build_info{version,git_sha}` (`git_sha` comes from the
  `ASD_GIT_SHA` build-time variable, `unknown` otherwise). `route` is the matched path template
  (`/v1/sessions/{id}`) or `unmatched`; no id, e-mail or token ever becomes a label value.
- Every API response carries `Strict-Transport-Security: max-age=63072000; includeSubDomains`.

## Conventions

- Licence `Apache-2.0` on every crate; dependencies must pass `rust/deny.toml`.
- `asd-core` never depends on tokio, sqlx, reqwest, uniffi or other I/O crates.
- `serde_json` is used without `preserve_order`; contract bytes have sorted keys.
- Compare cross-language JSON through RFC 8785 (JCS), never raw bytes.
- Types never set `deny_unknown_fields` (Swift tolerates extra keys).
