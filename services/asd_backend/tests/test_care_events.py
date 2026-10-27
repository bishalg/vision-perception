import json
import uuid


def test_carekit_sync_and_storage(client):
    child_id = "PEDIATRIC-TEST-001"
    payload = {
        "child_id": child_id,
        "events": [
            {"task_id": "morning_eye_contact", "is_completed": True, "completed_at": "2026-09-14T08:00:00Z", "category": "social"},
            {"task_id": "afternoon_play", "is_completed": True, "completed_at": "2026-09-14T14:00:00Z", "category": "play"},
        ],
        "medications": [
            {"medication_id": "omega3", "dose_index": 0, "dose_string": "1 capsule", "taken_at": "2026-09-14T08:05:00Z"},
        ],
        "diary": [
            {"mood": "happy", "eye_contact_rating": 4, "joint_attention_count": 8, "notes": "Engaged well today", "date": "2026-09-14"},
        ],
        "sync_timestamp": "2026-09-14T08:15:00Z",
    }

    res = client.post(
        "/api/v1/carekit/sync",
        json=payload,
        headers={"X-Idempotency-Key": "CK-IDEMP-CARE-001"},
    )
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "success"
    assert data["child_id"] == child_id
    assert data["events_ingested"] >= 4
    assert data["receipt_id"].startswith("CK-")


def test_carekit_idempotency_deduplication(client):
    child_id = "PEDIATRIC-TEST-002"
    payload = {
        "child_id": child_id,
        "events": [{"task_id": "duplicate_check", "is_completed": True, "completed_at": "2026-09-14T09:00:00Z"}],
    }
    idem_key = f"CK-IDEMP-DUP-{uuid.uuid4().hex[:8]}"

    # First request
    res1 = client.post("/api/v1/carekit/sync", json=payload, headers={"X-Idempotency-Key": idem_key})
    assert res1.status_code == 200
    d1 = res1.json()
    assert d1["status"] == "success"

    # Second request with same key
    res2 = client.post("/api/v1/carekit/sync", json=payload, headers={"X-Idempotency-Key": idem_key})
    assert res2.status_code == 200
    d2 = res2.json()
    assert d2["status"] == "duplicate"
    assert d2["idempotency_key"] == idem_key


def test_carekit_summary_aggregation(client):
    child_id = "PEDIATRIC-TEST-003"
    payload = {
        "child_id": child_id,
        "events": [
            {"task_id": "social_session_1", "is_completed": True, "completed_at": "2026-09-14T08:00:00Z", "metadata": {"type": "session"}},
            {"task_id": "daily_checkin_1", "is_completed": True, "completed_at": "2026-09-14T12:00:00Z", "metadata": {"type": "checkin"}},
        ],
        "medications": [
            {"medication_id": "vitamin_d", "dose_string": "1000 IU", "taken_at": "2026-09-14T08:30:00Z"},
        ],
        "diary": [
            {"mood": "calm", "eye_contact_rating": 5, "notes": "Great eye contact during storytime", "date": "2026-09-14"},
        ],
    }

    sync_res = client.post("/api/v1/carekit/sync", json=payload)
    assert sync_res.status_code == 200

    summary_res = client.get(f"/api/v1/carekit/summary/{child_id}")
    assert summary_res.status_code == 200
    data = summary_res.json()
    assert data["child_id"] == child_id
    assert data["summary"]["tasks_completed"] >= 2
    assert data["summary"]["medications_logged"] >= 1
    assert data["summary"]["diary_entries_logged"] >= 1
    assert "calm" in data["summary"]["recent_moods"]


def test_carekit_summary_empty(client):
    res = client.get("/api/v1/carekit/summary/NONEXISTENT-CHILD")
    assert res.status_code == 200
    data = res.json()
    assert data["child_id"] == "NONEXISTENT-CHILD"
    assert data["summary"]["tasks_completed"] == 0
    assert data["summary"]["medications_logged"] == 0
    assert data["summary"]["diary_entries_logged"] == 0
    assert data["summary"]["recent_moods"] == []
