#!/usr/bin/env bash
# Full iOS gate for the ASD gaze app and its kit (qa-validation.md, CI-1, CI-2).
#
# Usage: scripts/ios_test.sh [--dry-run] [-h|--help]
#
# Steps: kit tests, kit TSAN, kit strict-concurrency build, xcodegen, app tests
# on an iPhone simulator, unsigned Release build, then static gates (claims,
# privacy API audit, Release-binary symbol check).
#
# Environment:
#   DERIVED_DATA  derived-data root, never inside the repo (default /private/tmp/asd-app-dd).
#                 Kit scratch paths are "${DERIVED_DATA}-kit" and "${DERIVED_DATA}-kit-tsan";
#                 the Release build uses "${DERIVED_DATA}-rel".
#   SIM_NAME      simulator name (default "iPhone 18 Pro"; must exist, see
#                 `xcrun simctl list devices available`)
#   SIM_DESTINATION  full xcodebuild destination, overrides SIM_NAME
#
# --dry-run prints each command without running it.
set -euo pipefail

DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    -h|--help) sed -n '2,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $arg (try --help)" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DERIVED_DATA="${DERIVED_DATA:-/private/tmp/asd-app-dd}"
SIM_NAME="${SIM_NAME:-iPhone 18 Pro}"
SIM_DESTINATION="${SIM_DESTINATION:-platform=iOS Simulator,name=${SIM_NAME}}"
KIT_DIR="$ROOT/domains/asd_screening/ios"
APP_DIR="apps/ios_asd_gaze_app"
KIT_SCRATCH="${DERIVED_DATA}-kit"
KIT_TSAN_SCRATCH="${DERIVED_DATA}-kit-tsan"
REL_DD="${DERIVED_DATA}-rel"

case "$DERIVED_DATA" in
  "$ROOT"/*) echo "DERIVED_DATA must not be inside the repository: $DERIVED_DATA" >&2; exit 2 ;;
esac

step() { printf '\n== %s ==\n' "$1"; }
run() {
  printf '+ %s\n' "$*"
  if [ "$DRY_RUN" -eq 0 ]; then "$@"; fi
}

step "Kit tests"
run swift test --package-path "$KIT_DIR" --scratch-path "$KIT_SCRATCH"

step "Kit tests (ThreadSanitizer)"
run swift test --package-path "$KIT_DIR" --scratch-path "$KIT_TSAN_SCRATCH" --sanitize=thread

step "Kit strict concurrency, warnings as errors"
run swift build --package-path "$KIT_DIR" --scratch-path "$KIT_SCRATCH" \
  -Xswiftc -strict-concurrency=complete -Xswiftc -warnings-as-errors

cd "$ROOT"

step "XcodeGen"
run xcodegen generate --spec "$APP_DIR/project.yml"

step "App tests (simulator)"
run xcodebuild test \
  -project "$APP_DIR/AsdGazeCollector.xcodeproj" \
  -scheme AsdGazeCollector \
  -destination "$SIM_DESTINATION" \
  -derivedDataPath "$DERIVED_DATA" \
  -parallel-testing-enabled NO \
  -only-testing:AsdGazeCollectorTests \
  CODE_SIGNING_ALLOWED=NO

step "Unsigned Release build"
run xcodebuild build \
  -project "$APP_DIR/AsdGazeCollector.xcodeproj" \
  -scheme AsdGazeCollector -configuration Release \
  -destination 'generic/platform=iOS' \
  -derivedDataPath "$REL_DD" \
  CODE_SIGNING_ALLOWED=NO

step "Static gate: claims and secrets"
run python3 scripts/check_asd_claims.py --swift --secrets

step "Static gate: privacy API use"
run bash scripts/check_privacy_api_use.sh

step "Static gate: Release binary symbols"
run bash scripts/check_release_symbols.sh \
  "$REL_DD/Build/Products/Release-iphoneos/AsdGazeCollector.app/AsdGazeCollector"

step "ios_test.sh: all gates passed"
