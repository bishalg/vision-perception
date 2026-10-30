"""Deterministic synthetic ASD gaze session generator (WP-D.4).

Every record is marked synthetic (``synth_`` ids, ``captureSource="synthetic"``,
``rawPayload.synthetic == True``) and must never be mixed with real data.

Backend import path (repo root on sys.path):  ``from tools.asd_synth import generate_session, generate_sessions``
"""

from .generator import (  # noqa: F401
    AGE_BANDS,
    CLIPS,
    GENERATOR_VERSION,
    PROFILES,
    generate_session,
    generate_sessions,
    is_synthetic,
)
