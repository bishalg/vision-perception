#!/usr/bin/env python3
"""TDD tests for tools/nebius_nemotron_agent.py.

Transport is exercised over real localhost HTTP (http.server); the model is
never simulated — tests assert strict parsing, retry behavior, tiering, and
loud failure without credentials.
"""

import http.server
import json
import sys
import threading
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import nebius_nemotron_agent as agent

VALID_ACTION = {
    "decision": "ISSUE_ALERT",
    "severity": "CRITICAL",
    "evacuation_radius_m": 1500,
    "verification_target": None,
    "rationale": "Channel widening exceeds 2x with both control gates passing.",
}


def _evidence(alert="CRITICAL", width_ratio=2.2):
    return {
        "schema_version": "1.0.0",
        "event_id": "evt-1",
        "timestamp_iso": "2026-09-12T00:00:00Z",
        "payload_sha256": "b" * 64,
        "frame_dimensions": {"width": 640, "height": 480},
        "quality": {"is_usable": True, "blur_score": 300.0},
        "spatial_features": [],
        "hydrology_metrics": {"channel_width_ratio": width_ratio},
        "controls": {"spatial_control_pass": True, "temporal_control_pass": True},
        "alert_recommendation": alert,
    }


# --- pure logic: tiering / prompt / parsing (no network) ---


def test_tiering_critical_selects_ultra():
    assert agent.select_model(_evidence("CRITICAL", 2.2)) == agent.MODEL_ULTRA
    assert agent.select_model(_evidence("WARNING", 1.6)) == agent.MODEL_SUPER
    assert agent.select_model(_evidence("NORMAL", 1.0)) == agent.MODEL_NANO
    assert agent.select_model(_evidence("NORMAL", 2.5)) == agent.MODEL_ULTRA  # ratio overrides


def test_prompt_embeds_metrics():
    messages = agent.build_messages(_evidence())
    assert messages[0]["role"] == "system"
    user = messages[1]["content"]
    assert "evt-1" in user and "2.2" in user and "CRITICAL" in user


def test_parse_action_accepts_strict_json():
    assert agent.parse_action(json.dumps(VALID_ACTION))["decision"] == "ISSUE_ALERT"


def test_parse_action_strips_code_fences():
    assert agent.parse_action("```json\n" + json.dumps(VALID_ACTION) + "\n```")["severity"] == "CRITICAL"


def test_parse_action_rejects_garbage():
    with pytest.raises(agent.NemotronError):
        agent.parse_action("As an AI, I think we should evacuate maybe.")
    bad = dict(VALID_ACTION)
    del bad["rationale"]
    with pytest.raises(agent.NemotronError):
        agent.parse_action(json.dumps(bad))
    bad2 = dict(VALID_ACTION, decision="PANIC")
    with pytest.raises(agent.NemotronError):
        agent.parse_action(json.dumps(bad2))
    bad3 = dict(VALID_ACTION, evacuation_radius_m=-5)
    with pytest.raises(agent.NemotronError):
        agent.parse_action(json.dumps(bad3))


def test_missing_api_key_fails_loudly():
    with pytest.raises(agent.NemotronError):
        agent.chat_complete([], agent.MODEL_NANO, api_key="")


# --- real HTTP against localhost ---


class _Handler(http.server.BaseHTTPRequestHandler):
    """Scripted responder: plan is a list of (status, body) tuples."""

    plan: list = []
    seen_auth: list = []
    seen_paths: list = []

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        type(self).seen_auth.append(self.headers.get("Authorization"))
        type(self).seen_paths.append(self.path)
        status, body = type(self).plan.pop(0) if type(self).plan else (200, b"{}")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def _serve(plan):
    _Handler.plan = list(plan)
    _Handler.seen_auth = []
    _Handler.seen_paths = []
    server = http.server.HTTPServer(("127.0.0.1", 0), _Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def _chat_body(content: str) -> bytes:
    return json.dumps({"choices": [{"message": {"role": "assistant", "content": content}}]}).encode()


def test_chat_complete_success_over_real_http():
    server = _serve([(200, _chat_body(json.dumps(VALID_ACTION)))])
    try:
        out = agent.chat_complete(
            [{"role": "user", "content": "hi"}],
            agent.MODEL_NANO,
            api_key="test-key",
            endpoint=f"http://127.0.0.1:{server.server_port}/v1/chat",
            max_retries=0,
        )
        assert json.loads(out)["decision"] == "ISSUE_ALERT"
        assert _Handler.seen_auth == ["Bearer test-key"]
    finally:
        server.shutdown()


def test_chat_complete_retries_429_then_succeeds():
    server = _serve([(429, b'{"error":"rate limited"}'), (200, _chat_body("ok"))])
    try:
        out = agent.chat_complete(
            [{"role": "user", "content": "hi"}],
            agent.MODEL_NANO,
            api_key="k",
            endpoint=f"http://127.0.0.1:{server.server_port}/x",
            max_retries=2,
        )
        assert out == "ok"
        assert len(_Handler.seen_paths) == 2
    finally:
        server.shutdown()


def test_chat_complete_raises_on_400_without_retry():
    server = _serve([(400, b'{"error":"bad model"}')])
    try:
        with pytest.raises(agent.NemotronError):
            agent.chat_complete(
                [{"role": "user", "content": "hi"}],
                "nope",
                api_key="k",
                endpoint=f"http://127.0.0.1:{server.server_port}/x",
                max_retries=3,
            )
        assert len(_Handler.seen_paths) == 1
    finally:
        server.shutdown()


def test_dispatch_non_alert_not_sent():
    action = dict(VALID_ACTION, decision="STAND_DOWN")
    assert agent.dispatch_action(action, "") == {"sent": False, "action": action}


def test_dispatch_alert_without_webhook_raises():
    with pytest.raises(agent.NemotronError):
        agent.dispatch_action(VALID_ACTION, "")


def test_dispatch_alert_posts_to_webhook():
    server = _serve([(200, b'{"ok":true}')])
    try:
        res = agent.dispatch_action(VALID_ACTION, f"http://127.0.0.1:{server.server_port}/hook")
        assert res["sent"] is True and res["status"] == 200
    finally:
        server.shutdown()


def test_evaluate_evidence_file_end_to_end(tmp_path):
    ev = tmp_path / "ev.json"
    ev.write_text(json.dumps(_evidence()))
    server = _serve([(200, _chat_body(json.dumps(VALID_ACTION)))])
    try:
        result = agent.evaluate_evidence_file(
            ev, api_key="k", endpoint=f"http://127.0.0.1:{server.server_port}/v1"
        )
        assert result["model"] == agent.MODEL_ULTRA
        assert result["event_id"] == "evt-1"
        assert result["action"]["decision"] == "ISSUE_ALERT"
    finally:
        server.shutdown()
