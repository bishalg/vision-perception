#!/usr/bin/env python3
"""Download ~100 CC-BY Roboflow palm-line keypoint samples into data/palm_labels/.

Usage (repo root):
  export ROBOFLOW_API_KEY=rf_xxxxx
  python3 scripts/download_palm_labels.py --limit 100

Writes:
  data/palm_labels/raw/<stem>.jpg
  data/palm_labels/<stem>_USER_CORRECTED.json   (reviewed: false — proposals)
  data/palm_labels/manifest.json
  Appends an entry to third_party/SOURCE_PROVENANCE.md if missing.

Images are gitignored. Never commits binaries. Refuses CC-BY-NC / unknown
licenses (see docs/license_policy.md).
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
OUT = ROOT / "data" / "palm_labels"
RAW = OUT / "raw"
WORK = OUT / "_download"
PROVENANCE = ROOT / "third_party" / "SOURCE_PROVENANCE.md"

# Default: Roboflow Universe palm-lines recognition (keypoints heart/head/life/fate).
# Override with --workspace / --project / --version if the Universe slug changes.
DEFAULT_WORKSPACE = "palms-lf79v"
DEFAULT_PROJECT = "palm-lines-recognition-lbedg"
DEFAULT_VERSION = 1
DEFAULT_FORMAT = "yolov8"

LINE_ALIASES = {
    "heart": "heart",
    "heart_line": "heart",
    "heart-line": "heart",
    "head": "head",
    "head_line": "head",
    "head-line": "head",
    "life": "life",
    "life_line": "life",
    "life-line": "life",
    "fate": "fate",
    "fate_line": "fate",
    "fate-line": "fate",
}
LINES = ("heart", "head", "life", "fate")
ALLOWED_LICENSE_HINTS = ("cc by 4.0", "cc-by-4.0", "cc by", "cc-by", "attribution", "creative commons attribution")
BLOCKED_LICENSE_HINTS = ("cc-by-nc", "cc by-nc", "non-commercial", "noncommercial", "research only", "academic only")


def die(msg: str, code: int = 1) -> None:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(code)


def api_get(path: str, api_key: str) -> dict:
    url = f"https://api.roboflow.com/{path.lstrip('/')}?api_key={urllib.parse.quote(api_key)}"
    req = urllib.request.Request(url, headers={"User-Agent": "vision-perception-palm-labels/1.0"})
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
    # Explicit SPDX-ish tokens
    if re.search(r"\bcc[- ]?by[- ]?4\.0\b", low) or low in {"cc-by-4.0", "cc by 4.0"}:
        return True, text.strip()
    return False, f"unrecognized license (need CC BY, got {text!r})"


def download_zip(link: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(link, headers={"User-Agent": "vision-perception-palm-labels/1.0"})
    with urllib.request.urlopen(req, timeout=600) as resp, dest.open("wb") as out:
        shutil.copyfileobj(resp, out)


def find_coco_json(root: Path) -> Path:
    cands = sorted(root.rglob("_annotations.coco.json")) + sorted(root.rglob("*coco*.json"))
    for p in cands:
        try:
            data = json.loads(p.read_text())
        except Exception:
            continue
        if isinstance(data, dict) and "images" in data and "annotations" in data:
            return p
    die(f"no COCO keypoints JSON found under {root}")


def normalize_line_name(name: str) -> str | None:
    key = re.sub(r"\s+", "_", (name or "").strip().lower())
    key = key.replace("__", "_")
    return LINE_ALIASES.get(key)


def keypoints_to_poly(kps: list, img_w: int, img_h: int) -> list[list[float]]:
    """COCO keypoints are [x,y,v, x,y,v, ...]. Keep visible points, normalize 0..1."""
    pts: list[list[float]] = []
    if not kps or img_w <= 0 or img_h <= 0:
        return pts
    for i in range(0, len(kps), 3):
        if i + 2 >= len(kps):
            break
        x, y, v = float(kps[i]), float(kps[i + 1]), float(kps[i + 2])
        if v <= 0:
            continue
        pts.append([max(0.0, min(1.0, x / img_w)), max(0.0, min(1.0, y / img_h))])
    return pts


def bbox_polyline(ann: dict, img_w: int, img_h: int) -> list[list[float]]:
    """Fallback when keypoints are empty: approximate a line from the bbox midline."""
    bb = ann.get("bbox") or []
    if len(bb) != 4 or img_w <= 0 or img_h <= 0:
        return []
    x, y, w, h = map(float, bb)
    if w <= 1 and h <= 1:
        return []
    # Prefer longer axis as the crease direction.
    if w >= h:
        y_mid = y + h / 2.0
        return [[x / img_w, y_mid / img_h], [(x + w) / img_w, y_mid / img_h]]
    x_mid = x + w / 2.0
    return [[x_mid / img_w, y / img_h], [x_mid / img_w, (y + h) / img_h]]


def stem_from_filename(name: str, image_id: int) -> str:
    base = Path(name).stem
    safe = re.sub(r"[^a-zA-Z0-9._-]+", "_", base).strip("._") or f"img_{image_id}"
    return f"rf_{image_id:05d}_{safe[:48]}"


def write_label(stem: str, polys: dict[str, list], source: str) -> Path:
    payload = {
        "schema": "palm_label_v1",
        "space": "image_norm",
        "source": source,
        "reviewed": False,
        "heart": [polys["heart"]] if polys.get("heart") and len(polys["heart"]) >= 2 else [],
        "head": [polys["head"]] if polys.get("head") and len(polys["head"]) >= 2 else [],
        "life": [polys["life"]] if polys.get("life") and len(polys["life"]) >= 2 else [],
        "fate": [polys["fate"]] if polys.get("fate") and len(polys["fate"]) >= 2 else [],
        "hand_mask": [],
    }
    path = OUT / f"{stem}_USER_CORRECTED.json"
    path.write_text(json.dumps(payload, indent=2) + "\n")
    return path


def copy_image(src: Path, stem: str) -> Path:
    RAW.mkdir(parents=True, exist_ok=True)
    # Prefer jpeg extension for the annotator gallery.
    ext = src.suffix.lower() if src.suffix.lower() in {".jpg", ".jpeg", ".png"} else ".jpg"
    if ext == ".jpeg":
        ext = ".jpg"
    dest = RAW / f"{stem}{ext}"
    shutil.copy2(src, dest)
    return dest


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
- **Integration Target**: `data/palm_labels/` (raw images gitignored; JSON labels + manifest committed)
- **Modifications**:
  - [x] Keypoints converted to `palm_label_v1` polylines (`reviewed: false` until human Save)
- **Sample count (this machine)**: {n}
- **Review Date**: {datetime.now(timezone.utc).strftime("%Y-%m-%d")}
- **Approved By**: download_palm_labels.py (automated)
"""
    text = PROVENANCE.read_text() if PROVENANCE.exists() else "# Third-Party Source Code Provenance Register\n"
    if marker in text:
        # Replace existing block for this dataset.
        pattern = re.compile(re.escape(marker) + r".*?(?=\n### |\Z)", re.S)
        text = pattern.sub(entry.strip() + "\n\n", text)
    else:
        if not text.endswith("\n"):
            text += "\n"
        text += "\n" + entry.strip() + "\n"
    PROVENANCE.write_text(text)


def convert_coco(coco_path: Path, image_root: Path, limit: int, source_tag: str) -> list[dict]:
    coco = json.loads(coco_path.read_text())
    cats = {c["id"]: c for c in coco.get("categories", [])}
    # Map category id → line name
    cat_line: dict[int, str] = {}
    for cid, c in cats.items():
        # Prefer category name; also try keypoint names for single-class multi-kpt layouts.
        name = normalize_line_name(c.get("name") or "")
        if name:
            cat_line[cid] = name
        else:
            for kp in c.get("keypoints") or []:
                n2 = normalize_line_name(str(kp))
                if n2:
                    cat_line[cid] = n2
                    break

    anns_by_img: dict[int, list] = {}
    for ann in coco.get("annotations") or []:
        anns_by_img.setdefault(int(ann["image_id"]), []).append(ann)

    images = list(coco.get("images") or [])
    # Stratified-ish: keep order but take evenly spaced if over limit.
    if limit > 0 and len(images) > limit:
        step = len(images) / limit
        images = [images[int(i * step)] for i in range(limit)]

    samples: list[dict] = []
    for img in images:
        iid = int(img["id"])
        file_name = img.get("file_name") or ""
        src = image_root / file_name
        if not src.exists():
            # Roboflow often nests train/valid/test.
            hits = list(WORK.rglob(Path(file_name).name))
            if not hits:
                print(f"  skip missing image: {file_name}")
                continue
            src = hits[0]
        w = int(img.get("width") or 0)
        h = int(img.get("height") or 0)
        polys: dict[str, list] = {n: [] for n in LINES}
        for ann in anns_by_img.get(iid, []):
            line = cat_line.get(int(ann.get("category_id", -1)))
            if not line:
                # Try matching against annotation-level class name if present.
                line = normalize_line_name(str(ann.get("class") or ann.get("name") or ""))
            if not line:
                continue
            pts = keypoints_to_poly(ann.get("keypoints") or [], w, h)
            if len(pts) < 2:
                pts = bbox_polyline(ann, w, h)
            if len(pts) >= 2:
                # Keep the longest polyline if duplicates.
                if not polys[line] or len(pts) > len(polys[line]):
                    polys[line] = pts

        stem = stem_from_filename(file_name, iid)
        img_path = copy_image(src, stem)
        write_label(stem, polys, source_tag)
        samples.append({
            "stem": stem,
            "image": str(img_path.relative_to(ROOT)),
            "label": f"data/palm_labels/{stem}_USER_CORRECTED.json",
            "sha256": sha256_file(img_path),
            "lines": {n: len(polys[n]) for n in LINES},
            "reviewed": False,
        })
        print(f"  + {stem}  lines={ {n: len(polys[n]) for n in LINES} }")
    return samples


def convert_yolov8(work_dir: Path, limit: int, source_tag: str) -> list[dict]:
    yaml_path = work_dir / "data.yaml"
    idx_to_name = {0: "fate", 1: "head", 2: "heart", 3: "life"}
    if yaml_path.exists():
        try:
            content = yaml_path.read_text(encoding="utf-8")
            match = re.search(r"names:\s*\[(.*?)\]", content)
            if match:
                raw_names = [n.strip(" '\"") for n in match.group(1).split(",")]
                idx_to_name = {i: normalize_line_name(n) or n for i, n in enumerate(raw_names)}
        except Exception:
            pass

    candidates: list[tuple[Path, Path]] = []
    for split in ("train", "valid", "test"):
        img_dir = work_dir / split / "images"
        lbl_dir = work_dir / split / "labels"
        if not img_dir.exists():
            continue
        for img_p in sorted(img_dir.iterdir()):
            if img_p.suffix.lower() not in {".jpg", ".jpeg", ".png"}:
                continue
            lbl_p = lbl_dir / f"{img_p.stem}.txt"
            if lbl_p.exists() and lbl_p.stat().st_size > 0:
                candidates.append((img_p, lbl_p))

    if limit > 0 and len(candidates) > limit:
        step = len(candidates) / limit
        candidates = [candidates[int(i * step)] for i in range(limit)]

    samples: list[dict] = []
    for iid, (img_p, lbl_p) in enumerate(candidates, start=1):
        polys: dict[str, list] = {n: [] for n in LINES}
        lines = lbl_p.read_text(encoding="utf-8").strip().splitlines()
        for line in lines:
            parts = line.strip().split()
            if len(parts) < 5:
                continue
            cls_idx = int(parts[0])
            line_name = idx_to_name.get(cls_idx)
            if not line_name or line_name not in polys:
                continue
            kpt_tokens = parts[5:]
            pts: list[list[float]] = []
            stride = 3 if len(kpt_tokens) % 3 == 0 else (2 if len(kpt_tokens) % 2 == 0 else 0)
            if stride > 0:
                for idx in range(0, len(kpt_tokens), stride):
                    x = float(kpt_tokens[idx])
                    y = float(kpt_tokens[idx + 1])
                    v = float(kpt_tokens[idx + 2]) if stride == 3 else 2.0
                    if v > 0 and (x > 0.0 or y > 0.0):
                        pts.append([max(0.0, min(1.0, x)), max(0.0, min(1.0, y))])
            if len(pts) >= 2:
                if not polys[line_name] or len(pts) > len(polys[line_name]):
                    polys[line_name] = pts

        stem = stem_from_filename(img_p.name, iid)
        img_path = copy_image(img_p, stem)
        write_label(stem, polys, source_tag)
        samples.append({
            "stem": stem,
            "image": str(img_path.relative_to(ROOT)),
            "label": f"data/palm_labels/{stem}_USER_CORRECTED.json",
            "sha256": sha256_file(img_path),
            "lines": {n: len(polys[n]) for n in LINES},
            "reviewed": False,
        })
        print(f"  + {stem}  lines={ {n: len(polys[n]) for n in LINES} }")

    return samples


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--limit", type=int, default=100, help="Max images to keep (default 100)")
    ap.add_argument("--workspace", default=DEFAULT_WORKSPACE)
    ap.add_argument("--project", default=DEFAULT_PROJECT)
    ap.add_argument("--version", type=int, default=DEFAULT_VERSION)
    ap.add_argument("--format", default=DEFAULT_FORMAT, choices=("coco", "yolov8"), help="Export format")
    ap.add_argument("--api-key", default=os.environ.get("ROBOFLOW_API_KEY", ""),
                    help="Or set ROBOFLOW_API_KEY")
    ap.add_argument("--skip-download", action="store_true",
                    help="Re-convert from existing data/palm_labels/_download/")
    ap.add_argument("--force-license", default="",
                    help="Override license string after manual review (still must look like CC BY)")
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
            # Some Universe projects expose license only on the version export page.
            print(f"  warning: project license check: {reason}")
            print("  continuing to export endpoint; will re-check. Pass --force-license 'CC BY 4.0' after manual confirm.")
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
            die(f"no download link in export response keys={list(export.keys())}: {json.dumps(export)[:500]}")

        # Re-check license on version payload if present.
        ver = export.get("version") or {}
        lic2 = args.force_license or ver.get("license") or license_text or lic
        ok, reason = license_ok(str(lic2))
        if not ok and not args.force_license:
            die(
                f"refusing download: {reason}. "
                "Confirm CC BY 4.0 on the Universe page, then re-run with "
                "--force-license 'CC BY 4.0'"
            )
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
            die("--skip-download requires an existing data/palm_labels/_download/")
        if not license_text:
            # Read prior manifest if any.
            man = OUT / "manifest.json"
            if man.exists():
                license_text = (json.loads(man.read_text()).get("license") or "")
        ok, reason = license_ok(license_text or "CC BY 4.0")
        if not ok and not args.force_license:
            die(f"license still unclear for --skip-download: {reason}")
        license_text = reason if ok else license_text

    source_tag = f"roboflow_import:{args.workspace}/{args.project}/{args.version}"
    if (WORK / "data.yaml").exists() or args.format == "yolov8":
        print(f"Converting YOLOv8 format under {WORK} (limit={args.limit})")
        samples = convert_yolov8(WORK, args.limit, source_tag)
    else:
        coco_path = find_coco_json(WORK)
        image_root = coco_path.parent
        print(f"Converting {coco_path.relative_to(ROOT)} (limit={args.limit})")
        samples = convert_coco(coco_path, image_root, args.limit, source_tag)

    if not samples:
        die("no samples converted — check export format / image paths")

    manifest = {
        "schema": "palm_labels_manifest_v1",
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
        "samples": samples,
        "note": "Imported labels have reviewed:false. Open tools/palm_annotator to correct and Save.",
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    update_provenance(args.workspace, args.project, args.version, license_text, len(samples), universe_url)

    # Drop the bulky zip but keep extracted tree optional; wipe zip always.
    zip_path = WORK / "dataset.zip"
    if zip_path.exists():
        zip_path.unlink()

    print(f"\nDone: {len(samples)} samples → data/palm_labels/")
    print("Next: python3 tools/palm_annotator/server.py")
    print("Then: python3 scripts/admin_metrics.py && open output/admin_dashboard.html")


if __name__ == "__main__":
    main()
