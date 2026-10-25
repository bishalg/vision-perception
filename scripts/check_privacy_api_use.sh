#!/usr/bin/env bash
# PRV-2: every privacy manifest lists exactly the required-reason API categories
# its target's sources use.
#
# Fails when a category used in a target's sources is missing from its
# PrivacyInfo.xcprivacy, when a manifest declares a category the sources do not
# use, or when a target without a manifest uses any required-reason API.
#
# Usage: scripts/check_privacy_api_use.sh [repo root]
# Exit status: 0 clean, 1 mismatch, 2 setup error.
set -euo pipefail

root="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
app="apps/ios_asd_gaze_app"
kit="domains/asd_screening/ios/Sources/CurvAsdGazeKit"

# target name | source directory | manifest ("-" = the target ships no manifest)
targets=(
  "AsdGazeCollector|$app/Sources|$app/Resources/PrivacyInfo.xcprivacy"
  "CurvAsdGazeKit|$kit|$kit/Resources/PrivacyInfo.xcprivacy"
  "AsdGazeCareWidget|$app/Widget|-"
  "CareActivityKit|$app/CareActivityKit/Sources|-"
)

# Apple required-reason API categories and the symbols that put a target in them
# (https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api).
categories=(
  "NSPrivacyAccessedAPICategoryUserDefaults|\\bUserDefaults\\b|NSUserDefaults|@AppStorage\\b|NSUbiquitousKeyValueStore"
  "NSPrivacyAccessedAPICategoryFileTimestamp|\\.creationDate\\b|\\.modificationDate\\b|NSFileCreationDate|NSFileModificationDate|contentModificationDate|creationDateKey|fileModificationDate|\\bgetattrlist(bulk|at)?\\b|\\bfgetattrlist\\b|\\b(f|l)?stat(at)?\\("
  "NSPrivacyAccessedAPICategorySystemBootTime|\\bsystemUptime\\b|\\bmach_absolute_time\\b"
  "NSPrivacyAccessedAPICategoryDiskSpace|volumeAvailableCapacity|volumeTotalCapacity|NSFileSystemFreeSize|NSFileSystemSize|systemFreeSize|systemSize\\b|\\bstatv?fs\\b"
  "NSPrivacyAccessedAPICategoryActiveKeyboards|activeInputModes"
)

# Source lines with comments removed, as "file:line:text".
code_lines() {
  local dir="$1"
  find "$dir" -type f \( -name '*.swift' -o -name '*.m' -o -name '*.mm' -o -name '*.c' -o -name '*.h' \) -print0 |
    xargs -0 grep -nH '' 2>/dev/null |
    sed -E 's#^([^:]*:[0-9]+:)[[:space:]]*(///?|\*|/\*).*$#\1#; s#([^:])//[^"]*$#\1#' || true
}

fail=0
for entry in "${targets[@]}"; do
  IFS='|' read -r name src manifest <<<"$entry"
  if [ ! -d "$root/$src" ]; then
    echo "setup error: $name sources not found at $src" >&2
    exit 2
  fi
  code="$(code_lines "$root/$src")"

  used=()
  for cat_entry in "${categories[@]}"; do
    category="${cat_entry%%|*}"
    pattern="${cat_entry#*|}"
    hits="$(printf '%s\n' "$code" | grep -E "^[^:]*:[0-9]+:.*($pattern)" || true)"
    if [ -n "$hits" ]; then
      used+=("$category")
      first="$(printf '%s\n' "$hits" | head -1 | sed "s#^$root/##" | cut -c1-160)"
      echo "$name uses $category (e.g. $first)"
    fi
  done

  declared=()
  if [ "$manifest" != "-" ]; then
    if [ ! -f "$root/$manifest" ]; then
      echo "FAIL $name: manifest missing at $manifest" >&2
      fail=1
      continue
    fi
    if command -v plutil >/dev/null 2>&1 && ! plutil -lint -s "$root/$manifest"; then
      echo "FAIL $name: $manifest is not a valid property list" >&2
      fail=1
    fi
    while IFS= read -r category; do
      [ -n "$category" ] && declared+=("$category")
    done < <(grep -oE 'NSPrivacyAccessedAPICategory[A-Za-z]+' "$root/$manifest" | sort -u)
  fi

  for category in ${used[@]+"${used[@]}"}; do
    if [ "$manifest" = "-" ]; then
      echo "FAIL $name: uses $category but ships no PrivacyInfo.xcprivacy" >&2
      fail=1
    elif ! printf '%s\n' ${declared[@]+"${declared[@]}"} | grep -qxF "$category"; then
      echo "FAIL $name: uses $category but $manifest does not declare it" >&2
      fail=1
    fi
  done
  for category in ${declared[@]+"${declared[@]}"}; do
    if ! printf '%s\n' ${used[@]+"${used[@]}"} | grep -qxF "$category"; then
      echo "FAIL $name: $manifest declares $category but no source uses it" >&2
      fail=1
    fi
  done
done

if [ "$fail" -eq 0 ]; then
  echo "privacy API use ok"
fi
exit "$fail"
