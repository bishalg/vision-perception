#!/usr/bin/env bash
# Local metadata checks only; this is not an App Store submission certification.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

for app in ios_palm_app ios_asd_gaze_app; do
    printf '\nValidating local metadata: %s\n' "$app"
    (cd "$ROOT_DIR/apps/$app/fastlane" && fastlane ios validate_metadata)
    printf 'Local metadata validation passed: %s\n' "$app"
done

printf '\nMetadata checks passed only. Not checked: builds/signing, privacy declarations,\n'
printf 'real screenshots, device QA, TestFlight, agreements, or Apple review status.\n'
