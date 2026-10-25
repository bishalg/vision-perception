#!/usr/bin/env bash
# Generate iOS app screenshots from the BDD/Maestro flows on an iPhone simulator.
#
# Prerequisites: Xcode, Maestro (brew install maestro), a booted or available iPhone simulator.
# Usage:
#   ./scripts/run_maestro_screens.sh                 # build + run flows into maestro/screenshots/
#   OUT=/tmp/shots ./scripts/run_maestro_screens.sh  # custom output dir
#   ./scripts/run_maestro_screens.sh --copy-to ../path/to/site/public/ios-app
#
# Flow order matters: 01 creates app state (mode selection), 02 completes onboarding,
# 03 continues into the dashboard flows. Each flow is also runnable standalone:
#   maestro test apps/ios_asd_gaze_app/maestro/flows/01-mode-selection.yaml
set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCHEME="AsdGazeCollector"
BUNDLE="org.curv.asd.gazecollector"
DEVICE="${DEVICE:-iPhone 18 Pro}"
OUT="${OUT:-$APP_DIR/maestro/screenshots}"
APP_PATH="$APP_DIR/build-sim/Build/Products/Debug-iphonesimulator/AsdGazeCollector.app"

COPY_TO=""
while [ $# -gt 0 ]; do
  case "$1" in
    --copy-to) COPY_TO="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

command -v maestro >/dev/null || { echo "maestro is not installed (brew install maestro)" >&2; exit 1; }

echo "── Booting simulator: $DEVICE"
if ! xcrun simctl list devices | grep -q "($DEVICE) (Booted)"; then
  xcrun simctl boot "$DEVICE" 2>/dev/null || true
fi

echo "── Building for simulator"
xcodebuild -project "$APP_DIR/AsdGazeCollector.xcodeproj" \
  -scheme "$SCHEME" -configuration Debug -sdk iphonesimulator \
  -destination "platform=iOS Simulator,name=$DEVICE" \
  -derivedDataPath "$APP_DIR/build-sim" build \
  2>&1 | tail -3

echo "── Installing $BUNDLE"
xcrun simctl install "$DEVICE" "$APP_PATH"

echo "── Running BDD/Maestro flows"
mkdir -p "$OUT"
cd "$APP_DIR/maestro"
maestro test \
  flows/01-mode-selection.yaml \
  flows/02-onboarding.yaml \
  flows/03-dashboard-and-guardians.yaml \
  --flatten-debug-output \
  --test-output-dir "$OUT" \
  --no-ansi

echo "── Screenshots in: $OUT/screenshots"
ls -la "$OUT/screenshots" 2>/dev/null || ls -la "$OUT"

if [ -n "$COPY_TO" ]; then
  echo "── Copying curated screenshots to $COPY_TO"
  mkdir -p "$COPY_TO"
  # Curated for the website showcase; add or trim entries here.
  for f in mode-selection onboarding-privacy onboarding-consent child-profile \
           activities-download getting-ready dashboard mchat-survey settings; do
    [ -f "$OUT/screenshots/$f.png" ] && cp "$OUT/screenshots/$f.png" "$COPY_TO/$f.png"
  done
fi
