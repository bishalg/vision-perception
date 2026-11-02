#!/usr/bin/env bash
# Builds the ASD Rust core (rust/crates/asd-ffi) for Android (spec AND-1..3, ADR-0011 §7,
# ADR-0006 §7): the same UniFFI definitions as the Apple xcframework, as Kotlin bindings.
#
# Usage: scripts/build_asd_android.sh [--debug] [--host] [--test] [-h|--help]
#
#   (no flag)  libasd_ffi.so for arm64-v8a, armeabi-v7a, x86_64 (cargo ndk), the Kotlin
#              bindings, and the AAR (gradle :core:assembleRelease)
#   --host     skip the NDK and the AAR: only the host library and the Kotlin bindings
#              (enough for the JVM unit test; no Android NDK needed)
#   --test     also run the JVM golden parity test (gradle :core:testDebugUnitTest), which
#              loads the host library through JNA (`jna.library.path`)
#   --debug    cargo debug profile (default release)
#
# Output (git-ignored, never committed; like build/asd/AsdCore.xcframework):
#   build/asd/android/jniLibs/<abi>/libasd_ffi.so      Android shared libraries
#   build/asd/android/kotlin/org/curv/asd/core/asd_core.kt
#                                                      UniFFI Kotlin bindings (package
#                                                      org.curv.asd.core, rust/crates/asd-ffi/uniffi.toml)
#   build/asd/android/host/libasd_ffi.{dylib,so}       host library for the JVM tests
#   apps/android_asd_core/core/build/outputs/aar/core-release.aar
#                                                      AAR org.curv.asd:core (not with --host)
#
# Steps: rustup target add (if missing), cargo build of the host cdylib, uniffi-bindgen
# (library mode, Kotlin) on it, cargo ndk per ABI, gradle.
#
# Environment:
#   CARGO_TARGET_DIR          cargo output (default rust/target)
#   ASD_ANDROID_OUT           output dir (default build/asd/android)
#   ANDROID_NDK_HOME          NDK r27+ (cargo ndk also finds $ANDROID_HOME/ndk/<latest>)
#   ANDROID_HOME              SDK, for gradle (or apps/android_asd_core/local.properties)
#   ASD_ANDROID_API           minimum API level of the .so files (default 26 = minSdk)
#   GRADLE                    gradle executable (default `gradle` on PATH; 8.14+, JDK 17+)
set -euo pipefail

PROFILE=release
HOST_ONLY=0
RUN_TEST=0
for arg in "$@"; do
  case "$arg" in
    --debug) PROFILE=debug ;;
    --host) HOST_ONLY=1 ;;
    --test) RUN_TEST=1 ;;
    -h|--help) sed -n '2,36p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $arg (try --help)" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/rust/Cargo.toml"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/rust/target}"
OUT="${ASD_ANDROID_OUT:-$ROOT/build/asd/android}"
API="${ASD_ANDROID_API:-26}"
GRADLE="${GRADLE:-gradle}"
PROJECT="$ROOT/apps/android_asd_core"

# ABI -> Rust target (cargo ndk takes either name).
ABIS=(arm64-v8a armeabi-v7a x86_64)
TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)

step() { printf '\n==> %s\n' "$*"; }
die() { echo "build_asd_android: $*" >&2; exit 1; }

command -v cargo >/dev/null || die "cargo not found (https://rustup.rs)"
case "$(uname -s)" in
  Darwin) HOST_LIB=libasd_ffi.dylib ;;
  Linux) HOST_LIB=libasd_ffi.so ;;
  *) die "unsupported host $(uname -s) (macOS or Linux)" ;;
esac
if [[ $HOST_ONLY == 0 || $RUN_TEST == 1 ]]; then
  command -v "$GRADLE" >/dev/null || die "gradle not found (set GRADLE=/path/to/gradle; 8.14+)"
fi

cd "$ROOT/rust"

cargo_flags=(--manifest-path "$MANIFEST" -p asd-ffi --lib --locked)
[[ "$PROFILE" == release ]] && cargo_flags+=(--release)

step "cargo build host cdylib ($PROFILE)"
CARGO_TARGET_DIR="$TARGET_DIR" cargo build "${cargo_flags[@]}"
[[ -f "$TARGET_DIR/$PROFILE/$HOST_LIB" ]] || die "missing $TARGET_DIR/$PROFILE/$HOST_LIB"
rm -rf "$OUT/host"
mkdir -p "$OUT/host"
cp "$TARGET_DIR/$PROFILE/$HOST_LIB" "$OUT/host/"

step "uniffi-bindgen (Kotlin)"
rm -rf "$OUT/kotlin"
mkdir -p "$OUT/kotlin"
CARGO_TARGET_DIR="$TARGET_DIR" cargo run --manifest-path "$MANIFEST" -q --locked \
  -p asd-ffi --features cli --bin uniffi-bindgen -- \
  generate --library "$OUT/host/$HOST_LIB" \
  --language kotlin --out-dir "$OUT/kotlin" --no-format
[[ -f "$OUT/kotlin/org/curv/asd/core/asd_core.kt" ]] \
  || die "uniffi-bindgen did not write org/curv/asd/core/asd_core.kt"

if [[ $HOST_ONLY == 0 ]]; then
  command -v cargo-ndk >/dev/null || die "cargo-ndk not found (cargo install cargo-ndk --locked)"

  step "rust targets"
  if command -v rustup >/dev/null; then
    installed="$(rustup target list --installed)"
    for t in "${TARGETS[@]}"; do
      grep -qx "$t" <<<"$installed" || rustup target add "$t"
    done
  else
    echo "rustup not found; assuming ${TARGETS[*]} are installed"
  fi

  step "cargo ndk ${ABIS[*]} (API $API, $PROFILE)"
  rm -rf "$OUT/jniLibs"
  mkdir -p "$OUT/jniLibs"
  ndk_flags=()
  for abi in "${ABIS[@]}"; do ndk_flags+=(-t "$abi"); done
  CARGO_TARGET_DIR="$TARGET_DIR" cargo ndk "${ndk_flags[@]}" --platform "$API" \
    -o "$OUT/jniLibs" build "${cargo_flags[@]}"
  for abi in "${ABIS[@]}"; do
    [[ -f "$OUT/jniLibs/$abi/libasd_ffi.so" ]] || die "missing $OUT/jniLibs/$abi/libasd_ffi.so"
  done
fi

gradle_tasks=()
[[ $RUN_TEST == 1 ]] && gradle_tasks+=(:core:testDebugUnitTest)
[[ $HOST_ONLY == 0 ]] && gradle_tasks+=(:core:assembleRelease)
if ((${#gradle_tasks[@]})); then
  step "gradle ${gradle_tasks[*]}"
  "$GRADLE" -p "$PROJECT" --no-daemon --stacktrace \
    -PasdAndroidOut="$OUT" "${gradle_tasks[@]}"
fi

echo
echo "build_asd_android: wrote"
echo "  $OUT/kotlin/org/curv/asd/core/asd_core.kt"
echo "  $OUT/host/$HOST_LIB"
if [[ $HOST_ONLY == 0 ]]; then
  for abi in "${ABIS[@]}"; do echo "  $OUT/jniLibs/$abi/libasd_ffi.so"; done
  echo "  $PROJECT/core/build/outputs/aar/core-release.aar"
fi
[[ $RUN_TEST == 1 ]] && echo "  (JVM golden parity test passed)"
exit 0
