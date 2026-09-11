#!/usr/bin/env bash
# Build PalmScanner for a connected iPhone, install + launch via devicectl.
# Usage: ./scripts/run_ios_device.sh [device-name-or-udid]
# Default: Bishal's iPhone 17 Pro (override: DEV="Name" ./scripts/run_ios_device.sh)
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DEV="${1:-${DEV:-00008150-001409290AF0401C}}"
echo "==> Devices:"; xcrun devicectl list devices 2>/dev/null | head -n 10 || true
echo "==> Target: ${DEV}"

xcodebuild -project apps/ios_palm_app/PalmScanner.xcodeproj \
  -scheme PalmScanner \
  -destination "platform=iOS,id=${DEV}" \
  -allowProvisioningUpdates \
  -derivedDataPath /tmp/PalmDerivedDevice build | tail -n 3

APP=/tmp/PalmDerivedDevice/Build/Products/Debug-iphoneos/PalmScanner.app
xcrun devicectl device install app --device "${DEV}" "${APP}"
xcrun devicectl device process launch --device "${DEV}" org.curv.palmapp
