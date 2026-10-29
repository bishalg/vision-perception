#!/usr/bin/env bash
# Build AsdGazeCollector for a connected iPhone, install + launch via devicectl.
# Usage: ./scripts/run_gaze_app_device.sh [device-udid]
# Default: Bishal’s iPhone 17 Pro (00008150-001409290AF0401C)
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DEFAULT_DEV="00008150-001409290AF0401C"
DEV="${1:-${DEV:-${DEFAULT_DEV}}}"

echo "==> Checking Connected Devices:"
xcrun devicectl list devices 2>/dev/null || true

echo ""
echo "==> Deploying AsdGazeCollector to device: ${DEV}..."

xcodebuild -project apps/ios_asd_gaze_app/AsdGazeCollector.xcodeproj \
  -scheme AsdGazeCollector \
  -destination "platform=iOS,id=${DEV}" \
  -allowProvisioningUpdates \
  -derivedDataPath /tmp/AsdGazeDerivedDevice build | tail -n 10

APP=/tmp/AsdGazeDerivedDevice/Build/Products/Debug-iphoneos/AsdGazeCollector.app

echo "==> Installing app on device..."
xcrun devicectl device install app --device "${DEV}" "${APP}"

echo "==> Launching Eye Gaze..."
xcrun devicectl device process launch --device "${DEV}" com.open-spectrum.eye-gaze

echo "==> Done! App launched on device."
