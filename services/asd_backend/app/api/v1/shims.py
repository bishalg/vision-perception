from __future__ import annotations

import json
import uuid
from typing import Any, Optional
from fastapi import APIRouter, Depends, Header, HTTPException, Request
from sqlalchemy import select
from sqlalchemy.orm import Session

from app.core.config import settings
from app.db.session import get_db
from app.models import Organization, Participant, SessionRecord, CareEvent, IdempotencyKey
from app.schemas.common import StudyInfoResponse

router = APIRouter(tags=["Legacy Shims"])


def get_default_org(db: Session) -> Organization:
    org = db.scalar(select(Organization).where(Organization.kind == "home"))
    if not org:
        org = Organization(
            id="default-home-org",
            name="Default Public Organization",
            kind="home"
        )
        db.add(org)
        db.commit()
        db.refresh(org)
    return org


def get_or_create_participant(db: Session, org_id: str, token: str) -> Participant:
    part = db.scalar(select(Participant).where(Participant.token == token))
    if not part:
        part = Participant(
            org_id=org_id,
            token=token,
            age_bracket_at_enrol="age_12_24",
            language="en",
            consent_version=settings.ASD_CONSENT_VERSION or "1.0",
            mode="parent",
        )
        db.add(part)
        db.commit()
        db.refresh(part)
    return part


@router.get("/api/v1/research/study-info", response_model=StudyInfoResponse)
def get_study_info() -> StudyInfoResponse:
    return StudyInfoResponse(
        status=settings.ASD_STUDY_STATUS,
        study_identifier=settings.ASD_STUDY_IDENTIFIER,
        study_id=settings.ASD_STUDY_IDENTIFIER,
        title="Curv Pediatric ASD Gaze Biomarker Study",
        irb_protocol_id=settings.ASD_IRB_PROTOCOL_ID,
        irb_protocol=settings.ASD_IRB_PROTOCOL_ID,
        institution=settings.ASD_STUDY_INSTITUTION,
        principal_investigator=settings.ASD_STUDY_PI,
        consent_version=settings.ASD_CONSENT_VERSION,
        required_instruments=["ORKConsentReviewStep", "ORKActiveStepResult"],
        zero_raw_video_guarantee=True,
    )


@router.post("/api/v1/research/ingest")
async def ingest_research_record(request: Request, db: Session = Depends(get_db)) -> dict[str, Any]:
    try:
        raw_body = await request.body()
        if not raw_body:
            raise HTTPException(status_code=400, detail="Empty request body")
        body = json.loads(raw_body.decode("utf-8"))
    except HTTPException:
        raise
    except Exception:
        raise HTTPException(status_code=400, detail="Invalid JSON body")

    # Extract participantId
    participant_data = body.get("participant") if isinstance(body.get("participant"), dict) else {}
    part_token = (
        body.get("participant_id")
        or body.get("participantToken")
        or participant_data.get("participantId")
        or body.get("child_id")
        or f"child-{uuid.uuid4().hex[:6]}"
    )

    # Extract sessionId
    session_id = (
        body.get("session_id")
        or body.get("sessionId")
        or body.get("record_id")
    )
    if not session_id and "taskResults" in body and isinstance(body["taskResults"], list) and len(body["taskResults"]) > 0:
        first_step = body["taskResults"][0].get("activeStepResult", {})
        session_id = first_step.get("sessionId")

    if not session_id:
        session_id = f"sess_{uuid.uuid4().hex[:12]}"

    # Check for duplicate
    existing = db.scalar(select(SessionRecord).where(SessionRecord.id == session_id))
    if existing:
        return {
            "status": "duplicate",
            "session_id": session_id,
            "receipt_id": f"RCPT-{session_id.upper()}",
            "participant_id": part_token,
            "message": "Record already ingested",
        }

    org = get_default_org(db)
    participant = get_or_create_participant(db, org.id, part_token)

    summary = body.get("summary", body)
    social_score = float(summary.get("socialPreferenceScore") or summary.get("social_preference_score") or 0.5)
    valid_ratio = float(summary.get("validRatio") or summary.get("valid_ratio") or 0.95)
    duration_sec = float(summary.get("durationSec") or summary.get("duration_sec") or 30.0)
    capture_source = str(summary.get("captureSource") or summary.get("capture_source") or "truedepth")

    record = SessionRecord(
        id=session_id,
        org_id=org.id,
        participant_id=participant.id,
        capture_source=capture_source,
        duration_sec=duration_sec,
        valid_frame_ratio=valid_ratio,
        social_preference=social_score,
        attention_flag="green" if social_score >= 0.5 else "yellow",
        summary=body,
        idempotency_key=session_id,
    )
    db.add(record)
    db.commit()

    receipt_id = f"RCPT-{uuid.uuid4().hex[:12].upper()}"
    return {
        "status": "success",
        "receipt_id": receipt_id,
        "record_id": body.get("record_id") or session_id,
        "participant_id": part_token,
        "session_id": session_id,
        "records_count": 1,
    }


@router.get("/api/v1/research/records")
def list_research_records(db: Session = Depends(get_db)) -> dict[str, Any]:
    records = db.scalars(select(SessionRecord).order_by(SessionRecord.started_at.desc()).limit(50)).all()
    formatted = []
    for r in records:
        entry = dict(r.summary) if isinstance(r.summary, dict) else {}
        entry["id"] = r.id
        entry["participant_id"] = r.participant_id
        entry["created_at"] = r.started_at.isoformat()
        formatted.append(entry)
    return {
        "total_records": len(records),
        "records": formatted,
    }


@router.post("/api/v1/carekit/sync")
@router.post("/api/v1/batch/sync")
async def sync_carekit(
    request: Request,
    x_idempotency_key: Optional[str] = Header(None, alias="X-Idempotency-Key"),
    db: Session = Depends(get_db),
) -> dict[str, Any]:
    try:
        raw_body = await request.body()
        if not raw_body:
            raise HTTPException(status_code=400, detail="Empty request body")
        body = json.loads(raw_body.decode("utf-8"))
    except HTTPException:
        raise
    except Exception:
        raise HTTPException(status_code=400, detail="Invalid JSON body")

    child_id = body.get("child_id") or body.get("participant_id")
    if not child_id:
        raise HTTPException(status_code=400, detail="Missing child_id or participant_id")

    idem_key = x_idempotency_key or body.get("sync_id") or body.get("idempotency_key")
    if idem_key:
        existing_key = db.scalar(select(IdempotencyKey).where(IdempotencyKey.key == idem_key))
        if existing_key:
            return {
                "status": "duplicate",
                "idempotency_key": idem_key,
                "message": "Payload with this idempotency key was already ingested",
            }

    org = get_default_org(db)
    participant = get_or_create_participant(db, org.id, child_id)

    synced = 0
    # Process tasks / events
    tasks_list = body.get("events") or body.get("task_events") or []
    for task in tasks_list:
        db.add(CareEvent(
            org_id=org.id,
            participant_id=participant.id,
            kind="task",
            payload=task,
        ))
        synced += 1

    # Process medications
    meds_list = body.get("medications") or body.get("medication_doses") or []
    for med in meds_list:
        db.add(CareEvent(
            org_id=org.id,
            participant_id=participant.id,
            kind="medication",
            payload=med,
        ))
        synced += 1

    # Process diary
    diary_list = body.get("diary") or body.get("diary_entries") or []
    for diary in diary_list:
        db.add(CareEvent(
            org_id=org.id,
            participant_id=participant.id,
            kind="diary",
            payload=diary,
        ))
        synced += 1

    if idem_key:
        db.add(IdempotencyKey(
            key=idem_key,
            scope="carekit",
            response_hash="success",
            status_code=200,
        ))

    db.commit()

    receipt_id = f"CK-{uuid.uuid4().hex[:12].upper()}"
    return {
        "status": "success",
        "child_id": child_id,
        "participant_id": child_id,
        "events_ingested": synced,
        "accepted": synced,
        "receipt_id": receipt_id,
    }


@router.get("/api/v1/carekit/summary/{child_id}")
def get_carekit_summary(child_id: str, db: Session = Depends(get_db)) -> dict[str, Any]:
    part = db.scalar(select(Participant).where(Participant.token == child_id))
    if not part:
        return {
            "child_id": child_id,
            "participant_id": child_id,
            "total_events": 0,
            "session_count": 0,
            "checkin_count": 0,
            "summary": {
                "tasks_completed": 0,
                "medications_logged": 0,
                "diary_entries_logged": 0,
                "completion_rate_percent": 0.0,
                "recent_moods": [],
            },
        }

    events = db.scalars(select(CareEvent).where(CareEvent.participant_id == part.id)).all()
    tasks = [e for e in events if e.kind == "task"]
    meds = [e for e in events if e.kind == "medication"]
    diaries = [e for e in events if e.kind == "diary"]

    recent_moods = []
    for d in diaries:
        payload = d.payload if isinstance(d.payload, dict) else {}
        mood = payload.get("mood")
        if mood and mood not in recent_moods:
            recent_moods.append(mood)

    session_count = sum(
        1 for t in tasks
        if isinstance(t.payload, dict) and (
            "session" in str(t.payload.get("task_id", ""))
            or t.payload.get("metadata", {}).get("type") == "session"
        )
    )
    checkin_count = sum(
        1 for t in tasks
        if isinstance(t.payload, dict) and (
            "checkin" in str(t.payload.get("task_id", ""))
            or t.payload.get("metadata", {}).get("type") == "checkin"
        )
    )

    completion_rate = 100.0 if len(tasks) > 0 else 0.0

    return {
        "child_id": child_id,
        "participant_id": child_id,
        "total_events": len(events),
        "session_count": session_count,
        "checkin_count": checkin_count,
        "total_task_completions": len(tasks),
        "total_medication_doses": len(meds),
        "total_diary_entries": len(diaries),
        "summary": {
            "tasks_completed": len(tasks),
            "medications_logged": len(meds),
            "diary_entries_logged": len(diaries),
            "completion_rate_percent": completion_rate,
            "recent_moods": recent_moods,
        },
    }
