#!/usr/bin/env bash
# Builds the ASD Rust core (rust/crates/asd-ffi) for Apple platforms (spec FFI-2, ADR-0006 §7).
#
# Usage: scripts/build_asd_xcframework.sh [--debug] [-h|--help]
#
# Output (git-ignored, never committed; WS-4):
#   build/asd/AsdCore.xcframework      static libasd_ffi.a slices: ios-arm64,
#                                      ios-arm64-simulator, macos-arm64 (so `swift test`
#                                      on macOS links), each with Headers/asd_coreFFI/
#                                      {asd_coreFFI.h, module.modulemap} (C module `asd_coreFFI`)
#   build/asd/swift/asd_core.swift     UniFFI Swift bindings; compile them in the SwiftPM
#                                      wrapper target that depends on the binary target
#                                      `AsdCoreFFI` (names must differ: trap 10)
#
# Steps: rustup target add (if missing), cargo build --release per target, uniffi-bindgen
# (library mode, Swift) on the macOS slice, xcodebuild -create-xcframework.
#
# Environment:
#   CARGO_TARGET_DIR             cargo output (default rust/target)
#   ASD_XCFRAMEWORK_OUT          output dir (default build/asd)
#   IPHONEOS_DEPLOYMENT_TARGET   default 17.0 (CurvAsdGazeKit Package.swift)
#   MACOSX_DEPLOYMENT_TARGET     default 14.0
#
# Android (WP-A.12, TODO): the same crate builds with
#   cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o <jniLibs> build -p asd-ffi --release
# and Kotlin bindings come from
#   cargo run -p asd-ffi --features cli --bin uniffi-bindgen -- generate --library <lib> --language kotlin
# (package `org.curv.asd.core`, set in rust/crates/asd-ffi/uniffi.toml). Not wired yet.
set -euo pipefail

PROFILE=release
for arg in "$@"; do
  case "$arg" in
    --debug) PROFILE=debug ;;
    -h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $arg (try --help)" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/rust/Cargo.toml"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/rust/target}"
OUT="${ASD_XCFRAMEWORK_OUT:-$ROOT/build/asd}"
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"

TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin)
LIB=libasd_ffi.a
FFI_MODULE=asd_coreFFI

step() { printf '\n==> %s\n' "$*"; }
die() { echo "build_asd_xcframework: $*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || die "needs macOS with Xcode (xcodebuild)"
command -v xcodebuild >/dev/null || die "xcodebuild not found (install Xcode)"
command -v cargo >/dev/null || die "cargo not found (https://rustup.rs)"

cd "$ROOT/rust"

step "rust targets"
if command -v rustup >/dev/null; then
  installed="$(rustup target list --installed)"
  for t in "${TARGETS[@]}"; do
    grep -qx "$t" <<<"$installed" || rustup target add "$t"
  done
else
  echo "rustup not found; assuming ${TARGETS[*]} are installed"
fi

cargo_flags=(--manifest-path "$MANIFEST" -p asd-ffi --lib --locked)
[[ "$PROFILE" == release ]] && cargo_flags+=(--release)

for t in "${TARGETS[@]}"; do
  step "cargo build $t ($PROFILE)"
  CARGO_TARGET_DIR="$TARGET_DIR" cargo build "${cargo_flags[@]}" --target "$t"
  [[ -f "$TARGET_DIR/$t/$PROFILE/$LIB" ]] || die "missing $TARGET_DIR/$t/$PROFILE/$LIB"
done

step "uniffi-bindgen (Swift)"
GEN="$OUT/generated"
rm -rf "$GEN"
mkdir -p "$GEN"
CARGO_TARGET_DIR="$TARGET_DIR" cargo run --manifest-path "$MANIFEST" -q --locked \
  -p asd-ffi --features cli --bin uniffi-bindgen -- \
  generate --library "$TARGET_DIR/aarch64-apple-darwin/$PROFILE/$LIB" \
  --language swift --out-dir "$GEN" --no-format
for f in asd_core.swift "$FFI_MODULE.h" "$FFI_MODULE.modulemap"; do
  [[ -f "$GEN/$f" ]] || die "uniffi-bindgen did not write $f"
done

step "assemble $OUT/AsdCore.xcframework"
# Headers live in a module-named subdirectory so this module map cannot collide with
# another xcframework's top-level module.modulemap in the same build.
HEADERS="$OUT/headers"
rm -rf "$HEADERS" "$OUT/AsdCore.xcframework" "$OUT/swift"
mkdir -p "$HEADERS/$FFI_MODULE" "$OUT/swift"
cp "$GEN/$FFI_MODULE.h" "$HEADERS/$FFI_MODULE/"
cp "$GEN/$FFI_MODULE.modulemap" "$HEADERS/$FFI_MODULE/module.modulemap"
cp "$GEN/asd_core.swift" "$OUT/swift/"

xc_args=()
for t in "${TARGETS[@]}"; do
  xc_args+=(-library "$TARGET_DIR/$t/$PROFILE/$LIB" -headers "$HEADERS")
done
xcodebuild -create-xcframework "${xc_args[@]}" -output "$OUT/AsdCore.xcframework"
rm -rf "$HEADERS" "$GEN"

echo
echo "build_asd_xcframework: wrote"
echo "  $OUT/AsdCore.xcframework"
echo "  $OUT/swift/asd_core.swift"
