#!/usr/bin/env python3
"""TDD tests for tools/cctv_fetcher.py — real I/O, real local HTTP, no mocks.

A localhost http.server exercises the genuine urllib fetch path; failure
cases use unroutable endpoints. Fixture assertions decode with real cv2.
"""

import functools
import http.server
import json
import os
import sys
import threading
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cctv_fetcher

FIXTURE_PNG = Path(__file__).resolve().parent.parent / "data/fixtures/cctv/day_clear/caltrans_i80_day.png"


@pytest.fixture()
def workdir(tmp_path):
    return tmp_path


def test_create_mock_fixture_writes_decodable_640x480(workdir):
    pytest.importorskip("cv2")
    import cv2

    out = workdir / "mock_day.png"
    cctv_fetcher.create_mock_fixture("caltrans_i80_drum", out, mode="day_clear")
    assert out.exists()
    img = cv2.imread(str(out), cv2.IMREAD_COLOR)
    assert img is not None
    assert img.shape[1] == 640 and img.shape[0] == 480
    meta = json.loads((workdir / "mock_day.json").read_text())
    assert meta["agency"] == "Caltrans District 3"


def test_unknown_camera_returns_false(workdir):
    out = workdir / "x.png"
    assert cctv_fetcher.fetch_live_frame("no_such_cam", out) is False
    assert not out.exists()


def test_mock_registry_entry_requires_explicit_fallback(workdir):
    out = workdir / "pier.png"
    # Honest default: no live feed exists, must not silently synthesize.
    assert cctv_fetcher.fetch_live_frame("bhote_koshi_pier", out) is False
    assert not out.exists()
    # Explicit opt-in still works and is logged as synthetic.
    assert cctv_fetcher.fetch_live_frame("bhote_koshi_pier", out, allow_fallback=True) is True
    assert out.exists()


def test_network_failure_returns_false_without_fallback(workdir):
    registry = {
        "dead_cam": {
            "agency": "Test",
            "name": "Dead",
            "location": {"lat": 0.0, "lon": 0.0, "elevation_m": 0.0},
            "pose": {"heading_deg": 0.0, "pitch_deg": 0.0, "fov_deg": 60.0},
            "url": "http://127.0.0.1:9/unreachable.jpg",
        }
    }
    out = workdir / "dead.jpg"
    assert (
        cctv_fetcher.fetch_live_frame("dead_cam", out, timeout=1.0, registry=registry) is False
    )
    assert not out.exists()


def test_network_failure_with_fallback_writes_fixture(workdir):
    registry = {
        "dead_cam": {
            "agency": "Test",
            "name": "Dead",
            "location": {"lat": 0.0, "lon": 0.0, "elevation_m": 0.0},
            "pose": {"heading_deg": 0.0, "pitch_deg": 0.0, "fov_deg": 60.0},
            "url": "http://127.0.0.1:9/unreachable.jpg",
        }
    }
    out = workdir / "dead.png"
    assert (
        cctv_fetcher.fetch_live_frame(
            "dead_cam", out, timeout=1.0, allow_fallback=True, registry=registry
        )
        is True
    )
    assert out.exists()


class _Handler(http.server.BaseHTTPRequestHandler):
    body = b""
    content_type = "image/jpeg"

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", self.content_type)
        self.send_header("Content-Length", str(len(self.body)))
        self.end_headers()
        self.wfile.write(self.body)

    def log_message(self, *args):
        pass


def _serve(body: bytes, content_type: str):
    handler = type("_H", (_Handler,), {"body": body, "content_type": content_type})
    server = http.server.HTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def _registry_for(server, path="/cam.jpg"):
    return {
        "local_cam": {
            "agency": "Test",
            "name": "Local",
            "location": {"lat": 1.0, "lon": 2.0, "elevation_m": 3.0},
            "pose": {"heading_deg": 0.0, "pitch_deg": 0.0, "fov_deg": 60.0},
            "url": f"http://127.0.0.1:{server.server_port}{path}",
        }
    }


def test_fetch_real_png_from_local_server(workdir):
    body = FIXTURE_PNG.read_bytes()
    assert body[:8] == b"\x89PNG\r\n\x1a\n"
    server = _serve(body, "image/png")
    try:
        out = workdir / "real.png"
        assert (
            cctv_fetcher.fetch_live_frame(
                "local_cam", out, timeout=5.0, registry=_registry_for(server)
            )
            is True
        )
        assert out.read_bytes() == body
        meta = json.loads((workdir / "real.json").read_text())
        assert meta["agency"] == "Test"
    finally:
        server.shutdown()


def test_fetch_rejects_html_error_page(workdir):
    server = _serve(b"<html><body>Access Denied</body></html>" * 100, "text/html")
    try:
        out = workdir / "evil.jpg"
        assert (
            cctv_fetcher.fetch_live_frame(
                "local_cam", out, timeout=5.0, registry=_registry_for(server)
            )
            is False
        )
        assert not out.exists()
    finally:
        server.shutdown()


def test_is_decodable_image_rejects_small_and_html():
    assert cctv_fetcher.is_decodable_image(b"\xff\xd8" + b"\x00" * 2000) is True
    assert cctv_fetcher.is_decodable_image(b"\x89PNG\r\n\x1a\n" + b"\x00" * 2000) is True
    assert cctv_fetcher.is_decodable_image(b"<html>" * 500) is False
    assert cctv_fetcher.is_decodable_image(b"\xff\xd8") is False  # too small
