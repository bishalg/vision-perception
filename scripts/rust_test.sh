#!/usr/bin/env bash
# All Rust gates for the ASD workspace (spec WS-5), in order:
#   fmt --check, clippy -D warnings, test, cargo deny, sqlx prepare --check, asd-cli schema check.
#
# Env:
#   CARGO_TARGET_DIR      build output (default rust/target, git-ignored)
#   DATABASE_URL          Postgres for backend tests / sqlx check
#   SQLX_OFFLINE=true     use committed rust/.sqlx data; skips the live-DB part
#   ASD_REQUIRE_ALL=1     fail instead of skipping when a tool or step is unavailable (CI sets it)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/rust/Cargo.toml"
cd "$ROOT/rust"

step() { printf '\n==> %s\n' "$*"; }
skip() {
  if [[ "${ASD_REQUIRE_ALL:-0}" == "1" ]]; then
    echo "FAIL (ASD_REQUIRE_ALL=1): $*" >&2
    exit 1
  fi
  echo "SKIP: $*"
}

step "cargo fmt --check"
cargo fmt --manifest-path "$MANIFEST" --all -- --check

step "cargo clippy (warnings are errors)"
cargo clippy --manifest-path "$MANIFEST" --workspace --all-targets -- -D warnings

step "cargo test"
cargo test --manifest-path "$MANIFEST" --workspace

step "cargo deny check"
if cargo deny --version >/dev/null 2>&1; then
  cargo deny --manifest-path "$MANIFEST" check
else
  skip "cargo-deny not installed (cargo install cargo-deny --locked)"
fi

step "cargo sqlx prepare --check"
if [[ ! -d "$ROOT/rust/.sqlx" ]]; then
  skip "rust/.sqlx absent (created by WP-B.1)"
elif ! cargo sqlx --version >/dev/null 2>&1; then
  skip "cargo-sqlx not installed (cargo install sqlx-cli --locked)"
elif [[ -z "${DATABASE_URL:-}" && "${SQLX_OFFLINE:-}" != "true" ]]; then
  skip "set DATABASE_URL or SQLX_OFFLINE=true"
else
  cargo sqlx prepare --workspace --check
fi

step "asd-cli schema check"
if grep -q 'pub fn' "$ROOT/rust/crates/asd-cli/src/cmd/schema.rs" 2>/dev/null; then
  cargo run --manifest-path "$MANIFEST" -q -p asd-cli -- schema check
else
  skip "asd-cli schema not implemented yet (WP-R.3)"
fi

echo
echo "rust_test.sh: all gates passed"
