#!/usr/bin/env python3
"""Check iOS/Android parity for the palm reader apps.

The two apps are developed iOS-first and mirrored on Android in parallel
(see docs/mobile_parity.md). This script locks the machine-checkable parts
of that contract so a fix landing on one side cannot silently miss the other.

Contract violations are ERRORS (exit 1). Behavioural drift that may be
legitimate is a WARNING (exit 0) — confirm it, then either port it or record
it in docs/mobile_parity.md under "Documented divergences".

Checks:
  1. line_types      — CreaseLineType identity (name/hex/displayKey) matches.
  2. editor_tools    — editor.tool* key sets match in both editor files.
  3. editor_coaching — editor.coaching* key sets match in both editor files.
  4. languages       — AppLanguage codes + display names match.
  5. handside        — HandSide cases match.
  6. files           — every palm app/overlay file has a counterpart
                       (modulo ALIASES; INLINE_COMPONENTS are verified to
                       exist inside the mapped file).
  7. screen_strings  — per-screen tr() key sets match for the three screen
                       pairs (modulo EXCEPTIONS).

Usage: ./scripts/check_mobile_parity.py
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IOS_APP = ROOT / "apps/ios_palm_app/Sources"
ANDROID_APP = ROOT / "apps/android_palm_app/app/src/main/java/org/curv/palmapp"
IOS_OVERLAY = ROOT / "apps/mobile_overlay/ios/Sources"
ANDROID_OVERLAY = ROOT / "apps/mobile_overlay/android/src/main/java/org/curv/overlay"

IOS_EDITOR = IOS_APP / "Views/CreaseLineEditorView.swift"
ANDROID_EDITOR = (
    ANDROID_APP / "view/CreaseLineEditorScreen.kt"
)
IOS_MODEL = IOS_APP / "Models/PalmAnalysisResult.swift"
ANDROID_MODEL = ANDROID_APP / "model/PalmAnalysisResult.kt"
IOS_STRINGS = IOS_APP / "Models/LocalizedStrings.swift"
ANDROID_STRINGS = ANDROID_APP / "model/LocalizedStrings.kt"
IOS_HAND = IOS_OVERLAY / "CurvPalmOverlay/HandSide.swift"
ANDROID_HAND = ANDROID_OVERLAY / "palm/HandSide.kt"

# Screen pairs whose tr() key sets must match.
SCREEN_PAIRS = [
    (IOS_APP / "Views/MainScannerView.swift", ANDROID_APP / "view/MainScannerScreen.kt"),
    (IOS_APP / "Views/CaptureResultView.swift", ANDROID_APP / "view/CaptureResultView.kt"),
    (IOS_APP / "Views/SamudrikaReadingSheet.swift", ANDROID_APP / "view/ReadingBottomSheet.kt"),
]

# Platform-idiomatic divergences: same UX, different native expression.
# Format: (key, reason).
EXCEPTIONS = {
    ("editor.cancel", "iOS uses a Cancel text button in the toolbar; "
     "Android uses a Close icon button in the TopAppBar (platform idiom)."),
}

# Intentional cross-platform renames: ios_stem -> android_stem.
ALIASES = {
    "PalmOverlayProfile": "PalmOverlay",
    "DeviceTiltProvider": "DeviceTiltMonitor",
    "VastuMandalaView": "VastuMandala",
    "VastuOverlayProfile": "VastuOverlay",
    "CameraPreviewView": "CameraPreview",
    "HandSideToggleView": "HandSideToggle",
    "PalmSilhouetteView": "PalmSilhouette",
    "TiltRailView": "TiltRail",
    "SamudrikaReadingSheet": "ReadingBottomSheet",
    "CreaseLineEditorView": "CreaseLineEditorScreen",
    "MainScannerView": "MainScannerScreen",
    "VisionHandTracker": "MLKitHandTracker",
}

# Components implemented inline instead of a separate file.
# Format: component_stem -> (host_file, needle_that_must_appear_in_it).
# The needle check fails the run if the inline implementation disappears.
INLINE_IOS_COMPONENTS = {
    # iOS-only file; Android implements the loupe inline in its editor.
    "MagnifyingLoupeView": (ANDROID_APP / "view/CreaseLineEditorScreen.kt", "loupe"),
}
INLINE_ANDROID_COMPONENTS = {
    # Android-only file; iOS defines it inside its result view.
    "HandOutlineOverlay": (IOS_APP / "Views/CaptureResultView.swift", "HandOutlineOverlay"),
}

# Overlay files that are platform-specific by nature (no counterpart needed).
OVERLAY_EXCLUDE = set()

TR_LITERAL = re.compile(r'tr\(\s*"([^"]+)"')


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def tr_keys(path: Path):
    return set(TR_LITERAL.findall(read(path)))


def check_line_types(errors):
    ios = read(IOS_MODEL)
    # case .heart: return "#FF2D55" ... displayNameKey switch nearby:
    #   case .heart: return "line.heart"
    hexes = dict(re.findall(r'case \.(\w+): return "(#[0-9A-Fa-f]{6})"', ios))
    names = dict(re.findall(r'case \.(\w+): return "(line\.\w+)"', ios))
    android = read(ANDROID_MODEL)
    # HEART("heart", "#FF2D55", "line.heart")
    entries = re.findall(r'(\w+)\("(\w+)", "(#[0-9A-Fa-f]{6})", "(line\.\w+)"\)', android)
    ios_set = {(k, hexes.get(k, "").upper(), names.get(k, "")) for k in hexes}
    android_set = {(n.lower(), h.upper(), d) for _, n, h, d in entries}
    # normalize iOS case names to lowercase line names
    ios_norm = set()
    for case, hx, disp in ios_set:
        line = disp.split(".", 1)[1] if disp.startswith("line.") else case
        ios_norm.add((line, hx, disp))
    if ios_norm != android_set:
        errors.append(
            "line_types: mismatch\n"
            f"  iOS:     {sorted(ios_norm)}\n"
            f"  Android: {sorted(android_set)}"
        )
    else:
        print(f"line_types: OK ({len(android_set)} lines, colors + display keys match)")


def check_key_sets(errors, warnings, label, pattern, ios_path, android_path, strict=True):
    ios_keys = {k for k in tr_keys(ios_path) if k.startswith(pattern)}
    android_keys = {k for k in tr_keys(android_path) if k.startswith(pattern)}
    if ios_keys == android_keys:
        print(f"{label}: OK ({len(ios_keys)} keys: {sorted(ios_keys)})")
        return
    only_ios = sorted(ios_keys - android_keys)
    only_android = sorted(android_keys - ios_keys)
    msg = f"{label}: iOS-only={only_ios} Android-only={only_android}"
    (errors if strict else warnings).append(msg)


def check_languages(errors):
    ios = read(IOS_STRINGS)
    ios_codes = dict(re.findall(r'case (\w+) = "(\w+)"', ios))
    ios_names = dict(re.findall(r'case \.(\w+): return "([^"]+)"', ios))
    android = read(ANDROID_STRINGS)
    android_entries = re.findall(r'(\w+)\("(\w+)", "([^"]+)"\)', android)
    ios_norm = {code: (case.lower(), ios_names.get(case, "")) for case, code in ios_codes.items()}
    android_norm = {code: (case.lower(), name) for case, code, name in android_entries}
    if ios_norm != android_norm:
        errors.append(
            f"languages: mismatch\n  iOS: {ios_norm}\n  Android: {android_norm}"
        )
    else:
        print(f"languages: OK ({sorted(ios_norm)})")


def check_handside(errors):
    ios = set(re.findall(r'case (auto|left|right)\b', read(IOS_HAND)))
    android = {c.lower() for c in re.findall(r'\b(AUTO|LEFT|RIGHT)\b', read(ANDROID_HAND))}
    if ios != android:
        errors.append(f"handside: iOS={sorted(ios)} Android={sorted(android)}")
    else:
        print(f"handside: OK ({sorted(ios)})")


def stem(path: Path) -> str:
    return path.stem


def check_files(errors, warnings):
    def collect(root: Path, exts):
        return {p for p in root.rglob("*") if p.suffix in exts and p.is_file()}

    ios_files = collect(IOS_APP, {".swift"}) | collect(IOS_OVERLAY, {".swift"})
    android_files = collect(ANDROID_APP, {".kt"}) | collect(ANDROID_OVERLAY, {".kt"})
    # Skip tests.
    ios_files = {p for p in ios_files if "Tests" not in p.parts}
    android_names = {stem(p) for p in android_files}

    problems = []
    for p in sorted(ios_files):
        name = stem(p)
        if name in OVERLAY_EXCLUDE:
            continue
        if name in android_names:
            continue
        if name in ALIASES and ALIASES[name] in android_names:
            continue
        if name in INLINE_IOS_COMPONENTS:
            mapped, needle = INLINE_IOS_COMPONENTS[name]
            if not mapped.is_file() or needle not in read(mapped).lower():
                problems.append(
                    f"{name}: mapped inline file {mapped.relative_to(ROOT)} "
                    "no longer contains the component"
                )
            else:
                print(f"files: OK ({name} implemented inline in {mapped.name})")
            continue
        problems.append(f"{name}: no Android counterpart ({p.relative_to(ROOT)})")

    # Reverse: Android files with no iOS counterpart.
    ios_names = {stem(p) for p in ios_files}
    reverse_aliases = {v: k for k, v in ALIASES.items()}
    inline_targets = {stem(p) for p, _ in INLINE_IOS_COMPONENTS.values()}
    for p in sorted(android_files):
        name = stem(p)
        if name in ios_names or name in reverse_aliases or name in inline_targets:
            continue
        if name in INLINE_ANDROID_COMPONENTS:
            mapped, needle = INLINE_ANDROID_COMPONENTS[name]
            if not mapped.is_file() or needle not in read(mapped):
                problems.append(
                    f"{name}: mapped inline file {mapped.relative_to(ROOT)} "
                    "no longer contains the component"
                )
            else:
                print(f"files: OK ({name} defined inline in iOS {mapped.name})")
            continue
        # Theme/app-shell files are Android-idiomatic (no iOS counterpart by design).
        if name in {"Theme", "MainActivity"}:
            continue
        problems.append(f"{name}: no iOS counterpart ({p.relative_to(ROOT)})")

    if problems:
        errors.append("files: missing counterparts:\n  " + "\n  ".join(problems))
    else:
        print(f"files: OK ({len(ios_files)} iOS files all have Android counterparts)")


def check_screen_strings(errors, warnings):
    for ios_path, android_path in SCREEN_PAIRS:
        label = f"screen_strings:{ios_path.stem}"
        ios_keys = tr_keys(ios_path)
        android_keys = tr_keys(android_path)
        exc = {k for (k, _) in EXCEPTIONS}
        only_ios = sorted((ios_keys - android_keys) - exc)
        only_android = sorted((android_keys - ios_keys) - exc)
        if not only_ios and not only_android:
            print(f"{label}: OK ({len(ios_keys)} keys match)")
        else:
            warnings.append(
                f"{label}: drift iOS-only={only_ios} Android-only={only_android}"
            )


def main() -> int:
    for p in [IOS_EDITOR, ANDROID_EDITOR, IOS_MODEL, ANDROID_MODEL,
              IOS_STRINGS, ANDROID_STRINGS, IOS_HAND, ANDROID_HAND]:
        if not p.is_file():
            print(f"ERROR: expected file missing: {p}")
            return 1
    errors, warnings = [], []
    check_line_types(errors)
    check_key_sets(errors, warnings, "editor_tools", "editor.tool", IOS_EDITOR, ANDROID_EDITOR)
    check_key_sets(errors, warnings, "editor_coaching", "editor.coaching", IOS_EDITOR, ANDROID_EDITOR)
    check_languages(errors)
    check_handside(errors)
    check_files(errors, warnings)
    check_screen_strings(errors, warnings)

    print()
    for w in warnings:
        print(f"WARNING: {w}")
    for e in errors:
        print(f"ERROR: {e}")
    print(f"parity: {len(errors)} error(s), {len(warnings)} warning(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
