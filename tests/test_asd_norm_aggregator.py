#!/usr/bin/env python3
"""Tests for the normative baseline aggregator (tools/asd_norm_aggregator.py)."""

import json
import math
from pathlib import Path

import pytest

from tools.asd_norm_aggregator import (
    DEFAULT_AGE_BRACKETS,
    bracket_id_for,
    build_candidate_baseline,
    extract_observations,
    load_session_records,
    percentile,
)


def make_score_record(session_id: str, age_months: int, rows_by_video: dict) -> dict:
    return {
        "type": "per_video_score",
        "score": {
            "ageBracketId": bracket_id_for(age_months, DEFAULT_AGE_BRACKETS),
            "ageMonths": age_months,
            "compositeScore": 0.7,
            "baselineStatus": "provisional-literature-seeded",
            "baselineSchemaVersion": "1.0.0",
            "scoredAtUtc": "2026-09-28T00:00:00Z",
            "videos": [
                {
                    "stimulusId": stimulus,
                    "title": stimulus,
                    "subscore": 0.8,
                    "hasBaseline": True,
                    "lowConfidence": False,
                    "validSampleRatio": 0.9,
                    "rows": rows,
                }
                for stimulus, rows in rows_by_video.items()
            ],
        },
    }


def write_jsonl(path: Path, lines: list) -> None:
    path.write_text("\n".join(json.dumps(line) for line in lines) + "\n", encoding="utf-8")


class TestBracketResolution:
    def test_inside_bracket(self):
        assert bracket_id_for(20, DEFAULT_AGE_BRACKETS) == "18-23"
        assert bracket_id_for(48, DEFAULT_AGE_BRACKETS) == "48-60"

    def test_outside_all_brackets_returns_none(self):
        assert bracket_id_for(6, DEFAULT_AGE_BRACKETS) is None
        assert bracket_id_for(72, DEFAULT_AGE_BRACKETS) is None


class TestPercentile:
    def test_exact_indices(self):
        assert percentile([1.0, 2.0, 3.0, 4.0], 0.5) == 2.5
        assert percentile([1.0, 2.0, 3.0], 0.0) == 1.0
        assert percentile([1.0, 2.0, 3.0], 1.0) == 3.0


class TestExtractObservations:
    def test_jsonl_vault_lines_are_aggregated(self, tmp_path):
        write_jsonl(
            tmp_path / "session_A.jsonl",
            [
                {"schemaVersion": "1.1.0", "sessionId": "A"},
                make_score_record(
                    "A", 24,
                    {"stim_05_geopref_split_10s": [
                        {"segmentId": "s", "biomarker": "social_dwell_ratio", "observed": 0.72, "imputed": False},
                        {"segmentId": "s", "biomarker": "social_dwell_ratio", "observed": 0.68, "imputed": False},
                    ]},
                ),
            ],
        )
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        assert len(buckets[("stim_05_geopref_split_10s", "social_dwell_ratio", "24-35")]) == 2

    def test_imputed_rows_are_excluded(self, tmp_path):
        write_jsonl(
            tmp_path / "session_B.jsonl",
            [make_score_record("B", 20, {"stim_02_gaze_cue_right_8s": [
                {"segmentId": "s", "biomarker": "first_entry_latency_ms", "observed": 8000.0, "imputed": True},
                {"segmentId": "s", "biomarker": "first_entry_latency_ms", "observed": 540.0, "imputed": False},
            ]})],
        )
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        values = buckets[("stim_02_gaze_cue_right_8s", "first_entry_latency_ms", "18-23")]
        assert values == [540.0]

    def test_researchkit_share_export_shape(self, tmp_path):
        record = make_score_record("C", 40, {"stim_01_social_peekaboo_8s": [
            {"segmentId": "s", "biomarker": "eye_dwell_ratio", "observed": 0.33, "imputed": False},
        ]})
        (tmp_path / "session_C.json").write_text(
            json.dumps({"per_video": record["score"]}), encoding="utf-8"
        )
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        assert buckets[("stim_01_social_peekaboo_8s", "eye_dwell_ratio", "36-47")] == [0.33]

    def test_out_of_range_age_is_skipped(self, tmp_path):
        write_jsonl(tmp_path / "session_D.jsonl", [make_score_record("D", 9, {"stim_05_geopref_split_10s": [
            {"segmentId": "s", "biomarker": "social_dwell_ratio", "observed": 0.5, "imputed": False},
        ]})])
        assert extract_observations(tmp_path, DEFAULT_AGE_BRACKETS) == {}


class TestCandidateBaseline:
    def _seed(self, tmp_path: Path, n: int, age_months: int = 24):
        lines = []
        for i in range(n):
            lines.append(make_score_record(f"S{i}", age_months, {"stim_05_geopref_split_10s": [
                {"segmentId": "s", "biomarker": "social_dwell_ratio", "observed": 0.60 + (i % 5) * 0.02, "imputed": False},
            ]}))
        write_jsonl(tmp_path / "seed.jsonl", lines)

    def test_cells_below_min_n_are_withheld(self, tmp_path):
        self._seed(tmp_path, n=5)
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        candidate = build_candidate_baseline(buckets, min_n=12, brackets=DEFAULT_AGE_BRACKETS)
        expectation = candidate["videos"]["stim_05_geopref_split_10s"]["segments"][0]["expectations"][0]
        assert all(value is None for value in expectation["byAge"].values())
        assert expectation["source"] == "collected-candidate"

    def test_qualifying_cells_emit_median_and_p10(self, tmp_path):
        self._seed(tmp_path, n=12)
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        candidate = build_candidate_baseline(buckets, min_n=12, brackets=DEFAULT_AGE_BRACKETS)
        expectation = candidate["videos"]["stim_05_geopref_split_10s"]["segments"][0]["expectations"][0]
        cell = expectation["byAge"]["24-35"]
        assert cell is not None
        assert cell["n"] == 12
        assert math.isclose(cell["expected"], 0.63, abs_tol=1e-6)
        assert "acceptableLow" in cell
        assert candidate["status"] == "candidate-collected"

    def test_latency_direction_is_lower_better(self, tmp_path):
        lines = [make_score_record(f"L{i}", 28, {"stim_01_social_peekaboo_8s": [
            {"segmentId": "s", "biomarker": "first_entry_latency_ms", "observed": 300.0 + i * 10.0, "imputed": False},
        ]}) for i in range(12)]
        write_jsonl(tmp_path / "latency.jsonl", lines)
        buckets = extract_observations(tmp_path, DEFAULT_AGE_BRACKETS)
        candidate = build_candidate_baseline(buckets, min_n=12, brackets=DEFAULT_AGE_BRACKETS)
        expectation = candidate["videos"]["stim_01_social_peekaboo_8s"]["segments"][0]["expectations"][0]
        assert expectation["direction"] == "lowerBetter"
        assert "acceptableHigh" in expectation["byAge"]["24-35"]

    def test_cli_end_to_end(self, tmp_path):
        import subprocess
        import sys

        self._seed(tmp_path, n=12)
        output = tmp_path / "candidate.json"
        result = subprocess.run(
            [sys.executable, "tools/asd_norm_aggregator.py", "--input", str(tmp_path), "--output", str(output)],
            capture_output=True, text=True, cwd=Path(__file__).resolve().parents[1],
        )
        assert result.returncode == 0, result.stderr
        candidate = json.loads(output.read_text(encoding="utf-8"))
        assert candidate["schemaVersion"] == "1.0.0-candidate"
