#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_ios_xcframework.sh - Compiles CurvEngine into an Apple XCFramework
# ==============================================================================
#
# Requires an OpenCV install discoverable under CMAKE_SYSTEM_NAME=iOS
# (macOS Homebrew OpenCV will NOT work for device/simulator slices).
# Set OpenCV_DIR to an iOS OpenCV build if CMake cannot find one automatically.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/build/CurvEngine.xcframework"

# ------------------------------------------------------------------------------
# OpenCV for iOS
# ------------------------------------------------------------------------------
# Build (or reuse) third_party/ios/opencv2.xcframework, then locate the per-slice
# build-tree OpenCVConfig.cmake so find_package(OpenCV) resolves the iOS build.
"${SCRIPT_DIR}/build_opencv_ios.sh"

OPENCV_XCF="${ROOT_DIR}/third_party/ios/opencv2.xcframework"
OPENCV_BUILD="$(ls -d "${ROOT_DIR}"/third_party/ios/opencv-build-* 2>/dev/null | head -1 || true)"

find_opencv_dir() {
    # $1 = platform build subdir (iphoneos | iphonesimulator)
    # Prefer the top-level per-arch build-tree config (…/build-<arch>-<sdk>/
    # OpenCVConfig.cmake); the nested install/ and unix-install/ copies carry
    # stale imported-target paths (e.g. a phantom libprotobuf.a).
    local d
    d="$(find "${OPENCV_BUILD}/$1" -path '*/build-*/OpenCVConfig.cmake' \
         -not -path '*/install/*' -not -path '*/unix-install/*' 2>/dev/null | head -1)"
    [[ -z "${d}" ]] && d="$(find "${OPENCV_BUILD}/$1" -name OpenCVConfig.cmake 2>/dev/null | head -1)"
    [[ -n "${d}" ]] && dirname "${d}"
}

: "${OpenCV_DIR_DEVICE:=$(find_opencv_dir iphoneos)}"
: "${OpenCV_DIR_SIM:=$(find_opencv_dir iphonesimulator)}"

# An explicit OpenCV_DIR env var overrides both slices.
if [[ -n "${OpenCV_DIR:-}" ]]; then
    OpenCV_DIR_DEVICE="${OpenCV_DIR}"
    OpenCV_DIR_SIM="${OpenCV_DIR}"
fi

if [[ -z "${OpenCV_DIR_DEVICE}" || -z "${OpenCV_DIR_SIM}" ]]; then
    echo "ERROR: could not locate per-slice OpenCVConfig.cmake under ${OPENCV_BUILD}." >&2
    echo "       Set OpenCV_DIR explicitly or delete third_party/ios/ and re-run." >&2
    exit 1
fi
echo "--> OpenCV (device): ${OpenCV_DIR_DEVICE}"
echo "--> OpenCV (sim):    ${OpenCV_DIR_SIM}"

echo "=================================================="
echo "Building CurvEngine.xcframework (iOS Device + Simulator)"
echo "=================================================="

BUILD_DEVICE="${ROOT_DIR}/build-ios-device"
BUILD_SIM="${ROOT_DIR}/build-ios-sim"

# Clear stale iOS cmake caches so SPDLOG_FMT_EXTERNAL / OpenCV options re-evaluate
rm -rf "${BUILD_DEVICE}" "${BUILD_SIM}"

# Unix Makefiles (not -G Xcode): FetchContent subbuilds of nlohmann_json/spdlog
# fail under the Xcode generator on Xcode 26 (ZERO_CHECK / ReRunCMake.make).
# Output .a lands at adapters/libcurv_capi_static.a rather than Release-iphoneos/.
GENERATOR="Unix Makefiles"
if command -v ninja >/dev/null 2>&1; then
    GENERATOR="Ninja"
fi

configure_ios_slice() {
    local build_dir="$1"
    local sysroot="$2"
    local opencv_dir="$3"
    cmake -B "${build_dir}" -G "${GENERATOR}" \
        -DCMAKE_SYSTEM_NAME=iOS \
        -DCMAKE_OSX_SYSROOT="${sysroot}" \
        -DCMAKE_OSX_ARCHITECTURES="arm64" \
        -DCMAKE_OSX_DEPLOYMENT_TARGET=16.3 \
        -DCMAKE_BUILD_TYPE=Release \
        -DVISION_WERROR=OFF \
        -DSPDLOG_FMT_EXTERNAL=OFF \
        -DOpenCV_DIR="${opencv_dir}" \
        "${ROOT_DIR}"
}

# 1. Configure and build for iOS Device (arm64)
echo "--> [1/3] Building iOS Device slice (arm64) with ${GENERATOR}..."
configure_ios_slice "${BUILD_DEVICE}" iphoneos "${OpenCV_DIR_DEVICE}"
cmake --build "${BUILD_DEVICE}" --target curv_capi_static -j

# 2. Configure and build for iOS Simulator (arm64)
echo "--> [2/3] Building iOS Simulator slice (arm64) with ${GENERATOR}..."
configure_ios_slice "${BUILD_SIM}" iphonesimulator "${OpenCV_DIR_SIM}"
cmake --build "${BUILD_SIM}" --target curv_capi_static -j

# 3. Assemble XCFramework
echo "--> [3/3] Creating universal XCFramework..."
rm -rf "${OUTPUT_DIR}"

DEVICE_LIB="${BUILD_DEVICE}/adapters/libcurv_capi_static.a"
SIM_LIB="${BUILD_SIM}/adapters/libcurv_capi_static.a"
if [[ ! -f "${DEVICE_LIB}" || ! -f "${SIM_LIB}" ]]; then
    echo "ERROR: expected static libs not found:" >&2
    echo "  ${DEVICE_LIB}" >&2
    echo "  ${SIM_LIB}" >&2
    exit 1
fi

xcodebuild -create-xcframework \
    -library "${DEVICE_LIB}" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -library "${SIM_LIB}" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -output "${OUTPUT_DIR}"

# 4. Ship OpenCV alongside CurvEngine so the app can link both. They are kept as
#    two separate XCFrameworks (merging static archives across slices is fragile
#    and SPM/Xcode link multiple binary targets fine).
rm -rf "${ROOT_DIR}/build/opencv2.xcframework"
cp -R "${OPENCV_XCF}" "${ROOT_DIR}/build/opencv2.xcframework"

echo "=================================================="
echo "Successfully generated:"
echo "  ${OUTPUT_DIR}"
echo "  ${ROOT_DIR}/build/opencv2.xcframework"
echo "=================================================="
