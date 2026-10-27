from __future__ import annotations

from typing import Any, Optional
from pydantic import BaseModel, Field


class SessionPerVideoScore(BaseModel):
    clip_id: str
    order: int = 0
    dwell_social: float = 0.0
    dwell_geometric: float = 0.0
    dwell_eye: float = 0.0
    dwell_mouth: float = 0.0
    orienting_latency_ms: Optional[float] = None
    gaze_cue_accuracy: Optional[float] = None
    fixation_count: int = 0
    mean_fixation_ms: float = 0.0
    status: str = "onTarget"
    score: float = 0.0
    weight: float = 1.0
    baseline_ref: Optional[str] = None
    details: Optional[dict[str, Any]] = None


class SessionIngestRequest(BaseModel):
    sessionId: str
    participantToken: Optional[str] = None
    captureSource: str = "truedepth"
    durationSec: float = 30.0
    validRatio: float = 0.95
    socialPreferenceScore: float = 0.5
    socialDwellSec: Optional[float] = None
    geometricDwellSec: Optional[float] = None
    eyeDwellRatio: Optional[float] = None
    mouthDwellRatio: Optional[float] = None
    firstEyeLatencyMs: Optional[float] = None
    qualityVerdict: str = "optimal"
    calibratedDistanceCm: Optional[float] = None
    attentionFlag: Optional[str] = "green"
    battery: Optional[str] = "express_30s"
    schemaVersion: str = "1.6"
    perVideo: Optional[list[SessionPerVideoScore]] = None
    rawPayload: Optional[dict[str, Any]] = None


class SessionIngestResponse(BaseModel):
    sessionId: str
    status: str = "ingested"
    duplicate: bool = False
    frames_upload_url: Optional[str] = None
    report_upload_url: Optional[str] = None
