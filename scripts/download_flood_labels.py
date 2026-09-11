#!/usr/bin/env python3
"""Download CC-BY Roboflow river flood segmentation dataset into data/flood_labels/.

Usage (repo root):
  export ROBOFLOW_API_KEY=rf_xxxxx
  python3 scripts/download_flood_labels.py --limit 50

Writes:
  data/flood_labels/raw/<stem>.jpg
  data/flood_labels/<stem>_USER_CORRECTED.json   (reviewed: false)
  data/flood_labels/manifest.json
  Appends an entry to third_party/SOURCE_PROVENANCE.md if missing.

Images are gitignored. Never commits binaries.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "data" / "flood_labels"
RAW = OUT / "raw"
WORK = OUT / "_download"
PROVENANCE = ROOT / "third_party" / "SOURCE_PROVENANCE.md"

DEFAULT_WORKSPACE = "unitec-gr8dy"
DEFAULT_PROJECT = "river-flood-detection"
DEFAULT_VERSION = 6
DEFAULT_FORMAT = "coco-segmentation"

ALLOWED_LICENSE_HINTS = (
    "cc by 4.0", "cc-by-4.0", "cc by", "cc-by", "attribution", "creative commons attribution"
)
BLOCKED_LICENSE_HINTS = (
    "cc-by-nc", "cc by-nc", "non-commercial", "noncommercial", "research only", "academic only"
)


def die(msg: str, code: int = 1) -> None:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(code)


def api_get(path: str, api_key: str) -> dict:
    url = f"https://api.roboflow.com/{path.lstrip('/')}?api_key={urllib.parse.quote(api_key)}"
    req = urllib.request.Request(url, headers={"User-Agent": "vision-perception-flood-labels/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace")
        die(f"Roboflow API {e.code} for {path}: {body[:400]}")
    except urllib.error.URLError as e:
        die(f"network error talking to Roboflow: {e}")


def license_ok(text: str) -> tuple[bool, str]:
    low = (text or "").lower().strip()
    if not low:
        return False, "missing license metadata"
    for bad in BLOCKED_LICENSE_HINTS:
        if bad in low:
            return False, f"blocked license ({bad})"
    for ok in ALLOWED_LICENSE_HINTS:
        if ok in low:
            return True, text.strip()
    if re.search(r"\bcc[- ]?by[- ]?4\.0\b", low) or low in {"cc-by-4.0", "cc by 4.0"}:
        return True, text.strip()
    return False, f"unrecognized license (need CC BY, got {text!r})"


def download_zip(link: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(link, headers={"User-Agent": "vision-perception-flood-labels/1.0"})
    with urllib.request.urlopen(req, timeout=600) as resp, dest.open("wb") as out:
        shutil.copyfileobj(resp, out)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def update_provenance(workspace: str, project: str, version: int, license_text: str, n: int, url: str) -> None:
    marker = f"### Dataset: Roboflow {workspace}/{project}"
    entry = f"""
{marker}
- **Upstream Repository**: {url}
- **Commit SHA / Release Tag**: version {version}
- **Original Author / Copyright**: Roboflow Universe project `{workspace}/{project}`
- **License**: {license_text} (CC BY required; verified by download script)
- **Integration Target**: `data/flood_labels/` (raw images gitignored; JSON labels + manifest committed)
- **Modifications**:
  - [x] COCO segmentation polygons converted to `curv_v1` polylines (`reviewed: false` until human Save)
- **Sample count (this machine)**: {n}
- **Review Date**: {datetime.now(timezone.utc).strftime("%Y-%m-%d")}
- **Approved By**: download_flood_labels.py (automated)
"""
    text = PROVENANCE.read_text() if PROVENANCE.exists() else "# Third-Party Source Code Provenance Register\n"
    if marker in text:
        pattern = re.compile(re.escape(marker) + r".*?(?=\n### |\Z)", re.S)
        text = pattern.sub(entry.strip() + "\n\n", text)
    else:
        if not text.endswith("\n"):
            text += "\n"
        text += "\n" + entry.strip() + "\n"
    PROVENANCE.write_text(text)


def write_label(stem: str, lines: dict[str, list], source: str) -> Path:
    payload = {
        "schema": "curv_v1",
        "domain": "flood",
        "source": source,
        "reviewed": False,
        "waterline": lines.get("waterline", []),
        "flood_boundary": lines.get("flood_boundary", []),
        "river_centerline": lines.get("river_centerline", []),
        "bank_line": lines.get("bank_line", []),
    }
    path = OUT / f"{stem}_USER_CORRECTED.json"
    path.write_text(json.dumps(payload, indent=2) + "\n")
    return path


def convert_coco_segmentation(work_dir: Path, limit: int, source_tag: str) -> list[dict]:
    coco_files = sorted(work_dir.rglob("_annotations.coco.json"))
    if not coco_files:
        coco_files = sorted(work_dir.rglob("*coco*.json"))
    if not coco_files:
        die(f"no COCO segmentation JSON files found under {work_dir}")

    RAW.mkdir(parents=True, exist_ok=True)
    samples: list[dict] = []

    for cfile in coco_files:
        split_dir = cfile.parent
        split_name = split_dir.name.lower()
        if split_name not in {"train", "valid", "test"}:
            split_name = "train"

        try:
            data = json.loads(cfile.read_text(encoding="utf-8"))
        except Exception as e:
            print(f"  warning: failed to read {cfile}: {e}")
            continue

        cats = {c["id"]: c.get("name", "").lower() for c in data.get("categories", [])}
        anns_by_img: dict[int, list] = {}
        for ann in data.get("annotations", []):
            anns_by_img.setdefault(int(ann.get("image_id", -1)), []).append(ann)

        images = list(data.get("images", []))
        for img in images:
            if limit > 0 and len(samples) >= limit:
                break

            iid = int(img["id"])
            file_name = img.get("file_name") or ""
            img_src = split_dir / file_name
            if not img_src.exists():
                hits = list(work_dir.rglob(Path(file_name).name))
                if hits:
                    img_src = hits[0]
                else:
                    continue

            w = float(img.get("width") or 1)
            h = float(img.get("height") or 1)

            parsed_lines: dict[str, list] = {
                "waterline": [],
                "flood_boundary": [],
                "river_centerline": [],
                "bank_line": [],
            }

            for ann in anns_by_img.get(iid, []):
                cat_name = cats.get(int(ann.get("category_id", -1)), "")
                seg = ann.get("segmentation") or []
                for poly in seg:
                    if not isinstance(poly, list) or len(poly) < 4:
                        continue
                    norm_pts = []
                    for k in range(0, len(poly) - 1, 2):
                        norm_pts.append([
                            round(max(0.0, min(1.0, poly[k] / w)), 5),
                            round(max(0.0, min(1.0, poly[k + 1] / h)), 5),
                        ])
                    if len(norm_pts) >= 3:
                        if "increase" in cat_name or "flood" in cat_name:
                            parsed_lines["flood_boundary"].append(norm_pts)
                        else:
                            parsed_lines["waterline"].append(norm_pts)

            base_stem = re.sub(r"[^a-zA-Z0-9_-]+", "_", Path(file_name).stem)[:42].strip("_")
            stem = f"rf_flood_{len(samples) + 1:04d}_{base_stem}"
            dest_img = RAW / f"{stem}.jpg"
            shutil.copy2(img_src, dest_img)

            write_label(stem, parsed_lines, source_tag)
            samples.append({
                "stem": stem,
                "image": str(dest_img.relative_to(ROOT)),
                "label": f"data/flood_labels/{stem}_USER_CORRECTED.json",
                "sha256": sha256_file(dest_img),
                "split": split_name,
                "lines": {k: len(v) for k, v in parsed_lines.items()},
                "reviewed": False,
            })
            print(f"  + {stem} [{split_name}] waterlines={len(parsed_lines['waterline'])} flood_polys={len(parsed_lines['flood_boundary'])}")

    return samples


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--limit", type=int, default=50, help="Max images to keep (default 50)")
    ap.add_argument("--workspace", default=DEFAULT_WORKSPACE)
    ap.add_argument("--project", default=DEFAULT_PROJECT)
    ap.add_argument("--version", type=int, default=DEFAULT_VERSION)
    ap.add_argument("--format", default=DEFAULT_FORMAT, help="Export format")
    ap.add_argument("--api-key", default=os.environ.get("ROBOFLOW_API_KEY", ""),
                    help="Or set ROBOFLOW_API_KEY")
    ap.add_argument("--skip-download", action="store_true",
                    help="Re-convert from existing data/flood_labels/_download/")
    ap.add_argument("--force-license", default="",
                    help="Override license string after manual confirm")
    args = ap.parse_args()

    if not args.api_key and not args.skip_download:
        die("set ROBOFLOW_API_KEY or pass --api-key (get one at https://app.roboflow.com/settings/api)")

    OUT.mkdir(parents=True, exist_ok=True)
    RAW.mkdir(parents=True, exist_ok=True)

    universe_url = f"https://universe.roboflow.com/{args.workspace}/{args.project}"
    license_text = args.force_license.strip()

    if not args.skip_download:
        print(f"Fetching project metadata: {args.workspace}/{args.project} v{args.version}")
        meta = api_get(f"{args.workspace}/{args.project}", args.api_key)
        project = meta.get("project") or meta
        lic = (
            args.force_license
            or project.get("license")
            or project.get("license_name")
            or (project.get("project") or {}).get("license")
            or meta.get("license")
            or ""
        )
        ok, reason = license_ok(str(lic))
        if not ok:
            print(f"  warning: project license check: {reason}")
        else:
            license_text = reason
            print(f"  license ok: {license_text}")

        export_path = f"{args.workspace}/{args.project}/{args.version}/{args.format}"
        print(f"Requesting export: {export_path}")
        export = api_get(export_path, args.api_key)
        link = ((export.get("export") or {}).get("link")
                or export.get("link")
                or (export.get("version") or {}).get("export", {}).get("link"))
        if not link:
            die(f"no download link in export response: {json.dumps(export)[:400]}")

        ver = export.get("version") or {}
        lic2 = args.force_license or ver.get("license") or license_text or lic
        ok, reason = license_ok(str(lic2))
        if not ok and not args.force_license:
            die(f"refusing download: {reason}. Confirm CC BY 4.0 on Universe, then re-run with --force-license 'CC BY 4.0'")
        license_text = reason if ok else str(lic2)

        if WORK.exists():
            shutil.rmtree(WORK)
        WORK.mkdir(parents=True)
        zip_path = WORK / "dataset.zip"
        print(f"Downloading zip → {zip_path}")
        download_zip(link, zip_path)
        print("Extracting…")
        with zipfile.ZipFile(zip_path) as zf:
            zf.extractall(WORK)
    else:
        if not WORK.exists():
            die("--skip-download requires an existing data/flood_labels/_download/")
        license_text = "CC BY 4.0"

    source_tag = f"roboflow_import:{args.workspace}/{args.project}/{args.version}"
    print(f"Converting COCO segmentation annotations (limit={args.limit})")
    samples = convert_coco_segmentation(WORK, args.limit, source_tag)

    if not samples:
        die("no samples converted — check dataset files")

    manifest = {
        "schema": "flood_labels_manifest_v1",
        "status": "ready",
        "source": {
            "workspace": args.workspace,
            "project": args.project,
            "version": args.version,
            "format": args.format,
            "url": universe_url,
            "tag": source_tag,
        },
        "license": license_text,
        "downloaded_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "limit": args.limit,
        "count": len(samples),
        "splits": {
            "train": sum(1 for s in samples if s.get("split") == "train"),
            "valid": sum(1 for s in samples if s.get("split") == "valid"),
            "test": sum(1 for s in samples if s.get("split") == "test"),
        },
        "samples": samples,
        "note": "Imported labels have reviewed:false. Open CurvStudio to review and correct.",
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    update_provenance(args.workspace, args.project, args.version, license_text, len(samples), universe_url)

    zip_path = WORK / "dataset.zip"
    if zip_path.exists():
        zip_path.unlink()

    print(f"\nDone: {len(samples)} samples → data/flood_labels/")
    print("Next: pnpm studio to inspect and annotate!")


if __name__ == "__main__":
    main()
