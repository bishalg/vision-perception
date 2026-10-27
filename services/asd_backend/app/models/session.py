from __future__ import annotations

import uuid
from datetime import datetime
from typing import Any, Optional
from sqlalchemy import DateTime, Float, ForeignKey, Integer, JSON, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.db.base import Base


class SessionRecord(Base):
    __tablename__ = "sessions"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)  # sessionId from app
    org_id: Mapped[str] = mapped_column(String(64), ForeignKey("organizations.id"), nullable=False, index=True)
    participant_id: Mapped[str] = mapped_column(String(64), ForeignKey("participants.id"), nullable=False, index=True)
    device_id: Mapped[Optional[str]] = mapped_column(String(64), ForeignKey("devices.id"), nullable=True)

    capture_source: Mapped[str] = mapped_column(String(32), default="truedepth")  # truedepth, simulated
    started_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow, index=True)
    ended_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
    duration_sec: Mapped[float] = mapped_column(Float, default=30.0)

    battery: Mapped[str] = mapped_column(String(64), default="express_30s")
    age_pack: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    lang: Mapped[Optional[str]] = mapped_column(String(8), default="en")
    bundle_version: Mapped[Optional[str]] = mapped_column(String(32), nullable=True)
    schema_version: Mapped[str] = mapped_column(String(16), default="1.6")

    quality_verdict: Mapped[str] = mapped_column(String(32), default="optimal")
    valid_frame_ratio: Mapped[float] = mapped_column(Float, default=0.95)
    calibrated_distance_cm: Mapped[Optional[float]] = mapped_column(Float, nullable=True)

    social_preference: Mapped[float] = mapped_column(Float, default=0.5)
    eye_dwell: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    mouth_dwell: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    first_eye_latency_ms: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    affect: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, nullable=True)

    composite_score: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    composite_confidence: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    attention_flag: Mapped[str] = mapped_column(String(32), default="green")  # green, yellow, red, unreliable

    summary: Mapped[dict[str, Any]] = mapped_column(JSON, default=dict)
    frames_blob_key: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)
    report_pdf_key: Mapped[Optional[str]] = mapped_column(String(255), nullable=True)
    received_at: Mapped[datetime] = mapped_column(DateTime, default=datetime.utcnow)
    idempotency_key: Mapped[Optional[str]] = mapped_column(String(128), unique=True, nullable=True)

    clip_scores: Mapped[list["SessionClipScore"]] = relationship("SessionClipScore", back_populates="session", cascade="all, delete-orphan")


class SessionClipScore(Base):
    __tablename__ = "session_clip_scores"

    id: Mapped[str] = mapped_column(String(64), primary_key=True, default=lambda: str(uuid.uuid4()))
    session_id: Mapped[str] = mapped_column(String(64), ForeignKey("sessions.id"), nullable=False, index=True)
    clip_id: Mapped[str] = mapped_column(String(64), nullable=False)
    order: Mapped[int] = mapped_column(Integer, default=0)

    dwell_social: Mapped[float] = mapped_column(Float, default=0.0)
    dwell_geometric: Mapped[float] = mapped_column(Float, default=0.0)
    dwell_eye: Mapped[float] = mapped_column(Float, default=0.0)
    dwell_mouth: Mapped[float] = mapped_column(Float, default=0.0)
    orienting_latency_ms: Mapped[Optional[float]] = mapped_column(Float, nullable=True)
    gaze_cue_accuracy: Mapped[Optional[float]] = mapped_column(Float, nullable=True)

    fixation_count: Mapped[int] = mapped_column(Integer, default=0)
    mean_fixation_ms: Mapped[float] = mapped_column(Float, default=0.0)
    status: Mapped[str] = mapped_column(String(32), default="onTarget")  # onTarget, monitor, atypical
    score: Mapped[float] = mapped_column(Float, default=0.0)
    weight: Mapped[float] = mapped_column(Float, default=1.0)
    baseline_ref: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    details: Mapped[Optional[dict[str, Any]]] = mapped_column(JSON, default=dict)

    session: Mapped[SessionRecord] = relationship("SessionRecord", back_populates="clip_scores")
