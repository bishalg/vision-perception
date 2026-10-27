import uuid


def test_study_info_shim(client):
    response = client.get("/api/v1/research/study-info")
    assert response.status_code == 200
    data = response.json()
    assert "study_id" in data
    assert "title" in data
    assert "irb_protocol" in data


def test_research_ingest_and_list_shim(client):
    record_id = str(uuid.uuid4())
    payload = {
        "record_id": record_id,
        "participant_id": "PART-001",
        "session_id": "SESS-001",
        "data": {
            "overallScore": 0.82,
            "metrics": {"social_visual_preference": 0.85},
        },
    }

    # Ingest
    res_ingest = client.post("/api/v1/research/ingest", json=payload)
    assert res_ingest.status_code == 200
    ingest_data = res_ingest.json()
    assert ingest_data["status"] in ("success", "received")
    assert ingest_data["record_id"] == record_id

    # List
    res_list = client.get("/api/v1/research/records")
    assert res_list.status_code == 200
    listing = res_list.json()
    record_items = listing.get("records", listing) if isinstance(listing, dict) else listing
    assert len(record_items) >= 1
    found = any(r.get("id") == "SESS-001" or r.get("record_id") == record_id for r in record_items)
    assert found


def test_carekit_sync_and_summary_shim(client):
    participant_id = "CHILD-42"
    batch = {
        "participant_id": participant_id,
        "events": [
            {
                "event_id": str(uuid.uuid4()),
                "task_id": "social_eye_tracking_session",
                "occurred_at": "2026-10-27T12:00:00Z",
                "values": {"duration": 180},
                "metadata": {"type": "session"},
            },
            {
                "event_id": str(uuid.uuid4()),
                "task_id": "parent_daily_checkin",
                "occurred_at": "2026-10-27T12:05:00Z",
                "values": {"completed": 1},
                "metadata": {"type": "checkin"},
            },
        ],
    }

    # Sync batch
    res_sync = client.post("/api/v1/carekit/sync", json=batch)
    assert res_sync.status_code == 200
    data = res_sync.json()
    assert data["accepted"] == 2

    # Query summary
    res_summary = client.get(f"/api/v1/carekit/summary/{participant_id}")
    assert res_summary.status_code == 200
    summary = res_summary.json()
    assert summary["participant_id"] == participant_id
    assert summary["total_events"] == 2
    assert summary["session_count"] == 1
    assert summary["checkin_count"] == 1
