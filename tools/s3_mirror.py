#!/usr/bin/env python3
"""s3_mirror.py — validated flood-evidence publisher (Phase 1 real tool).

Validates flood_evidence_v1.json against the repo schema contract, then
publishes it to S3 (real boto3 upload) and/or a local mirror directory for
offline / CI consumption by Bedrock/Nemotron agents and the Cesium twin.

No silent fakes: validation failures abort before any upload; missing boto3
with an S3 target is a hard error, not a simulated success.
"""

from __future__ import annotations

import argparse
import datetime
import json
import re
import shutil
import sys
from pathlib import Path

SCHEMA_VERSION = "1.0.0"
ALERT_LEVELS = ("CRITICAL", "WARNING", "NORMAL")
_SHA_RE = re.compile(r"^[a-f0-9]{64}$")

REQUIRED_TOP = (
    "schema_version",
    "event_id",
    "timestamp_iso",
    "payload_sha256",
    "frame_dimensions",
    "quality",
    "spatial_features",
    "hydrology_metrics",
    "controls",
    "alert_recommendation",
)


def validate_flood_evidence(doc: dict) -> list[str]:
    """Returns a list of schema violations (empty = valid). Pure function."""
    errors: list[str] = []
    if not isinstance(doc, dict):
        return ["document must be a JSON object"]

    for key in REQUIRED_TOP:
        if key not in doc:
            errors.append(f"missing required key: {key}")

    if doc.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION!r}")

    if not isinstance(doc.get("event_id"), str) or not doc["event_id"]:
        errors.append("event_id must be a non-empty string")

    ts = doc.get("timestamp_iso")
    try:
        datetime.datetime.fromisoformat(str(ts).replace("Z", "+00:00"))
    except (ValueError, TypeError, AttributeError):
        errors.append(f"timestamp_iso is not ISO-8601: {ts!r}")

    if not isinstance(doc.get("payload_sha256"), str) or not _SHA_RE.match(doc["payload_sha256"]):
        errors.append("payload_sha256 must be 64 lowercase hex chars")

    dims = doc.get("frame_dimensions", {})
    for axis in ("width", "height"):
        if not isinstance(dims.get(axis), int) or dims[axis] < 1:
            errors.append(f"frame_dimensions.{axis} must be a positive int")

    quality = doc.get("quality", {})
    if not isinstance(quality.get("is_usable"), bool):
        errors.append("quality.is_usable must be bool")
    if not isinstance(quality.get("blur_score"), (int, float)):
        errors.append("quality.blur_score must be numeric")

    features = doc.get("spatial_features")
    if not isinstance(features, list):
        errors.append("spatial_features must be an array")
    else:
        for i, feat in enumerate(features):
            if not isinstance(feat, dict):
                errors.append(f"spatial_features[{i}] must be an object")
                continue
            if feat.get("feature_class") not in (
                "river_channel",
                "tributary",
                "minor_feature",
                "waterline",
                "flood_polygon",
            ):
                errors.append(f"spatial_features[{i}].feature_class is unknown")
            pts = feat.get("points")
            if not isinstance(pts, list) or not pts:
                errors.append(f"spatial_features[{i}].points must be a non-empty array")
            if feat.get("point_count") != (len(pts) if isinstance(pts, list) else -1):
                errors.append(f"spatial_features[{i}].point_count mismatches points length")

    hydro = doc.get("hydrology_metrics", {})
    for key in ("total_features", "river_channel_count", "tributary_count", "total_feature_length_px"):
        if key not in hydro:
            errors.append(f"hydrology_metrics missing: {key}")

    controls = doc.get("controls", {})
    for key in ("spatial_control_pass", "temporal_control_pass"):
        if not isinstance(controls.get(key), bool):
            errors.append(f"controls.{key} must be bool")

    if doc.get("alert_recommendation") not in ALERT_LEVELS:
        errors.append(f"alert_recommendation must be one of {ALERT_LEVELS}")

    return errors


def load_and_validate(path: Path) -> dict:
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"[-] Cannot read evidence JSON {path}: {exc}")
    errors = validate_flood_evidence(doc)
    if errors:
        raise SystemExit(f"[-] Schema validation failed for {path}:\n  - " + "\n  - ".join(errors))
    return doc


def mirror_to_local(src: Path, dest_dir: Path) -> Path:
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / src.name
    shutil.copy2(src, dest)
    print(f"[✓] Mirrored {src} -> {dest}")
    return dest


def upload_to_s3(src: Path, bucket: str, key: str) -> str:
    try:
        import boto3
    except ImportError:
        raise SystemExit(
            "[-] boto3 is not installed; cannot upload to S3. "
            "Install boto3 or use --local-mirror for offline publishing."
        )
    client = boto3.client("s3")
    client.upload_file(str(src), bucket, key)
    uri = f"s3://{bucket}/{key}"
    print(f"[✓] Uploaded {src} -> {uri}")
    return uri


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path, help="flood_evidence_v1.json path")
    parser.add_argument("--bucket", default="", help="S3 bucket for upload")
    parser.add_argument("--key", default="", help="S3 key (defaults to events/<event_id>/flood_delta.json)")
    parser.add_argument("--local-mirror", default=None, type=Path, help="Local directory mirror target")
    parser.add_argument("--validate-only", action="store_true", help="Validate schema and exit")
    args = parser.parse_args()

    if not args.evidence.exists():
        print(f"[-] Evidence file not found: {args.evidence}", file=sys.stderr)
        return 1

    doc = load_and_validate(args.evidence)
    print(f"[✓] Schema valid: {args.evidence} (event {doc['event_id']}, {doc['alert_recommendation']})")

    if args.validate_only:
        return 0
    if not args.bucket and not args.local_mirror:
        print("[-] Nothing to do: pass --bucket and/or --local-mirror (or --validate-only).", file=sys.stderr)
        return 1

    if args.local_mirror:
        mirror_to_local(args.evidence, Path(args.local_mirror))
    if args.bucket:
        key = args.key or f"events/{doc['event_id']}/flood_delta.json"
        upload_to_s3(args.evidence, args.bucket, key)
    return 0


if __name__ == "__main__":
    sys.exit(main())
