#!/usr/bin/env python3
"""
cctv_fetcher.py - Public CCTV Snapshot Fetcher & Mock Fixture Generator
-----------------------------------------------------------------------
Fetches static snapshot frames from open municipal CCTV camera networks
(Caltrans, City of Austin, Transport for London) or generates calibrated
mock fixtures for offline local development and CI testing.
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

# Built-in public snapshot feeds
PUBLIC_CCTV_REGISTRY = {
    "caltrans_i80_drum": {
        "agency": "Caltrans District 3",
        "name": "I-80 at Drum Forebay",
        "location": {"lat": 39.3142, "lon": -120.7321, "elevation_m": 1420.0},
        "pose": {"heading_deg": 65.0, "pitch_deg": -15.0, "fov_deg": 55.0},
        "url": "https://cwwp2.dot.ca.gov/data/d3/cctv/image/i80atdrumforebay/i80atdrumforebay.jpg",
    },
    "caltrans_us101_sf": {
        "agency": "Caltrans District 4",
        "name": "US-101 at Cesar Chavez, San Francisco",
        "location": {"lat": 37.7485, "lon": -122.4042, "elevation_m": 15.0},
        "pose": {"heading_deg": 350.0, "pitch_deg": -10.0, "fov_deg": 60.0},
        "url": "https://cwwp2.dot.ca.gov/data/d4/cctv/image/us101cesarchavez/us101cesarchavez.jpg",
    },
    "austin_congress_6th": {
        "agency": "City of Austin Mobility",
        "name": "Congress Ave at 6th St",
        "location": {"lat": 30.2683, "lon": -97.7428, "elevation_m": 150.0},
        "pose": {"heading_deg": 0.0, "pitch_deg": -20.0, "fov_deg": 70.0},
        "url": "https://data.austintexas.gov/views/b4k4-adkb/files/sample_snap.jpg",
    },
    "bhote_koshi_pier": {
        "agency": "Nepal Hydropower / DHM",
        "name": "Bhote Koshi Bridge Pier Station",
        "location": {"lat": 28.2734, "lon": 85.3807, "elevation_m": 1820.0},
        "pose": {"heading_deg": 185.0, "pitch_deg": -25.0, "fov_deg": 65.0},
        "url": "mock://nepal_flood_pier",
    },
}


def create_mock_fixture(camera_id: str, output_path: Path, mode: str = "day_clear") -> None:
    """
    Creates an uncompressed mock PPM/PGM or draws via basic format when OpenCV/PIL is not guaranteed.
    Generates a valid binary PPM image with drawn road/pier curvilinear lines.
    """
    width, height = 640, 480
    output_path.parent.mkdir(parents=True, exist_ok=True)

    # Generate a PPM (P6 binary) image with curvilinear features
    header = f"P6\n{width} {height}\n255\n".encode("ascii")
    pixels = bytearray(width * height * 3)

    is_night = "night" in mode
    is_rain = "rain" in mode
    is_flood = "pier" in camera_id or "flood" in mode

    base_val = 30 if is_night else 140
    if is_rain:
        base_val = 110

    # Fill background
    for i in range(width * height):
        noise = (i * 17 + (i >> 3)) % 15
        val = max(0, min(255, base_val + noise))
        pixels[i * 3] = val
        pixels[i * 3 + 1] = val
        pixels[i * 3 + 2] = val

    # Draw high-contrast curvilinear features (lane dividers or water pier marks)
    for y in range(height):
        # Curved line 1: x = 200 + 40 * sin(y / 60)
        import math
        x1 = int(220 + 50.0 * math.sin(y / 70.0) + (y * 0.3))
        x2 = int(420 - 40.0 * math.cos(y / 80.0) - (y * 0.2))

        for x_center in (x1, x2):
            for dx in range(-2, 3):
                px = x_center + dx
                if 0 <= px < width:
                    idx = (y * width + px) * 3
                    if is_flood:
                        # Waterline / dark sediment line
                        pixels[idx] = 40
                        pixels[idx + 1] = 60
                        pixels[idx + 2] = 80
                    else:
                        # Bright highway lane line
                        line_val = 250 if not is_night else 200
                        pixels[idx] = line_val
                        pixels[idx + 1] = line_val
                        pixels[idx + 2] = 20 if not is_night else line_val

    # If rain mode, simulate blur by averaging neighbor pixels
    if is_rain:
        blurred = bytearray(pixels)
        for y in range(1, height - 1):
            for x in range(1, width - 1):
                idx = (y * width + x) * 3
                for c in range(3):
                    avg = (
                        pixels[((y - 1) * width + x) * 3 + c]
                        + pixels[((y + 1) * width + x) * 3 + c]
                        + pixels[(y * width + (x - 1)) * 3 + c]
                        + pixels[(y * width + (x + 1)) * 3 + c]
                        + pixels[idx + c] * 4
                    ) // 8
                    blurred[idx + c] = avg
        pixels = blurred

    # If target is .png, convert via cv2 if available, else write ppm
    try:
        import cv2
        import numpy as np
        img_np = np.frombuffer(pixels, dtype=np.uint8).reshape((height, width, 3))
        cv2.imwrite(str(output_path), img_np)
    except ImportError:
        # Fallback to direct PPM or raw write
        ppm_path = output_path.with_suffix(".ppm")
        with open(ppm_path, "wb") as f:
            f.write(header)
            f.write(pixels)
        if output_path != ppm_path:
            # Copy if needed
            os.replace(ppm_path, output_path)

    # Write accompanying metadata JSON
    meta_path = output_path.with_suffix(".json")
    camera_info = PUBLIC_CCTV_REGISTRY.get(
        camera_id,
        {
            "agency": "Custom",
            "name": camera_id,
            "location": {"lat": 0.0, "lon": 0.0, "elevation_m": 0.0},
            "pose": {"heading_deg": 0.0, "pitch_deg": 0.0, "fov_deg": 60.0},
        },
    )
    with open(meta_path, "w", encoding="utf-8") as mf:
        json.dump(camera_info, mf, indent=2)


def fetch_live_frame(camera_id: str, output_path: Path, timeout: float = 8.0) -> bool:
    """Fetches a real frame from a public CCTV endpoint."""
    if camera_id not in PUBLIC_CCTV_REGISTRY:
        print(f"[-] Unknown camera ID: {camera_id}", file=sys.stderr)
        return False

    url = PUBLIC_CCTV_REGISTRY[camera_id]["url"]
    if url.startswith("mock://"):
        create_mock_fixture(camera_id, output_path)
        return True

    headers = {"User-Agent": "Mozilla/5.0 (vision-perception/1.0 CCTV Fetcher)"}
    req = urllib.request.Request(url, headers=headers)

    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            data = response.read()
            output_path.parent.mkdir(parents=True, exist_ok=True)
            with open(output_path, "wb") as f:
                f.write(data)
            # Save metadata
            meta_path = output_path.with_suffix(".json")
            with open(meta_path, "w", encoding="utf-8") as mf:
                json.dump(PUBLIC_CCTV_REGISTRY[camera_id], mf, indent=2)
            print(f"[✓] Successfully downloaded snapshot from {camera_id} -> {output_path}")
            return True
    except (urllib.error.URLError, TimeoutError) as e:
        print(f"[!] Network error fetching {camera_id}: {e}. Falling back to calibrated mock fixture.")
        create_mock_fixture(camera_id, output_path)
        return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Public CCTV Snapshot Fetcher & Fixture Generator")
    parser.add_argument("--camera", "-c", default="caltrans_i80_drum", help="Camera identifier from registry")
    parser.add_argument("--output", "-o", default="data/fixtures/cctv/day_clear/caltrans_i80_day.png", help="Output file path")
    parser.add_argument("--mock", action="store_true", help="Generate calibrated mock fixture without network")
    parser.add_argument("--mode", default="day_clear", choices=["day_clear", "night_lowlux", "weather_rain"], help="Fixture visual condition")
    parser.add_argument("--list", action="store_true", help="List registered public CCTV cameras")

    args = parser.parse_args()

    if args.list:
        print(json.dumps(PUBLIC_CCTV_REGISTRY, indent=2))
        return 0

    out_path = Path(args.output)
    if args.mock:
        create_mock_fixture(args.camera, out_path, mode=args.mode)
        print(f"[✓] Created mock fixture: {out_path}")
        return 0

    success = fetch_live_frame(args.camera, out_path)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
