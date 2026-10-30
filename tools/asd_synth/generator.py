"""Seeded generator of schema-valid session-v1.6 payloads.

Parameters (documented so the Swift ``SummaryFixture`` port can mirror them):
  * AGE_BANDS: same brackets as ``tools/asd_norm_aggregator.py``.
  * CLIPS: stimulus ids/durations from the bundled catalog.
  * PROFILES: ``typical`` (social-preferring), ``reduced_social`` (geometric-preferring),
    ``low_quality`` (tracking loss, verdict ``insufficient``), ``short`` (early abort).
Determinism: output depends only on (seed, index); no wall clock, no global RNG.
"""

from __future__ import annotations

import random
from typing import Dict, Iterable, List, Optional

GENERATOR_VERSION = "1.0"
SCHEMA_VERSION = "1.6"
SYNTHETIC_CAPTURE_SOURCE = "synthetic"
ID_PREFIX = "synth_"
TOKEN_PREFIX = "synth-"

AGE_BANDS = [
    {"id": "12-17", "minMonths": 12, "maxMonths": 17},
    {"id": "18-23", "minMonths": 18, "maxMonths": 23},
    {"id": "24-35", "minMonths": 24, "maxMonths": 35},
    {"id": "36-47", "minMonths": 36, "maxMonths": 47},
    {"id": "48-60", "minMonths": 48, "maxMonths": 60},
]

# (clip_id, duration_sec, kind)
CLIPS = [
    ("stim_01_social_peekaboo_8s", 8.0, "social"),
    ("stim_02_gaze_cue_right_8s", 8.0, "gaze_cue"),
    ("stim_03_gaze_cue_left_8s", 8.0, "gaze_cue"),
    ("stim_04_surprise_giftbox_10s", 10.0, "social"),
    ("stim_05_geopref_split_10s", 10.0, "geopref"),
    ("stim_06_dynamic_gears_marbles_8s", 8.0, "geometric"),
    ("stim_07_cartoon_slapstick_penguin_8s", 8.0, "social"),
    ("stim_09_social_turn_taking_10s", 10.0, "social"),
]

BATTERIES = {"express_30s": 4, "comprehensive_60s": 8}
PROFILES = ("typical", "reduced_social", "low_quality", "short")
# Cycled so small N still covers every profile; typical is repeated to dominate.
_PROFILE_CYCLE = ("typical", "typical", "reduced_social", "low_quality", "short")


def _r(x: float, nd: int = 3) -> float:
    return round(float(x), nd)


def _clip_score(rng: random.Random, order: int, clip, profile: str, valid: float) -> dict:
    clip_id, dur, kind = clip
    social_bias = {"typical": 0.65, "reduced_social": 0.25}.get(profile, 0.5)
    if kind == "geometric":
        social_bias = 1.0 - social_bias
    on_screen = dur * max(0.0, min(1.0, valid)) * rng.uniform(0.8, 0.97)
    frac = max(0.02, min(0.98, rng.gauss(social_bias, 0.08)))
    social = on_screen * frac
    geometric = on_screen * (1.0 - frac) * rng.uniform(0.5, 0.95)
    eye_share = rng.uniform(0.3, 0.6) if profile == "typical" else rng.uniform(0.1, 0.4)
    eye = social * eye_share
    mouth = social * rng.uniform(0.1, 0.3)
    fixations = max(1, int(on_screen / rng.uniform(0.2, 0.45)))
    imputed = profile == "low_quality" and rng.random() < 0.4
    cue = kind == "gaze_cue"
    latency = None if imputed else _r(rng.uniform(250, 1200) * (1.5 if profile == "reduced_social" else 1.0), 1)
    status = "onTarget" if frac >= 0.4 and not imputed else "offTarget"
    return {
        "clip_id": clip_id,
        "order": order,
        "dwell_social": _r(social),
        "dwell_geometric": _r(geometric),
        "dwell_eye": _r(eye),
        "dwell_mouth": _r(mouth),
        "orienting_latency_ms": latency,
        "gaze_cue_accuracy": _r(rng.uniform(0.3, 1.0) if profile != "reduced_social" else rng.uniform(0.0, 0.6)) if cue else None,
        "fixation_count": fixations,
        "mean_fixation_ms": _r(on_screen / fixations * 1000.0, 1),
        "status": status,
        "score": _r(max(0.0, min(1.0, frac * (0.6 + 0.4 * valid)))),
        "weight": 1.0,
        "baseline_ref": None,
        "details": {"imputed": imputed},
    }


def generate_session(seed: int, index: int = 0, profile: Optional[str] = None) -> Dict:
    """Return one synthetic payload; identical for identical (seed, index, profile)."""
    rng = random.Random(f"asd_synth:{GENERATOR_VERSION}:{seed}:{index}")
    profile = profile or _PROFILE_CYCLE[index % len(_PROFILE_CYCLE)]
    if profile not in PROFILES:
        raise ValueError(f"unknown profile {profile!r}; expected one of {PROFILES}")

    band = AGE_BANDS[index % len(AGE_BANDS)]
    age = rng.randint(band["minMonths"], band["maxMonths"])
    battery = "express_30s" if rng.random() < 0.6 else "comprehensive_60s"
    n_clips = BATTERIES[battery]
    if profile == "short":
        n_clips = max(1, n_clips // 2)
    clips = rng.sample(CLIPS, min(n_clips, len(CLIPS)))

    if profile == "low_quality":
        valid, verdict, flag = rng.uniform(0.15, 0.5), "insufficient", "yellow"
    else:
        valid = rng.uniform(0.82, 0.99)
        verdict = "optimal" if valid > 0.9 else rng.choice(["acceptableLow", "acceptableHigh"])
        flag = "green" if profile == "typical" else rng.choice(["yellow", "red"])
    if profile == "short":
        valid = rng.uniform(0.6, 0.9)
        verdict = "acceptableLow"

    per_video = [_clip_score(rng, i + 1, c, profile, valid) for i, c in enumerate(clips)]
    duration = _r(sum(c[1] for c in clips) * (0.6 if profile == "short" else 1.0) + rng.uniform(0, 2), 2)
    social = sum(p["dwell_social"] for p in per_video)
    geometric = sum(p["dwell_geometric"] for p in per_video)
    eye = sum(p["dwell_eye"] for p in per_video)
    mouth = sum(p["dwell_mouth"] for p in per_video)
    lat = [p["orienting_latency_ms"] for p in per_video if p["orienting_latency_ms"] is not None]
    total = social + geometric

    return {
        "sessionId": f"{ID_PREFIX}{seed}_{index:05d}",
        "participantToken": f"{TOKEN_PREFIX}{seed}-{index:05d}",
        "captureSource": SYNTHETIC_CAPTURE_SOURCE,
        "durationSec": duration,
        "validRatio": _r(valid),
        "socialPreferenceScore": _r(social / total if total else 0.5),
        "socialDwellSec": _r(social),
        "geometricDwellSec": _r(geometric),
        "eyeDwellRatio": _r(eye / social) if social else None,
        "mouthDwellRatio": _r(mouth / social) if social else None,
        "firstEyeLatencyMs": _r(min(lat), 1) if lat else None,
        "qualityVerdict": verdict,
        "calibratedDistanceCm": _r(rng.uniform(30, 55), 1),
        "attentionFlag": flag,
        "battery": battery,
        "schemaVersion": SCHEMA_VERSION,
        "perVideo": per_video,
        "rawPayload": {
            "synthetic": True,
            "generator": "tools.asd_synth",
            "generatorVersion": GENERATOR_VERSION,
            "seed": seed,
            "index": index,
            "profile": profile,
            "ageMonths": age,
            "ageBand": band["id"],
            "notice": "SYNTHETIC DATA - not a real child, not for clinical or norm use",
        },
    }


def generate_sessions(n: int, seed: int = 0) -> List[Dict]:
    return [generate_session(seed, i) for i in range(n)]


def is_synthetic(payload: Dict) -> bool:
    raw = payload.get("rawPayload") or {}
    return (
        str(payload.get("sessionId", "")).startswith(ID_PREFIX)
        and payload.get("captureSource") == SYNTHETIC_CAPTURE_SOURCE
        and raw.get("synthetic") is True
    )
