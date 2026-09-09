#!/usr/bin/env python3
"""Dump palm major-line metrics from mobile_captures evidence JSON.

Used to refresh tables in docs/palm_crease_baseline.md after a tuning pass.

Usage (repo root):
  python3 scripts/palm_crease_baseline_metrics.py
  python3 scripts/palm_crease_baseline_metrics.py --markdown
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = ROOT / "data" / "samples" / "mobile_captures"
LINE_ORDER = ("heart_line", "head_line", "life_line", "fate_line")


def summarize_line(info: dict) -> dict:
    pts = info.get("points_canonical_norm") or []
    img = info.get("points_image_norm") or []
    if pts:
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        span_x = max(xs) - min(xs)
        y0, y1 = min(ys), max(ys)
    else:
        span_x = 0.0
        y0 = y1 = 0.0
    return {
        "present": bool(info.get("present")),
        "confidence": float(info.get("confidence") or 0.0),
        "arc_px": float(info.get("arc_length_px") or 0.0),
        "segs": len(info.get("segment_ids") or []),
        "n_img": len(img),
        "span_x": span_x,
        "y0": y0,
        "y1": y1,
    }


def load_fixtures() -> list[tuple[str, dict]]:
    rows: list[tuple[str, dict]] = []
    for path in sorted(FIXTURE_DIR.glob("*_evidence.json")):
        data = json.loads(path.read_text())
        payload = data.get("domain_payload") or {}
        lines = payload.get("major_lines") or {}
        stem = path.name.replace("_evidence.json", "")
        rows.append((stem, lines))
    return rows


def print_text(rows: list[tuple[str, dict]]) -> None:
    for stem, lines in rows:
        print(f"\n=== {stem} ===")
        for name in LINE_ORDER:
            info = lines.get(name)
            if not info:
                print(f"  {name}: MISSING")
                continue
            s = summarize_line(info)
            print(
                f"  {name}: present={s['present']} conf={s['confidence']:.3f} "
                f"arc={s['arc_px']:.1f} segs={s['segs']} n_img={s['n_img']} "
                f"span_x={s['span_x']:.3f} y=[{s['y0']:.3f},{s['y1']:.3f}]"
            )


def print_markdown(rows: list[tuple[str, dict]]) -> None:
    print("| Fixture | Line | present | arc_px | segs | n_img | canon span_x | canon y range |")
    print("| :--- | :--- | :---: | ---: | ---: | ---: | ---: | :--- |")
    for stem, lines in rows:
        for name in LINE_ORDER:
            info = lines.get(name) or {}
            s = summarize_line(info)
            short = name.replace("_line", "")
            present = "yes" if s["present"] else "no"
            yrange = f"{s['y0']:.3f}–{s['y1']:.3f}" if s["present"] else "—"
            print(
                f"| {stem} | {short} | {present} | {s['arc_px']:.1f} | {s['segs']} | "
                f"{s['n_img']} | {s['span_x']:.3f} | {yrange} |"
            )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--markdown",
        action="store_true",
        help="Emit a markdown table for pasting into docs/palm_crease_baseline.md",
    )
    args = parser.parse_args()
    rows = load_fixtures()
    if not rows:
        raise SystemExit(f"No *_evidence.json under {FIXTURE_DIR}")
    if args.markdown:
        print_markdown(rows)
    else:
        print_text(rows)


if __name__ == "__main__":
    main()
