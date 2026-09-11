#!/usr/bin/env bash
#!/usr/bin/env python3
"""
scripts/verify_region_containment.py
Audits PalmRegionGeometry corridors and mounts against human truth (01)
and detected evidence (02), and checks static link symbols.
Outputs a structured confidence ledger.
"""

from __future__ import annotations
import json
import math
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Tuple

ROOT = Path(__file__).resolve().parent.parent
CAPTURES_DIR = ROOT / "data" / "samples" / "mobile_captures"
CANONICAL_DIM = 512.0


def lerp(a: Tuple[float, float], b: Tuple[float, float], t: float) -> Tuple[float, float]:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def mid(a: Tuple[float, float], b: Tuple[float, float]) -> Tuple[float, float]:
    return lerp(a, b, 0.5)


def depth(mcp: Tuple[float, float], wrist: Tuple[float, float], t: float) -> Tuple[float, float]:
    return lerp(mcp, wrist, t)


def compute_palm_regions(landmarks: List[Dict[str, float]]) -> Tuple[Dict[str, List[Tuple[float, float]]], Dict[str, Any]]:
    wrist = (landmarks[0]["x"], landmarks[0]["y"])
    thumb_cmc = (landmarks[1]["x"], landmarks[1]["y"])
    thumb_mcp = (landmarks[2]["x"], landmarks[2]["y"])
    index_mcp = (landmarks[5]["x"], landmarks[5]["y"])
    middle_mcp = (landmarks[9]["x"], landmarks[9]["y"])
    ring_mcp = (landmarks[13]["x"], landmarks[13]["y"])
    pinky_mcp = (landmarks[17]["x"], landmarks[17]["y"])

    idx_mid = mid(index_mcp, middle_mcp)
    mid_mid = mid(middle_mcp, ring_mcp)
    ring_mid = mid(ring_mcp, pinky_mcp)

    ux = pinky_mcp[0] - index_mcp[0]
    uy = pinky_mcp[1] - index_mcp[1]
    ulen = max(1e-4, math.hypot(ux, uy))
    uw = ulen * 0.38
    ox = (ux / ulen) * uw
    oy = (uy / ulen) * uw

    def push(p: Tuple[float, float]) -> Tuple[float, float]:
        return (p[0] + ox, p[1] + oy)

    def finger_mount(top_a: Tuple[float, float], top_b: Tuple[float, float], outward: bool = False) -> List[Tuple[float, float]]:
        a = top_a
        b = push(top_b) if outward else top_b
        return [a, b, depth(b, wrist, 0.22), depth(a, wrist, 0.22)]

    venus: List[Tuple[float, float]] = [
        thumb_cmc,
        thumb_mcp,
        lerp(index_mcp, thumb_mcp, 0.45),
        lerp(wrist, index_mcp, 0.35),
        lerp(thumb_mcp, wrist, 0.35),
    ]
    luna_top = lerp(wrist, pinky_mcp, 0.55)
    luna: List[Tuple[float, float]] = [luna_top, push(luna_top), push(wrist), wrist]
    mars: List[Tuple[float, float]] = [
        depth(middle_mcp, wrist, 0.40),
        depth(pinky_mcp, wrist, 0.40),
        push(depth(pinky_mcp, wrist, 0.58)),
        push(depth(middle_mcp, wrist, 0.58)),
    ]

    mounts: Dict[str, List[Tuple[float, float]]] = {
        "jupiter": finger_mount(index_mcp, idx_mid),
        "saturn": finger_mount(idx_mid, mid_mid),
        "apollo": finger_mount(mid_mid, ring_mid),
        "mercury": finger_mount(ring_mid, pinky_mcp, outward=True),
        "venus": venus,
        "luna": luna,
        "mars": mars,
    }

    life_origin = lerp(index_mcp, thumb_mcp, 0.45)
    heart: List[Tuple[float, float]] = [
        depth(pinky_mcp, wrist, 0.06),
        depth(ring_mcp, wrist, 0.10),
        depth(middle_mcp, wrist, 0.12),
        depth(index_mcp, wrist, 0.10),
    ]
    head: List[Tuple[float, float]] = [
        lerp(life_origin, wrist, 0.18),
        depth(index_mcp, wrist, 0.32),
        depth(middle_mcp, wrist, 0.36),
        depth(ring_mcp, wrist, 0.34),
        depth(pinky_mcp, wrist, 0.30),
    ]
    life: List[Tuple[float, float]] = [
        lerp(life_origin, wrist, 0.08),
        lerp(thumb_mcp, index_mcp, 0.25),
        lerp(thumb_mcp, wrist, 0.35),
        lerp((landmarks[3]["x"], landmarks[3]["y"]), wrist, 0.55),
        lerp(thumb_cmc, wrist, 0.72),
    ]

    def make_echoes(pts: List[Tuple[float, float]]) -> List[List[Tuple[float, float]]]:
        in_echo = [lerp(p, wrist, 0.05) for p in pts]
        out_echo = []
        for p in pts:
            dx = p[0] - wrist[0]
            dy = p[1] - wrist[1]
            dist_val = max(1e-4, math.hypot(dx, dy))
            step = dist_val * 0.05
            out_echo.append((p[0] + dx / dist_val * step, p[1] + dy / dist_val * step))
        return [in_echo, out_echo]

    corridors: Dict[str, Any] = {
        "heart": {"center": heart, "echoes": make_echoes(heart)},
        "head": {"center": head, "echoes": make_echoes(head)},
        "life": {"center": life, "echoes": make_echoes(life)},
    }
    return mounts, corridors


def p_seg_dist(p: Tuple[float, float], a: Tuple[float, float], b: Tuple[float, float]) -> float:
    abx = b[0] - a[0]
    aby = b[1] - a[1]
    abl2 = abx * abx + aby * aby
    if abl2 == 0:
        return math.hypot(p[0] - a[0], p[1] - a[1])
    t = max(0.0, min(1.0, ((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / abl2))
    proj = (a[0] + t * abx, a[1] + t * aby)
    return math.hypot(p[0] - proj[0], p[1] - proj[1])


def polyline_dist(p: Tuple[float, float], poly: List[Tuple[float, float]]) -> float:
    return min(p_seg_dist(p, poly[i], poly[i + 1]) for i in range(len(poly) - 1))


def polygon_area(poly: List[Tuple[float, float]]) -> float:
    n = len(poly)
    return 0.5 * abs(sum(poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1] for i in range(n)))


def ccw(A: Tuple[float, float], B: Tuple[float, float], C: Tuple[float, float]) -> bool:
    return (C[1] - A[1]) * (B[0] - A[0]) > (B[1] - A[1]) * (C[0] - A[0])


def intersect(A: Tuple[float, float], B: Tuple[float, float], C: Tuple[float, float], D: Tuple[float, float]) -> bool:
    return ccw(A, C, D) != ccw(B, C, D) and ccw(A, B, C) != ccw(A, B, D)


def check_self_intersect(poly: List[Tuple[float, float]]) -> bool:
    n = len(poly)
    for i in range(n):
        for j in range(i + 2, n):
            if i == 0 and j == n - 1:
                continue
            if intersect(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n]):
                return True
    return False


def audit_containment() -> Dict[str, Any]:
    results: Dict[str, Any] = {"fixtures": {}, "mounts": {}}

    # --- Fixture 01: Left Bishal (Human Truth) ---
    lms_01 = json.loads((CAPTURES_DIR / "01_left_bishal_evidence_landmarks.json").read_text())
    truth_01 = json.loads((CAPTURES_DIR / "01_left_bishal_USER_CORRECTED.json").read_text())
    mounts_01, corridors_01 = compute_palm_regions(lms_01)

    f01_lines: Dict[str, Any] = {}
    for line_key in ["heart", "head", "life"]:
        raw_polys = truth_01.get(line_key) or []
        pts: List[Tuple[float, float]] = []
        for poly in raw_polys:
            for p in poly:
                pts.append((float(p[0]), float(p[1])))

        corr_center = corridors_01[line_key]["center"]
        dists = [polyline_dist(pt, corr_center) for pt in pts]
        dists_px = [d * CANONICAL_DIM for d in dists]

        f01_lines[line_key] = {
            "num_points": len(pts),
            "mean_dist_px": sum(dists_px) / len(dists_px) if dists_px else 0.0,
            "max_dist_px": max(dists_px) if dists_px else 0.0,
            "pct_within_35px": (sum(1 for d in dists_px if d <= 35.0) / len(dists_px) * 100.0) if dists_px else 0.0,
            "pct_within_50px": (sum(1 for d in dists_px if d <= 50.0) / len(dists_px) * 100.0) if dists_px else 0.0,
        }
    results["fixtures"]["01_left_bishal_truth"] = f01_lines

    # --- Fixture 02: Right Bishal (Auto Detected Lines) ---
    lms_02 = json.loads((CAPTURES_DIR / "02_right_bishal_evidence_landmarks.json").read_text())
    ev_02 = json.loads((CAPTURES_DIR / "02_right_bishal_evidence.json").read_text())
    mounts_02, corridors_02 = compute_palm_regions(lms_02)
    ev_02_ml = ev_02.get("domain_payload", {}).get("major_lines", {})

    f02_lines: Dict[str, Any] = {}
    mapping = {"heart": "heart_line", "head": "head_line", "life": "life_line"}
    for line_key, ev_key in mapping.items():
        line_data = ev_02_ml.get(ev_key) or {}
        raw_pts = line_data.get("points_canonical_norm") or []
        pts = [(float(p[0]), float(p[1])) for p in raw_pts]

        corr_center = corridors_02[line_key]["center"]
        if pts:
            dists = [polyline_dist(pt, corr_center) for pt in pts]
            dists_px = [d * CANONICAL_DIM for d in dists]
            f02_lines[line_key] = {
                "num_points": len(pts),
                "mean_dist_px": sum(dists_px) / len(dists_px),
                "max_dist_px": max(dists_px),
                "pct_within_35px": sum(1 for d in dists_px if d <= 35.0) / len(dists_px) * 100.0,
                "pct_within_50px": sum(1 for d in dists_px if d <= 50.0) / len(dists_px) * 100.0,
            }
    results["fixtures"]["02_right_bishal_detected"] = f02_lines

    # --- Mounts Geometry & Non-degeneracy ---
    mount_stats: Dict[str, Any] = {}
    for name, poly in mounts_01.items():
        area = polygon_area(poly)
        self_intersect = check_self_intersect(poly)
        mount_stats[name] = {
            "vertex_count": len(poly),
            "area_norm": area,
            "area_px2": area * CANONICAL_DIM * CANONICAL_DIM,
            "self_intersect": self_intersect,
            "is_valid": (area > 1e-4) and (not self_intersect),
        }
    results["mounts"] = mount_stats
    return results


def audit_symbols() -> Dict[str, Any]:
    dylib_path = Path("/tmp/PalmSim/Build/Products/Debug-iphonesimulator/PalmScanner.app/PalmScanner.debug.dylib")
    if not dylib_path.exists():
        # Fallback to PalmDerivedTest
        dylib_path = Path("/tmp/PalmDerivedTest/Build/Products/Debug-iphonesimulator/PalmScanner.app/PalmScanner.debug.dylib")

    out_otool = subprocess.check_output(["otool", "-L", str(dylib_path)], text=True)
    has_opencv_dynamic = "opencv2" in out_otool

    out_nm = subprocess.check_output(["nm", "-gU", str(dylib_path)], text=True)
    has_capi_process = "_curv_pipeline_process_landmarks_roi" in out_nm
    has_capi_create = "_curv_pipeline_create" in out_nm
    has_opencv_symbols = "__ZN2cv8fastFreeEPv" in out_nm

    return {
        "dylib_checked": str(dylib_path),
        "opencv2_framework_dynamic_load_removed": not has_opencv_dynamic,
        "curv_capi_symbols_present": has_capi_process and has_capi_create,
        "opencv_static_symbols_present": has_opencv_symbols,
        "strip_safety_verified": (not has_opencv_dynamic) and has_capi_process and has_opencv_symbols,
    }


def main() -> None:
    containment = audit_containment()
    symbols = audit_symbols()

    print("================================================================================")
    print("           PALM REGION OVERLAY & ENGINE CONFIDENCE LEDGER                       ")
    print("================================================================================")
    print("\n1. NATIVE ENGINE LINKAGE & STRIP SAFETY:")
    print(f"   - Target dylib: {symbols['dylib_checked']}")
    print(f"   - Dynamic opencv2.framework load removed: {'PASS' if symbols['opencv2_framework_dynamic_load_removed'] else 'FAIL'}")
    print(f"   - CurvEngine C-ABI symbols statically present: {'PASS' if symbols['curv_capi_symbols_present'] else 'FAIL'}")
    print(f"   - OpenCV core symbols statically present: {'PASS' if symbols['opencv_static_symbols_present'] else 'FAIL'}")
    print(f"   -> Overall Strip Safety: {'CONFIRMED SAFE' if symbols['strip_safety_verified'] else 'FAILED'}")

    print("\n2. MOUNT TOPOLOGY & NON-DEGENERACY (01 LANDMARKS):")
    all_mounts_valid = True
    for name, st in containment["mounts"].items():
        valid_str = "VALID" if st["is_valid"] else "INVALID"
        if not st["is_valid"]:
            all_mounts_valid = False
        print(f"   - {name:<9}: {st['vertex_count']} vertices, area={st['area_px2']:6.1f} px^2 ({valid_str})")
    print(f"   -> All 7 Mounts Non-Degenerate: {'PASS' if all_mounts_valid else 'FAIL'}")

    print("\n3. CORRIDOR CONTAINMENT VS 01 LEFT BISHAL HUMAN TRUTH:")
    print("   Line   | Points | Mean Dist | Max Dist | <= 35px | <= 50px")
    print("   -------|--------|-----------|----------|---------|--------")
    for line, st in containment["fixtures"]["01_left_bishal_truth"].items():
        print(f"   {line:<6} | {st['num_points']:<6} | {st['mean_dist_px']:5.1f} px  | {st['max_dist_px']:5.1f} px | {st['pct_within_35px']:5.1f}%  | {st['pct_within_50px']:5.1f}%")

    print("\n4. CORRIDOR CONTAINMENT VS 02 RIGHT BISHAL DETECTED:")
    print("   Line   | Points | Mean Dist | Max Dist | <= 35px | <= 50px")
    print("   -------|--------|-----------|----------|---------|--------")
    for line, st in containment["fixtures"]["02_right_bishal_detected"].items():
        print(f"   {line:<6} | {st['num_points']:<6} | {st['mean_dist_px']:5.1f} px  | {st['max_dist_px']:5.1f} px | {st['pct_within_35px']:5.1f}%  | {st['pct_within_50px']:5.1f}%")

    # Write summary json
    summary_path = ROOT / "output" / "region_containment_ledger.json"
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_data = {
        "containment": containment,
        "symbols": symbols,
        "status": "ALL_VERIFIED_PASS" if (symbols["strip_safety_verified"] and all_mounts_valid) else "CHECK_FAILED",
    }
    summary_path.write_text(json.dumps(summary_data, indent=2))
    print(f"\nReport written to: {summary_path}")


if __name__ == "__main__":
    main()
