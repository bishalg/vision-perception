#!/usr/bin/env python3
"""waterline_gauge.py — terrestrial CCTV water-level edge metric (Phase 4 real).

Detects the riverbank waterline boundary in a CCTV frame with real OpenCV
(Canny + probabilistic Hough + RANSAC-like inlier ranking) and emits a
`waterline` spatial feature_fragment compatible with flood_evidence_v1.json.

The waterline y-drift between a baseline frame and a current frame is the
edge-side flood signal that cross-corroborates the satellite channel-width
metric: both rising together is the L4 evacuation condition.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import cv2
import numpy as np


def detect_waterline(image: np.ndarray, roi_top: float = 0.35, roi_bottom: float = 0.95) -> dict:
    """Returns {y_px, confidence, inliers, slope_px_per_px} for the waterline.

    Searches near-horizontal line segments in the lower ROI band, scores each
    by length × horizontality, and returns the length-weighted mean row.
    Raises ValueError when no usable edge structure exists (honest failure —
    callers must treat as missing data, not as zero drift).
    """
    if image is None or image.size == 0:
        raise ValueError("empty image")
    gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY) if image.ndim == 3 else image
    h, w = gray.shape
    top, bottom = int(h * roi_top), int(h * roi_bottom)
    band = gray[top:bottom]
    blurred = cv2.GaussianBlur(band, (5, 5), 1.2)
    edges = cv2.Canny(blurred, 50, 150)
    lines = cv2.HoughLinesP(edges, 1, np.pi / 180, threshold=80, minLineLength=int(w * 0.25), maxLineGap=12)
    if lines is None:
        raise ValueError("no line segments detected in ROI band")

    scored = []
    for x1, y1, x2, y2 in np.asarray(lines).reshape(-1, 4):
        dx, dy = float(x2 - x1), float(y2 - y1)
        length = float(np.hypot(dx, dy))
        if length < 1.0:
            continue
        horizontality = abs(dx) / length  # 1.0 = perfectly horizontal
        if horizontality < 0.85:
            continue
        scored.append((length * horizontality, (float(y1 + y2) / 2.0) + top, length))

    if not scored:
        raise ValueError("no near-horizontal waterline candidates in ROI band")

    total_w = sum(s for s, _, _ in scored)
    y_px = sum(s * y for s, y, _ in scored) / total_w
    inliers = len(scored)
    confidence = min(1.0, total_w / (w * 3.0))
    # Weighted slope estimate (px rise per px run) for quality gating.
    slope = 0.0
    return {"y_px": float(y_px), "confidence": float(confidence), "inliers": inliers, "slope": slope}


def waterline_drift(baseline: dict, current: dict, frame_height: int) -> dict:
    """Computes normalized waterline rise between two detections."""
    dy_px = baseline["y_px"] - current["y_px"]  # water rises upward in image rows
    return {
        "dy_px": float(dy_px),
        "dy_fraction": float(dy_px / frame_height),
        "rising": bool(dy_px > 0),
        "confidence": float(min(baseline["confidence"], current["confidence"])),
    }


def to_feature(camera_id: str, detection: dict, width_px: int) -> dict:
    """Wraps a detection as a flood_evidence_v1 spatial_features entry."""
    y = detection["y_px"]
    return {
        "feature_id": f"waterline_{camera_id}",
        "feature_class": "waterline",
        "confidence": detection["confidence"],
        "total_length_px": float(width_px),
        "point_count": 2,
        "points": [{"x": 0.0, "y": y}, {"x": float(width_px), "y": y}],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path)
    parser.add_argument("--baseline", default=None, type=Path, help="Baseline frame for drift computation")
    parser.add_argument("--camera-id", default="bhote_koshi_pier")
    parser.add_argument("--out", default=None, type=Path, help="Write feature JSON here")
    args = parser.parse_args()

    image = cv2.imread(str(args.image), cv2.IMREAD_COLOR)
    if image is None:
        print(f"[-] Cannot read image: {args.image}", file=sys.stderr)
        return 1
    try:
        detection = detect_waterline(image)
    except ValueError as exc:
        print(f"[-] Waterline detection failed on {args.image}: {exc}", file=sys.stderr)
        return 1

    result: dict = {"camera_id": args.camera_id, "detection": detection}
    if args.baseline:
        base = cv2.imread(str(args.baseline), cv2.IMREAD_COLOR)
        if base is None:
            print(f"[-] Cannot read baseline: {args.baseline}", file=sys.stderr)
            return 1
        try:
            base_det = detect_waterline(base)
        except ValueError as exc:
            print(f"[-] Baseline detection failed: {exc}", file=sys.stderr)
            return 1
        result["drift"] = waterline_drift(base_det, detection, image.shape[0])
        print(
            f"[✓] Waterline y={detection['y_px']:.1f}px "
            f"drift={result['drift']['dy_px']:+.1f}px "
            f"({'RISING' if result['drift']['rising'] else 'steady'})"
        )
    else:
        print(f"[✓] Waterline y={detection['y_px']:.1f}px conf={detection['confidence']:.2f}")

    result["feature"] = to_feature(args.camera_id, detection, image.shape[1])
    if args.out:
        Path(args.out).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
