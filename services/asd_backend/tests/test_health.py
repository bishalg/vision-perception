def test_root_health(client):
    response = client.get("/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
    assert data["service"] == "asd_backend"
    assert "version" in data


def test_v1_health(client):
    response = client.get("/v1/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
