#!/usr/bin/env python3
"""Validate the shared palm strings used by both mobile apps.

Single source of truth: apps/mobile_overlay/shared/strings/palm_strings.json
Checks:
  1. The JSON parses and every key defines non-empty en/ne/hi values.
  2. Every `tr("some.key")` literal referenced in the iOS (Sources/) and
     Android (palmapp/) code exists in the JSON — a missing key would silently
     render as the raw key at runtime.
Unused keys are reported as warnings (exit 0); schema/reference errors fail.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STRINGS_JSON = ROOT / "apps/mobile_overlay/shared/strings/palm_strings.json"
IOS_SOURCES = ROOT / "apps/ios_palm_app/Sources"
ANDROID_SOURCES = (
    ROOT / "apps/android_palm_app/app/src/main/java/org/curv/palmapp"
)
LANGS = ("en", "ne", "hi")
TR_LITERAL = re.compile(r'\btr\(\s*"([^"]+)"')


def main() -> int:
    try:
        table = json.loads(STRINGS_JSON.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"ERROR: cannot load {STRINGS_JSON}: {exc}")
        return 1
    if not isinstance(table, dict) or not table:
        print("ERROR: strings JSON must be a non-empty object")
        return 1

    errors = 0
    for key, langs in table.items():
        if not isinstance(langs, dict) or set(langs) != set(LANGS):
            print(f"ERROR: key {key!r} must define exactly {LANGS}")
            errors += 1
            continue
        for lang in LANGS:
            if not isinstance(langs[lang], str) or not langs[lang].strip():
                print(f"ERROR: key {key!r} has empty {lang!r} value")
                errors += 1

    referenced: dict[str, list[str]] = {}
    for base in (IOS_SOURCES, ANDROID_SOURCES):
        if not base.is_dir():
            print(f"ERROR: missing source dir {base}")
            errors += 1
            continue
        for path in sorted(base.rglob("*")):
            if path.suffix not in (".swift", ".kt"):
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except OSError as exc:
                print(f"ERROR: cannot read {path}: {exc}")
                errors += 1
                continue
            for key in TR_LITERAL.findall(text):
                # Skip dynamic lookups (displayNameKey is resolved via line.* keys).
                referenced.setdefault(key, []).append(
                    str(path.relative_to(ROOT))
                )

    for key in sorted(referenced):
        if key not in table:
            files = ", ".join(sorted(set(referenced[key])))
            print(f"ERROR: key {key!r} referenced in {files} but missing from JSON")
            errors += 1

    # `line.*` keys resolve dynamically via CreaseLineType.displayNameKey on both
    # platforms — count them referenced when the convention is present in code.
    convention_roots = []
    for base, marker in (
        (IOS_SOURCES, "displayNameKey"),
        (ANDROID_SOURCES, "displayNameKey"),
    ):
        hits = [
            p for p in base.rglob("*")
            if p.suffix in (".swift", ".kt") and marker in p.read_text(encoding="utf-8")
        ]
        if hits:
            convention_roots.append(base)
    if len(convention_roots) == 2:
        for key in table:
            if key.startswith("line."):
                referenced.setdefault(key, ["<displayNameKey convention>"])

    unused = sorted(set(table) - set(referenced))
    if unused:
        print(f"WARNING: {len(unused)} unused key(s): {', '.join(unused)}")

    print(
        f"checked {len(table)} keys, {len(referenced)} referenced "
        f"({errors} error(s))"
    )
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
