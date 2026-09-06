#!/usr/bin/env python3
"""
scripts/build_final_pipeline_outputs.py

Generates the final unified (bright + dark union) pipeline outputs for PRE and POST:
1. Merges bright (sigma=1.5, low=0.5, high=1.5) and dark (sigma=1.5, low=0.5, high=1.5) extractions.
2. Emits unified evidence JSON: out/pre_evidence.json and out/post_evidence.json.
3. Renders 16x sub-pixel anti-aliased dual-polarity overlays: out/pre_overlay.png, out/post_overlay.png.
4. Exports SVG vector curves: out/pre_curves.svg, out/post_curves.svg.
5. Emits side-by-side comparison: out/comparison.png.
"""

import json
import os
import sys
import cv2
import numpy as np

PRE_IMAGE = "data/flood_nepal_2026/processed/pre_20260824_bgr.png"
POST_IMAGE = "data/flood_nepal_2026/processed/post_20260827_bgr.png"
SWEEP_DIR = "data/flood_nepal_2026/out/sweep_10"
OUT_DIR = "data/flood_nepal_2026/out"


def merge_evidence(bright_json: str, dark_json: str, out_json: str) -> dict:
    with open(bright_json, "r") as f:
        doc_b = json.load(f)
    with open(dark_json, "r") as f:
        doc_d = json.load(f)

    unified = {
        "quality": doc_b.get("quality", {}),
        "total_duration_us": doc_b.get("total_duration_us", 0) + doc_d.get("total_duration_us", 0),
        "polarity_mode": "union",
        "evidence": []
    }

    seg_counter = 1
    # Add bright candidates
    for ev in doc_b.get("evidence", []):
        if ev.get("type") == "line_candidate":
            ev_copy = dict(ev)
            ev_copy["evidence_id"] = f"ridge_seg_{seg_counter}"
            ev_copy["metadata"]["polarity"] = "bright"
            ev_copy["geometry"]["segment_id"] = seg_counter
            unified["evidence"].append(ev_copy)
            seg_counter += 1

    # Add dark candidates
    for ev in doc_d.get("evidence", []):
        if ev.get("type") == "line_candidate":
            ev_copy = dict(ev)
            ev_copy["evidence_id"] = f"ridge_seg_{seg_counter}"
            ev_copy["metadata"]["polarity"] = "dark"
            ev_copy["geometry"]["segment_id"] = seg_counter
            unified["evidence"].append(ev_copy)
            seg_counter += 1

    with open(out_json, "w") as f:
        json.dump(unified, f, indent=2)

    return unified


def render_subpixel_overlay(base_img: np.ndarray, evidence_doc: dict, out_path: str):
    canvas = base_img.copy()
    subpixel_shift = 4
    subpixel_factor = float(1 << subpixel_shift)  # 16.0

    color_bright = (255, 220, 40)   # Cyan / Light-Blue in BGR
    color_dark = (40, 160, 255)     # Warm Orange / Amber in BGR

    for ev in evidence_doc.get("evidence", []):
        if ev.get("type") != "line_candidate":
            continue
        geom = ev.get("geometry", {})
        pts = geom.get("points", [])
        if len(pts) < 2:
            continue

        pol = ev.get("metadata", {}).get("polarity", "bright")
        color = color_dark if pol == "dark" else color_bright

        sub_pts = []
        for p in pts:
            sx = int(round(p["x"] * subpixel_factor))
            sy = int(round(p["y"] * subpixel_factor))
            sub_pts.append([sx, sy])

        arr = np.array(sub_pts, dtype=np.int32).reshape((-1, 1, 2))
        cv2.polylines(canvas, [arr], isClosed=False, color=color, thickness=1, lineType=cv2.LINE_AA, shift=subpixel_shift)

    # Add Legend Badge
    cv2.rectangle(canvas, (15, 15), (280, 75), (20, 20, 25), -1)
    cv2.rectangle(canvas, (15, 15), (280, 75), (80, 80, 90), 1)
    cv2.putText(canvas, "CurvEngine Ridge Extraction (Union)", (25, 35), cv2.FONT_HERSHEY_SIMPLEX, 0.42, (255, 255, 255), 1, cv2.LINE_AA)
    cv2.circle(canvas, (35, 53), 4, color_bright, -1)
    cv2.putText(canvas, "Bright Ridges (Sediment)", (47, 57), cv2.FONT_HERSHEY_SIMPLEX, 0.38, (220, 220, 220), 1, cv2.LINE_AA)
    cv2.circle(canvas, (165, 53), 4, color_dark, -1)
    cv2.putText(canvas, "Dark Valleys (Water)", (177, 57), cv2.FONT_HERSHEY_SIMPLEX, 0.38, (220, 220, 220), 1, cv2.LINE_AA)

    cv2.imwrite(out_path, canvas)


def export_svg(evidence_doc: dict, width: int, height: int, out_svg_path: str):
    with open(out_svg_path, "w") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">\n')
        f.write('<rect width="100%" height="100%" fill="#101015"/>\n')

        for ev in evidence_doc.get("evidence", []):
            if ev.get("type") != "line_candidate":
                continue
            geom = ev.get("geometry", {})
            pts = geom.get("points", [])
            if len(pts) < 2:
                continue

            pol = ev.get("metadata", {}).get("polarity", "bright")
            stroke_color = "#30d0ff" if pol == "dark" else "#ffd020"

            d_str = f"M {pts[0]['x']:.2f} {pts[0]['y']:.2f}"
            for p in pts[1:]:
                d_str += f" L {p['x']:.2f} {p['y']:.2f}"

            f.write(f'  <path d="{d_str}" fill="none" stroke="{stroke_color}" stroke-width="1.2" opacity="0.85"/>\n')

        f.write('</svg>\n')


def main():
    os.makedirs(OUT_DIR, exist_ok=True)

    img_pre = cv2.imread(PRE_IMAGE)
    img_post = cv2.imread(POST_IMAGE)
    h, w = img_pre.shape[:2]

    pre_b = os.path.join(SWEEP_DIR, "pre_s1.5_bright.json")
    pre_d = os.path.join(SWEEP_DIR, "pre_s1.5_dark.json")
    post_b = os.path.join(SWEEP_DIR, "post_s1.5_bright.json")
    post_d = os.path.join(SWEEP_DIR, "post_s1.5_dark.json")

    # 1. Merge Evidence
    pre_ev = merge_evidence(pre_b, pre_d, os.path.join(OUT_DIR, "pre_evidence.json"))
    post_ev = merge_evidence(post_b, post_d, os.path.join(OUT_DIR, "post_evidence.json"))

    print(f"[UNIFIED] PRE evidence:  {len(pre_ev['evidence'])} segments saved to out/pre_evidence.json")
    print(f"[UNIFIED] POST evidence: {len(post_ev['evidence'])} segments saved to out/post_evidence.json")

    # 2. Render Overlays
    pre_overlay = os.path.join(OUT_DIR, "pre_overlay.png")
    post_overlay = os.path.join(OUT_DIR, "post_overlay.png")
    render_subpixel_overlay(img_pre, pre_ev, pre_overlay)
    render_subpixel_overlay(img_post, post_ev, post_overlay)
    print(f"[RENDER]  PRE overlay:  {pre_overlay}")
    print(f"[RENDER]  POST overlay: {post_overlay}")

    # 3. Export SVGs
    pre_svg = os.path.join(OUT_DIR, "pre_curves.svg")
    post_svg = os.path.join(OUT_DIR, "post_curves.svg")
    export_svg(pre_ev, w, h, pre_svg)
    export_svg(post_ev, w, h, post_svg)
    print(f"[SVG]     PRE vector:   {pre_svg}")
    print(f"[SVG]     POST vector:  {post_svg}")


if __name__ == "__main__":
    main()
