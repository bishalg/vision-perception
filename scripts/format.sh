#!/usr/bin/env bash
# ==============================================================================
# Code Formatting Script (clang-format)
#
#   ./scripts/format.sh           format all first-party C++ sources in place
#   ./scripts/format.sh --check   verify only; exit 1 listing non-conforming files
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK=0
if [[ "${1:-}" == "--check" || "${1:-}" == "-c" ]]; then
    CHECK=1
fi

if ! command -v clang-format >/dev/null 2>&1; then
    echo "ERROR: clang-format not found. Install the pinned version first:" >&2
    echo "  python3 -m pip install clang-format==18.1.8" >&2
    exit 1
fi

if [[ ! -f "$ROOT_DIR/.clang-format" ]]; then
    echo "ERROR: $ROOT_DIR/.clang-format missing; refusing to format with a default style." >&2
    exit 1
fi

echo "==> clang-format ($([[ $CHECK -eq 1 ]] && echo 'check' || echo 'in-place'))..."

FILE_LIST="$(mktemp)"
trap 'rm -f "$FILE_LIST"' EXIT
find \
    "$ROOT_DIR/core" \
    "$ROOT_DIR/adapters" \
    "$ROOT_DIR/domains" \
    "$ROOT_DIR/apps" \
    "$ROOT_DIR/tools" \
    -type f \( -name '*.cpp' -o -name '*.cc' -o -name '*.hpp' -o -name '*.h' \) 2>/dev/null >"$FILE_LIST"

COUNT=0
EXIT_CODE=0
while IFS= read -r file; do
    COUNT=$((COUNT + 1))
    if [[ $CHECK -eq 1 ]]; then
        if ! clang-format --dry-run --Werror "$file" >/dev/null 2>&1; then
            echo "  needs formatting: ${file#"$ROOT_DIR"/}"
            EXIT_CODE=1
        fi
    else
        clang-format -i "$file"
    fi
done <"$FILE_LIST"

if [[ $COUNT -eq 0 ]]; then
    echo "No source files found."
    exit 0
fi

if [[ $CHECK -eq 1 && $EXIT_CODE -ne 0 ]]; then
    echo "==> Format check FAILED. Run ./scripts/format.sh and commit the result."
elif [[ $CHECK -eq 1 ]]; then
    echo "==> Format check complete: all $COUNT files conform."
else
    echo "==> Formatting complete ($COUNT files)."
fi
exit "$EXIT_CODE"
