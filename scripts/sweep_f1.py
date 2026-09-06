#!/usr/bin/env python3
"""
scripts/sweep_f1.py

Executes a 10-configuration Scale x Polarity sweep:
  sigma in {1.5, 3.0, 5.0, 8.0, 12.0} x extract_dark_lines in {false, true}
Evaluates weakly-supervised F1 against dilated color flood masks (water + sediment, cloud-excluded)
restricted to valid pixels for both PRE and POST images.
Selects the optimal config maximizing Score = F1_post + F1_pre.
"""

import json
import os
import subprocess
import sys
import cv2
import numpy as np

CLI_BIN = "./build/tools/curv_cli"
PRE_IMAGE = "data/flood_nepal_2026/processed/pre_20260824_bgr.png"
POST_IMAGE = "data/flood_nepal_2026/processed/post_20260827_bgr.png"
PRE_MASK = "data/flood_nepal_2026/processed/pre_valid_mask.png"
POST_MASK = "data/flood_nepal_2026/processed/post_valid_mask.png"
SWEEP_DIR = "data/flood_nepal_2026/out/sweep_10"
CONFIG_PATH = "configs/geospatial_wide_ridge.json"


SIGMAS = [1.5, 3.0, 5.0, 8.0, 12.0]
POLARITIES = [("bright", False), ("dark", True)]

THRESHOLDS = {
    1.5: (0.5, 1.5),
    3.0: (0.3, 1.0),
    5.0: (0.2, 0.8),
    8.0: (0.1, 0.5),
    12.0: (0.05, 0.3),
}


def build_color_flood_mask(img_bgr: np.ndarray, valid_mask: np.ndarray) -> np.ndarray:
    """Computes color-based flood mask (water + sediment) with cloud exclusion, restricted to valid pixels."""
    hsv = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2HSV)
    lab = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2LAB)

    # 1. Water mask (HSV blue-cyan channel)
    water_mask = cv2.inRange(hsv, np.array([90, 20, 20]), np.array([140, 255, 180]))

    # 2. Sediment mask (Lab yellowish-brown turbid deposits)
    sediment_mask = cv2.inRange(lab, np.array([100, 125, 130]), np.array([220, 145, 170]))

    # 3. Cloud mask (HSV high-V, low-S)
    cloud_mask = cv2.inRange(hsv, np.array([0, 0, 200]), np.array([180, 30, 255]))

    # Combined flood mask excluding clouds and non-valid pixels
    flood_mask = cv2.bitwise_or(water_mask, sediment_mask)
    flood_mask = cv2.bitwise_and(flood_mask, cv2.bitwise_not(cloud_mask))
    flood_mask = cv2.bitwise_and(flood_mask, valid_mask)
    return flood_mask


def rasterize_curves(evidence_json_path: str, shape: tuple) -> np.ndarray:
    """Rasterizes sub-pixel curve points from evidence JSON into a binary image."""
    canvas = np.zeros(shape[:2], dtype=np.uint8)
    if not os.path.isfile(evidence_json_path):
        return canvas

    with open(evidence_json_path, "r") as f:
        doc = json.load(f)

    for ev in doc.get("evidence", []):
        if ev.get("type") == "line_candidate":
            geom = ev.get("geometry", {})
            for pt in geom.get("points", []):
                x = int(round(pt.get("x", 0.0)))
                y = int(round(pt.get("y", 0.0)))
                if 0 <= y < canvas.shape[0] and 0 <= x < canvas.shape[1]:
                    canvas[y, x] = 255

    return canvas


def compute_weak_f1(curve_img: np.ndarray, flood_mask: np.ndarray, valid_mask: np.ndarray) -> float:
    """Computes F1 score between 3px-dilated curves and 3px-dilated flood mask within valid pixels."""
    kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (3, 3))
    dilated_curves = cv2.dilate(curve_img, kernel, iterations=1)
    dilated_flood = cv2.dilate(flood_mask, kernel, iterations=1)

    # Restrict to valid pixels
    curves_valid = (dilated_curves > 0) & (valid_mask > 0)
    flood_valid = (dilated_flood > 0) & (valid_mask > 0)

    intersection = np.count_nonzero(curves_valid & flood_valid)
    num_curves = np.count_nonzero(curves_valid)
    num_flood = np.count_nonzero(flood_valid)

    if num_curves == 0 or num_flood == 0:
        return 0.0

    precision = intersection / num_curves
    recall = intersection / num_flood

    if precision + recall == 0:
        return 0.0

    return float(2.0 * precision * recall / (precision + recall))


def extract_curve_stats(evidence_json_path: str):
    if not os.path.isfile(evidence_json_path):
        return 0, 0.0, 0.0, 0.0
    with open(evidence_json_path, "r") as f:
        doc = json.load(f)
    segments = [e["geometry"] for e in doc.get("evidence", []) if e.get("type") == "line_candidate"]
    tot_len = sum(s.get("total_length", 0.0) for s in segments)
    max_len = max((s.get("total_length", 0.0) for s in segments), default=0.0)
    mean_len = tot_len / len(segments) if segments else 0.0
    return len(segments), round(tot_len, 2), round(max_len, 2), round(mean_len, 2)


def main():
    os.makedirs(SWEEP_DIR, exist_ok=True)

    img_pre = cv2.imread(PRE_IMAGE)
    img_post = cv2.imread(POST_IMAGE)
    mask_pre = cv2.imread(PRE_MASK, cv2.IMREAD_GRAYSCALE)
    mask_post = cv2.imread(POST_MASK, cv2.IMREAD_GRAYSCALE)

    flood_mask_pre = build_color_flood_mask(img_pre, mask_pre)
    flood_mask_post = build_color_flood_mask(img_post, mask_post)

    results = []

    print("=" * 80)
    print("STARTING 10-CONFIGURATION SCALE x POLARITY SWEEP")
    print("=" * 80)

    for sigma in SIGMAS:
        low, high = THRESHOLDS[sigma]
        for pol_name, is_dark in POLARITIES:
            cfg_name = f"s{sigma}_{pol_name}"
            pre_json = os.path.join(SWEEP_DIR, f"pre_{cfg_name}.json")
            post_json = os.path.join(SWEEP_DIR, f"post_{cfg_name}.json")
            pre_out = os.path.join(SWEEP_DIR, f"pre_{cfg_name}.png")
            post_out = os.path.join(SWEEP_DIR, f"post_{cfg_name}.png")

            # 1. Run PRE
            cmd_pre = [
                CLI_BIN, "--image", PRE_IMAGE,
                "--sigma", str(sigma), "--low", str(low), "--high", str(high),
                "--min-blur", "15.0", "--out", pre_out, "--json", pre_json
            ]
            if is_dark:
                cmd_pre.append("--dark")
            subprocess.run(cmd_pre, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

            # 2. Run POST
            cmd_post = [
                CLI_BIN, "--image", POST_IMAGE,
                "--sigma", str(sigma), "--low", str(low), "--high", str(high),
                "--min-blur", "15.0", "--out", post_out, "--json", post_json
            ]
            if is_dark:
                cmd_post.append("--dark")
            subprocess.run(cmd_post, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

            # 3. Compute F1 on both
            curves_pre = rasterize_curves(pre_json, img_pre.shape)
            curves_post = rasterize_curves(post_json, img_post.shape)

            f1_pre = compute_weak_f1(curves_pre, flood_mask_pre, mask_pre)
            f1_post = compute_weak_f1(curves_post, flood_mask_post, mask_post)
            joint_score = f1_pre + f1_post

            segs_pre, len_pre, max_pre, mean_pre = extract_curve_stats(pre_json)
            segs_post, len_post, max_post, mean_post = extract_curve_stats(post_json)

            record = {
                "name": cfg_name,
                "sigma": sigma,
                "polarity": pol_name,
                "extract_dark_lines": is_dark,
                "low": low,
                "high": high,
                "f1_pre": round(f1_pre, 4),
                "f1_post": round(f1_post, 4),
                "joint_score": round(joint_score, 4),
                "pre_segments": segs_pre,
                "pre_total_length_px": len_pre,
                "post_segments": segs_post,
                "post_total_length_px": len_post,
                "combined_length_px": round(len_pre + len_post, 2)
            }
            results.append(record)
            print(f"[{cfg_name:12s}] F1_pre={f1_pre:.4f}, F1_post={f1_post:.4f} -> Joint Score: {joint_score:.4f} | PRE: {segs_pre:4d} segs ({len_pre:7.1f}px), POST: {segs_post:4d} segs ({len_post:7.1f}px)")

    # Sort results by joint_score desc, then combined_length_px desc
    results.sort(key=lambda r: (r["joint_score"], r["combined_length_px"]), reverse=True)
    winner = results[0]

    print("\n" + "=" * 80)
    print(f"WINNING CONFIGURATION: {winner['name']} (Score: {winner['joint_score']:.4f})")
    print(f"  sigma={winner['sigma']}, polarity={winner['polarity']}, low={winner['low']}, high={winner['high']}")
    print(f"  F1(PRE)={winner['f1_pre']:.4f}, F1(POST)={winner['f1_post']:.4f}")
    print("=" * 80)

    sweep_out_json = os.path.join(SWEEP_DIR, "sweep_10_results.json")
    with open(sweep_out_json, "w") as f:
        json.dump(results, f, indent=2)

    # Save to configs/geospatial_wide_ridge.json
    config_data = {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "name": "geospatial_wide_ridge",
        "description": "Winning scale-space Steger config from 10-config F1 sweep for Sentinel-2 10m flood corridor extraction",
        "version": "0.2.0",
        "parameters": {
            "sigma": winner["sigma"],
            "low_threshold": winner["low"],
            "high_threshold": winner["high"],
            "min_blur_score": 15.0,
            "extract_dark_lines": winner["extract_dark_lines"],
            "polarity": winner["polarity"],
            "min_segment_length": 5.0
        },
        "weak_supervision_metrics": {
            "winning_config": winner["name"],
            "f1_score_pre": winner["f1_pre"],
            "f1_score_post": winner["f1_post"],
            "joint_score": winner["joint_score"],
            "all_configs_tested": len(results)
        }
    }
    with open(CONFIG_PATH, "w") as f:
        json.dump(config_data, f, indent=2)

    print(f"[SAVED] Updated {CONFIG_PATH} with winning configuration.")


if __name__ == "__main__":
    main()
