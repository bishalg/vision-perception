#!/usr/bin/env python3
"""TDD tests for tools/geojson_exporter.py evidence overlay (Phase 4/5)."""

import json
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import geojson_exporter

REPO_ROOT = Path(__file__).resolve().parent.parent


def _doc():
    pts = [{"x": float(i), "y": 10.0} for i in range(10)]
    return {
        "schema_version": "1.0.0",
        "event_id": "evt-9",
        "timestamp_iso": "2026-09-12T00:00:00Z",
        "payload_sha256": "c" * 64,
        "frame_dimensions": {"width": 640, "height": 480},
        "quality": {"is_usable": True, "blur_score": 200.0},
        "spatial_features": [
            {
                "feature_id": "geo_feature_1",
                "feature_class": "river_channel",
                "confidence": 0.95,
                "total_length_px": 300.0,
                "point_count": 10,
                "points": pts,
            },
            {
                "feature_id": "geo_feature_2",
                "feature_class": "minor_feature",
                "confidence": 0.6,
                "total_length_px": 12.0,
                "point_count": 2,
                "points": [{"x": 0.0, "y": 0.0}, {"x": 1.0, "y": 1.0}],
            },
        ],
        "hydrology_metrics": {
            "total_features": 2,
            "river_channel_count": 1,
            "tributary_count": 0,
            "total_feature_length_px": 312.0,
        },
        "controls": {"spatial_control_pass": True, "temporal_control_pass": True},
        "alert_recommendation": "WARNING",
    }


def test_evidence_to_features_emits_river_only_with_centroid():
    feats = geojson_exporter.evidence_to_features(_doc())
    assert len(feats) == 1  # minor_feature excluded
    feat = feats[0]
    assert feat["geometry"]["coordinates"] == pytest.approx([4.5, 10.0])
    assert feat["properties"]["event_id"] == "evt-9"
    assert feat["properties"]["alert"] == "WARNING"
    assert feat["properties"]["kind"] == "flood_signal"


def test_evidence_to_features_rejects_invalid():
    bad = _doc()
    del bad["controls"]
    with pytest.raises(ValueError):
        geojson_exporter.evidence_to_features(bad)


def test_cli_evidence_flag_appends_features(tmp_path):
    ev = tmp_path / "ev.json"
    ev.write_text(json.dumps(_doc()))
    out = tmp_path / "cameras.geojson"
    proc = subprocess.run(
        [sys.executable, "tools/geojson_exporter.py", "--evidence", str(ev),
         "-o", str(out), "--no-copy-media"],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    assert proc.returncode == 0, proc.stderr
    collection = json.loads(out.read_text())
    kinds = [f["properties"].get("kind", "camera") for f in collection["features"]]
    assert "flood_signal" in kinds
    assert collection["properties"]["count"] == len(collection["features"])


def test_cli_invalid_evidence_fails(tmp_path):
    ev = tmp_path / "bad.json"
    ev.write_text(json.dumps({"nope": True}))
    out = tmp_path / "cameras.geojson"
    proc = subprocess.run(
        [sys.executable, "tools/geojson_exporter.py", "--evidence", str(ev),
         "-o", str(out), "--no-copy-media"],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    assert proc.returncode == 1


def test_default_run_still_emits_cameras(tmp_path):
    out = tmp_path / "cameras.geojson"
    proc = subprocess.run(
        [sys.executable, "tools/geojson_exporter.py", "-o", str(out), "--no-copy-media"],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    assert proc.returncode == 0, proc.stderr
    collection = json.loads(out.read_text())
    assert collection["properties"]["count"] >= 3
