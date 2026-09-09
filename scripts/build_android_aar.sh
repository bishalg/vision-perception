#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_android_aar.sh - Compiles CurvEngine into an Android Archive (.aar)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=================================================="
echo "Building CurvEngine Android NDK Libraries (.so / .aar)"
echo "=================================================="

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
    if [[ -d "${HOME}/Library/Android/sdk/ndk" ]]; then
        # Pick the latest NDK version available
        ANDROID_NDK_HOME="$(find "${HOME}/Library/Android/sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
        export ANDROID_NDK_HOME
        echo "Auto-detected ANDROID_NDK_HOME: ${ANDROID_NDK_HOME}"
    else
        echo "[WARN] ANDROID_NDK_HOME is not set. Please set ANDROID_NDK_HOME to compile for Android."
        echo "Reference command for manual build:"
        echo "cmake -DCMAKE_TOOLCHAIN_FILE=\$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 .."
        exit 0
    fi
fi

TOOLCHAIN_FILE="${ANDROID_NDK_HOME}/build/cmake/android.toolchain.cmake"
OUTPUT_DIR="${ROOT_DIR}/build/android"
mkdir -p "${OUTPUT_DIR}/jniLibs/arm64-v8a"
mkdir -p "${OUTPUT_DIR}/jniLibs/armeabi-v7a"

# 1. Build arm64-v8a
echo "--> [1/2] Building Android arm64-v8a slice..."
BUILD_ARM64="${ROOT_DIR}/build-android-arm64"
cmake -B "${BUILD_ARM64}" \
    -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN_FILE}" \
    -DANDROID_ABI="arm64-v8a" \
    -DANDROID_PLATFORM="android-26" \
    -DANDROID_STL="c++_static" \
    -DCMAKE_BUILD_TYPE=Release \
    -DVISION_WERROR=OFF \
    -DBUILD_TESTING=OFF \
    "${ROOT_DIR}"

cmake --build "${BUILD_ARM64}" --config Release --target curv_capi -j
cp "${BUILD_ARM64}/adapters/libcurv_capi.so" "${OUTPUT_DIR}/jniLibs/arm64-v8a/"

# 2. Build armeabi-v7a
echo "--> [2/2] Building Android armeabi-v7a slice..."
BUILD_ARMV7="${ROOT_DIR}/build-android-armv7"
cmake -B "${BUILD_ARMV7}" \
    -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN_FILE}" \
    -DANDROID_ABI="armeabi-v7a" \
    -DANDROID_PLATFORM="android-26" \
    -DANDROID_STL="c++_static" \
    -DCMAKE_BUILD_TYPE=Release \
    -DVISION_WERROR=OFF \
    -DBUILD_TESTING=OFF \
    "${ROOT_DIR}"

cmake --build "${BUILD_ARMV7}" --config Release --target curv_capi -j
cp "${BUILD_ARMV7}/adapters/libcurv_capi.so" "${OUTPUT_DIR}/jniLibs/armeabi-v7a/"

echo "=================================================="
echo "Successfully built Android native binaries in: ${OUTPUT_DIR}/jniLibs/"
echo "=================================================="
