#!/usr/bin/env python3
"""Normative Baseline Aggregator.

Consumes de-identified session exports (vault JSONL lines with
``{"type": "per_video_score", ...}`` and ``{"type": "norm_collection_meta", ...}``),
aggregates observed per-video biomarker values by stimulus x biomarker x age
bracket, and emits a candidate normative baseline JSON (same schema as
``normative_baselines.json``) for human review before promotion into the app.

Cells with fewer than ``--min-n`` observations are withheld (null ranges) so a
handful of sessions can never masquerade as a norm. Imputed rows (target never
fixated) are excluded: worst-case sentinels would skew the distribution.

Usage:
    python3 tools/asd_norm_aggregator.py --input ./session_exports \
        --output ./candidate_normative_baselines.json [--min-n 12]
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import defaultdict
from pathlib import Path
from statistics import mean, median, pstdev
from typing import Dict, List, Optional, Tuple

# Direction inference for candidate expectations. Latency and flat-affect metrics
# are "lower is better"; everything else defaults to higher-is-better.
LOWER_IS_BETTER = {"first_entry_latency_ms", "flat_affect_score"}

DEFAULT_AGE_BRACKETS = [
    {"id": "12-17", "minMonths": 12, "maxMonths": 17},
    {"id": "18-23", "minMonths": 18, "maxMonths": 23},
    {"id": "24-35", "minMonths": 24, "maxMonths": 35},
    {"id": "36-47", "minMonths": 36, "maxMonths": 47},
    {"id": "48-60", "minMonths": 48, "maxMonths": 60},
]


def bracket_id_for(age_months: int, brackets: List[dict]) -> Optional[str]:
    for bracket in brackets:
        if bracket["minMonths"] <= age_months <= bracket["maxMonths"]:
            return bracket["id"]
    return None


def load_session_records(path: Path) -> List[dict]:
    """Reads one session file (JSONL vault export or single JSON object)."""
    text = path.read_text(encoding="utf-8").strip()
    if not text:
        return []
    if text.startswith("{") and '"per_video"' in text and text.count("\n") == 0:
        record = json.loads(text)
        return [record]
    records = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return records


def extract_observations(
    session_dir: Path,
    brackets: List[dict],
) -> Dict[Tuple[str, str, str], List[float]]:
    """Aggregates observed values into (stimulus_id, biomarker, bracket_id) buckets.

    Accepts both vault JSONL exports (per_video_score lines) and ResearchKit
    share exports (a single JSON object with a ``per_video`` block).
    """
    buckets: Dict[Tuple[str, str, str], List[float]] = defaultdict(list)

    for path in sorted(session_dir.rglob("*.json*")):
        try:
            records = load_session_records(path)
        except (json.JSONDecodeError, OSError):
            continue

        for record in records:
            score = None
            if isinstance(record, dict) and record.get("type") == "per_video_score":
                score = record.get("score")
            elif isinstance(record, dict) and "per_video" in record:
                score = record["per_video"]
            if not isinstance(score, dict):
                continue

            age_months = int(score.get("ageMonths", 0) or 0)
            bracket = bracket_id_for(age_months, brackets)
            if bracket is None:
                continue

            for video in score.get("videos", []):
                stimulus_id = video.get("stimulusId", "")
                for row in video.get("rows", []):
                    if row.get("imputed"):
                        continue
                    biomarker = row.get("biomarker", "")
                    value = row.get("observed")
                    if biomarker and isinstance(value, (int, float)):
                        buckets[(stimulus_id, biomarker, bracket)].append(float(value))

    return buckets


def percentile(sorted_values: List[float], pct: float) -> float:
    if not sorted_values:
        return 0.0
    index = (len(sorted_values) - 1) * pct
    low = math.floor(index)
    high = math.ceil(index)
    if low == high:
        return sorted_values[low]
    return sorted_values[low] + (sorted_values[high] - sorted_values[low]) * (index - low)


def build_candidate_baseline(
    buckets: Dict[Tuple[str, str, str], List[float]],
    min_n: int,
    brackets: List[dict],
) -> dict:
    """Builds a candidate baseline document; cells below min-n are withheld (None)."""
    grouped: Dict[Tuple[str, str], Dict[str, List[float]]] = defaultdict(lambda: defaultdict(list))
    for (stimulus_id, biomarker, bracket), values in buckets.items():
        grouped[(stimulus_id, biomarker)][bracket] = values

    videos: Dict[str, dict] = {}
    for (stimulus_id, biomarker), by_bracket in sorted(grouped.items()):
        video_entry = videos.setdefault(
            stimulus_id,
            {
                "title": stimulus_id,
                "category": "Candidate Norm",
                "weight": 0.0,
                "segments": [
                    {
                        "id": "candidate",
                        "description": "Aggregated from collected sessions; segment boundaries pending review.",
                        "startSec": 0.0,
                        "endSec": 0.0,
                        "expectations": [],
                    }
                ],
            },
        )
        by_age: Dict[str, Optional[dict]] = {}
        for bracket in brackets:
            values = by_bracket.get(bracket["id"], [])
            if len(values) < min_n:
                by_age[bracket["id"]] = None
                continue
            ordered = sorted(values)
            spread = max(ordered) - min(ordered)
            # Withhold degenerate cells (constant values carry no distribution).
            if spread <= 0.0:
                by_age[bracket["id"]] = None
                continue
            expected = median(ordered)
            entry: dict = {"expected": round(expected, 4), "n": len(values)}
            if biomarker in LOWER_IS_BETTER:
                entry["acceptableHigh"] = round(percentile(ordered, 0.90), 4)
            else:
                entry["acceptableLow"] = round(percentile(ordered, 0.10), 4)
            by_age[bracket["id"]] = entry

        direction = "lowerBetter" if biomarker in LOWER_IS_BETTER else "higherBetter"
        video_entry["segments"][0]["expectations"].append(
            {
                "biomarker": biomarker,
                "direction": direction,
                "targetAOI": None,
                "byAge": by_age,
                "citation": (
                    f"Candidate norm from collected sessions (min n={min_n} per cell); "
                    "requires clinical review before promotion."
                ),
                "source": "collected-candidate",
            }
        )

    return {
        "schemaVersion": "1.0.0-candidate",
        "status": "candidate-collected",
        "provenance": (
            "Aggregated by tools/asd_norm_aggregator.py from consented norm-collection "
            "sessions. NOT approved for on-device scoring until reviewed and promoted."
        ),
        "ageBrackets": brackets,
        "videos": videos,
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Aggregate collected sessions into a candidate normative baseline.")
    parser.add_argument("--input", required=True, help="Directory of session exports (JSONL or ResearchKit JSON).")
    parser.add_argument("--output", default="candidate_normative_baselines.json", help="Output path for the candidate baseline.")
    parser.add_argument("--min-n", type=int, default=12, help="Minimum observations per cell to emit a value.")
    args = parser.parse_args(argv)

    session_dir = Path(args.input)
    if not session_dir.is_dir():
        print(f"error: input directory not found: {session_dir}", file=sys.stderr)
        return 2

    buckets = extract_observations(session_dir, DEFAULT_AGE_BRACKETS)
    candidate = build_candidate_baseline(buckets, args.min_n, DEFAULT_AGE_BRACKETS)

    output_path = Path(args.output)
    output_path.write_text(json.dumps(candidate, indent=2) + "\n", encoding="utf-8")

    total_cells = sum(
        1
        for expectation in (
            e for v in candidate["videos"].values() for s in v["segments"] for e in s["expectations"]
        )
        for value in expectation["byAge"].values()
        if value is not None
    )
    print(f"aggregated {sum(len(v) for v in buckets.values())} observations "
          f"into {total_cells} qualifying cells (min n={args.min_n}) -> {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
