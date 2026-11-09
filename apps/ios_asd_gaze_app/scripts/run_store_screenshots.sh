#!/usr/bin/env bash
# App Store screenshots from the store build (WP-S.7), produced by the Maestro store flows.
#
# Builds the StoreSim configuration (ASD_STORE_BUILD + simulation, store UI on a simulator),
# then for each device and language: boots the simulator, sets the system language and region,
# installs the app, runs maestro/flows/store/01..04 in order and copies the screenshots into
# fastlane's layout:
#
#   fastlane/screenshots/<en-US|hi|ne-NP>/<Device Name>-NN_name.png
#
# Maestro output (screenshots, logs, failure captures) stays under
# maestro/screenshots/<device>/<lang>/<flow>/ (git-ignored) so a run can be inspected later.
# The Gherkin specs for the same flows are Tests/bdd/features/store_*.feature.
#
# Usage:
#   scripts/run_store_screenshots.sh                       # both devices, en-US (S-14)
#   scripts/run_store_screenshots.sh --device "iPhone 18 Pro Max" --lang en-US
#   scripts/run_store_screenshots.sh --skip-build --lang hi --lang ne-NP   # parked locales
#
# Options (repeatable): --device NAME, --lang en-US|hi|ne-NP (default en-US). --skip-build reuses the last
# StoreSim build. Environment: DERIVED_DATA (default /private/tmp/asd-storesim-dd), MAESTRO.
# Any failing step or flow stops the run with a non-zero exit.
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCHEME="AsdGazeCollector"
BUNDLE="com.open-spectrum.eye-gaze"
CONFIGURATION="StoreSim"
DERIVED_DATA="${DERIVED_DATA:-/private/tmp/asd-storesim-dd}"
APP_PATH="$DERIVED_DATA/Build/Products/${CONFIGURATION}-iphonesimulator/AsdGazeCollector.app"
FLOW_DIR="$APP_DIR/maestro/flows/store"
FLOWS=(01-consent-and-onboarding 02-capture-entry 03-session-and-sessions 04-privacy-and-settings)
SHOTS=(01_consent_overview 02_consent_sign 03_before_you_begin 04_camera_primer
       05_recording_quality 06_sessions 07_privacy_your_data 08_settings_about)
DEBUG_ROOT="$APP_DIR/maestro/screenshots"
FASTLANE_SHOTS="$APP_DIR/fastlane/screenshots"

DEVICES=()
LANGS=()
SKIP_BUILD=0
while [ $# -gt 0 ]; do
  case "$1" in
    --device) DEVICES+=("${2:?--device needs a simulator name}"); shift 2 ;;
    --lang) LANGS+=("${2:?--lang needs en-US, hi or ne-NP}"); shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h|--help) sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done
[ ${#DEVICES[@]} -gt 0 ] || DEVICES=("iPhone 18 Pro Max" "iPad Pro 13-inch (M5)")
# S-14: the first submission lists en-US only; pass --lang hi / --lang ne-NP for the others.
[ ${#LANGS[@]} -gt 0 ] || LANGS=(en-US)

die() { echo "ERROR: $*" >&2; exit 1; }
step() { printf '\n== %s ==\n' "$*"; }

# Maestro: $MAESTRO, then PATH, then the installer's default location.
MAESTRO="${MAESTRO:-$(command -v maestro || true)}"
if [ -z "$MAESTRO" ]; then
  for candidate in "$HOME/.maestro/bin/maestro" "/Users/$(id -un)/.maestro/bin/maestro"; do
    if [ -x "$candidate" ]; then MAESTRO="$candidate"; break; fi
  done
fi
[ -n "$MAESTRO" ] && [ -x "$MAESTRO" ] || die "maestro not found (install it or set MAESTRO=/path/to/maestro)"

# System language and region per fastlane locale folder.
apple_language() {
  case "$1" in
    en-US) echo "en-US" ;;
    hi) echo "hi-IN" ;;
    ne-NP) echo "ne-NP" ;;
    *) die "unsupported language: $1 (expected en-US, hi or ne-NP)" ;;
  esac
}
apple_locale() {
  case "$1" in
    en-US) echo "en_US" ;;
    hi) echo "hi_IN" ;;
    ne-NP) echo "ne_NP" ;;
    *) die "unsupported language: $1" ;;
  esac
}

# UDID of an available simulator with exactly this name ("iPhone 18 Pro" never matches "... Max").
device_udid() {
  xcrun simctl list devices available \
    | grep -F "    $1 (" \
    | head -n 1 \
    | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/'
}

boot_device() {
  local udid="$1"
  xcrun simctl boot "$udid" 2>/dev/null || true
  xcrun simctl bootstatus "$udid" -b >/dev/null
}

for lang in "${LANGS[@]}"; do apple_language "$lang" >/dev/null; done
for name in "${DEVICES[@]}"; do
  [ -n "$(device_udid "$name")" ] || die "simulator not installed: $name (see: xcrun simctl list devices available)"
done

if [ "$SKIP_BUILD" -eq 0 ]; then
  first_udid="$(device_udid "${DEVICES[0]}")"
  step "Build $CONFIGURATION for ${DEVICES[0]} ($first_udid)"
  # A concrete simulator destination: the Rust static library is arm64-only, so
  # generic/platform=iOS Simulator (which adds x86_64) does not link.
  log="$DERIVED_DATA-build.log"
  mkdir -p "$(dirname "$log")"
  if ! xcodebuild -project "$APP_DIR/AsdGazeCollector.xcodeproj" \
      -scheme "$SCHEME" -configuration "$CONFIGURATION" -sdk iphonesimulator \
      -destination "platform=iOS Simulator,id=$first_udid" \
      -derivedDataPath "$DERIVED_DATA" build >"$log" 2>&1; then
    grep -E "error:" "$log" | head -20 >&2 || true
    die "StoreSim build failed (full log: $log)"
  fi
  echo "build ok (log: $log)"
fi
[ -d "$APP_PATH" ] || die "no StoreSim app at $APP_PATH (run without --skip-build)"

for name in "${DEVICES[@]}"; do
  udid="$(device_udid "$name")"
  for lang in "${LANGS[@]}"; do
    step "$name / $lang"
    boot_device "$udid"
    # System language and region; a reboot makes every process read them fresh.
    xcrun simctl spawn "$udid" defaults write .GlobalPreferences AppleLanguages -array "$(apple_language "$lang")"
    xcrun simctl spawn "$udid" defaults write .GlobalPreferences AppleLocale -string "$(apple_locale "$lang")"
    xcrun simctl shutdown "$udid"
    boot_device "$udid"
    # Clean status bar for store screenshots.
    xcrun simctl status_bar "$udid" override --time "9:41" --dataNetwork wifi --wifiMode active \
      --wifiBars 3 --cellularMode active --cellularBars 4 --batteryState charged --batteryLevel 100 \
      >/dev/null 2>&1 || true

    xcrun simctl uninstall "$udid" "$BUNDLE" >/dev/null 2>&1 || true
    xcrun simctl install "$udid" "$APP_PATH"

    run_dir="$DEBUG_ROOT/$name/$lang"
    rm -rf "$run_dir"
    mkdir -p "$run_dir"
    for flow in "${FLOWS[@]}"; do
      echo "-- flow $flow"
      flow_dir="$run_dir/$flow"
      mkdir -p "$flow_dir"
      # takeScreenshot writes into the working directory; the debug output goes next to it.
      if ! (cd "$flow_dir" && "$MAESTRO" --device "$udid" test "$FLOW_DIR/$flow.yaml" \
              --test-output-dir "$flow_dir" --debug-output "$flow_dir/debug" \
              --flatten-debug-output --no-ansi); then
        die "flow $flow failed on $name / $lang (Maestro output: $flow_dir)"
      fi
    done

    dest="$FASTLANE_SHOTS/$lang"
    mkdir -p "$dest"
    rm -f "$dest/$name-"*.png
    for shot in "${SHOTS[@]}"; do
      src="$(find "$run_dir" -name "$shot.png" -print -quit)"
      [ -n "$src" ] || die "screenshot $shot.png missing for $name / $lang (see $run_dir)"
      cp "$src" "$dest/$name-$shot.png"
      size="$(sips -g pixelWidth -g pixelHeight "$dest/$name-$shot.png" | awk '/pixel/ {print $2}' | paste -sd x -)"
      echo "   $lang/$name-$shot.png  $size"
    done
    xcrun simctl status_bar "$udid" clear >/dev/null 2>&1 || true
  done
done

step "Done: screenshots in $FASTLANE_SHOTS, Maestro output in $DEBUG_ROOT"
