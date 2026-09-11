#!/usr/bin/env python3
"""scripts/studio_server.py - Zero-dependency Local API & Web Studio Server.

Serves CurvStudio (apps/annotation_studio/index.html) and provides a REST API
for domain switching, fixture exploration, 1-click ground truth saving, and
metric recalculation.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import webbrowser
from http.server import HTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "configs" / "domain_registry.json"


def load_registry() -> dict:
    if REGISTRY_PATH.exists():
        try:
            return json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
        except Exception as e:
            sys.stderr.write(f"[WARN] Error loading domain_registry.json: {e}\n")
    return {"domains": []}


def dist(a: list, b: list) -> float:
    return math.hypot(a[0] - b[0], a[1] - b[1])


def poly_arc(poly: list) -> float:
    return sum(dist(poly[i - 1], poly[i]) for i in range(1, len(poly)))


def compare_polylines(auto_pts: list, human_polys: list, dim: float = 512.0) -> dict:
    auto_present = bool(auto_pts and len(auto_pts) >= 2)
    human_present = any(len(p) >= 2 for p in human_polys)
    tp = int(auto_present and human_present)
    fp = int(auto_present and not human_present)
    fn = int(not auto_present and human_present)
    tn = int(not auto_present and not human_present)
    err_px = None
    len_ratio = None

    if auto_present and human_present:
        hpts = [pt for p in human_polys for pt in p]
        if auto_pts and hpts:
            err_px = sum(min(dist(a, h) for h in hpts) for a in auto_pts) / len(auto_pts) * dim
        auto_arc = poly_arc(auto_pts) * dim
        human_arc = sum(poly_arc(p) for p in human_polys) * dim
        if auto_arc > 0 and human_arc > 0:
            len_ratio = auto_arc / human_arc

    return {
        "auto_present": auto_present,
        "human_present": human_present,
        "tp": tp,
        "fp": fp,
        "fn": fn,
        "tn": tn,
        "mean_err_px": err_px,
        "len_ratio": len_ratio,
    }


def parse_human_polys(human_data: dict, class_id: str) -> list:
    val = human_data.get(class_id)
    if val is None and class_id.endswith("_line"):
        val = human_data.get(class_id.replace("_line", ""))
    if isinstance(val, list) and val:
        if isinstance(val[0], list) and val[0] and isinstance(val[0][0], (int, float)):
            return [val]
        return [p for p in val if isinstance(p, list) and len(p) >= 2]
    return []


def get_samples_for_domain(domain_cfg: dict) -> list[dict]:
    target_dirs = []
    primary = ROOT / domain_cfg.get("dataset_dir", "")
    if primary.exists():
        target_dirs.append(primary)
    for extra in domain_cfg.get("extra_dataset_dirs", []):
        p = ROOT / extra
        if p.exists() and p not in target_dirs:
            target_dirs.append(p)

    if not target_dirs:
        return []

    evidence_dir = ROOT / domain_cfg.get("evidence_dir", domain_cfg.get("dataset_dir", ""))
    extensions = tuple(domain_cfg.get("image_extensions", [".jpg", ".png"]))
    excludes = tuple(domain_cfg.get("image_filter_exclude", []))
    classes = domain_cfg.get("classes", [])
    classes_by_id = {c["id"]: c for c in classes}

    seen_stems = set()
    samples = []
    for cur_dir in target_dirs:
        for img_path in sorted(cur_dir.iterdir()):
            if not img_path.is_file():
                continue
            if not any(img_path.name.lower().endswith(ext) for ext in extensions):
                continue
            if any(exc in img_path.name for exc in excludes):
                continue

            stem = img_path.stem
            if stem in seen_stems:
                continue
            seen_stems.add(stem)
            rel_img = str(img_path.relative_to(ROOT))

            # Check for evidence file
            evidence_data = None
            evidence_file = None
            for cand in [
                evidence_dir / f"{stem}_evidence.json",
                evidence_dir / f"{stem.replace('_bgr', '')}_evidence.json",
                cur_dir / f"{stem}_evidence.json",
                ROOT / "output" / f"palm_{stem}.json",
            ]:
                if cand.exists():
                    evidence_file = str(cand.relative_to(ROOT))
                    try:
                        evidence_data = json.loads(cand.read_text(encoding="utf-8"))
                    except Exception:
                        pass
                    break

            # Check for human ground truth
            truth_data = None
            truth_file = None
            for cand in [
                cur_dir / f"{stem}_USER_CORRECTED.json",
                cur_dir.parent / f"{stem}_USER_CORRECTED.json",
                cur_dir / f"{stem}_USER_CORRECTED_v2.json",
                evidence_dir / f"{stem}_USER_CORRECTED.json",
            ]:
                if cand.exists():
                    truth_file = str(cand.relative_to(ROOT))
                    try:
                        truth_data = json.loads(cand.read_text(encoding="utf-8"))
                    except Exception:
                        pass
                    break

            # Extract auto curves per class
            auto_lines = {}
            if evidence_data:
                payload = evidence_data.get("domain_payload") or {}
                major = payload.get("major_lines") or {}
                for cid in classes_by_id:
                    # Check major lines
                    if cid in major:
                        pts = major[cid].get("points_canonical_norm") or major[cid].get("points_norm") or []
                        if pts:
                            auto_lines[cid] = pts
                    # Check generic evidence items
                    if not auto_lines.get(cid) and "evidence" in evidence_data:
                        for ev in evidence_data.get("evidence", []):
                            if ev.get("evidence_id") == cid or ev.get("class_id") == cid:
                                pts = [[pt["x"], pt["y"]] for pt in ev.get("geometry", {}).get("points", [])]
                                if pts:
                                    auto_lines[cid] = pts

            # Compare metrics
            metrics = {}
            if truth_data:
                for cid in classes_by_id:
                    hpolys = parse_human_polys(truth_data, cid)
                    apts = auto_lines.get(cid) or []
                    metrics[cid] = compare_polylines(apts, hpolys)

            samples.append({
                "stem": stem,
                "filename": img_path.name,
                "image_url": f"/{rel_img}",
                "truth_file": truth_file or str((cur_dir / f"{stem}_USER_CORRECTED.json").relative_to(ROOT)),
                "has_truth": bool(truth_data),
                "truth_data": truth_data or {},
                "evidence_file": evidence_file,
                "has_evidence": bool(evidence_data),
                "auto_lines": auto_lines,
                "metrics": metrics,
                "split": "unassigned",
            })

    # Read split assignments from manifest if present
    manifest_candidates = [
        primary / "manifest.json",
        ROOT / "data" / "palm_labels" / "manifest.json",
        ROOT / "data" / "flood_labels" / "manifest.json",
    ]
    for mf in manifest_candidates:
        if mf.exists():
            try:
                mdata = json.loads(mf.read_text(encoding="utf-8"))
                split_map = {s.get("stem"): s.get("split") for s in mdata.get("samples", []) if s.get("stem")}
                for samp in samples:
                    if samp["stem"] in split_map and split_map[samp["stem"]]:
                        samp["split"] = split_map[samp["stem"]]
                break
            except Exception:
                pass

    return samples


class StudioHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT), **kwargs)

    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.end_headers()

    def do_GET(self):
        clean_path = self.path.split("?")[0]

        studio_pages = {
            "/",
            "/studio",
            "/apps/annotation_studio",
            "/apps/annotation_studio/",
            "/apps/annotation_studio/index.html",
            "/apps/annotation_studio/dist",
            "/apps/annotation_studio/dist/",
            "/apps/annotation_studio/dist/index.html",
        }
        if clean_path in studio_pages:
            dist_index = ROOT / "apps" / "annotation_studio" / "dist" / "index.html"
            target_html = dist_index if dist_index.exists() else (ROOT / "apps" / "annotation_studio" / "index.html")
            if target_html.exists():
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                data = target_html.read_bytes()
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return

        # Handle static bundled assets (/assets/*, /apps/annotation_studio/dist/assets/*, etc.)
        if "/assets/" in clean_path or clean_path.startswith("/assets/"):
            asset_subpath = clean_path.split("/assets/", 1)[1] if "/assets/" in clean_path else clean_path.lstrip("/assets/")
            cand = ROOT / "apps" / "annotation_studio" / "dist" / "assets" / asset_subpath
            if cand.exists() and cand.is_file():
                self.send_response(200)
                if cand.suffix == ".js":
                    self.send_header("Content-Type", "application/javascript; charset=utf-8")
                elif cand.suffix == ".css":
                    self.send_header("Content-Type", "text/css; charset=utf-8")
                elif cand.suffix == ".map":
                    self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(cand.stat().st_size))
                self.end_headers()
                self.wfile.write(cand.read_bytes())
                return

        # Fallback if unbundled /src/main.ts is requested by a cached browser session
        if clean_path.endswith("/src/main.ts") or clean_path == "/src/main.ts":
            dist_js = ROOT / "apps" / "annotation_studio" / "dist" / "assets" / "index.js"
            if dist_js.exists():
                self.send_response(200)
                self.send_header("Content-Type", "application/javascript; charset=utf-8")
                self.send_header("Content-Length", str(dist_js.stat().st_size))
                self.end_headers()
                self.wfile.write(dist_js.read_bytes())
                return

        if self.path == "/api/domains":
            reg = load_registry()
            self.send_json(reg)
            return

        if self.path.startswith("/api/samples"):
            from urllib.parse import parse_qs, urlparse
            query = parse_qs(urlparse(self.path).query)
            domain_id = query.get("domain", ["palm"])[0]
            reg = load_registry()
            matching = [d for d in reg.get("domains", []) if d["id"] == domain_id]
            if not matching:
                self.send_json({"error": f"Domain '{domain_id}' not found"}, status=404)
                return
            samples = get_samples_for_domain(matching[0])
            self.send_json({"domain": matching[0], "samples": samples})
            return

        if self.path == "/api/flood_delta":
            delta_file = ROOT / "data" / "flood_nepal_2026" / "out" / "flood_delta.json"
            if delta_file.exists():
                try:
                    delta_data = json.loads(delta_file.read_text(encoding="utf-8"))
                    self.send_json({"ok": True, "delta": delta_data})
                    return
                except Exception as e:
                    self.send_json({"error": str(e)}, status=500)
                    return
            self.send_json({"ok": False, "message": "flood_delta.json not yet generated"}, status=404)
            return

        super().do_GET()

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
        try:
            payload = json.loads(body)
        except Exception as e:
            self.send_json({"error": f"Invalid JSON payload: {e}"}, status=400)
            return

        if self.path == "/api/save_truth":
            try:
                rel_path = payload.get("target_file")
                data = payload.get("data")
                if not rel_path or data is None:
                    self.send_json({"error": "Missing target_file or data"}, status=400)
                    return
                full_path = ROOT / rel_path
                full_path.parent.mkdir(parents=True, exist_ok=True)
                full_path.write_text(json.dumps(data, indent=2), encoding="utf-8")

                # Recalculate metrics for this sample
                reg = load_registry()
                domain_id = payload.get("domain", "palm")
                matching = [d for d in reg.get("domains", []) if d["id"] == domain_id]
                metrics = {}
                if matching:
                    for cid in [c["id"] for c in matching[0].get("classes", [])]:
                        hpolys = parse_human_polys(data, cid)
                        apts = payload.get("auto_lines", {}).get(cid) or []
                        metrics[cid] = compare_polylines(apts, hpolys)

                self.send_json({"ok": True, "saved": str(rel_path), "metrics": metrics})
            except Exception as e:
                self.send_json({"error": str(e)}, status=500)
            return

        if self.path == "/api/dataset_split":
            try:
                domain_id = payload.get("domain", "palm")
                split_cfg = payload.get("split", {"train": 70, "valid": 20, "test": 10})
                labeled_only = payload.get("labeled_only", False)

                reg = load_registry()
                matching = [d for d in reg.get("domains", []) if d["id"] == domain_id]
                if not matching:
                    self.send_json({"error": f"Domain '{domain_id}' not found"}, status=404)
                    return

                domain_cfg = matching[0]
                samples = get_samples_for_domain(domain_cfg)
                target_samples = [s for s in samples if s["has_truth"]] if labeled_only else samples

                if not target_samples:
                    self.send_json({"error": "No matching samples found to split"}, status=400)
                    return

                # Compute splits deterministically
                n = len(target_samples)
                train_p = split_cfg.get("train", 70) / 100.0
                valid_p = split_cfg.get("valid", 20) / 100.0
                n_train = int(round(n * train_p))
                n_valid = int(round(n * valid_p))
                if n_train + n_valid > n:
                    n_valid = max(0, n - n_train)
                n_test = max(0, n - n_train - n_valid)

                # Assign splits
                assignments = []
                for i, s in enumerate(target_samples):
                    if i < n_train:
                        split_tag = "train"
                    elif i < n_train + n_valid:
                        split_tag = "valid"
                    else:
                        split_tag = "test"
                    assignments.append({
                        "stem": s["stem"],
                        "filename": s["filename"],
                        "split": split_tag,
                        "has_truth": s["has_truth"],
                    })

                # Persist manifest
                import datetime
                dataset_dir = ROOT / domain_cfg.get("dataset_dir", "")
                dataset_dir.mkdir(parents=True, exist_ok=True)
                manifest_path = dataset_dir / "manifest.json"

                manifest_data = {
                    "schema": "curvstudio_dataset_split_v1",
                    "domain": domain_id,
                    "updated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    "split_ratio": split_cfg,
                    "labeled_only": labeled_only,
                    "counts": {
                        "total": n,
                        "train": n_train,
                        "valid": n_valid,
                        "test": n_test,
                    },
                    "samples": assignments,
                }
                manifest_path.write_text(json.dumps(manifest_data, indent=2), encoding="utf-8")

                self.send_json({
                    "ok": True,
                    "manifest_file": str(manifest_path.relative_to(ROOT)),
                    "counts": manifest_data["counts"],
                    "samples": assignments,
                })
            except Exception as e:
                self.send_json({"error": str(e)}, status=500)
            return

        if self.path == "/api/run_inference":
            try:
                import base64
                import subprocess
                import time

                domain_id = payload.get("domain", "palm")
                image_url = payload.get("image_url", "")
                image_b64 = payload.get("image_base64", "")
                conf_thresh = float(payload.get("confidence_threshold", 0.5))

                target_img: Path | None = None
                if image_b64:
                    upload_dir = ROOT / "output" / "studio_uploads"
                    upload_dir.mkdir(parents=True, exist_ok=True)
                    target_img = upload_dir / f"upload_{int(time.time()*1000)}.png"
                    # Strip data URL prefix if present
                    if "," in image_b64:
                        image_b64 = image_b64.split(",", 1)[1]
                    target_img.write_bytes(base64.b64decode(image_b64))
                elif image_url:
                    clean_path = image_url.lstrip("/")
                    cand = ROOT / clean_path
                    if cand.exists():
                        target_img = cand

                if not target_img or not target_img.exists():
                    self.send_json({"error": "Target image not found"}, status=400)
                    return

                out_json = ROOT / "output" / "studio_infer_evidence.json"
                out_overlay = ROOT / "output" / "studio_infer_overlay.png"
                out_json.parent.mkdir(parents=True, exist_ok=True)

                start_t = time.perf_counter()
                if domain_id == "palm":
                    bin_path = ROOT / "build" / "tools" / "hand_inspector_cli"
                    if not bin_path.exists():
                        self.send_json({"error": f"Binary {bin_path} not found. Run cmake --build build"}, status=500)
                        return
                    cmd = [
                        str(bin_path),
                        "--image", str(target_img),
                        "--out-json", str(out_json),
                        "--out-overlay", str(out_overlay),
                    ]
                else:
                    bin_path = ROOT / "build" / "tools" / "curv_cli"
                    if not bin_path.exists():
                        self.send_json({"error": f"Binary {bin_path} not found. Run cmake --build build"}, status=500)
                        return
                    cmd = [
                        str(bin_path),
                        "--image", str(target_img),
                        "--min-blur", "0.0",
                        "--json", str(out_json),
                        "--out", str(out_overlay),
                    ]

                proc = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True, timeout=20)
                dur_ms = round((time.perf_counter() - start_t) * 1000, 1)

                predictions = []
                evidence = {}
                if out_json.exists():
                    try:
                        evidence = json.loads(out_json.read_text(encoding="utf-8"))
                        if domain_id == "palm":
                            major = (evidence.get("domain_payload") or {}).get("major_lines") or {}
                            for cid, info in major.items():
                                conf = float(info.get("confidence") or 0.0)
                                if conf >= conf_thresh:
                                    pts = info.get("points_canonical_norm") or info.get("points_norm") or []
                                    predictions.append({
                                        "class_id": cid,
                                        "confidence": conf,
                                        "points": pts,
                                        "length_px": info.get("arc_length_px", 0),
                                    })
                        else:
                            for ev in evidence.get("evidence", [])[:20]:
                                conf = float(ev.get("confidence") or 0.8)
                                if conf >= conf_thresh:
                                    pts = [[p["x"], p["y"]] for p in ev.get("geometry", {}).get("points", [])]
                                    predictions.append({
                                        "class_id": ev.get("class_id") or ev.get("evidence_id") or "waterline",
                                        "confidence": conf,
                                        "points": pts,
                                    })
                    except Exception as ex:
                        sys.stderr.write(f"[WARN] Error reading evidence: {ex}\n")

                overlay_url = "/output/studio_infer_overlay.png" if out_overlay.exists() else None

                self.send_json({
                    "ok": proc.returncode == 0,
                    "duration_ms": dur_ms,
                    "stdout": proc.stdout,
                    "predictions": predictions,
                    "overlay_url": overlay_url,
                    "evidence": evidence,
                })
            except Exception as e:
                self.send_json({"error": str(e)}, status=500)
            return

        self.send_json({"error": "Not Found"}, status=404)

    def send_json(self, data: Any, status: int = 200):
        content = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)


def main():
    parser = argparse.ArgumentParser(description="CurvStudio annotation server")
    parser.add_argument("--port", type=int, default=8080, help="Port to serve on (default: 8080)")
    parser.add_argument("--open", action="store_true", help="Automatically open Studio in browser")
    args = parser.parse_args()

    port = args.port
    max_attempts = 10
    httpd = None
    for attempt in range(max_attempts):
        try:
            httpd = HTTPServer(("", port), StudioHandler)
            break
        except OSError as e:
            if "Address already in use" in str(e):
                port += 1
            else:
                raise

    if not httpd:
        sys.stderr.write("[ERROR] Could not bind server to any port.\n")
        sys.exit(1)

    url = f"http://localhost:{port}/apps/annotation_studio/index.html"
    print("================================================================")
    print(f"CurvStudio (Interactive Annotation & Training Studio)")
    print(f"URL: {url}")
    print("Serving multi-domain datasets (Palm Creases + Flood Geospatial)")
    print("Press Ctrl+C to stop the server.")
    print("================================================================")

    if args.open:
        webbrowser.open(url)

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping CurvStudio server...")
        httpd.server_close()


if __name__ == "__main__":
    main()
