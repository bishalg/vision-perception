#!/usr/bin/env python3
"""
scripts/prepare_flood_data.py

Reproducible ingestion script for Sentinel-2 Nepal Flood (2026-08-26) image pair
plus optional pre-flood temporal control (2026-08-12):
1. Copies raw source PNGs to data/flood_nepal_2026/raw/
2. Asserts identical spatial dimensions across all acquisitions
3. Generates binary valid masks (uint8, 255 where alpha >= 250 else 0)
4. Computes and reports valid_fraction for each acquisition
5. Flattens 4-channel BGRA to 3-channel BGR over a solid white background
6. Writes processed BGR copies and valid masks to data/flood_nepal_2026/processed/
"""

import argparse
import os
import shutil
import sys
import numpy as np

try:
    import cv2
except ImportError:
    print("[ERROR] OpenCV Python bindings (cv2) not found.", file=sys.stderr)
    sys.exit(1)


DEFAULT_PRE_SOURCE = (
    "/Users/bishalghimire/Downloads/Flood/RAW/"
    "2026-08-24-00_00_2026-08-24-23_59_Sentinel-2_L2A_True_color.png"
)
DEFAULT_POST_SOURCE = (
    "/Users/bishalghimire/Downloads/Flood/RAW/"
    "2026-08-27-00_00_2026-08-27-23_59_Sentinel-2_L2A_True_color.png"
)
DEFAULT_CTRL_SOURCE = (
    "/Users/bishalghimire/Downloads/Flood/RAW/"
    "2026-08-12-00_00_2026-08-12-23_59_Sentinel-2_L2A_True_color.png"
)


def compute_valid_mask(img: np.ndarray, alpha_threshold: int = 250) -> np.ndarray:
    """Returns uint8 mask with 255 where alpha >= threshold, else 0."""
    if len(img.shape) == 2 or img.shape[2] == 3:
        return np.full(img.shape[:2], 255, dtype=np.uint8)
    alpha = img[:, :, 3]
    mask = (alpha >= alpha_threshold).astype(np.uint8) * 255
    return mask


def flatten_bgra_to_bgr(img: np.ndarray) -> np.ndarray:
    """Flattens 4-channel BGRA onto a solid white background (255, 255, 255)."""
    if len(img.shape) == 2:
        return cv2.cvtColor(img, cv2.COLOR_GRAY2BGR)
    if img.shape[2] == 3:
        return img
    if img.shape[2] == 4:
        alpha = img[:, :, 3].astype(np.float32) / 255.0
        bgr = img[:, :, :3].astype(np.float32)
        white = np.full_like(bgr, 255.0)
        flattened = (bgr * alpha[:, :, None] + white * (1.0 - alpha[:, :, None])).round().astype(np.uint8)
        return flattened
    raise ValueError(f"Unexpected image shape: {img.shape}")


def main():
    parser = argparse.ArgumentParser(description="Ingest Sentinel-2 flood image pair & control.")
    parser.add_argument("--pre", default=DEFAULT_PRE_SOURCE, help="Path to PRE flood PNG (2026-08-24)")
    parser.add_argument("--post", default=DEFAULT_POST_SOURCE, help="Path to POST flood PNG (2026-08-27)")
    parser.add_argument("--ctrl", default=DEFAULT_CTRL_SOURCE, help="Path to CTRL flood PNG (2026-08-12)")
    parser.add_argument("--out-dir", default="data/flood_nepal_2026", help="Target flood data root")
    args = parser.parse_args()

    raw_dir = os.path.join(args.out_dir, "raw")
    proc_dir = os.path.join(args.out_dir, "processed")
    out_dir = os.path.join(args.out_dir, "out")

    os.makedirs(raw_dir, exist_ok=True)
    os.makedirs(proc_dir, exist_ok=True)
    os.makedirs(out_dir, exist_ok=True)

    targets = [
        ("pre", args.pre, "pre_20260824.png", "pre_20260824_bgr.png", "pre_valid_mask.png"),
        ("post", args.post, "post_20260827.png", "post_20260827_bgr.png", "post_valid_mask.png"),
    ]

    if os.path.isfile(args.ctrl):
        targets.append(("ctrl", args.ctrl, "ctrl_20260812.png", "ctrl_20260812_bgr.png", "ctrl_valid_mask.png"))
    else:
        print(f"[INFO] Optional temporal control file {args.ctrl} not found. Skipping control ingest.")

    base_shape = None

    for label, src_path, raw_name, proc_bgr_name, mask_name in targets:
        if not os.path.isfile(src_path):
            print(f"[ERROR] Required input {label.upper()} missing: {src_path}", file=sys.stderr)
            sys.exit(1)

        raw_dst = os.path.join(raw_dir, raw_name)
        print(f"[INGEST] Copying {label.upper()}: {src_path} -> {raw_dst}")
        shutil.copy2(src_path, raw_dst)

        img = cv2.imread(raw_dst, cv2.IMREAD_UNCHANGED)
        if img is None:
            print(f"[ERROR] Failed to read {raw_dst}", file=sys.stderr)
            sys.exit(1)

        h, w = img.shape[:2]
        channels = img.shape[2] if len(img.shape) > 2 else 1
        print(f"[VERIFY] {label.upper()} shape: {w}x{h}, {channels} channels")

        if base_shape is None:
            base_shape = (h, w)
        else:
            assert (h, w) == base_shape, (
                f"Dimension mismatch for {label}: expected {base_shape[1]}x{base_shape[0]}, got {w}x{h}"
            )

        # 1. Valid-pixel mask (alpha >= 250)
        valid_mask = compute_valid_mask(img, alpha_threshold=250)
        valid_pixels = int(np.count_nonzero(valid_mask))
        total_pixels = int(valid_mask.size)
        valid_fraction = valid_pixels / total_pixels

        mask_dst = os.path.join(proc_dir, mask_name)
        cv2.imwrite(mask_dst, valid_mask)
        print(f"[MASK]   {label.upper()} valid mask: {valid_pixels}/{total_pixels} valid ({valid_fraction:.4%}) -> {mask_dst}")

        # 2. BGR flattened image
        bgr = flatten_bgra_to_bgr(img)
        proc_bgr_dst = os.path.join(proc_dir, proc_bgr_name)
        cv2.imwrite(proc_bgr_dst, bgr)

        # Standard alias
        std_dst = os.path.join(proc_dir, f"{label}_image.png")
        cv2.imwrite(std_dst, bgr)
        print(f"[SAVED]  {label.upper()} BGR flattened: {bgr.shape[1]}x{bgr.shape[0]}x{bgr.shape[2]} -> {proc_bgr_dst}")

    print("\n[SUCCESS] Phase 1 ingestion & valid-pixel mask generation complete.")


if __name__ == "__main__":
    main()
