#!/usr/bin/env bash
# ==============================================================================
# Coverage pipeline for vision-perception.
#
#   1. configure + build the "coverage" preset (Debug + VISION_COVERAGE=ON,
#      i.e. -fprofile-instr-generate -fcoverage-mapping)
#   2. run the full ctest suite with LLVM_PROFILE_FILE capturing .profraw files
#   3. merge profiles (llvm-profdata) and export JSON summaries (llvm-cov)
#   4. enforce the 100% line + branch gate (scripts/coverage_gate.py)
#   5. emit a browsable HTML report into build-coverage/html/
#
# Environment overrides: JOBS, LLVM_PROFDATA, LLVM_COV, SKIP_HTML=1.
# Exits non-zero if any test fails or the coverage gate fails.
# ==============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$ROOT/build-coverage"
PROFILES="$BUILD_DIR/profiles"
JOBS="${JOBS:-8}"

# ------------------------------------------------------------------------------
# Locate the llvm tools. The Xcode toolchain llvm-cov/llvm-profdata are
# version-matched to the Apple clang that compiles the project; Homebrew LLVM
# is the first fallback, PATH the last.
# ------------------------------------------------------------------------------
find_llvm_dir() {
    local xc
    xc="$(xcode-select -p 2>/dev/null || true)"
    local candidates=(
        "${xc}/Toolchains/XcodeDefault.xctoolchain/usr/bin"
        "/opt/homebrew/opt/llvm/bin"
        "/usr/local/opt/llvm/bin"
    )
    local d
    for d in "${candidates[@]}"; do
        if [ -n "$d" ] && [ -x "$d/llvm-profdata" ] && [ -x "$d/llvm-cov" ]; then
            printf '%s\n' "$d"
            return 0
        fi
    done
    if command -v llvm-profdata >/dev/null 2>&1 && command -v llvm-cov >/dev/null 2>&1; then
        printf '%s\n' "$(dirname "$(command -v llvm-profdata)")"
        return 0
    fi
    return 1
}

if [ -z "${LLVM_PROFDATA:-}" ] || [ -z "${LLVM_COV:-}" ]; then
    if LLVM_DIR="$(find_llvm_dir)"; then
        LLVM_PROFDATA="${LLVM_PROFDATA:-$LLVM_DIR/llvm-profdata}"
        LLVM_COV="${LLVM_COV:-$LLVM_DIR/llvm-cov}"
    else
        echo "error: llvm-profdata/llvm-cov not found (set LLVM_PROFDATA and LLVM_COV)" >&2
        exit 2
    fi
fi
echo "[coverage] llvm-profdata: $LLVM_PROFDATA"
echo "[coverage] llvm-cov:      $LLVM_COV"

# ------------------------------------------------------------------------------
# 1. Build instrumented binaries.
# ------------------------------------------------------------------------------
cmake --preset coverage
cmake --build --preset coverage -j "$JOBS"

# ------------------------------------------------------------------------------
# 2. Run the test suite with per-binary profile output.
# ------------------------------------------------------------------------------
rm -rf "$PROFILES"
mkdir -p "$PROFILES"
export LLVM_PROFILE_FILE="$PROFILES/%p-%m.profraw"
ctest --test-dir "$BUILD_DIR" --output-on-failure

# ------------------------------------------------------------------------------
# 3. Merge profiles. Objects passed to llvm-cov must include every binary that
# carries instrumented code: the test executables AND shared libraries
# (libcurv_capi.dylib) linked into them.
# ------------------------------------------------------------------------------
shopt -s nullglob
profraws=("$PROFILES"/*.profraw)
shopt -u nullglob
if [ "${#profraws[@]}" -eq 0 ]; then
    echo "error: no .profraw files produced; was the build instrumented?" >&2
    exit 2
fi
"$LLVM_PROFDATA" merge -sparse "${profraws[@]}" -o "$BUILD_DIR/coverage.profdata"

objects=()
shopt -s nullglob
for f in "$BUILD_DIR"/tests/test_*; do
    if [ -f "$f" ] && [ -x "$f" ]; then
        objects+=("$f")
    fi
done
shopt -u nullglob
while IFS= read -r -d '' sharedlib; do
    objects+=("$sharedlib")
done < <(find "$BUILD_DIR" -maxdepth 2 \( -name "*.dylib" -o -name "*.so" \) -type f -perm -u+x -print0)

if [ "${#objects[@]}" -eq 0 ]; then
    echo "error: no test objects found under $BUILD_DIR" >&2
    exit 2
fi
echo "[coverage] ${#objects[@]} object(s) instrumented"

# llvm-cov takes ONE binary positionally; every additional object must go
# through -object (extra positional args are treated as source filters).
cov_objects=()
for ((i = 1; i < ${#objects[@]}; i++)); do
    cov_objects+=(-object "${objects[$i]}")
done

# ------------------------------------------------------------------------------
# 4. Export per-file summaries (with branch data) and run the gate.
# Full export (not --summary-only): the gate needs per-function branch records
# to apply justified function-level exemptions.
# ------------------------------------------------------------------------------
"$LLVM_COV" export "${objects[0]}" ${cov_objects[@]+"${cov_objects[@]}"} \
    -instr-profile="$BUILD_DIR/coverage.profdata" \
    --show-branch-summary --skip-expansions > "$BUILD_DIR/coverage-export.json"

python3 "$ROOT/scripts/coverage_gate.py" "$BUILD_DIR/coverage-export.json" \
    --summary "$BUILD_DIR/coverage-summary.json"

# ------------------------------------------------------------------------------
# 5. HTML report (skippable for fast gate runs).
# ------------------------------------------------------------------------------
if [ "${SKIP_HTML:-0}" != "1" ]; then
    "$LLVM_COV" show "${objects[0]}" ${cov_objects[@]+"${cov_objects[@]}"} \
        -instr-profile="$BUILD_DIR/coverage.profdata" \
        --format=html --show-branches=count --show-line-counts-or-regions \
        -output-dir "$BUILD_DIR/html" > /dev/null
    echo "[coverage] HTML report: $BUILD_DIR/html/index.html"
fi
echo "[coverage] gate summary:  $BUILD_DIR/coverage-summary.json"
