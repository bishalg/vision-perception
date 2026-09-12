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

# Merge transitive static dependencies into a single archive per slice so SPM's
# binaryTarget `curv_capi` resolves all curv_* symbols. The thin capi archive
# alone lacks curv_core / curv_domains / curv_adapters / spdlog objects.
DEVICE_MERGED="${BUILD_DEVICE}/adapters/libcurv_capi_merged.a"
SIM_MERGED="${BUILD_SIM}/adapters/libcurv_capi_merged.a"
echo "--> Merging device slice dependencies..."
libtool -static -o "${DEVICE_MERGED}" \
    "${BUILD_DEVICE}/core/libcurv_core.a" \
    "${BUILD_DEVICE}/domains/libcurv_domains.a" \
    "${BUILD_DEVICE}/adapters/libcurv_adapters.a" \
    "${DEVICE_LIB}" \
    "${BUILD_DEVICE}/_deps/spdlog-build/libspdlog.a"
echo "--> Merging simulator slice dependencies..."
libtool -static -o "${SIM_MERGED}" \
    "${BUILD_SIM}/core/libcurv_core.a" \
    "${BUILD_SIM}/domains/libcurv_domains.a" \
    "${BUILD_SIM}/adapters/libcurv_adapters.a" \
    "${SIM_LIB}" \
    "${BUILD_SIM}/_deps/spdlog-build/libspdlog.a"

xcodebuild -create-xcframework \
    -library "${DEVICE_MERGED}" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -library "${SIM_MERGED}" \
    -headers "${ROOT_DIR}/adapters/capi/include" \
    -output "${OUTPUT_DIR}"

# Normalize output to the conventional static name expected by SPM consumers.
# The XCFramework's Info.plist references BinaryPath/LibraryPath; we created it
# with the merged names, then copy to the canonical libcurv_capi_static.a.
for slice in ios-arm64 ios-arm64-simulator; do
    if [[ -f "${OUTPUT_DIR}/${slice}/libcurv_capi_merged.a" ]]; then
        mv "${OUTPUT_DIR}/${slice}/libcurv_capi_merged.a" "${OUTPUT_DIR}/${slice}/libcurv_capi_static.a"
    fi
done
# Patch XCFramework Info.plist if it still references the temporary merged name.
if grep -q "libcurv_capi_merged" "${OUTPUT_DIR}/Info.plist" 2>/dev/null; then
    OUTPUT_DIR_ENV="${OUTPUT_DIR}" python3 - << 'PY'
import plistlib, pathlib, os
p = pathlib.Path(os.environ["OUTPUT_DIR_ENV"]) / "Info.plist"
if p.exists():
    data = plistlib.loads(p.read_bytes())
    for lib in data.get("AvailableLibraries", []):
        lib["BinaryPath"] = lib["BinaryPath"].replace("libcurv_capi_merged.a", "libcurv_capi_static.a")
        lib["LibraryPath"] = lib["LibraryPath"].replace("libcurv_capi_merged.a", "libcurv_capi_static.a")
    p.write_bytes(plistlib.dumps(data))
    print(f"Patched {p}")
PY
fi

# 4. Package OpenCV as a static library XCFramework (libopencv2.a + Headers).
#    Because it is structured as a static library (.a) rather than a .framework,
#    Xcode and SPM link it statically and never attempt to embed it in App/Frameworks/,
#    eliminating all "No such file or directory" copy collisions and CFBundleExecutable issues.
echo "--> Converting OpenCV XCFramework to clean static library format..."
TMP_OPENCV_DIR="$(mktemp -d)"
DEVICE_LIB="${TMP_OPENCV_DIR}/libopencv2_device.a"
SIM_LIB="${TMP_OPENCV_DIR}/libopencv2_sim.a"

# Extract archive binaries
if [[ -f "${OPENCV_XCF}/ios-arm64/opencv2.framework/Versions/A/opencv2" ]]; then
    cp "${OPENCV_XCF}/ios-arm64/opencv2.framework/Versions/A/opencv2" "${DEVICE_LIB}"
    cp -R "${OPENCV_XCF}/ios-arm64/opencv2.framework/Versions/A/Headers" "${TMP_OPENCV_DIR}/Headers_device"
elif [[ -f "${OPENCV_XCF}/ios-arm64/opencv2.framework/opencv2" ]]; then
    cp "${OPENCV_XCF}/ios-arm64/opencv2.framework/opencv2" "${DEVICE_LIB}"
    cp -R "${OPENCV_XCF}/ios-arm64/opencv2.framework/Headers" "${TMP_OPENCV_DIR}/Headers_device"
fi

if [[ -f "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/Versions/A/opencv2" ]]; then
    cp "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/Versions/A/opencv2" "${SIM_LIB}"
    cp -R "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/Versions/A/Headers" "${TMP_OPENCV_DIR}/Headers_sim"
elif [[ -f "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/opencv2" ]]; then
    cp "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/opencv2" "${SIM_LIB}"
    cp -R "${OPENCV_XCF}/ios-arm64-simulator/opencv2.framework/Headers" "${TMP_OPENCV_DIR}/Headers_sim"
fi

rm -rf "${ROOT_DIR}/build/opencv2.xcframework"
xcodebuild -create-xcframework \
    -library "${DEVICE_LIB}" \
    -headers "${TMP_OPENCV_DIR}/Headers_device" \
    -library "${SIM_LIB}" \
    -headers "${TMP_OPENCV_DIR}/Headers_sim" \
    -output "${ROOT_DIR}/build/opencv2.xcframework"

for slice in ios-arm64 ios-arm64-simulator; do
    if [[ -f "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2_device.a" ]]; then
        mv "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2_device.a" "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2.a"
    fi
    if [[ -f "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2_sim.a" ]]; then
        mv "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2_sim.a" "${ROOT_DIR}/build/opencv2.xcframework/${slice}/libopencv2.a"
    fi
done

ROOT_DIR_ENV="${ROOT_DIR}" python3 - << 'PY2'
import plistlib, pathlib, os
p = pathlib.Path(os.environ["ROOT_DIR_ENV"]) / "build/opencv2.xcframework/Info.plist"
if p.exists():
    data = plistlib.loads(p.read_bytes())
    for lib in data.get("AvailableLibraries", []):
        lib["BinaryPath"] = "libopencv2.a"
        lib["LibraryPath"] = "libopencv2.a"
    p.write_bytes(plistlib.dumps(data))
    print(f"Normalized static {p}")
PY2

rm -rf "${TMP_OPENCV_DIR}"

echo "=================================================="
echo "Successfully generated:"
echo "  ${OUTPUT_DIR}"
echo "  ${ROOT_DIR}/build/opencv2.xcframework"
echo "=================================================="
