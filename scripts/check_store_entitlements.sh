#!/usr/bin/env bash
# S-22 (store-collection-build.md): the App Store build signs with a HealthKit-free
# entitlements file. Static gate, run by scripts/ios_test.sh and `fastlane ios release_check`.
#
# Asserts:
#   - apps/ios_asd_gaze_app/Resources/AsdGazeCollector-Store.entitlements exists and is a valid plist;
#   - it has no key containing "healthkit" (any case);
#   - it sets com.apple.developer.declared-age-range to true;
#   - project.yml maps the Release and StoreSim configurations' CODE_SIGN_ENTITLEMENTS to it,
#     and the generated project.pbxproj does too.
#
# Usage: scripts/check_store_entitlements.sh   (from any directory; exits non-zero on failure)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT/apps/ios_asd_gaze_app"
REL_PATH="Resources/AsdGazeCollector-Store.entitlements"
STORE_FILE="$APP_DIR/$REL_PATH"
SPEC="$APP_DIR/project.yml"
PBXPROJ="$APP_DIR/AsdGazeCollector.xcodeproj/project.pbxproj"
AGE_KEY="com.apple.developer.declared-age-range"

fail=0
err() { echo "check_store_entitlements: $*" >&2; fail=1; }

if [ ! -f "$STORE_FILE" ]; then
  err "missing $STORE_FILE (S-22: the store build signs with it)"
  exit 1
fi

if ! plutil -lint -s "$STORE_FILE"; then
  err "$REL_PATH is not a valid property list"
  exit 1
fi

if ! json="$(plutil -convert json -o - "$STORE_FILE" 2>/dev/null)"; then
  err "$REL_PATH must be an XML or binary plist dictionary of entitlements"
  exit 1
fi
if ! keys="$(printf '%s' "$json" | ruby -rjson -e 'd = JSON.parse(STDIN.read); abort unless d.is_a?(Hash); puts d.keys' 2>/dev/null)"; then
  err "$REL_PATH must be a plist dictionary of entitlements"
  exit 1
fi
if printf '%s\n' "$keys" | grep -qi 'healthkit'; then
  err "$REL_PATH must not carry a HealthKit entitlement; found: $(printf '%s\n' "$keys" | grep -i healthkit | tr '\n' ' ')"
fi

age_value="$(/usr/libexec/PlistBuddy -c "Print :$AGE_KEY" "$STORE_FILE" 2>/dev/null || true)"
if [ "$age_value" != "true" ]; then
  err "$REL_PATH must set $AGE_KEY to true (got: '${age_value:-<missing>}')"
fi

for config in Release StoreSim; do
  mapped="$(ruby -ryaml -e '
    spec = YAML.load_file(ARGV[0])
    value = spec.dig("targets", "AsdGazeCollector", "settings", "configs", ARGV[1], "CODE_SIGN_ENTITLEMENTS")
    puts value.to_s
  ' "$SPEC" "$config")"
  if [ "$mapped" != "$REL_PATH" ]; then
    err "project.yml: $config CODE_SIGN_ENTITLEMENTS is '${mapped:-<unset>}', expected '$REL_PATH'"
  fi
done

if [ -f "$PBXPROJ" ]; then
  count="$(grep -c "AsdGazeCollector-Store.entitlements" "$PBXPROJ" || true)"
  if [ "${count:-0}" -lt 2 ]; then
    err "project.pbxproj references $REL_PATH $count time(s), expected 2 (Release, StoreSim); run xcodegen generate"
  fi
fi

if [ "$fail" -ne 0 ]; then
  echo "check_store_entitlements: FAILED" >&2
  exit 1
fi
echo "check_store_entitlements: OK ($REL_PATH: no HealthKit, $AGE_KEY = true; Release and StoreSim use it)"
