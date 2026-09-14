#!/usr/bin/env python3
"""Unit and Integration Tests for ResearchKit Backend Ingestion Server."""

import json
import socket
import threading
import time
from http.client import HTTPConnection
from pathlib import Path
import pytest

from tools.asd_research_server import (
    Fixation,
    GazeSample,
    RectAOI,
    ResearchServerHandler,
    analyze_session,
    detect_fixations_ivt,
)
import socketserver


def get_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


@pytest.fixture(scope="module")
def server_port(tmp_path_factory):
    port = get_free_port()
    tmp_vault = tmp_path_factory.mktemp("test_research_vault")
    ResearchServerHandler.vault_dir = tmp_vault
    ResearchServerHandler.carekit_db_path = tmp_vault / "carekit_test.db"
    ResearchServerHandler._db_instance = None

    httpd = socketserver.TCPServer(("127.0.0.1", port), ResearchServerHandler)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    time.sleep(0.1)

    yield port
    httpd.shutdown()
    httpd.server_close()


def test_ivt_fixation_segmentation():
    # 5 samples within threshold -> 1 fixation
    samples = [
        GazeSample(t=0.0, x=100.0, y=100.0),
        GazeSample(t=20.0, x=101.0, y=100.5),
        GazeSample(t=40.0, x=100.5, y=101.0),
        GazeSample(t=60.0, x=101.5, y=100.0),
        GazeSample(t=85.0, x=100.0, y=100.0),
    ]
    fixations = detect_fixations_ivt(samples, velocity_threshold=150.0, min_fixation_ms=80.0)
    assert len(fixations) == 1
    assert fixations[0].duration_ms == 85.0
    assert 99.0 <= fixations[0].x <= 102.0


def test_analyze_session_geopref():
    samples = [
        GazeSample(t=0.0, x=150.0, y=150.0),
        GazeSample(t=100.0, x=150.0, y=150.0),
    ]
    aois = [
        RectAOI("Social", 0.0, 0.0, 300.0, 300.0),
        RectAOI("Geometric", 400.0, 0.0, 700.0, 300.0),
    ]
    report = analyze_session(samples, aois, "test_session", "child_001", 24)
    assert "features" in report
    assert report["features"]["social_dwell_ratio"] > 0.9
    assert report["features"]["social_preference_score"] == 1.0


def test_study_info_endpoint(server_port):
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request("GET", "/api/v1/research/study-info")
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert data["study_identifier"] == "CURV-ASD-GAZE-2026"
    assert data["irb_protocol_id"] == "IRB-ASD-2026-088"
    assert data["zero_raw_video_guarantee"] is True
    conn.close()


def test_ingest_researchkit_payload(server_port):
    payload = {
        "bundleType": "ResearchKitStudyPackage",
        "schemaVersion": "v1.4",
        "studyIdentifier": "CURV-ASD-GAZE-2026",
        "irbProtocolId": "IRB-ASD-2026-088",
        "participant": {
            "participantId": "STUDY-ASD-TEST01",
            "isConsented": True,
            "childAgeMonths": 26,
            "consentVersion": "v1.3-IRB2026"
        },
        "taskResults": [
            {
                "identifier": "AsdResearchStudyTask",
                "participantId": "STUDY-ASD-TEST01",
                "activeStepResult": {
                    "identifier": "ORKActiveStepResult",
                    "sessionId": "sess_unit_test",
                    "durationSec": 20.0,
                    "isValid": True,
                    "validSampleRatio": 0.95,
                    "qualityVerdict": "optimal",
                    "socialPreferenceScore": 0.65,
                    "eyeDwellRatio": 0.52,
                    "mouthDwellRatio": 0.22,
                    "firstEyeLatencyMs": 320.0,
                    "socialSmileIntensity": 0.75,
                    "smileReactivityIndex": 0.81,
                    "flatAffectScore": 0.12,
                    "surpriseScoreMean": 0.05
                }
            }
        ],
        "surveys": [
            {
                "totalFailed": 1,
                "riskLevel": "Low Risk",
                "failedQuestionIds": [5],
                "clinicalRecommendation": "Low developmental risk score."
            }
        ]
    }

    body = json.dumps(payload).encode("utf-8")
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request(
        "POST",
        "/api/v1/research/ingest",
        body=body,
        headers={"Content-Type": "application/json"}
    )
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert data["status"] == "success"
    assert data["receipt_id"].startswith("RCPT-")
    assert data["participant_id"] == "STUDY-ASD-TEST01"
    conn.close()


def test_records_listing_endpoint(server_port):
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request("GET", "/api/v1/research/records")
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert "total_records" in data
    assert data["total_records"] >= 1
    conn.close()


def test_ingest_empty_payload_rejected(server_port):
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request(
        "POST",
        "/api/v1/research/ingest",
        body=b"",
        headers={"Content-Type": "application/json"}
    )
    res = conn.getresponse()
    assert res.status == 400
    conn.close()


def test_carekit_sync_endpoint(server_port):
    payload = {
        "child_id": "PEDIATRIC-TEST-001",
        "events": [
            {"task_id": "morning_eye_contact", "is_completed": True, "completed_at": "2026-09-14T08:00:00Z", "category": "social"},
            {"task_id": "afternoon_play", "is_completed": True, "completed_at": "2026-09-14T14:00:00Z", "category": "play"}
        ],
        "medications": [
            {"medication_id": "omega3", "dose_index": 0, "dose_string": "1 capsule", "taken_at": "2026-09-14T08:05:00Z"}
        ],
        "diary": [
            {"mood": "happy", "eye_contact_rating": 4, "joint_attention_count": 8, "notes": "Engaged well today", "date": "2026-09-14"}
        ],
        "sync_timestamp": "2026-09-14T08:15:00Z"
    }

    body = json.dumps(payload).encode("utf-8")
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request(
        "POST",
        "/api/v1/carekit/sync",
        body=body,
        headers={"Content-Type": "application/json", "X-Idempotency-Key": "CK-IDEMP-001"}
    )
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert data["status"] == "success"
    assert data["child_id"] == "PEDIATRIC-TEST-001"
    assert data["events_ingested"] >= 4
    assert data["receipt_id"].startswith("CK-")
    conn.close()


def test_carekit_idempotency_key_deduplication(server_port):
    payload = {
        "child_id": "PEDIATRIC-TEST-001",
        "events": [{"task_id": "duplicate_check", "is_completed": True, "completed_at": "2026-09-14T09:00:00Z"}]
    }
    body = json.dumps(payload).encode("utf-8")
    key = "CK-IDEMP-DUP-999"

    # First request
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request("POST", "/api/v1/carekit/sync", body=body, headers={"Content-Type": "application/json", "X-Idempotency-Key": key})
    res = conn.getresponse()
    assert res.status == 200
    d1 = json.loads(res.read().decode("utf-8"))
    assert d1["status"] == "success"
    conn.close()

    # Second request with identical key -> duplicate
    conn2 = HTTPConnection("127.0.0.1", server_port)
    conn2.request("POST", "/api/v1/carekit/sync", body=body, headers={"Content-Type": "application/json", "X-Idempotency-Key": key})
    res2 = conn2.getresponse()
    assert res2.status == 200
    d2 = json.loads(res2.read().decode("utf-8"))
    assert d2["status"] == "duplicate"
    assert d2["idempotency_key"] == key
    conn2.close()


def test_carekit_summary_endpoint(server_port):
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request("GET", "/api/v1/carekit/summary/PEDIATRIC-TEST-001")
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert data["child_id"] == "PEDIATRIC-TEST-001"
    assert "summary" in data
    summary = data["summary"]
    assert summary["tasks_completed"] >= 2
    assert summary["medications_logged"] >= 1
    assert summary["diary_entries_logged"] >= 1
    assert summary["completion_rate_percent"] >= 50.0
    assert "happy" in summary["recent_moods"]
    conn.close()


def test_batch_sync_endpoint(server_port):
    batch_payload = {
        "carekit": {
            "child_id": "BATCH-CHILD-01",
            "events": [{"task_id": "speech_session", "is_completed": True, "completed_at": "2026-09-14T10:00:00Z"}],
            "medications": [],
            "diary": []
        },
        "researchkit": {
            "bundleType": "ResearchKitStudyPackage",
            "schemaVersion": "v1.4",
            "participant": {"participantId": "BATCH-PART-01"},
            "taskResults": []
        }
    }

    body = json.dumps(batch_payload).encode("utf-8")
    conn = HTTPConnection("127.0.0.1", server_port)
    conn.request("POST", "/api/v1/batch/sync", body=body, headers={"Content-Type": "application/json"})
    res = conn.getresponse()
    assert res.status == 200
    data = json.loads(res.read().decode("utf-8"))
    assert data["status"] == "success"
    assert "receipts" in data
    assert data["receipts"]["carekit_events"] >= 1
    assert data["receipts"]["researchkit_receipt"].startswith("RCPT-")
    conn.close()

