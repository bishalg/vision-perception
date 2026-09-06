#!/usr/bin/env python3
"""
scripts/sweep_sigma.py

Runs a documented scale-space sigma sweep on the POST flood image
to identify the optimal Steger ridge extraction parameters for wide river corridors.
"""

import json
import os
import subprocess
import sys

POST_IMAGE = "data/flood_nepal_2026/processed/post_20260827_bgr.png"
SWEEP_DIR = "data/flood_nepal_2026/out/sweep"
CLI_BIN = "./build/tools/curv_cli"

CONFIGS = [
    {"name": "thin_default", "sigma": 1.5, "low": 0.5, "high": 1.5, "min_blur": 15.0},
    {"name": "medium",       "sigma": 3.0, "low": 0.3, "high": 1.0, "min_blur": 15.0},
    {"name": "wide",         "sigma": 5.0, "low": 0.2, "high": 0.8, "min_blur": 15.0},
    {"name": "ultrawide",    "sigma": 8.0, "low": 0.1, "high": 0.5, "min_blur": 15.0},
    {"name": "macro",        "sigma": 12.0, "low": 0.05, "high": 0.3, "min_blur": 15.0},
]

def main():
    os.makedirs(SWEEP_DIR, exist_ok=True)
    results = []

    for cfg in CONFIGS:
        name = cfg["name"]
        overlay_path = os.path.join(SWEEP_DIR, f"overlay_{name}.png")
        json_path = os.path.join(SWEEP_DIR, f"evidence_{name}.json")

        cmd = [
            CLI_BIN,
            "--image", POST_IMAGE,
            "--sigma", str(cfg["sigma"]),
            "--low", str(cfg["low"]),
            "--high", str(cfg["high"]),
            "--min-blur", str(cfg["min_blur"]),
            "--out", overlay_path,
            "--json", json_path,
        ]

        print(f"[RUNNING] {name}: sigma={cfg['sigma']}, low={cfg['low']}, high={cfg['high']}")
        proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if proc.returncode != 0:
            print(f"[ERROR] {name} failed:\n{proc.stderr}", file=sys.stderr)
            continue

        # Parse output evidence
        if not os.path.isfile(json_path):
            print(f"[ERROR] {json_path} not found", file=sys.stderr)
            continue

        with open(json_path, "r") as f:
            evidence_doc = json.load(f)

        # Extract metrics
        segments = []
        for ev in evidence_doc.get("evidence", []):
            if ev.get("type") == "line_candidate":
                geom = ev.get("geometry", {})
                segments.append(geom)

        total_length = sum(s.get("total_length", 0.0) for s in segments)
        max_length = max((s.get("total_length", 0.0) for s in segments), default=0.0)
        mean_length = total_length / len(segments) if segments else 0.0

        duration_us = evidence_doc.get("total_duration_us", 0)

        record = {
            "name": name,
            "sigma": cfg["sigma"],
            "low": cfg["low"],
            "high": cfg["high"],
            "segment_count": len(segments),
            "total_length_px": round(total_length, 2),
            "max_segment_length_px": round(max_length, 2),
            "mean_segment_length_px": round(mean_length, 2),
            "duration_ms": round(duration_us / 1000.0, 2),
            "overlay": overlay_path,
            "json": json_path,
        }
        results.append(record)
        print(f"  -> Segments: {len(segments)}, Total Length: {record['total_length_px']} px, "
              f"Max Seg: {record['max_segment_length_px']} px, Mean Seg: {record['mean_segment_length_px']} px, "
              f"Duration: {record['duration_ms']} ms")


    sweep_report_path = os.path.join(SWEEP_DIR, "sweep_results.json")
    with open(sweep_report_path, "w") as f:
        json.dump(results, f, indent=2)

    print(f"\n[REPORT] Sweep results saved to {sweep_report_path}")

if __name__ == "__main__":
    main()
