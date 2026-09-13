#!/usr/bin/env python3
"""nebius_nemotron_agent.py — Physical AI flood-reasoning client (Nebius Track).

Open-source client: ingests validated flood_evidence_v1.json, reasons over it
with NVIDIA Nemotron via the Nebius Token Factory OpenAI-compatible endpoint,
and emits a strict JSON action (alert dispatch or secondary verification).

Real HTTP throughout (urllib, retries with backoff on 429/5xx). No simulated
model output: without credentials the tool fails loudly instead of inventing
a decision. Contains zero proprietary C++ internals — only the public
telemetry schema crosses the IP boundary.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

DEFAULT_ENDPOINT = "https://api.tokenfactory.nebius.com/v1/chat/completions"
MODEL_NANO = "nvidia/nemotron-3-nano"
MODEL_SUPER = "nvidia/nemotron-3-super"
MODEL_ULTRA = "nvidia/nemotron-3-ultra"

RETRYABLE = {429, 500, 502, 503, 504}
REQUIRED_ACTION_KEYS = ("decision", "severity", "evacuation_radius_m", "rationale")

SYSTEM_PROMPT = """You are a flood-hazard reasoning agent for a physical environmental monitoring system.
You receive quantitative river-telemetry evidence (channel geometry, dual control gates, quality flags).
Reason step by step:
1. Evaluate channel width surge and flood signature ratios.
2. Verify spatial and temporal control gates (a gate failure means the signal may be clouds/shadows, not water).
3. Check frame quality (reject blurred/unusable frames).
4. Issue exactly one decision.

Respond with STRICT JSON only, no prose, matching this schema:
{"decision": "ISSUE_ALERT" | "REQUEST_VERIFICATION" | "STAND_DOWN",
 "severity": "CRITICAL" | "WARNING" | "NORMAL",
 "evacuation_radius_m": <non-negative number>,
 "verification_target": <cctv camera id string or null>,
 "rationale": <one sentence>}"""


class NemotronError(RuntimeError):
    """Raised for transport, auth, or protocol failures (never silently faked)."""


def select_model(evidence: dict) -> str:
    """Tiering: Nano for routine polling, Ultra for anomalous deltas."""
    hydro = evidence.get("hydrology_metrics", {})
    width_ratio = hydro.get("channel_width_ratio", hydro.get("width_ratio", 1.0))
    try:
        width_ratio = float(width_ratio)
    except (TypeError, ValueError):
        width_ratio = 1.0
    if evidence.get("alert_recommendation") == "CRITICAL" or width_ratio >= 2.0:
        return MODEL_ULTRA
    if evidence.get("alert_recommendation") == "WARNING" or width_ratio >= 1.5:
        return MODEL_SUPER
    return MODEL_NANO


def build_messages(evidence: dict) -> list[dict]:
    controls = evidence.get("controls", {})
    hydro = evidence.get("hydrology_metrics", {})
    quality = evidence.get("quality", {})
    user = (
        "Flood telemetry evidence:\n"
        f"- event_id: {evidence.get('event_id')}\n"
        f"- timestamp: {evidence.get('timestamp_iso')}\n"
        f"- alert_recommendation (C++ engine): {evidence.get('alert_recommendation')}\n"
        f"- hydrology_metrics: {json.dumps(hydro, sort_keys=True)}\n"
        f"- spatial_control_pass={controls.get('spatial_control_pass')} "
        f"(ratio={controls.get('spatial_control_ratio')})\n"
        f"- temporal_control_pass={controls.get('temporal_control_pass')} "
        f"(ratio={controls.get('temporal_control_ratio')})\n"
        f"- frame usable={quality.get('is_usable')} blur={quality.get('blur_score')}\n"
        f"- feature count: {len(evidence.get('spatial_features', []))}\n"
        "Decide."
    )
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": user},
    ]


def chat_complete(
    messages: list[dict],
    model: str,
    api_key: str,
    endpoint: str = DEFAULT_ENDPOINT,
    timeout: float = 60.0,
    max_retries: int = 4,
) -> str:
    """POSTs a chat request; retries 429/5xx with exponential backoff."""
    if not api_key:
        raise NemotronError("NEBIUS_API_KEY is not set; refusing to simulate a decision.")
    payload = json.dumps({"model": model, "messages": messages, "temperature": 0.1}).encode()
    last_error: str = ""
    for attempt in range(max_retries + 1):
        req = urllib.request.Request(
            endpoint,
            data=payload,
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key}"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                body = json.loads(resp.read().decode("utf-8"))
            return body["choices"][0]["message"]["content"]
        except urllib.error.HTTPError as exc:
            last_error = f"HTTP {exc.code}: {exc.read(200).decode('utf-8', 'replace')}"
            if exc.code not in RETRYABLE or attempt == max_retries:
                raise NemotronError(last_error) from exc
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            last_error = f"transport error: {exc}"
            if attempt == max_retries:
                raise NemotronError(last_error) from exc
        except (KeyError, IndexError, json.JSONDecodeError) as exc:
            raise NemotronError(f"malformed model response envelope: {exc}") from exc
        time.sleep(min(2.0**attempt, 15.0))
    raise NemotronError(last_error or "exhausted retries")


def parse_action(raw: str) -> dict:
    """Parses and validates the strict-JSON action (strips code fences)."""
    text = raw.strip()
    if text.startswith("```"):
        text = text.strip("`").strip()
        if text.lower().startswith("json"):
            text = text[4:].strip()
    try:
        action = json.loads(text)
    except json.JSONDecodeError as exc:
        raise NemotronError(f"model did not return strict JSON: {exc}") from exc
    for key in REQUIRED_ACTION_KEYS:
        if key not in action:
            raise NemotronError(f"model action missing required key: {key}")
    if action["decision"] not in ("ISSUE_ALERT", "REQUEST_VERIFICATION", "STAND_DOWN"):
        raise NemotronError(f"unknown decision: {action['decision']}")
    if action["severity"] not in ("CRITICAL", "WARNING", "NORMAL"):
        raise NemotronError(f"unknown severity: {action['severity']}")
    if not isinstance(action["evacuation_radius_m"], (int, float)) or action["evacuation_radius_m"] < 0:
        raise NemotronError("evacuation_radius_m must be a non-negative number")
    return action


def dispatch_action(action: dict, webhook_url: str = "", timeout: float = 15.0) -> dict:
    """POSTs an ISSUE_ALERT action to the civil-defense webhook.

    Non-alert decisions are returned unsent. Empty webhook URL with an alert
    is a hard error (alerts must never vanish silently).
    """
    if action["decision"] != "ISSUE_ALERT":
        return {"sent": False, "action": action}
    if not webhook_url:
        raise NemotronError("ISSUE_ALERT decided but no --webhook-url configured; refusing to drop alert.")
    req = urllib.request.Request(
        webhook_url,
        data=json.dumps(action).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return {"sent": True, "status": resp.status, "action": action}
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise NemotronError(f"webhook dispatch failed: {exc}") from exc


def evaluate_evidence_file(
    path: Path, api_key: str, endpoint: str = DEFAULT_ENDPOINT, model_override: str = ""
) -> dict:
    evidence = json.loads(path.read_text(encoding="utf-8"))
    model = model_override or select_model(evidence)
    raw = chat_complete(build_messages(evidence), model, api_key, endpoint)
    action = parse_action(raw)
    return {"model": model, "event_id": evidence.get("event_id"), "action": action}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--webhook-url", default="")
    parser.add_argument("--model", default="", help="Override tiered model selection")
    parser.add_argument("--endpoint", default=DEFAULT_ENDPOINT)
    parser.add_argument("--print-prompt", action="store_true", help="Print prompt and exit (no API call)")
    args = parser.parse_args()

    evidence = json.loads(args.evidence.read_text(encoding="utf-8"))
    if args.print_prompt:
        print(json.dumps(build_messages(evidence), indent=2))
        return 0

    api_key = os.environ.get("NEBIUS_API_KEY", "")
    try:
        result = evaluate_evidence_file(args.evidence, api_key, args.endpoint, args.model)
        if args.webhook_url:
            result["dispatch"] = dispatch_action(result["action"], args.webhook_url)
        print(json.dumps(result, indent=2))
    except NemotronError as exc:
        print(f"[-] {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
