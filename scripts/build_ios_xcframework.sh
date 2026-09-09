#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_ios_xcframework.sh - Compiles CurvEngine into an Apple XCFramework
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/build/CurvEngine.xcframework"

echo "=================================================="
echo "Building CurvEngine.xcframework (iOS Device + Simulator)"
echo "=================================================="

BUILD_DEVICE="${ROOT_DIR}/build-ios-device"
BUILD_SIM="${ROOT_DIR}/build-ios-sim"

# 1. Configure and build for iOS Device (arm64)
echo "--> [1/3] Building iOS Device slice (arm64)..."
cmake -B "${BUILD_DEVICE}" -G Xcode \
    -DCMAKE_SYSTEM_NAME=iOS \
    -DCMAKE_OSX_SYSROOT=iphoneos \
    -DCMAKE_OSX_ARCHITECTURES="arm64" \
    -DCMAKE_BUILD_TYPE=Release \
    -DVISION_WERROR=OFF \
    -DBUILD_TESTING=OFF \
    "${ROOT_DIR}"

cmake --build "${BUILD_DEVICE}" --config Release --target curv_capi_static -j

# 2. Configure and build for iOS Simulator (arm64)
echo "--> [2/3] Building iOS Simulator slice (arm64)..."
cmake -B "${BUILD_SIM}" -G Xcode \
    -DCMAKE_SYSTEM_NAME=iOS \
    -DCMAKE_OSX_SYSROOT=iphonesimulator \
    -DCMAKE_OSX_ARCHITECTURES="arm64" \
    -DCMAKE_BUILD_TYPE=Release \
    -DVISION_WERROR=OFF \
    -DBUILD_TESTING=OFF \
    "${ROOT_DIR}"

cmake --build "${BUILD_SIM}" --config Release --target curv_capi_static -j

# 3. Assemble XCFramework
echo "--> [3/3] Creating universal XCFramework..."
rm -rf "${OUTPUT_DIR}"

xcodebuild -create-xcframework \
    -library "${BUILD_DEVICE}/adapters/Release-iphoneos/libcurv_capi_static.a" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -library "${BUILD_SIM}/adapters/Release-iphonesimulator/libcurv_capi_static.a" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -output "${OUTPUT_DIR}"

echo "=================================================="
echo "Successfully generated: ${OUTPUT_DIR}"
echo "=================================================="
