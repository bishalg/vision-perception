#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_opencv_ios.sh - Builds OpenCV as an iOS XCFramework (device + simulator)
# ==============================================================================
#
# Produces third_party/ios/opencv2.xcframework with slices:
#   - ios-arm64            (iphoneos)
#   - ios-arm64-simulator  (iphonesimulator)
#
# Modules kept:    core, imgproc, imgcodecs
# Modules dropped: dnn (OpenCV 4.11 FP16 NEON intrinsics break under Xcode 16+;
#                  the iOS app uses Apple Vision for landmarks, not cv::dnn),
#                  videoio, highgui, gapi, objc/python/java/ts bindings
#
# Idempotent: exits early if the xcframework already exists. Delete
# third_party/ios/ to force a full rebuild.
#
# The packaged .xcframework has NO OpenCVConfig.cmake. The per-slice intermediate
# build trees under third_party/ios/opencv-build-<ver>/{iphoneos,iphonesimulator}
# DO - scripts/build_ios_xcframework.sh locates those and passes them as
# -DOpenCV_DIR per slice.
# ==============================================================================

OPENCV_VERSION="4.11.0"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

CACHE_DIR="${ROOT_DIR}/third_party/ios"
SRC_DIR="${CACHE_DIR}/opencv-src-${OPENCV_VERSION}"
BUILD_DIR="${CACHE_DIR}/opencv-build-${OPENCV_VERSION}"
XCF="${CACHE_DIR}/opencv2.xcframework"

if [[ -d "${XCF}" ]]; then
    echo "[build_opencv_ios] ${XCF} already exists - nothing to do."
    echo "[build_opencv_ios] Delete ${CACHE_DIR} to force a rebuild."
    exit 0
fi

mkdir -p "${CACHE_DIR}"

# 1. Fetch OpenCV source at the pinned tag (shallow).
if [[ ! -d "${SRC_DIR}" ]]; then
    echo "[build_opencv_ios] Cloning OpenCV ${OPENCV_VERSION}..."
    git clone --depth 1 --branch "${OPENCV_VERSION}" \
        https://github.com/opencv/opencv.git "${SRC_DIR}"
else
    echo "[build_opencv_ios] Reusing source at ${SRC_DIR}"
fi

BUILDER="${SRC_DIR}/platforms/apple/build_xcframework.py"
if [[ ! -f "${BUILDER}" ]]; then
    echo "ERROR: ${BUILDER} not found (unexpected OpenCV layout)." >&2
    exit 1
fi

# 2. Build the XCFramework (device arm64 + simulator arm64).
# `--disable FEATURE` becomes `-DWITH_FEATURE=OFF` in platforms/ios/build_framework.py.
# `--cmake_option=-D...` is accepted by build_xcframework.py then silently dropped
# by the iOS builder (parse_known_args) — that is why WITH_PROTOBUF stayed ON.
echo "[build_opencv_ios] Building XCFramework into ${BUILD_DIR} ..."
rm -rf "${BUILD_DIR}"
python3 "${BUILDER}" \
    --out "${BUILD_DIR}" \
    --iphoneos_archs arm64 \
    --iphonesimulator_archs arm64 \
    --build_only_specified_archs \
    --disable-bitcode \
    --iphoneos_deployment_target 13.0 \
    --without dnn \
    --without videoio \
    --without highgui \
    --without gapi \
    --without objc \
    --without python \
    --without java \
    --without ts \
    --disable protobuf

# 3. Publish the xcframework to the stable cache path.
BUILT_XCF="${BUILD_DIR}/opencv2.xcframework"
if [[ ! -d "${BUILT_XCF}" ]]; then
    echo "ERROR: expected ${BUILT_XCF} was not produced." >&2
    exit 1
fi
rm -rf "${XCF}"
cp -R "${BUILT_XCF}" "${XCF}"

echo "=================================================="
echo "[build_opencv_ios] Done: ${XCF}"
plutil -p "${XCF}/Info.plist" 2>/dev/null | grep -E 'LibraryIdentifier|SupportedPlatform' || true
echo "=================================================="
