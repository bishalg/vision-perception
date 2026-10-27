#!/usr/bin/env bash
# Self-test for scripts/ios_test.sh: --dry-run must list every gate and run nothing.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script="$here/ios_test.sh"
bash -n "$script"
out="$(DERIVED_DATA=/private/tmp/asd-selftest-dd bash "$script" --dry-run)"
fail=0
for needle in \
  "swift test --package-path" "--scratch-path /private/tmp/asd-selftest-dd-kit " \
  "--sanitize=thread" "-strict-concurrency=complete" "-warnings-as-errors" \
  "xcodegen generate --spec apps/ios_asd_gaze_app/project.yml" \
  "xcodebuild test" "-only-testing:AsdGazeCollectorTests" "platform=iOS Simulator,name=" \
  "-derivedDataPath /private/tmp/asd-selftest-dd " \
  "-configuration Release" "generic/platform=iOS" "-derivedDataPath /private/tmp/asd-selftest-dd-rel" \
  "scripts/check_asd_claims.py --swift --secrets" \
  "scripts/check_privacy_api_use.sh" \
  "scripts/check_release_symbols.sh /private/tmp/asd-selftest-dd-rel/Build/Products/Release-iphoneos/AsdGazeCollector.app/AsdGazeCollector"; do
  grep -qF -- "$needle" <<<"$out" || { echo "dry-run missing: $needle" >&2; fail=1; }
done
[ -e /private/tmp/asd-selftest-dd-kit ] && { echo "dry-run created files" >&2; fail=1; }
if bash "$script" --bogus >/dev/null 2>&1; then echo "bogus flag accepted" >&2; fail=1; fi
if DERIVED_DATA="$here/../dd" bash "$script" --dry-run >/dev/null 2>&1; then echo "in-repo DERIVED_DATA accepted" >&2; fail=1; fi
[ "$fail" -eq 0 ] && echo "test_ios_test_sh ok"
exit "$fail"
