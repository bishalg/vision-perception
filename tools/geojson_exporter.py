#!/usr/bin/env python3
"""
geojson_exporter.py — Phase 4 sidecar JSON → GeoJSON FeatureCollection
--------------------------------------------------------------------
Reads CCTV / flood fixture sidecar JSON (lat/lon/heading/fov) and emits an
OGC GeoJSON FeatureCollection for the CesiumJS digital-twin viewer.
Optionally copies linked PNG fixtures into the viewer public/media tree.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path


def discover_sidecars(roots: list[Path]) -> list[Path]:
    found: list[Path] = []
    for root in roots:
        if not root.exists():
            continue
        found.extend(sorted(root.rglob("*.json")))
    # Skip generated geojson and video metadata that already is not a camera sidecar
    return [p for p in found if p.name != "cameras.geojson"]


def sidecar_to_feature(path: Path, repo_root: Path) -> dict | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None

    loc = data.get("location") or {}
    pose = data.get("pose") or {}
    lat = loc.get("lat")
    lon = loc.get("lon")
    if lat is None or lon is None:
        return None

    image_path = path.with_suffix(".png")
    if not image_path.exists():
        image_path = path.with_suffix(".avi")  # video clip: no still; skip image
        still = None
    else:
        still = image_path

    camera_id = path.stem
    props = {
        "id": camera_id,
        "name": data.get("name", camera_id),
        "agency": data.get("agency", ""),
        "elevation_m": float(loc.get("elevation_m", 0.0)),
        "heading_deg": float(pose.get("heading_deg", 0.0)),
        "pitch_deg": float(pose.get("pitch_deg", -15.0)),
        "fov_deg": float(pose.get("fov_deg", 60.0)),
        "url": data.get("url", ""),
        "sidecar": str(path.relative_to(repo_root)),
        "image": None,
    }
    if still is not None and still.suffix.lower() == ".png":
        props["image"] = str(still.relative_to(repo_root))

    return {
        "type": "Feature",
        "geometry": {
            "type": "Point",
            "coordinates": [float(lon), float(lat), float(loc.get("elevation_m", 0.0))],
        },
        "properties": props,
    }


def copy_media(features: list[dict], repo_root: Path, media_dir: Path) -> None:
    media_dir.mkdir(parents=True, exist_ok=True)
    for feat in features:
        rel = feat["properties"].get("image")
        if not rel:
            continue
        src = repo_root / rel
        if not src.exists():
            continue
        dest = media_dir / Path(rel).name
        shutil.copy2(src, dest)
        feat["properties"]["image"] = f"media/{dest.name}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("apps/cesium_viewer/public/cameras.geojson"),
        help="Output GeoJSON path",
    )
    parser.add_argument(
        "--media-dir",
        type=Path,
        default=Path("apps/cesium_viewer/public/media"),
        help="Directory to copy PNG stills into",
    )
    parser.add_argument(
        "--no-copy-media",
        action="store_true",
        help="Do not copy fixture PNGs into the viewer public tree",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    roots = [
        repo_root / "data/fixtures/cctv",
        repo_root / "data/fixtures/cctv_flood",
        repo_root / "data/fixtures/cctv_video",
    ]
    features: list[dict] = []
    for path in discover_sidecars(roots):
        feat = sidecar_to_feature(path, repo_root)
        if feat is not None:
            features.append(feat)

    if not args.no_copy_media:
        copy_media(features, repo_root, args.media_dir if args.media_dir.is_absolute() else repo_root / args.media_dir)

    collection = {
        "type": "FeatureCollection",
        "features": features,
        "properties": {
            "generator": "tools/geojson_exporter.py",
            "phase": 4,
            "count": len(features),
        },
    }

    out_path = args.output if args.output.is_absolute() else repo_root / args.output
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(collection, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {out_path} with {len(features)} cameras")
    return 0


if __name__ == "__main__":
    sys.exit(main())
