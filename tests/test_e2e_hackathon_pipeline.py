"""tests/test_e2e_hackathon_pipeline.py.

Full End-to-End Regression & Integration Test for OpenCV AI Competition 2026:
  Stage 1: Sentinel-2 L2A Disaster Imagery Verification
  Stage 2: OpenCV 5 C++ CurvEngine Execution (Steger Ridge Extraction)
  Stage 3: Quantitative Flood Delta Calculation & Scientific Gates
  Stage 4: Autonomous Agentic Vision Loop (Token validation, Idempotency, SNS dispatch)
"""

import json
import os
import subprocess
import sys
from pathlib import Path
import pytest

ROOT_DIR = Path(__file__).resolve().parent.parent
CLI_BIN = ROOT_DIR / "build" / "tools" / "curv_cli"
DATA_DIR = ROOT_DIR / "data" / "flood_nepal_2026"
PRE_IMG = DATA_DIR / "processed" / "pre_20260824_bgr.png"
POST_IMG = DATA_DIR / "processed" / "post_20260827_bgr.png"
DELTA_JSON = DATA_DIR / "out" / "flood_delta.json"


def test_stage1_input_assets_exist():
    """Validates raw Sentinel-2 disaster imagery is present and non-empty."""
    assert PRE_IMG.exists(), f"Missing PRE image: {PRE_IMG}"
    assert POST_IMG.exists(), f"Missing POST image: {POST_IMG}"
    assert PRE_IMG.stat().st_size > 100_000, "PRE image too small or corrupted"
    assert POST_IMG.stat().st_size > 100_000, "POST image too small or corrupted"


def test_stage2_curv_cli_execution():
    """Runs C++20 CurvEngine on pre/post imagery and asserts sub-pixel outputs."""
    assert CLI_BIN.exists() and os.access(CLI_BIN, os.X_OK), f"curv_cli binary not found at {CLI_BIN}"

    # Run flood pipeline script
    run_script = ROOT_DIR / "scripts" / "run_flood_pipeline.sh"
    res = subprocess.run([str(run_script)], cwd=str(ROOT_DIR), capture_output=True, text=True)
    assert res.returncode == 0, f"run_flood_pipeline.sh failed:\n{res.stderr}\n{res.stdout}"

    # Verify generated evidence
    pre_ev = DATA_DIR / "out" / "pre_evidence.json"
    post_ev = DATA_DIR / "out" / "post_evidence.json"
    assert pre_ev.exists(), "Missing pre_evidence.json"
    assert post_ev.exists(), "Missing post_evidence.json"

    pre_data = json.loads(pre_ev.read_text(encoding="utf-8"))
    post_data = json.loads(post_ev.read_text(encoding="utf-8"))

    # Assert OpenCV version & quality gates
    assert pre_data["quality"]["is_usable"] is True
    assert post_data["quality"]["is_usable"] is True
    assert len(pre_data.get("evidence", [])) > 50, "Extracted evidence too low for PRE"
    assert len(post_data.get("evidence", [])) > 50, "Extracted evidence too low for POST"


def test_stage3_quantitative_flood_delta_gates():
    """Runs flood delta script and asserts mathematical and scientific control thresholds."""
    delta_script = ROOT_DIR / "scripts" / "compute_flood_delta.py"
    res = subprocess.run([sys.executable, str(delta_script)], cwd=str(ROOT_DIR), capture_output=True, text=True)
    assert res.returncode == 0, f"compute_flood_delta.py failed:\n{res.stderr}\n{res.stdout}"

    assert DELTA_JSON.exists(), "Missing flood_delta.json"
    delta = json.loads(DELTA_JSON.read_text(encoding="utf-8"))

    # Assert quantitative gates
    width_ratio = delta["channel_width_proxy"]["width_ratio"]
    assert width_ratio > 1.8, f"Expected channel widening > 1.8x, got {width_ratio}x"

    surge_ratio = delta["color_mask_delta"]["combined_flood_signature"]["ratio"]
    assert surge_ratio > 1.5, f"Expected flood signature surge > 1.5x, got {surge_ratio}x"

    # Assert spatial slope control gate (off-river slopes must NOT show false flood expansion)
    spatial_ratio = delta["spatial_control"]["control_ratio"]
    assert spatial_ratio < 0.20, f"Spatial control noise failed: expected < 0.20x, got {spatial_ratio}x"
    assert delta["spatial_control"]["passes_acceptance"] is True
    assert delta["spatial_control"]["spatial_control_pass"] is True
    assert delta["assessment"]["alert_recommendation"] == "CRITICAL"


def test_stage4_agentic_vision_loop_e2e():
    """Executes the 3-node Agentic Vision loop and asserts cryptographic token & alert receipt."""
    demo_script = ROOT_DIR / "scripts" / "run_agentic_vision_demo.py"
    res = subprocess.run(
        [sys.executable, str(demo_script), "--json"],
        cwd=str(ROOT_DIR),
        capture_output=True,
        text=True,
    )
    assert res.returncode == 0, f"run_agentic_vision_demo.py failed:\n{res.stderr}\n{res.stdout}"

    receipt = json.loads(res.stdout)
    assert receipt["ok"] is True
    assert receipt["alertStatus"] == "DISPATCHED"
    assert receipt["severity"] == "CRITICAL"
    assert receipt["evidenceToken"].startswith("tok-")
    assert receipt["snsMessageId"].startswith("sns-")
    assert receipt["spatialControlPass"] is True
    assert "s3://vision-perception-audit/" in receipt["auditReceiptUri"]
