#!/usr/bin/env bash
# ==============================================================================
# Code Formatting Script (clang-format)
# ==============================================================================
set -euo pipefail

echo "==> Formatting C++ code with clang-format..."

if command -v clang-format >/dev/null 2>&1; then
    find core adapters apps tools -name "*.cpp" -o -name "*.hpp" | while read -r file; do
        clang-format -i "$file"
    done
    echo "Formatting complete."
else
    echo "clang-format not installed; skipping format."
fi
