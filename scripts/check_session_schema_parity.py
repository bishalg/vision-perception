#!/usr/bin/env python3
"""Contract parity check for ASD Gaze Session JSON Schema (WP-D.2).

Validates that:
1. docs/spec/schemas/asd/session-v1.6.json exists (the stored contract; Rust backend types must match it).
2. Canonical fixture payloads conform to the schema.
3. Malformed payloads (e.g. missing sessionId) fail validation.
"""

import json
import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parent.parent

def validate_payload(schema: dict, payload: dict) -> list[str]:
    """Lightweight pure-python JSON schema validation for key required properties."""
    errors = []
    # Check required fields
    for req in schema.get("required", []):
        if req not in payload:
            errors.append(f"Missing required field: {req}")

    # Check property types
    props = schema.get("properties", {})
    for k, v in payload.items():
        if k in props and v is not None:
            expected_type = props[k].get("type")
            if expected_type == "string" and not isinstance(v, str):
                errors.append(f"Field {k} expected string, got {type(v).__name__}")
            elif expected_type == "number" and not isinstance(v, (int, float)):
                errors.append(f"Field {k} expected number, got {type(v).__name__}")
            elif expected_type == "integer" and not isinstance(v, int):
                errors.append(f"Field {k} expected integer, got {type(v).__name__}")
            elif expected_type == "boolean" and not isinstance(v, bool):
                errors.append(f"Field {k} expected boolean, got {type(v).__name__}")
            elif expected_type == "array" and not isinstance(v, list):
                errors.append(f"Field {k} expected array, got {type(v).__name__}")
            elif expected_type == "object" and not isinstance(v, dict):
                errors.append(f"Field {k} expected object, got {type(v).__name__}")

    return errors


def main():
    schema_path = repo_root / "docs" / "spec" / "schemas" / "asd" / "session-v1.6.json"
    if not schema_path.exists():
        print(f"FAILED: Schema file not found at {schema_path}", file=sys.stderr)
        sys.exit(1)

    with open(schema_path, "r", encoding="utf-8") as f:
        stored_schema = json.load(f)

    # 1. Validate canonical valid fixture
    valid_payload = {
        "sessionId": "sess_parity_test_001",
        "participantToken": "child-test001",
        "captureSource": "truedepth",
        "durationSec": 480.0,
        "validRatio": 0.95,
        "socialPreferenceScore": 0.72,
        "qualityVerdict": "optimal",
        "attentionFlag": "green",
        "battery": "express_30s",
        "schemaVersion": "1.6",
        "perVideo": [
            {
                "clip_id": "stim_01_social_smile_8s",
                "order": 1,
                "dwell_social": 6.5,
                "dwell_geometric": 0.8,
                "dwell_eye": 3.2,
                "dwell_mouth": 1.1,
                "fixation_count": 14,
                "mean_fixation_ms": 280.0,
                "status": "onTarget",
                "score": 0.89,
                "weight": 1.0,
            }
        ]
    }
    errs = validate_payload(stored_schema, valid_payload)
    if errs:
        print(f"FAILED: Valid payload failed schema check: {errs}", file=sys.stderr)
        sys.exit(1)

    # 2. Validate rejection on missing required sessionId
    invalid_payload = {
        "participantToken": "child-missing-session",
        "socialPreferenceScore": 0.5,
    }
    errs = validate_payload(stored_schema, invalid_payload)
    if not errs or "Missing required field: sessionId" not in errs:
        print("FAILED: Malformed payload was unexpectedly accepted!", file=sys.stderr)
        sys.exit(1)

    print("OK: ASD Gaze Session JSON Schema parity check passed.")


if __name__ == "__main__":
    main()
