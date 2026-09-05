#!/usr/bin/env bash
# ==============================================================================
# Linter Script (clang-tidy static analysis over first-party sources)
#
# Requires a configured build directory (compile_commands.json).
#   ./scripts/build.sh        (or: cmake -S . -B build)
#   ./scripts/lint.sh         analyze everything, fail on any finding
#   ./scripts/lint.sh a.cpp   analyze specific files only
#   ./scripts/lint.sh --fix   apply clang-tidy's safe auto-fixes, then report
#
# adapters/android and adapters/apple are excluded: they are compiled by the
# Android NDK / Xcode toolchains, not by the desktop compile database.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$ROOT_DIR/build}"

if ! command -v clang-tidy >/dev/null 2>&1; then
    # Common Homebrew LLVM location when only the full llvm formula is installed
    if [[ -x "/opt/homebrew/opt/llvm/bin/clang-tidy" ]]; then
        PATH="/opt/homebrew/opt/llvm/bin:$PATH"
    elif [[ -x "/usr/local/opt/llvm/bin/clang-tidy" ]]; then
        PATH="/usr/local/opt/llvm/bin:$PATH"
    else
        echo "ERROR: clang-tidy not found. Install it first:" >&2
        echo "  macOS:  brew install llvm (then ensure /opt/homebrew/opt/llvm/bin is on PATH)" >&2
        echo "  Ubuntu: apt install clang-tidy" >&2
        exit 1
    fi
fi

COMPILE_DB="$BUILD_DIR/compile_commands.json"
if [[ ! -f "$COMPILE_DB" ]]; then
    echo "ERROR: $COMPILE_DB not found." >&2
    echo "  Run scripts/build.sh (or: cmake -S . -B build) first so a compile database exists." >&2
    exit 1
fi

echo "==> Running clang-tidy (build dir: $BUILD_DIR)..."

FILE_LIST="$(mktemp)"
REPORT="$(mktemp)"
trap 'rm -f "$FILE_LIST" "$REPORT"' EXIT

FIX=0
if [[ "${1:-}" == "--fix" ]]; then
    FIX=1
    shift
fi

if [[ $# -gt 0 ]]; then
    for arg in "$@"; do
        echo "$arg" >>"$FILE_LIST"
    done
else
    find \
        "$ROOT_DIR/core" \
        "$ROOT_DIR/domains" \
        "$ROOT_DIR/apps" \
        "$ROOT_DIR/tools" \
        "$ROOT_DIR/adapters" \
        -type f \( -name '*.cpp' -o -name '*.cc' \) \
        -not -path "$ROOT_DIR/adapters/android/*" \
        -not -path "$ROOT_DIR/adapters/apple/*" \
        2>/dev/null >"$FILE_LIST"
fi

if [[ ! -s "$FILE_LIST" ]]; then
    echo "No source files found; nothing to lint."
    exit 0
fi

EXIT_CODE=0
COUNT=0

# Apple's /usr/bin/c++ injects the macOS SDK sysroot implicitly; upstream
# clang-tidy does not, so pass it explicitly to keep <fstream> & friends resolvable.
EXTRA_ARGS=()
if [[ "$(uname)" == "Darwin" ]] && command -v xcrun >/dev/null 2>&1; then
    SDK_PATH="$(xcrun --show-sdk-path 2>/dev/null || true)"
    if [[ -n "$SDK_PATH" ]]; then
        EXTRA_ARGS=(-extra-arg=-isysroot -extra-arg="$SDK_PATH")
    fi
fi

while IFS= read -r file; do
    COUNT=$((COUNT + 1))
    rel="${file#"$ROOT_DIR"/}"
    printf '  analyzing %s\n' "$rel"
    FIX_ARGS=()
    if [[ $FIX -eq 1 ]]; then
        FIX_ARGS=(-fix -fix-errors)
    fi
    # `${ARR[@]+...}` guards empty-array expansion under `set -u` on bash 3.x (macOS)
    if ! clang-tidy -p "$BUILD_DIR" ${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"} ${FIX_ARGS[@]+"${FIX_ARGS[@]}"} "$file" >"$REPORT" 2>&1; then
        echo "clang-tidy reported a fatal error for $rel:" >&2
        cat "$REPORT" >&2
        EXIT_CODE=1
        continue
    fi
    if grep -qE '(warning|error): ' "$REPORT"; then
        cat "$REPORT"
        EXIT_CODE=1
    fi
done <"$FILE_LIST"

if [[ $EXIT_CODE -ne 0 ]]; then
    echo "==> Lint FAILED over $COUNT files: fix the findings above (or add a justified NOLINT)."
else
    echo "==> Lint check complete: $COUNT files, clean."
fi
exit "$EXIT_CODE"
