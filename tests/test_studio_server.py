#!/usr/bin/env python3
"""tests/test_studio_server.py — Comprehensive unit tests for CurvStudio server REST endpoints."""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import studio_server  # noqa: E402


class MockHandlerWrapper:
    def __init__(self, method: str, path: str, payload: dict | None = None):
        self.method = method
        self.path = path
        body = json.dumps(payload).encode("utf-8") if payload is not None else b""
        self.headers = {
            "Content-Length": str(len(body)),
            "Content-Type": "application/json",
        }
        self.rfile = io.BytesIO(body)
        self.wfile = io.BytesIO()
        self.response_code = 200
        self.response_headers = {}
        self.response_data = None

        # Instantiate handler without socket setup
        self.handler = studio_server.StudioHandler.__new__(studio_server.StudioHandler)
        self.handler.path = self.path
        self.handler.headers = self.headers
        self.handler.rfile = self.rfile
        self.handler.wfile = self.wfile
        self.handler.send_response = self._send_response
        self.handler.send_header = self._send_header
        self.handler.end_headers = self._end_headers
        self.handler.send_json = self._send_json

    def _send_response(self, code, message=None):
        self.response_code = code

    def _send_header(self, k, v):
        self.response_headers[k] = v

    def _end_headers(self):
        pass

    def _send_json(self, data, status=200):
        self.response_code = status
        self.response_data = data
        self.wfile.write(json.dumps(data).encode("utf-8"))

    def execute(self) -> dict:
        if self.method == "GET":
            self.handler.do_GET()
        elif self.method == "POST":
            self.handler.do_POST()
        else:
            raise ValueError(f"Unsupported method {self.method}")
        return self.response_data


def test_curv_studio_endpoints():
    print("================================================================")
    print("CurvStudio REST API Test Suite")
    print("================================================================")

    # 1. GET /api/domains
    print("1. Testing GET /api/domains...")
    h1 = MockHandlerWrapper("GET", "/api/domains")
    data1 = h1.execute()
    assert h1.response_code == 200, f"Expected 200, got {h1.response_code}"
    domains = [d["id"] for d in data1.get("domains", [])]
    assert "palm" in domains, "Palm domain missing"
    assert "flood" in domains, "Flood domain missing"
    print(f"   [PASS] Found domains: {domains}")

    # 2. GET /api/samples?domain=palm
    print("2. Testing GET /api/samples?domain=palm...")
    h2 = MockHandlerWrapper("GET", "/api/samples?domain=palm")
    data2 = h2.execute()
    assert h2.response_code == 200
    assert data2["domain"]["id"] == "palm"
    samples_palm = data2["samples"]
    assert len(samples_palm) > 0, "No palm samples loaded"
    print(f"   [PASS] Loaded {len(samples_palm)} palm samples. First: {samples_palm[0]['filename']}")

    # 3. GET /api/samples?domain=flood
    print("3. Testing GET /api/samples?domain=flood...")
    h3 = MockHandlerWrapper("GET", "/api/samples?domain=flood")
    data3 = h3.execute()
    assert h3.response_code == 200
    assert data3["domain"]["id"] == "flood"
    samples_flood = data3["samples"]
    assert len(samples_flood) > 0, "No flood samples loaded"
    print(f"   [PASS] Loaded {len(samples_flood)} flood samples. First: {samples_flood[0]['filename']}")

    # 4. POST /api/save_truth
    print("4. Testing POST /api/save_truth...")
    test_truth_path = "output/test_studio_truth.json"
    h4 = MockHandlerWrapper("POST", "/api/save_truth", {
        "domain": "palm",
        "target_file": test_truth_path,
        "data": {
            "heart_line": [[[100, 100], [200, 200]]],
            "head_line": [[[120, 150], [220, 250]]],
            "life_line": [[[80, 120], [85, 200], [90, 300]]],
        },
        "auto_lines": {},
    })
    data4 = h4.execute()
    assert h4.response_code == 200
    assert data4.get("ok") is True
    assert (ROOT / test_truth_path).exists()
    print("   [PASS] Ground truth JSON successfully written and verified on disk.")

    # 5. POST /api/dataset_split
    print("5. Testing POST /api/dataset_split (Train 70% / Valid 20% / Test 10%)...")
    h5 = MockHandlerWrapper("POST", "/api/dataset_split", {
        "domain": "palm",
        "split": {"train": 70, "valid": 20, "test": 10},
        "labeled_only": False,
    })
    data5 = h5.execute()
    assert h5.response_code == 200
    assert data5.get("ok") is True
    counts = data5.get("counts", {})
    assert counts.get("total") == len(samples_palm)
    assert counts.get("train") + counts.get("valid") + counts.get("test") == counts.get("total")
    print(f"   [PASS] Partitioned dataset: {counts['train']} Train / {counts['valid']} Valid / {counts['test']} Test.")

    # 6. POST /api/run_inference (palm)
    print("6. Testing POST /api/run_inference (palm)...")
    sample_img = samples_palm[0]["image_url"]
    h6 = MockHandlerWrapper("POST", "/api/run_inference", {
        "domain": "palm",
        "image_url": sample_img,
        "confidence_threshold": 0.3,
    })
    data6 = h6.execute()
    assert h6.response_code == 200
    assert data6.get("ok") is True
    assert "predictions" in data6
    print(f"   [PASS] Native C++ CurvEngine inference completed in {data6['duration_ms']} ms.")
    print(f"   [PASS] Extracted {len(data6['predictions'])} major lines with confidence.")

    # 7. GET /api/flood_delta
    print("7. Testing GET /api/flood_delta...")
    h7 = MockHandlerWrapper("GET", "/api/flood_delta")
    data7 = h7.execute()
    assert h7.response_code == 200
    assert data7.get("ok") is True
    delta = data7.get("delta", {})
    assert "event" in delta
    print(f"   [PASS] Loaded morphology delta: {delta.get('event')[:45]}...")

    # 8. GET / (CurvStudio Shell HTML)
    print("8. Testing GET / (CurvStudio HTML shell)...")
    h8 = MockHandlerWrapper("GET", "/")
    h8.execute()
    assert h8.response_code == 200
    html_output = h8.wfile.getvalue().decode("utf-8")
    assert "two-tier-sidebar" in html_output, "Two-tier sidebar markup missing from HTML shell"
    assert "floatingBottomBar" in html_output, "Floating bottom bar missing from HTML shell"
    print("   [PASS] CurvStudio HTML shell delivered with two-tier layout & floating bottom bar.")

    # 9. GET /assets/index.js & /assets/index.css (Vite Bundles)
    print("9. Testing GET /assets/index.js & /assets/index.css...")
    h9_js = MockHandlerWrapper("GET", "/assets/index.js")
    h9_js.execute()
    assert h9_js.response_code == 200
    assert len(h9_js.wfile.getvalue()) > 1000

    h9_css = MockHandlerWrapper("GET", "/assets/index.css")
    h9_css.execute()
    assert h9_css.response_code == 200
    assert len(h9_css.wfile.getvalue()) > 1000
    print("   [PASS] Vite static bundles verified and served.")

    print("================================================================")
    print("ALL CURVSTUDIO ENDPOINT TESTS PASSED (9/9)")
    print("================================================================")


if __name__ == "__main__":
    test_curv_studio_endpoints()

