"""Tests for tools.asd_synth. Run: python3 -m pytest tools/asd_synth -q (from repo root)."""

import json
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO))

jsonschema = pytest.importorskip("jsonschema")

from tools.asd_synth import AGE_BANDS, PROFILES, generate_session, generate_sessions, is_synthetic  # noqa: E402
from tools.asd_synth.cli import main  # noqa: E402

SCHEMA = json.loads((REPO / "docs/spec/schemas/asd/session-v1.6.json").read_text())


def _valid(payload):
    jsonschema.Draft202012Validator(SCHEMA).validate(payload)


def test_all_outputs_schema_valid():
    for p in generate_sessions(200, seed=3):
        _valid(p)


def test_deterministic_and_seed_sensitive():
    assert generate_sessions(30, seed=5) == generate_sessions(30, seed=5)
    assert generate_session(5, 1) != generate_session(6, 1)
    assert generate_session(5, 2) == generate_sessions(3, seed=5)[2]


def test_clearly_synthetic():
    for p in generate_sessions(50):
        assert is_synthetic(p)
        assert p["participantToken"].startswith("synth-")
        assert "SYNTHETIC" in p["rawPayload"]["notice"]


def test_variety_and_edge_cases():
    ps = generate_sessions(100, seed=1)
    assert {p["rawPayload"]["ageBand"] for p in ps} == {b["id"] for b in AGE_BANDS}
    assert {p["rawPayload"]["profile"] for p in ps} == set(PROFILES)
    assert {p["battery"] for p in ps} == {"express_30s", "comprehensive_60s"}
    assert len({c["clip_id"] for p in ps for c in p["perVideo"]}) >= 6
    low = [p for p in ps if p["rawPayload"]["profile"] == "low_quality"]
    assert low and all(p["qualityVerdict"] == "insufficient" and p["validRatio"] < 0.55 for p in low)


def test_value_ranges_and_unique_ids():
    ps = generate_sessions(100)
    assert len({p["sessionId"] for p in ps}) == 100
    for p in ps:
        assert 0 <= p["validRatio"] <= 1 and 0 <= p["socialPreferenceScore"] <= 1
        for c in p["perVideo"]:
            assert c["dwell_social"] >= 0 and c["fixation_count"] >= 1


def test_unknown_profile_rejected():
    with pytest.raises(ValueError):
        generate_session(0, 0, profile="bogus")


def test_cli_writes_valid_files(tmp_path):
    assert main(["--count", "12", "--seed", "9", "--out", str(tmp_path / "o")]) == 0
    files = sorted((tmp_path / "o").glob("*.json"))
    assert len(files) == 12
    for f in files:
        _valid(json.loads(f.read_text()))
    first = files[0].read_text()
    main(["--count", "12", "--seed", "9", "--out", str(tmp_path / "o2")])
    assert (tmp_path / "o2" / files[0].name).read_text() == first
