#!/usr/bin/env python3
"""
cctv_video_fixture.py - Stage 2 offline recorded-video fixture generator
------------------------------------------------------------------------
Synthesizes a short MJPEG/AVI clip with moving curvilinear lane features for
deterministic ring-buffer / CctvStreamAdapter tests (no network required).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path


def synthesize_clip(output: Path, frames: int = 30, fps: int = 10, width: int = 320, height: int = 240) -> None:
    try:
        import cv2
        import numpy as np
    except ImportError as exc:  # pragma: no cover
        raise SystemExit(f"OpenCV + NumPy required: {exc}") from exc

    output.parent.mkdir(parents=True, exist_ok=True)
    fourcc = cv2.VideoWriter_fourcc(*"MJPG")
    writer = cv2.VideoWriter(str(output), fourcc, float(fps), (width, height))
    if not writer.isOpened():
        raise SystemExit(f"Failed to open VideoWriter for {output}")

    for i in range(frames):
        img = np.full((height, width, 3), 130, dtype=np.uint8)
        phase = i * 0.35
        for y in range(height):
            x1 = int(80 + 25.0 * math.sin(y / 40.0 + phase) + y * 0.15)
            x2 = int(200 - 20.0 * math.cos(y / 45.0 + phase * 0.8) - y * 0.1)
            for xc in (x1, x2):
                for dx in range(-2, 3):
                    px = xc + dx
                    if 0 <= px < width:
                        img[y, px] = (250, 250, 250)
        # Moving bright marker for temporal continuity checks
        mx = int((i * 7) % width)
        cv2.circle(img, (mx, height // 2), 6, (40, 40, 220), -1)
        writer.write(img)

    writer.release()

    sidecar = {
        "agency": "Caltrans District 3",
        "name": "I-80 Drum Forebay (synthetic Stage 2 clip)",
        "location": {"lat": 39.3142, "lon": -120.7321, "elevation_m": 1420.0},
        "pose": {"heading_deg": 65.0, "pitch_deg": -15.0, "fov_deg": 55.0},
        "url": "file://synthetic_stage2_clip",
        "video": {"frames": frames, "fps": fps, "width": width, "height": height, "codec": "MJPG"},
    }
    json_path = output.with_suffix(".json")
    json_path.write_text(json.dumps(sidecar, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {output} ({frames} frames @ {fps} fps) and {json_path}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("data/fixtures/cctv/video/caltrans_i80_day_clip.avi"),
        help="Output AVI path",
    )
    parser.add_argument("--frames", type=int, default=30)
    parser.add_argument("--fps", type=int, default=10)
    parser.add_argument("--width", type=int, default=320)
    parser.add_argument("--height", type=int, default=240)
    args = parser.parse_args()
    synthesize_clip(args.output, args.frames, args.fps, args.width, args.height)
    return 0


if __name__ == "__main__":
    sys.exit(main())
