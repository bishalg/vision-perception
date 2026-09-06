#!/usr/bin/env python3
"""
scripts/make_comparison.py

Generates a publication-quality side-by-side comparison image:
  Left:  PRE-FLOOD (2026-08-24) with extracted sub-pixel curvilinear ridges
  Right: POST-FLOOD (2026-08-27) with extracted sub-pixel curvilinear ridges
Also extracts and prints quantitative comparison metrics from evidence JSONs.
"""

import json
import os
import sys
import cv2
import numpy as np

PRE_OVERLAY = "data/flood_nepal_2026/out/pre_overlay.png"
POST_OVERLAY = "data/flood_nepal_2026/out/post_overlay.png"
PRE_EVIDENCE = "data/flood_nepal_2026/out/pre_evidence.json"
POST_EVIDENCE = "data/flood_nepal_2026/out/post_evidence.json"
OUT_COMPARISON = "data/flood_nepal_2026/out/comparison.png"


def load_evidence_metrics(json_path: str) -> dict:
    with open(json_path, "r") as f:
        doc = json.load(f)

    segments = []
    junctions = []
    for ev in doc.get("evidence", []):
        etype = ev.get("type")
        if etype == "line_candidate":
            segments.append(ev.get("geometry", {}))
        elif etype == "junction":
            junctions.append(ev.get("geometry", {}))

    total_len = sum(s.get("total_length", 0.0) for s in segments)
    max_len = max((s.get("total_length", 0.0) for s in segments), default=0.0)
    mean_len = total_len / len(segments) if segments else 0.0

    return {
        "segment_count": len(segments),
        "total_length_px": round(total_len, 2),
        "max_length_px": round(max_len, 2),
        "mean_length_px": round(mean_len, 2),
        "junction_count": len(junctions),
        "blur_score": round(doc.get("quality", {}).get("blur_score", 0.0), 2),
        "brightness": round(doc.get("quality", {}).get("brightness_score", 0.0), 2),
        "contrast": round(doc.get("quality", {}).get("contrast_score", 0.0), 2),
        "duration_us": doc.get("total_duration_us", 0),
    }


def draw_label(img: np.ndarray, title: str, subtitle: str, metrics_str: str) -> np.ndarray:
    """Adds a stylish header and footer overlay badge to the image panel."""
    canvas = img.copy()
    h, w = canvas.shape[:2]

    # Header banner
    banner_h = 75
    banner = np.zeros((banner_h, w, 3), dtype=np.uint8)
    banner[:] = (20, 20, 25)

    cv2.putText(banner, title, (20, 32), cv2.FONT_HERSHEY_SIMPLEX, 0.85, (255, 255, 255), 2, cv2.LINE_AA)
    cv2.putText(banner, subtitle, (20, 60), cv2.FONT_HERSHEY_SIMPLEX, 0.55, (160, 200, 255), 1, cv2.LINE_AA)

    # Footer metrics bar
    footer_h = 45
    footer = np.zeros((footer_h, w, 3), dtype=np.uint8)
    footer[:] = (25, 30, 35)
    cv2.putText(footer, metrics_str, (20, 28), cv2.FONT_HERSHEY_SIMPLEX, 0.50, (220, 220, 220), 1, cv2.LINE_AA)

    return np.vstack([banner, canvas, footer])


def main():
    if not os.path.isfile(PRE_OVERLAY) or not os.path.isfile(POST_OVERLAY):
        print(f"[ERROR] Overlay images missing. Run run_flood_pipeline.sh first.", file=sys.stderr)
        sys.exit(1)

    pre_img = cv2.imread(PRE_OVERLAY)
    post_img = cv2.imread(POST_OVERLAY)

    pre_m = load_evidence_metrics(PRE_EVIDENCE)
    post_m = load_evidence_metrics(POST_EVIDENCE)

    pre_metrics_str = f"Segments: {pre_m['segment_count']} | Length: {pre_m['total_length_px']}px | Mean: {pre_m['mean_length_px']}px | Blur: {pre_m['blur_score']}"
    post_metrics_str = f"Segments: {post_m['segment_count']} | Length: {post_m['total_length_px']}px | Mean: {post_m['mean_length_px']}px | Blur: {post_m['blur_score']}"

    labeled_pre = draw_label(
        pre_img,
        "PRE-FLOOD: 2026-08-24",
        "Sentinel-2 L2A True Color (B4/B3/B2) -- Baseline Channel",
        pre_metrics_str,
    )
    labeled_post = draw_label(
        post_img,
        "POST-FLOOD: 2026-08-27",
        "Sentinel-2 L2A True Color (B4/B3/B2) -- Post-Glacier Collapse",
        post_metrics_str,
    )

    # Thin vertical separator
    sep_w = 4
    separator = np.full((labeled_pre.shape[0], sep_w, 3), 180, dtype=np.uint8)

    combined = np.hstack([labeled_pre, separator, labeled_post])

    cv2.imwrite(OUT_COMPARISON, combined)
    print(f"[SUCCESS] Side-by-side comparison saved to: {OUT_COMPARISON}")
    print(f"  Resolution: {combined.shape[1]}x{combined.shape[0]} px")

    print("\n" + "=" * 60)
    print("QUANTITATIVE COMPARISON METRICS (Wide-Ridge Config, sigma=5.0)")
    print("=" * 60)
    print(f"{'Metric':<30} | {'PRE (2026-08-24)':<18} | {'POST (2026-08-27)':<18}")
    print("-" * 72)
    print(f"{'Extracted Segments':<30} | {pre_m['segment_count']:<18} | {post_m['segment_count']:<18}")
    print(f"{'Total Curve Length (px)':<30} | {pre_m['total_length_px']:<18} | {post_m['total_length_px']:<18}")
    print(f"{'Mean Segment Length (px)':<30} | {pre_m['mean_length_px']:<18} | {post_m['mean_length_px']:<18}")
    print(f"{'Max Segment Length (px)':<30} | {pre_m['max_length_px']:<18} | {post_m['max_length_px']:<18}")
    print(f"{'Junction Count':<30} | {pre_m['junction_count']:<18} | {post_m['junction_count']:<18}")
    print(f"{'Blur Score (Laplacian)':<30} | {pre_m['blur_score']:<18} | {post_m['blur_score']:<18}")
    print(f"{'Mean Brightness':<30} | {pre_m['brightness']:<18} | {post_m['brightness']:<18}")
    print(f"{'RMS Contrast':<30} | {pre_m['contrast']:<18} | {post_m['contrast']:<18}")
    print(f"{'Core Execution Time (ms)':<30} | {round(pre_m['duration_us']/1000, 2):<18} | {round(post_m['duration_us']/1000, 2):<18}")
    print("=" * 72)


if __name__ == "__main__":
    main()
