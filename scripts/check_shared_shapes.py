#!/usr/bin/env python3
"""Validate the shared mobile shape/theme specs used by both native apps.

Single source of truth: apps/mobile_overlay/shared/shapes/*.json
Checks (violations are ERRORS, exit 1):
  1. silhouette.json: schema (segments/dots/creases/brace), every coord in
     [0,1], every curve is 6 numbers, and closed_loop_order chains end to
     start (eps 1e-9) back to the first move — the loop must close exactly.
  2. silhouette.json numbers are a subset of FullHandGuideShape's fractions
     in PalmSilhouetteView.swift (spec must match the code it will replace).
  3. vastu_mandala.json: 3x3 grid, and every zone key appears in BOTH
     VastuZoneInfo tables (Swift + Kotlin).
  4. overlay_theme.json: canonical hexes match the theme constants in code
     (iOS OverlayPalette floats converted, Android 0xAARRGGBB literals).
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SHAPES = ROOT / "apps/mobile_overlay/shared/shapes"
SILHOUETTE_JSON = SHAPES / "silhouette.json"
VASTU_JSON = SHAPES / "vastu_mandala.json"
THEME_JSON = SHAPES / "overlay_theme.json"
SWIFT_SIL = ROOT / "apps/mobile_overlay/ios/Sources/CurvPalmOverlay/PalmSilhouetteView.swift"
SWIFT_VASTU = ROOT / "apps/mobile_overlay/ios/Sources/CurvVastuOverlay/VastuOverlayProfile.swift"
KT_VASTU = ROOT / "apps/mobile_overlay/android/src/main/java/org/curv/overlay/vastu/VastuOverlay.kt"
SWIFT_HUD = ROOT / "apps/mobile_overlay/ios/Sources/CurvCameraOverlay/HUD/LiquidGlassHUD.swift"
KT_HUD = ROOT / "apps/mobile_overlay/android/src/main/java/org/curv/overlay/hud/LiquidGlassHUD.kt"

EPS = 1e-9
errors = 0


def fail(msg: str) -> None:
    global errors
    print(f"ERROR: {msg}")
    errors += 1


def load_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path}: {exc}")
        return None


def check_segment(name: str, seg: dict) -> tuple[list[float], list[float]]:
    """Returns (start, end) of an open segment; validates ranges."""
    move = seg.get("move")
    curves = seg.get("curves")
    if not (isinstance(move, list) and len(move) == 2):
        fail(f"{name}: move must be [x, y]")
        return [0.0, 0.0], [0.0, 0.0]
    pts = [float(v) for v in move]
    for c in curves or []:
        if not (isinstance(c, list) and len(c) == 6):
            fail(f"{name}: every curve must be 6 numbers")
            continue
        pts.extend(float(v) for v in c)
    for v in pts:
        if not 0.0 - EPS <= v <= 1.0 + EPS:
            fail(f"{name}: coord {v} outside normalized [0,1]")
    end = pts[-2:] if len(pts) >= 2 else [0.0, 0.0]
    return pts[:2], end


def get_path(doc: dict, dotted: str) -> dict:
    node: dict = doc["segments"]
    for part in dotted.split("."):
        node = node[part]
    return node


def close(a: list[float], b: list[float]) -> bool:
    return abs(a[0] - b[0]) <= EPS and abs(a[1] - b[1]) <= EPS


# --- 1+2. silhouette schema, ranges, loop chaining, code subset ---
sil = load_json(SILHOUETTE_JSON)
if sil is not None:
    order = sil.get("closed_loop_order", [])
    prev_end = None
    first_start = None
    for dotted in order:
        try:
            seg = get_path(sil, dotted)
        except (KeyError, TypeError):
            fail(f"closed_loop_order entry {dotted!r} not found in segments")
            continue
        start, end = check_segment(dotted, seg)
        if first_start is None:
            first_start = start
        if prev_end is not None and not close(prev_end, start):
            fail(f"loop break: {dotted} starts at {start}, previous ends at {prev_end}")
        prev_end = end
    if first_start is not None and prev_end is not None and not close(prev_end, first_start):
        fail(f"loop does not close: ends at {prev_end}, starts at {first_start}")

    for group in ("dots",):
        for key, val in (sil.get(group) or {}).items():
            pts = val if isinstance(val[0], list) else [val]
            for p in pts:
                if not (len(p) == 2 and all(0.0 - EPS <= float(v) <= 1.0 + EPS for v in p)):
                    fail(f"dots.{key} has out-of-range point {p}")
    for key, feat in (sil.get("hint_creases") or {}).items():
        check_segment(f"hint_creases.{key}", feat)
    check_segment("wrist_brace", sil.get("wrist_brace", {}))

    try:
        swift_nums = {round(float(v), 4) for v in re.findall(r"0\.\d+|\d+\.\d+", SWIFT_SIL.read_text())}
    except OSError as exc:
        fail(f"cannot read {SWIFT_SIL}: {exc}")
        swift_nums = set()
    if swift_nums:
        def json_nums(node) -> list[float]:
            out: list[float] = []
            if isinstance(node, (int, float)) and not isinstance(node, bool):
                out.append(float(node))
            elif isinstance(node, list):
                for v in node:
                    out.extend(json_nums(v))
            elif isinstance(node, dict):
                for v in node.values():
                    out.extend(json_nums(v))
            return out
        for v in json_nums(sil):
            if round(v, 4) not in swift_nums:
                fail(f"silhouette number {v} not found in PalmSilhouetteView.swift fractions")

# --- 3. vastu grid + cross-platform keys ---
vastu = load_json(VASTU_JSON)
if vastu is not None:
    rows = vastu.get("rows", [])
    if len(rows) != 3 or any(len(r) != 3 for r in rows):
        fail("vastu rows must be 3x3")
    keys = [z["key"] for r in rows for z in r]
    if vastu.get("emphasis_key") not in keys:
        fail("vastu emphasis_key must be one of the grid keys")
    for path in (SWIFT_VASTU, KT_VASTU):
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as exc:
            fail(f"cannot read {path}: {exc}")
            continue
        for key in keys:
            if f'"{key}"' not in text:
                fail(f"vastu key {key!r} missing from {path.relative_to(ROOT)}")

# --- 4. theme hexes match code constants ---
theme = load_json(THEME_JSON)
if theme is not None:
    for name, hexval in (theme.get("colors") or {}).items():
        if not re.fullmatch(r"#[0-9A-Fa-f]{6}", str(hexval)):
            fail(f"theme color {name}={hexval!r} is not #RRGGBB")
    try:
        floats = re.findall(
            r"static let (emerald|emeraldDeep|amber)\s*=\s*Color\(red:\s*([\d.]+),\s*green:\s*([\d.]+),\s*blue:\s*([\d.]+)\)",
            SWIFT_HUD.read_text(encoding="utf-8"),
        )
        swift_hex = {n: "#%02X%02X%02X" % (round(float(r) * 255), round(float(g) * 255), round(float(b) * 255)) for n, r, g, b in floats}
        expected = {"emerald": theme["colors"]["emerald"], "emeraldDeep": theme["colors"]["emerald_deep"], "amber": theme["colors"]["amber"]}
        for name, want in expected.items():
            if swift_hex.get(name) != want.upper():
                fail(f"iOS {name} is {swift_hex.get(name)}, theme says {want}")
    except OSError as exc:
        fail(f"cannot read {SWIFT_HUD}: {exc}")
    try:
        kt_hexes = set(re.findall(r"0xFF([0-9A-Fa-f]{6})", KT_HUD.read_text(encoding="utf-8")))
        for want_key in ("emerald", "emerald_deep", "amber"):
            want = theme["colors"][want_key].lstrip("#").upper()
            if want not in kt_hexes:
                fail(f"Android LiquidGlassHUD.kt lacks theme {want_key} #{want}")
    except OSError as exc:
        fail(f"cannot read {KT_HUD}: {exc}")

print(f"shape spec check complete ({errors} error(s))")
sys.exit(1 if errors else 0)
