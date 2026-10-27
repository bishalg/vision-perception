from __future__ import annotations

from typing import Any, Optional
from pydantic import BaseModel, Field


class HealthResponse(BaseModel):
    status: str = "ok"
    service: str = "asd_backend"
    environment: str = "development"
    version: str = "0.1.0"
    database: str = "connected"


class StudyInfoResponse(BaseModel):
    status: str = "active"
    study_identifier: str = "CURV-ASD-GAZE-2026"
    study_id: str = "CURV-ASD-GAZE-2026"
    title: str = "Curv Pediatric ASD Gaze Biomarker Study"
    irb_protocol_id: str = "IRB-ASD-2026-088"
    irb_protocol: str = "IRB-ASD-2026-088"
    institution: str = "Children's Health Neurodevelopment Center"
    principal_investigator: str = "Dr. Elena Vance, MD PhD"
    consent_version: str = "v1.3-IRB2026"
    required_instruments: list[str] = ["ORKConsentReviewStep", "ORKActiveStepResult"]
    zero_raw_video_guarantee: bool = True


class ProblemDetails(BaseModel):
    type: str = "about:blank"
    title: str
    status: int
    detail: Optional[str] = None
    instance: Optional[str] = None
    invalid_params: Optional[list[dict[str, Any]]] = None
