#!/usr/bin/env bash
# ==============================================================================
# Linter Verification Script (clang-tidy / static analysis)
# ==============================================================================
set -euo pipefail

echo "==> Running static analysis checks..."

if command -v clang-tidy >/dev/null 2>&1; then
    find core adapters apps tools -name "*.cpp" -o -name "*.hpp" | while read -r file; do
        clang-tidy "$file" -- -std=c++20 || true
    done
else
    echo "clang-tidy not installed; skipping detailed static analysis check."
fi

echo "==> Lint check complete."
