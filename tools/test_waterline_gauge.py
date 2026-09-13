#!/usr/bin/env python3
"""TDD tests for tools/waterline_gauge.py — real OpenCV on synthetic frames."""

import json
import sys
from pathlib import Path

import pytest

cv2 = pytest.importorskip("cv2")
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import waterline_gauge as gauge


def _river_frame(height=480, width=640, water_y=300) -> np.ndarray:
    """Synthetic riverbank: bright bank above, dark water below, crisp edge."""
    img = np.full((height, width, 3), 170, dtype=np.uint8)
    img[water_y:, :] = (40, 60, 90)
    # Add mild texture so Canny has structure but the dominant edge wins.
    rng = np.random.default_rng(7)
    noise = rng.integers(-6, 7, size=img.shape, dtype=np.int16)
    return np.clip(img.astype(np.int16) + noise, 0, 255).astype(np.uint8)


def test_detects_horizontal_waterline():
    img = _river_frame(water_y=300)
    det = gauge.detect_waterline(img)
    assert det["y_px"] == pytest.approx(300, abs=6)
    assert det["confidence"] > 0.3
    assert det["inliers"] >= 1


def test_detects_shifted_waterline():
    img = _river_frame(water_y=260)
    det = gauge.detect_waterline(img)
    assert det["y_px"] == pytest.approx(260, abs=6)


def test_drift_reports_rise():
    base = gauge.detect_waterline(_river_frame(water_y=320))
    curr = gauge.detect_waterline(_river_frame(water_y=280))
    drift = gauge.waterline_drift(base, curr, 480)
    assert drift["dy_px"] == pytest.approx(40, abs=8)
    assert drift["rising"] is True
    assert drift["dy_fraction"] == pytest.approx(40 / 480, abs=0.02)


def test_drift_reports_fall_as_not_rising():
    base = gauge.detect_waterline(_river_frame(water_y=280))
    curr = gauge.detect_waterline(_river_frame(water_y=320))
    drift = gauge.waterline_drift(base, curr, 480)
    assert drift["rising"] is False


def test_blank_frame_raises_not_fake_zero():
    blank = np.full((480, 640, 3), 128, dtype=np.uint8)
    with pytest.raises(ValueError):
        gauge.detect_waterline(blank)


def test_empty_image_raises():
    with pytest.raises(ValueError):
        gauge.detect_waterline(np.zeros((0, 0, 3), dtype=np.uint8))


def test_feature_schema_shape():
    det = gauge.detect_waterline(_river_frame())
    feat = gauge.to_feature("pier_1", det, 640)
    assert feat["feature_class"] == "waterline"
    assert feat["point_count"] == 2
    assert feat["points"][0]["x"] == 0.0
    assert feat["points"][1]["x"] == 640.0


def test_real_fixture_produces_detection_or_honest_failure():
    """Pier fixture must either yield a detection or raise — never fake data."""
    img = cv2.imread("data/fixtures/cctv_flood/bhote_koshi_pier_sample.png", cv2.IMREAD_COLOR)
    assert img is not None
    try:
        det = gauge.detect_waterline(img)
        assert 0 <= det["y_px"] <= img.shape[0]
        assert 0.0 <= det["confidence"] <= 1.0
    except ValueError:
        pass  # honest missing-data signal is acceptable on synthetic art


def test_pier_fixture_waterline_near_staff_mark():
    """Staff-gauge mark sits at 65% frame height; gauge must lock onto it."""
    img = cv2.imread("data/fixtures/cctv_flood/bhote_koshi_pier_sample.png", cv2.IMREAD_COLOR)
    assert img is not None
    det = gauge.detect_waterline(img)
    assert det["y_px"] == pytest.approx(img.shape[0] * 0.65, abs=8)
    assert det["confidence"] > 0.5


def test_cli_without_baseline_succeeds(tmp_path):
    """Regression: unset --baseline/--out (Path defaults) must not break CLI."""
    import subprocess
    import sys

    proc = subprocess.run(
        [sys.executable, "tools/waterline_gauge.py",
         "--image", "data/fixtures/cctv_flood/bhote_koshi_pier_sample.png"],
        capture_output=True, text=True,
        cwd=str(Path(__file__).resolve().parent.parent),
    )
    assert proc.returncode == 0, proc.stderr
    assert "Waterline y=" in proc.stdout
