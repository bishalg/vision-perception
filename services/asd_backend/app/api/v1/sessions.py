from __future__ import annotations

from typing import Any, Optional
from fastapi import APIRouter, Depends, Header, HTTPException, Query
from sqlalchemy import select
from sqlalchemy.orm import Session

from app.db.session import get_db
from app.models import Organization, Participant, SessionRecord, SessionClipScore, ReviewItem
from app.schemas.session import SessionIngestRequest, SessionIngestResponse

router = APIRouter(prefix="/v1/sessions", tags=["Sessions"])


@router.post("", response_model=SessionIngestResponse)
def ingest_session(
    payload: SessionIngestRequest,
    x_idempotency_key: Optional[str] = Header(None, alias="X-Idempotency-Key"),
    x_schema_version: Optional[str] = Header(None, alias="X-Schema-Version"),
    db: Session = Depends(get_db),
) -> SessionIngestResponse:
    idem_key = x_idempotency_key or payload.sessionId

    # Check for idempotent replay
    existing = db.scalar(select(SessionRecord).where(
        (SessionRecord.id == payload.sessionId) | (SessionRecord.idempotency_key == idem_key)
    ))
    if existing:
        return SessionIngestResponse(
            sessionId=existing.id,
            status="ingested",
            duplicate=True,
            frames_upload_url=None,
            report_upload_url=None,
        )

    # Resolve Default Org
    org = db.scalar(select(Organization).where(Organization.kind == "home"))
    if not org:
        org = Organization(id="default-home-org", name="Default Public Org", kind="home")
        db.add(org)
        db.commit()
        db.refresh(org)

    # Resolve Participant
    part_token = payload.participantToken or f"child-{payload.sessionId[:8]}"
    participant = db.scalar(select(Participant).where(Participant.token == part_token))
    if not participant:
        participant = Participant(
            org_id=org.id,
            token=part_token,
            age_bracket_at_enrol="age_12_24",
            language="en",
            consent_version="1.0",
        )
        db.add(participant)
        db.commit()
        db.refresh(participant)

    # Map attention flag
    flag = payload.attentionFlag or ("green" if payload.socialPreferenceScore >= 0.5 else "yellow")
    if payload.validRatio < 0.80:
        flag = "unreliable"

    # Construct SessionRecord
    session = SessionRecord(
        id=payload.sessionId,
        org_id=org.id,
        participant_id=participant.id,
        capture_source=payload.captureSource,
        duration_sec=payload.durationSec,
        valid_frame_ratio=payload.validRatio,
        social_preference=payload.socialPreferenceScore,
        eye_dwell=payload.eyeDwellRatio,
        mouth_dwell=payload.mouthDwellRatio,
        first_eye_latency_ms=payload.firstEyeLatencyMs,
        quality_verdict=payload.qualityVerdict,
        calibrated_distance_cm=payload.calibratedDistanceCm,
        attention_flag=flag,
        battery=payload.battery or "express_30s",
        schema_version=x_schema_version or payload.schemaVersion,
        summary=payload.model_dump(),
        idempotency_key=idem_key,
    )
    db.add(session)

    # Add clip scores
    if payload.perVideo:
        for clip in payload.perVideo:
            score = SessionClipScore(
                session_id=session.id,
                clip_id=clip.clip_id,
                order=clip.order,
                dwell_social=clip.dwell_social,
                dwell_geometric=clip.dwell_geometric,
                dwell_eye=clip.dwell_eye,
                dwell_mouth=clip.dwell_mouth,
                orienting_latency_ms=clip.orienting_latency_ms,
                gaze_cue_accuracy=clip.gaze_cue_accuracy,
                fixation_count=clip.fixation_count,
                mean_fixation_ms=clip.mean_fixation_ms,
                status=clip.status,
                score=clip.score,
                weight=clip.weight,
                baseline_ref=clip.baseline_ref,
                details=clip.details or {},
            )
            db.add(score)

    # Auto-review rules
    if payload.validRatio < 0.80:
        db.add(ReviewItem(
            org_id=org.id,
            session_id=session.id,
            reason="low_quality",
            status="open",
        ))
    elif flag == "red":
        db.add(ReviewItem(
            org_id=org.id,
            session_id=session.id,
            reason="flag_red",
            status="open",
        ))

    db.commit()

    return SessionIngestResponse(
        sessionId=session.id,
        status="ingested",
        duplicate=False,
        frames_upload_url=f"/v1/sessions/{session.id}/upload/frames",
        report_upload_url=f"/v1/sessions/{session.id}/upload/report",
    )


@router.get("")
def list_sessions(
    limit: int = Query(50, ge=1, le=100),
    attention_flag: Optional[str] = None,
    db: Session = Depends(get_db),
) -> list[dict[str, Any]]:
    query = select(SessionRecord).order_by(SessionRecord.started_at.desc()).limit(limit)
    if attention_flag:
        query = query.where(SessionRecord.attention_flag == attention_flag)
    records = db.scalars(query).all()
    return [
        {
            "id": r.id,
            "participant_id": r.participant_id,
            "started_at": r.started_at.isoformat(),
            "duration_sec": r.duration_sec,
            "quality_verdict": r.quality_verdict,
            "social_preference": r.social_preference,
            "attention_flag": r.attention_flag,
        }
        for r in records
    ]


@router.get("/{session_id}")
def get_session_detail(session_id: str, db: Session = Depends(get_db)) -> dict[str, Any]:
    session = db.scalar(select(SessionRecord).where(SessionRecord.id == session_id))
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")

    clip_scores = db.scalars(select(SessionClipScore).where(SessionClipScore.session_id == session_id)).all()
    return {
        "id": session.id,
        "org_id": session.org_id,
        "participant_id": session.participant_id,
        "started_at": session.started_at.isoformat(),
        "duration_sec": session.duration_sec,
        "quality_verdict": session.quality_verdict,
        "social_preference": session.social_preference,
        "attention_flag": session.attention_flag,
        "summary": session.summary,
        "clip_scores": [
            {
                "clip_id": c.clip_id,
                "order": c.order,
                "score": c.score,
                "status": c.status,
                "dwell_social": c.dwell_social,
                "dwell_geometric": c.dwell_geometric,
            }
            for c in clip_scores
        ],
    }
