#!/usr/bin/env bash
# ==============================================================================
# Test Execution Script
# ==============================================================================
set -euo pipefail

BUILD_DIR="build"

if [ ! -d "${BUILD_DIR}" ]; then
    echo "Build directory does not exist. Running build first..."
    ./scripts/build.sh Debug
fi

echo "==> Running CTest suite..."
ctest --test-dir "${BUILD_DIR}" --output-on-failure
