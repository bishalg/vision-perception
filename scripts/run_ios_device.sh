#!/usr/bin/env bash
# Build PalmScanner for a connected iPhone, install + launch via devicectl.
# Usage: ./scripts/run_ios_device.sh [device-name-or-udid]
# Default: Bishal's iPhone 17 Pro (override: DEV="Name" ./scripts/run_ios_device.sh)
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DEFAULT_DEV="00008150-001409290AF0401C"
DEV="${1:-${DEV:-${DEFAULT_DEV}}}"

echo "==> Connected Devices:"
xcrun devicectl list devices 2>/dev/null || true

# Inspect device status via devicectl json
DEV_INFO=$(python3 -c '
import json, subprocess, sys
target = sys.argv[1]
try:
    raw = subprocess.check_output(["xcrun", "devicectl", "list", "devices", "--json-output", "/dev/stdout"], stderr=subprocess.DEVNULL)
    data = json.loads(raw)
    for d in data.get("result", {}).get("devices", []):
        hw = d.get("hardwareProperties") or {}
        dev_p = d.get("deviceProperties") or {}
        conn = d.get("connectionProperties") or {}
        props = d.get("properties") or {}
        name = dev_p.get("name") or hw.get("marketingName") or ""
        udid = hw.get("udid") or d.get("identifier") or ""
        state = conn.get("tunnelState") or props.get("connection", {}).get("state") or "unknown"
        if target in (name, udid):
            print(f"{name}|{udid}|{state}")
            sys.exit(0)
except Exception:
    pass
' "${DEV}" 2>/dev/null || true)

DEV_NAME="${DEV}"
DEV_STATE="unknown"
if [[ -n "${DEV_INFO}" ]]; then
  IFS='|' read -r D_NAME D_UDID D_STATE <<< "${DEV_INFO}"
  DEV_NAME="${D_NAME}"
  DEV="${D_UDID}"
  DEV_STATE="${D_STATE}"
fi

echo ""
echo "==> Selected Device: ${DEV_NAME} (${DEV})"
echo "==> Status: ${DEV_STATE}"

if [[ "${DEV_STATE}" == "unavailable" ]]; then
  echo ""
  echo "⚠️  WARNING: Device '${DEV_NAME}' is currently in state 'unavailable'."
  echo "   Common reasons:"
  echo "   1. iPhone screen is locked (unlock the iPhone with Face ID / Passcode)."
  echo "   2. 'Trust This Computer' prompt has not been accepted on the device."
  echo "   3. Cable or Wi-Fi sync tunnel is disconnected / sleeping."
  echo "   4. Developer Mode is not enabled in Settings -> Privacy & Security -> Developer Mode."
  echo ""
fi

echo "==> Building and deploying to ${DEV}..."

xcodebuild -project apps/ios_palm_app/PalmScanner.xcodeproj \
  -scheme PalmScanner \
  -destination "platform=iOS,id=${DEV}" \
  -allowProvisioningUpdates \
  -derivedDataPath /tmp/PalmDerivedDevice build | tail -n 3

APP=/tmp/PalmDerivedDevice/Build/Products/Debug-iphoneos/PalmScanner.app
# Same static-link strip as run_ios_sim.sh (Xcode re-embeds after our phase).
rm -rf "${APP}/Frameworks/opencv2.framework"
# Device installs need a real signature (ad-hoc won't install). Reuse the
# first Apple Development / iPhone Developer identity in the login keychain.
IDENT=$(security find-identity -v -p codesigning 2>/dev/null | grep -m1 -oE '[A-F0-9]{40}' || true)
if [[ -z "${IDENT}" ]]; then
  echo "ERROR: no codesigning identity found — open Xcode once to fetch provisioning." >&2
  exit 1
fi
codesign --force --sign "${IDENT}" --timestamp=none "${APP}"
xcrun devicectl device install app --device "${DEV}" "${APP}"
xcrun devicectl device process launch --device "${DEV}" org.curv.palmapp
