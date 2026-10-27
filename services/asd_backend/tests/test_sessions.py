import uuid


def test_session_ingest_and_idempotency(client):
    session_id = f"sess_{uuid.uuid4().hex[:12]}"
    idempotency_key = f"idem-{uuid.uuid4()}"
    participant_token = f"part-{uuid.uuid4().hex[:8]}"

    payload = {
        "sessionId": session_id,
        "participantToken": participant_token,
        "captureSource": "truedepth",
        "durationSec": 480.0,
        "validRatio": 0.95,
        "socialPreferenceScore": 0.84,
        "attentionFlag": "green",
        "perVideo": [
            {
                "clip_id": "stim_01_social_smile_8s",
                "dwell_social": 6.8,
                "dwell_geometric": 0.5,
                "score": 0.88,
                "status": "onTarget",
            }
        ],
    }

    headers = {"X-Idempotency-Key": idempotency_key}

    # 1. First submission -> ingested (duplicate: False)
    res1 = client.post("/v1/sessions", json=payload, headers=headers)
    assert res1.status_code == 200, res1.text
    data1 = res1.json()
    assert data1["sessionId"] == session_id
    assert data1["status"] == "ingested"
    assert data1["duplicate"] is False
    assert data1["frames_upload_url"] is not None

    # 2. Second submission with same Idempotency-Key -> duplicate: True
    res2 = client.post("/v1/sessions", json=payload, headers=headers)
    assert res2.status_code == 200, res2.text
    data2 = res2.json()
    assert data2["sessionId"] == session_id
    assert data2["status"] == "ingested"
    assert data2["duplicate"] is True

    # 3. Retrieve single session
    res3 = client.get(f"/v1/sessions/{session_id}")
    assert res3.status_code == 200
    retrieved = res3.json()
    assert retrieved["id"] == session_id
    assert retrieved["social_preference"] == 0.84
    assert len(retrieved["clip_scores"]) == 1

    # 4. List sessions
    res4 = client.get("/v1/sessions")
    assert res4.status_code == 200
    listing = res4.json()
    assert any(s["id"] == session_id for s in listing)


def test_device_register(client):
    payload = {
        "platform": "iOS",
        "app_version": "1.0.0",
        "label": "Clinic iPad Pro",
    }
    response = client.post("/v1/devices/register", json=payload)
    assert response.status_code == 200
    data = response.json()
    assert "device_id" in data
    assert "device_token" in data
    assert data["status"] == "registered"


def test_config_bundle(client):
    response = client.get("/v1/config/bundle")
    assert response.status_code == 200
    data = response.json()
    assert "bundle_version" in data
    assert "active_catalog_version" in data
    assert "scoring_profile" in data
