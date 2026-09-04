#!/usr/bin/env bash
# ==============================================================================
# Build & Test Script for vision-perception
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_TYPE="${1:-Release}"
BUILD_DIR="${ROOT_DIR}/build"

echo "==> Configuring build (${BUILD_TYPE}) into ${BUILD_DIR}..."

cmake -B "${BUILD_DIR}" \
      -S "${ROOT_DIR}" \
      -DCMAKE_BUILD_TYPE="${BUILD_TYPE}"

echo "==> Compiling targets..."
cmake --build "${BUILD_DIR}" --parallel "$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"

echo "==> Build successful."

# Run GoogleTest suite via CTest
echo "==> Running automated test suite..."
ctest --test-dir "${BUILD_DIR}" --output-on-failure

# Run post-build sanity checks
if [ -x "${BUILD_DIR}/tools/curv_cli" ]; then
    echo "==> Executing curv_cli post-build sanity test..."
    "${BUILD_DIR}/tools/curv_cli"
fi

if [ -x "${BUILD_DIR}/apps/desktop_pipeline_demo/desktop_pipeline_demo" ]; then
    echo "==> Executing desktop_pipeline_demo post-build smoke test..."
    "${BUILD_DIR}/apps/desktop_pipeline_demo/desktop_pipeline_demo"
fi

echo "==> All build and test stages completed successfully!"
