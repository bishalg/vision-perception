#!/usr/bin/env bash
# Builds the ASD Rust core for the web console (rust/crates/asd-wasm, WP-R.6, ADR-0006 §2):
# a wasm-bindgen `--target web` ES module package for the sandbox re-score (WP-C.5).
#
# Usage: scripts/build_asd_wasm.sh [--debug] [--test] [-h|--help]
#   --debug   unoptimised dev build (default: release)
#   --test    instead of building, run the asd-wasm tests compiled to wasm32 under Node
#             (wasm-bindgen-test): the same parity tests `cargo test -p asd-wasm` runs natively
#
# Output (git-ignored via build/, never committed; WS-4):
#   build/asd/wasm/asd_wasm.js          ES module: `init()` then the exports
#   build/asd/wasm/asd_wasm_bg.wasm     the module
#   build/asd/wasm/asd_wasm.d.ts        TypeScript declarations
#   build/asd/wasm/package.json         (wasm-pack only; `"type": "module"`)
#
# Tooling: wasm-pack (preferred; fetches the matching wasm-bindgen-cli and wasm-opt) or,
# without it, wasm-bindgen-cli at exactly the `wasm-bindgen` version in rust/Cargo.lock.
# Both need the `wasm32-unknown-unknown` target (added here if missing) and network access
# the first time.
#
# Environment:
#   CARGO_TARGET_DIR    cargo output (default rust/target)
#   ASD_WASM_OUT        output dir (default build/asd/wasm)
#   ASD_WASM_NO_OPT=1   skip wasm-opt (wasm-pack), e.g. offline
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/rust/crates/asd-wasm"
MANIFEST="$ROOT/rust/Cargo.toml"
OUT="${ASD_WASM_OUT:-$ROOT/build/asd/wasm}"
TARGET=wasm32-unknown-unknown
PROFILE=release
MODE=build

usage() { sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

for arg in "$@"; do
  case "$arg" in
    --debug) PROFILE=debug ;;
    --test) MODE="test" ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

# rust/rust-toolchain.toml selects the toolchain (rustup has no default outside rust/).
cd "$ROOT/rust"

if ! rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  echo "==> rustup target add $TARGET"
  rustup target add "$TARGET"
fi

# The wasm-bindgen crate version the workspace locked; a CLI must match it exactly.
locked_bindgen_version() {
  awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' \
    "$ROOT/rust/Cargo.lock"
}

if [[ "$MODE" == "test" ]]; then
  if ! command -v node >/dev/null 2>&1; then
    echo "node is required for --test" >&2
    exit 1
  fi
  if command -v wasm-pack >/dev/null 2>&1; then
    echo "==> wasm-pack test --node (asd-wasm)"
    (cd "$CRATE" && wasm-pack test --node)
  elif command -v wasm-bindgen-test-runner >/dev/null 2>&1; then
    echo "==> cargo test --target $TARGET (wasm-bindgen-test-runner)"
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
      cargo test --manifest-path "$MANIFEST" -p asd-wasm --target "$TARGET"
  else
    echo "install wasm-pack (cargo install wasm-pack) or wasm-bindgen-cli" \
      "$(locked_bindgen_version) (cargo install wasm-bindgen-cli --version $(locked_bindgen_version) --locked)" >&2
    exit 1
  fi
  exit 0
fi

rm -rf "$OUT"
mkdir -p "$OUT"

if command -v wasm-pack >/dev/null 2>&1; then
  args=(build "$CRATE" --target web --out-dir "$OUT" --out-name asd_wasm)
  [[ "$PROFILE" == "debug" ]] && args+=(--dev) || args+=(--release)
  [[ "${ASD_WASM_NO_OPT:-0}" == "1" ]] && args+=(--no-opt)
  echo "==> wasm-pack ${args[*]}"
  wasm-pack "${args[@]}"
  # wasm-pack drops a `*` .gitignore into the output; build/ is ignored already.
  rm -f "$OUT/.gitignore"
elif command -v wasm-bindgen >/dev/null 2>&1; then
  want="$(locked_bindgen_version)"
  have="$(wasm-bindgen --version | awk '{print $2}')"
  if [[ "$have" != "$want" ]]; then
    echo "wasm-bindgen-cli $have does not match the locked wasm-bindgen $want" \
      "(cargo install wasm-bindgen-cli --version $want --locked)" >&2
    exit 1
  fi
  cargo_args=(build --manifest-path "$MANIFEST" -p asd-wasm --lib --target "$TARGET")
  [[ "$PROFILE" == "release" ]] && cargo_args+=(--release)
  echo "==> cargo ${cargo_args[*]}"
  cargo "${cargo_args[@]}"
  target_dir="${CARGO_TARGET_DIR:-$ROOT/rust/target}"
  echo "==> wasm-bindgen --target web"
  wasm-bindgen --target web --out-dir "$OUT" --out-name asd_wasm \
    "$target_dir/$TARGET/$PROFILE/asd_wasm.wasm"
else
  echo "install wasm-pack (cargo install wasm-pack) or wasm-bindgen-cli $(locked_bindgen_version)" >&2
  exit 1
fi

echo "==> $OUT"
ls -l "$OUT"
