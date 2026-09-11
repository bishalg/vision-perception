#!/usr/bin/env python3
"""Local browser annotator for Heart/Head/Life/Fate polylines.

Usage (repo root):
  python3 tools/palm_annotator/server.py
  # open http://127.0.0.1:8765

Serves:
  - data/palm_labels/raw/*.jpg (+ sibling *_USER_CORRECTED.json)
  - data/samples/mobile_captures/*.jpg (same)

Saves reviewed labels as <stem>_USER_CORRECTED.json with reviewed:true.
"""

from __future__ import annotations

import json
import mimetypes
import re
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]
STATIC = Path(__file__).resolve().parent / "static"
PALM_LABELS = ROOT / "data" / "palm_labels"
MOBILE = ROOT / "data" / "samples" / "mobile_captures"
RAW = PALM_LABELS / "raw"
LINES = ("heart", "head", "life", "fate")
HOST = "127.0.0.1"
PORT = 8765


def stem_of_photo(name: str) -> str | None:
    low = name.lower()
    if not low.endswith((".jpg", ".jpeg", ".png")):
        return None
    if "overlay" in low or "canonical" in low or "USER_CORRECTED" in name:
        return None
    return Path(name).stem


def find_label(stem: str, dirs: list[Path]) -> Path | None:
    for d in dirs:
        hits = sorted(d.glob(f"{stem}_USER_CORRECTED*.json"))
        if hits:
            return hits[-1]
    return None


def find_evidence(stem: str, dirs: list[Path]) -> Path | None:
    for d in dirs:
        p = d / f"{stem}_evidence.json"
        if p.exists():
            return p
    return None


def find_image(stem: str) -> tuple[Path, str] | None:
    """Return (path, collection) for a stem."""
    for d, coll in ((RAW, "palm_labels"), (PALM_LABELS, "palm_labels"), (MOBILE, "mobile_captures")):
        for ext in (".jpg", ".jpeg", ".png"):
            p = d / f"{stem}{ext}"
            if p.exists():
                return p, coll
    return None


def list_items() -> list[dict]:
    items: dict[str, dict] = {}

    def add(stem: str, collection: str, image_rel: str, label_path: Path | None) -> None:
        reviewed = False
        source = None
        if label_path and label_path.exists():
            try:
                data = json.loads(label_path.read_text())
                source = data.get("source")
                # Legacy phone gold (no schema / no reviewed key) counts as reviewed.
                if "reviewed" in data:
                    reviewed = bool(data.get("reviewed"))
                else:
                    reviewed = True
            except Exception:
                reviewed = True
        items[stem] = {
            "stem": stem,
            "collection": collection,
            "image": image_rel,
            "has_label": bool(label_path and label_path.exists()),
            "reviewed": reviewed,
            "source": source,
            "has_evidence": find_evidence(stem, [PALM_LABELS, MOBILE, RAW]) is not None,
        }

    # Palm-label raw photos
    if RAW.exists():
        for p in sorted(RAW.iterdir()):
            stem = stem_of_photo(p.name)
            if not stem:
                continue
            add(stem, "palm_labels", f"data/palm_labels/raw/{p.name}", find_label(stem, [PALM_LABELS, RAW]))

    # Mobile captures
    if MOBILE.exists():
        for p in sorted(MOBILE.iterdir()):
            stem = stem_of_photo(p.name)
            if not stem or stem in items:
                continue
            add(stem, "mobile_captures", f"data/samples/mobile_captures/{p.name}",
                find_label(stem, [MOBILE]))

    # Orphans: label JSON without photo still listed if we can find any image
    for d in (PALM_LABELS, MOBILE):
        if not d.exists():
            continue
        for p in sorted(d.glob("*_USER_CORRECTED*.json")):
            m = re.match(r"(.+)_USER_CORRECTED", p.name)
            if not m:
                continue
            stem = m.group(1)
            if stem in items:
                continue
            found = find_image(stem)
            if not found:
                continue
            img_path, coll = found
            rel = str(img_path.relative_to(ROOT))
            add(stem, coll, rel, p)

    # Sort: unreviewed first, then by stem
    return sorted(items.values(), key=lambda x: (x["reviewed"], x["stem"]))


def empty_label(stem: str, collection: str) -> dict:
    return {
        "schema": "palm_label_v1",
        "space": "image_norm",
        "source": "annotator",
        "stem": stem,
        "collection": collection,
        "reviewed": False,
        "heart": [],
        "head": [],
        "life": [],
        "fate": [],
        "hand_mask": [],
    }


def load_item(stem: str) -> dict:
    found = find_image(stem)
    if not found:
        raise FileNotFoundError(stem)
    img_path, collection = found
    label_path = find_label(stem, [PALM_LABELS if collection == "palm_labels" else MOBILE, PALM_LABELS, MOBILE, RAW])
    if label_path and label_path.exists():
        label = json.loads(label_path.read_text())
    else:
        label = empty_label(stem, collection)

    # Normalize line keys to list-of-polys
    for name in LINES:
        val = label.get(name)
        if val is None:
            label[name] = []
        elif val and isinstance(val[0], list) and val[0] and isinstance(val[0][0], (int, float)):
            label[name] = [val]

    evidence_lines = {}
    ev_path = find_evidence(stem, [PALM_LABELS, MOBILE, RAW])
    if ev_path:
        try:
            ev = json.loads(ev_path.read_text())
            majors = (ev.get("domain_payload") or {}).get("major_lines") or {}
            for name in LINES:
                info = majors.get(f"{name}_line") or {}
                # Prefer image_norm for full-photo overlay; fall back to canonical.
                pts = info.get("points_image_norm") or info.get("points_canonical_norm") or []
                if info.get("present") and pts:
                    evidence_lines[name] = pts
        except Exception:
            pass

    return {
        "stem": stem,
        "collection": collection,
        "image_url": f"/file/{img_path.relative_to(ROOT).as_posix()}",
        "label": label,
        "evidence_lines": evidence_lines,
        "label_path": str((label_path or save_path_for(stem, collection)).relative_to(ROOT)),
    }


def save_path_for(stem: str, collection: str) -> Path:
    if collection == "mobile_captures":
        return MOBILE / f"{stem}_USER_CORRECTED.json"
    return PALM_LABELS / f"{stem}_USER_CORRECTED.json"


def save_label(stem: str, body: dict) -> dict:
    found = find_image(stem)
    if not found:
        raise FileNotFoundError(stem)
    _, collection = found
    out = {
        "schema": "palm_label_v1",
        "space": body.get("space") or "image_norm",
        "source": body.get("source") or "annotator",
        "reviewed": True,
        "heart": body.get("heart") or [],
        "head": body.get("head") or [],
        "life": body.get("life") or [],
        "fate": body.get("fate") or [],
        "hand_mask": body.get("hand_mask") or [],
    }
    # Validate structure lightly
    for name in LINES:
        polys = out[name]
        if not isinstance(polys, list):
            raise ValueError(f"{name} must be a list of polylines")
        cleaned = []
        for poly in polys:
            if not isinstance(poly, list):
                continue
            pts = [[float(p[0]), float(p[1])] for p in poly if isinstance(p, (list, tuple)) and len(p) >= 2]
            if len(pts) >= 2:
                cleaned.append(pts)
        out[name] = cleaned
    mask = out["hand_mask"]
    if isinstance(mask, list) and mask and isinstance(mask[0], (int, float)):
        # single polygon as flat list of pairs wrongly nested
        out["hand_mask"] = []
    elif isinstance(mask, list) and mask and isinstance(mask[0], list) and mask[0] and isinstance(mask[0][0], (int, float)):
        out["hand_mask"] = [mask]
    path = save_path_for(stem, collection)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, indent=2) + "\n")

    # Mirror reviewed flag into manifest if present
    man_path = PALM_LABELS / "manifest.json"
    if man_path.exists() and collection == "palm_labels":
        try:
            man = json.loads(man_path.read_text())
            for s in man.get("samples") or []:
                if s.get("stem") == stem:
                    s["reviewed"] = True
            man_path.write_text(json.dumps(man, indent=2) + "\n")
        except Exception:
            pass

    return {"ok": True, "path": str(path.relative_to(ROOT)), "label": out}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args) -> None:
        sys.stderr.write("[%s] %s\n" % (self.log_date_time_string(), fmt % args))

    def _cors(self) -> None:
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")

    def _json(self, code: int, obj: object) -> None:
        data = json.dumps(obj).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self._cors()
        self.end_headers()
        self.wfile.write(data)

    def _bytes(self, code: int, data: bytes, content_type: str) -> None:
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self._cors()
        self.end_headers()
        self.wfile.write(data)

    def do_OPTIONS(self) -> None:  # noqa: N802
        self.send_response(204)
        self._cors()
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        path = parsed.path

        if path in ("/", "/index.html"):
            html = (STATIC / "index.html").read_bytes()
            return self._bytes(200, html, "text/html; charset=utf-8")

        if path.startswith("/static/"):
            rel = path[len("/static/") :]
            f = (STATIC / rel).resolve()
            if not str(f).startswith(str(STATIC.resolve())) or not f.is_file():
                return self._json(404, {"error": "not found"})
            ctype = mimetypes.guess_type(str(f))[0] or "application/octet-stream"
            return self._bytes(200, f.read_bytes(), ctype)

        if path == "/api/items":
            return self._json(200, {"items": list_items()})

        if path.startswith("/api/item/"):
            stem = path[len("/api/item/") :]
            try:
                return self._json(200, load_item(stem))
            except FileNotFoundError:
                return self._json(404, {"error": f"no image for {stem}"})
            except Exception as e:
                return self._json(500, {"error": str(e)})

        if path.startswith("/file/"):
            rel = path[len("/file/") :]
            f = (ROOT / rel).resolve()
            if not str(f).startswith(str(ROOT.resolve())) or not f.is_file():
                return self._json(404, {"error": "not found"})
            # Only serve under data/
            if "data/" not in str(f.relative_to(ROOT)):
                return self._json(403, {"error": "forbidden"})
            ctype = mimetypes.guess_type(str(f))[0] or "application/octet-stream"
            return self._bytes(200, f.read_bytes(), ctype)

        return self._json(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        if not parsed.path.startswith("/api/save/"):
            return self._json(404, {"error": "not found"})
        stem = parsed.path[len("/api/save/") :]
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b"{}"
        try:
            body = json.loads(raw.decode("utf-8"))
            result = save_label(stem, body)
            return self._json(200, result)
        except FileNotFoundError:
            return self._json(404, {"error": f"no image for {stem}"})
        except Exception as e:
            return self._json(400, {"error": str(e)})


def main() -> None:
    STATIC.mkdir(parents=True, exist_ok=True)
    PALM_LABELS.mkdir(parents=True, exist_ok=True)
    RAW.mkdir(parents=True, exist_ok=True)
    addr = (HOST, PORT)
    httpd = ThreadingHTTPServer(addr, Handler)
    n = len(list_items())
    print(f"Palm annotator → http://{HOST}:{PORT}  ({n} images)")
    print("Collections: data/palm_labels/raw + data/samples/mobile_captures")
    print("Ctrl+C to stop")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nbye")
        httpd.server_close()


if __name__ == "__main__":
    main()
