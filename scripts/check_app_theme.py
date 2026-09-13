#!/usr/bin/env python3
"""Gate system-theme support for the iOS + Android apps (face + palm tracking).

Policy (see docs/overlay_graphics_architecture.md § theme):
- Every app follows the system theme on launch and stays legible in BOTH
  light and dark mode. Forcing a scheme is an ERROR (exit 1).
- Media-immersive surfaces (live camera, stimulus/calibration fields, editor
  canvas, photo review) stay dark by design with fixed light-on-dark
  treatments — like the native Camera app. Files that are entirely such
  surfaces are exempt from the hardcoded-paint warnings below.
- Intentional fixed treatments elsewhere carry a `theme:fixed-media` marker
  comment on a nearby line; unmarked hardcoded white/black paint outside the
  media allowlist is a WARNING (exit 0) for reviewer follow-up.

Checks:
  1. No `.preferredColorScheme(` in Swift (forced scheme).
  2. No `darkTheme = true/false` literals in PalmAppTheme calls (forced).
  3. No `UIUserInterfaceStyle` in plists, no `setDefaultNightMode` /
     `AppCompatDelegate` in Kotlin (forced).
  4. Warnings for hardcoded white/black paint outside media files.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IOS_DIRS = [ROOT / "apps/ios_palm_app", ROOT / "apps/ios_vastu_app", ROOT / "apps/ios_asd_gaze_app"]
ANDROID_DIRS = [ROOT / "apps/android_palm_app", ROOT / "apps/android_vastu_app"]

# Files that are fixed-dark media surfaces by design (camera/stimulus/
# calibration/editor-canvas/photo-review fields, overlay handles on media,
# plus the adaptive palette mechanism itself).
MEDIA_ALLOWLIST = {
    "MainGazeScannerView.swift",
    "StimulusScreeningView.swift",
    "ARFaceCameraView.swift",
    "BubbleCalibrationView.swift",
    "MainScannerView.swift",
    "CaptureResultView.swift",
    "CreaseLineEditorView.swift",
    "CreaseSplineOverlayView.swift",
    "MagnifyingLoupeView.swift",
    "MainScannerScreen.kt",
    "CaptureResultView.kt",
    "CreaseLineEditorScreen.kt",
    "CreaseSplineOverlayView.kt",
    "HandOutlineOverlay.kt",
    "LiquidGlassTheme.swift",
}
ESCAPE_MARKER = "theme:fixed-media"
# Shadow colors stay dark in both modes (correct) — never flag these.
SHADOW_RE = re.compile(r"\.shadow\s*\(")

FORCE_PATTERNS_SWIFT = [re.compile(r"\.preferredColorScheme\s*\(")]
FORCE_PATTERNS_KT = [
    re.compile(r"darkTheme\s*=\s*(true|false)"),
    re.compile(r"setDefaultNightMode|AppCompatDelegate"),
]
HARDCODED_RE = re.compile(
    r"Color\.white\b|Color\.black\b|Color\.White\b|Color\.Black\b"
    r"|foregroundColor\(\s*\.(white|black)\s*\)"
    r"|\.fill\(\s*Color\.white\s*\)"
)

errors = 0
warnings = 0


def fail(msg: str) -> None:
    global errors
    print(f"ERROR: {msg}")
    errors += 1


def warn(msg: str) -> None:
    global warnings
    print(f"WARNING: {msg}")
    warnings += 1


def iter_sources(dirs, suffixes):
    for base in dirs:
        if not base.is_dir():
            fail(f"missing source dir {base.relative_to(ROOT)}")
            continue
        for path in sorted(base.rglob("*")):
            if path.suffix in suffixes and ".build" not in path.parts:
                # Theme definition files ARE the adaptive mechanism.
                if "ui/theme/" in path.as_posix():
                    continue
                yield path


for path in iter_sources(IOS_DIRS, {".swift"}):
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        fail(f"cannot read {path}: {exc}")
        continue
    for pat in FORCE_PATTERNS_SWIFT:
        for m in pat.finditer(text):
            line = text.count("\n", 0, m.start()) + 1
            fail(f"{path.relative_to(ROOT)}:{line} forces color scheme ({m.group(0).strip()})")
    if path.name in MEDIA_ALLOWLIST:
        continue
    lines = text.splitlines()
    for i, line in enumerate(lines, 1):
        if ESCAPE_MARKER in line or SHADOW_RE.search(line):
            continue
        if HARDCODED_RE.search(line):
            nearby = "\n".join(lines[max(0, i - 3):min(len(lines), i + 2)])
            if ESCAPE_MARKER in nearby:
                continue
            warn(f"{path.relative_to(ROOT)}:{i} hardcoded paint outside media surface: {line.strip()[:90]}")


for path in iter_sources(ANDROID_DIRS, {".kt"}):
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        fail(f"cannot read {path}: {exc}")
        continue
    for pat in FORCE_PATTERNS_KT:
        for m in pat.finditer(text):
            line = text.count("\n", 0, m.start()) + 1
            fail(f"{path.relative_to(ROOT)}:{line} forces theme ({m.group(0).strip()})")
    if path.name in MEDIA_ALLOWLIST:
        continue
    lines = text.splitlines()
    for i, line in enumerate(lines, 1):
        if ESCAPE_MARKER in line or "shadow" in line.lower():
            continue
        if HARDCODED_RE.search(line):
            nearby = "\n".join(lines[max(0, i - 3):min(len(lines), i + 2)])
            if ESCAPE_MARKER in nearby:
                continue
            warn(f"{path.relative_to(ROOT)}:{i} hardcoded paint outside media surface: {line.strip()[:90]}")


for plist in ROOT.glob("apps/ios_*/**/*.plist"):
    try:
        if "UIUserInterfaceStyle" in plist.read_text(encoding="utf-8"):
            fail(f"{plist.relative_to(ROOT)} pins UIUserInterfaceStyle")
    except OSError:
        pass

print(f"theme check complete ({errors} error(s), {warnings} warning(s))")
sys.exit(1 if errors else 0)
