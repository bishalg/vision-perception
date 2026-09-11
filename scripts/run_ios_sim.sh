#!/usr/bin/env bash
# Boot the first available iPhone simulator, build PalmScanner, install + launch.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

UDID=$(xcrun simctl list devices available | grep -oE 'iPhone [^(]*\([A-Z0-9-]{36}\)' | head -n 1 | grep -oE '[A-Z0-9-]{36}')
if [[ -z "${UDID}" ]]; then
  echo "No available iPhone simulator found." >&2
  exit 1
fi
echo "==> Simulator: ${UDID}"
xcrun simctl boot "${UDID}" 2>/dev/null || true
xcrun simctl bootstatus "${UDID}" -b

xcodebuild -project apps/ios_palm_app/PalmScanner.xcodeproj \
  -scheme PalmScanner \
  -destination "platform=iOS Simulator,id=${UDID}" \
  -derivedDataPath /tmp/PalmSim build | tail -n 3

APP=/tmp/PalmSim/Build/Products/Debug-iphonesimulator/PalmScanner.app
xcrun simctl install "${UDID}" "${APP}"
xcrun simctl launch "${UDID}" org.curv.palmapp
