#!/usr/bin/env bash
# ==============================================================================
# Build Script for vision-perception
# ==============================================================================
set -euo pipefail

BUILD_TYPE="${1:-Release}"
BUILD_DIR="build"

echo "==> Configuring build (${BUILD_TYPE}) into ${BUILD_DIR}..."

cmake -B "${BUILD_DIR}" \
      -DCMAKE_BUILD_TYPE="${BUILD_TYPE}" \
      -DBUILD_TESTING=ON \
      -DBUILD_APPS=ON

echo "==> Compiling targets..."
cmake --build "${BUILD_DIR}" --parallel "$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)"

echo "==> Build successful."
