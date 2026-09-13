#!/usr/bin/env python3
"""TDD tests for tools/s3_mirror.py — real validation against real CLI output."""

import json
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import s3_mirror

REPO_ROOT = Path(__file__).resolve().parent.parent
CURV_CLI = REPO_ROOT / "build/tools/curv_cli"


def _minimal_valid_doc() -> dict:
    return {
        "schema_version": "1.0.0",
        "event_id": "input_line-20260912T000000Z",
        "timestamp_iso": "2026-09-12T00:00:00Z",
        "payload_sha256": "a" * 64,
        "frame_dimensions": {"width": 256, "height": 256},
        "quality": {"is_usable": True, "blur_score": 120.5, "recommendation": "Nominal"},
        "spatial_features": [
            {
                "feature_id": "geo_feature_1",
                "feature_class": "river_channel",
                "confidence": 0.95,
                "total_length_px": 200.0,
                "point_count": 2,
                "points": [{"x": 0.0, "y": 0.0}, {"x": 200.0, "y": 0.0}],
            }
        ],
        "hydrology_metrics": {
            "total_features": 1,
            "river_channel_count": 1,
            "tributary_count": 0,
            "total_feature_length_px": 200.0,
        },
        "controls": {
            "spatial_control_pass": True,
            "temporal_control_pass": True,
            "spatial_control_ratio": 0.0,
            "temporal_control_ratio": 1.33,
        },
        "alert_recommendation": "WARNING",
    }


def test_valid_doc_passes():
    assert s3_mirror.validate_flood_evidence(_minimal_valid_doc()) == []


def test_missing_keys_and_bad_enum_fail():
    doc = _minimal_valid_doc()
    del doc["controls"]
    doc["alert_recommendation"] = "PANIC"
    doc["payload_sha256"] = "xyz"
    doc["timestamp_iso"] = "not-a-date"
    errors = s3_mirror.validate_flood_evidence(doc)
    assert any("controls" in e for e in errors)
    assert any("alert_recommendation" in e for e in errors)
    assert any("payload_sha256" in e for e in errors)
    assert any("timestamp_iso" in e for e in errors)


def test_point_count_mismatch_and_bad_class_fail():
    doc = _minimal_valid_doc()
    doc["spatial_features"][0]["point_count"] = 99
    doc["spatial_features"][0]["feature_class"] = "ufo"
    errors = s3_mirror.validate_flood_evidence(doc)
    assert any("point_count" in e for e in errors)
    assert any("feature_class" in e for e in errors)


def test_waterline_class_accepted():
    doc = _minimal_valid_doc()
    doc["spatial_features"][0]["feature_class"] = "waterline"
    assert s3_mirror.validate_flood_evidence(doc) == []


def test_local_mirror_copies_bytes(tmp_path):
    src = tmp_path / "ev.json"
    src.write_text(json.dumps(_minimal_valid_doc()))
    dest = s3_mirror.mirror_to_local(src, tmp_path / "mirror")
    assert dest.read_bytes() == src.read_bytes()


def test_missing_file_exits_nonzero(tmp_path, capsys):
    with pytest.raises(SystemExit):
        s3_mirror.load_and_validate(tmp_path / "nope.json")


def test_cli_without_target_reports_nothing_to_do(tmp_path):
    """Regression: unset --local-mirror/--bucket must not resolve to '.'."""
    src = tmp_path / "ev.json"
    src.write_text(json.dumps(_minimal_valid_doc()))
    proc = subprocess.run(
        [sys.executable, "tools/s3_mirror.py", "--evidence", str(src)],
        capture_output=True, text=True, cwd=REPO_ROOT,
    )
    assert proc.returncode == 1
    assert "Nothing to do" in proc.stderr


def test_real_cli_export_passes_validation(tmp_path):
    """End-to-end: real C++ pipeline output must satisfy the publisher gate."""
    cv2 = pytest.importorskip("cv2")
    import numpy as np

    if not CURV_CLI.exists():
        pytest.skip("curv_cli binary not built")
    img = np.full((256, 256, 3), 20, dtype=np.uint8)
    cv2.line(img, (10, 128), (246, 128), (220, 220, 220), 3)
    src = tmp_path / "line.png"
    assert cv2.imwrite(str(src), img)
    out = tmp_path / "flood_evidence_v1.json"
    proc = subprocess.run(
        [str(CURV_CLI), "--image", str(src), "--domain", "geospatial", "--export-evidence", str(out)],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0, proc.stderr
    doc = json.loads(out.read_text())
    assert s3_mirror.validate_flood_evidence(doc) == []
    # event id traces the source frame
    assert "line" in doc["event_id"]
