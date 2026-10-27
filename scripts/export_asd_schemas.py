#!/usr/bin/env python3
"""Export Pydantic models to JSON Schema for contract parity testing (WP-D.2)."""

import json
import sys
from pathlib import Path

repo_root = Path(__file__).resolve().parent.parent
backend_app = repo_root / "services" / "asd_backend"
sys.path.insert(0, str(backend_app))

from app.schemas.session import SessionIngestRequest, SessionIngestResponse, SessionPerVideoScore


def export_schemas():
    output_dir = repo_root / "docs" / "spec" / "schemas" / "asd"
    output_dir.mkdir(parents=True, exist_ok=True)

    session_schema_path = output_dir / "session-v1.6.json"
    schema = SessionIngestRequest.model_json_schema()
    schema["$id"] = "https://schemas.curv.org/asd/session-v1.6.json"
    schema["description"] = "Curv Pediatric ASD Gaze Session Ingestion Schema (v1.6)"

    with open(session_schema_path, "w", encoding="utf-8") as f:
        json.dump(schema, f, indent=2)
        f.write("\n")

    print(f"Exported session schema to {session_schema_path}")


if __name__ == "__main__":
    export_schemas()
