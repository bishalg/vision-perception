#!/usr/bin/env python3
"""Admin dashboard metrics: auto-vs-human confusion matrix, trends, validation queue.

Compares pipeline output (data/samples/mobile_captures/*_evidence.json)
against human corrections (*_USER_CORRECTED*.json) and regenerates the
self-contained view at output/admin_dashboard.html.

Usage (repo root):
  python3 scripts/admin_metrics.py            # snapshot + history append + HTML
  python3 scripts/admin_metrics.py --markdown # markdown table for docs/PROGRESS.md
  python3 scripts/admin_metrics.py --no-history  # snapshot only, don't append history
"""

from __future__ import annotations

import argparse
import datetime
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE_DIR = ROOT / "data" / "samples" / "mobile_captures"
PALM_LABELS_DIR = ROOT / "data" / "palm_labels"
PALM_LABELS_RAW = PALM_LABELS_DIR / "raw"
FLOOD_LABELS_DIR = ROOT / "data" / "flood_labels"
FLOOD_LABELS_RAW = FLOOD_LABELS_DIR / "raw"
FLOOD_PROCESSED = ROOT / "data" / "flood_nepal_2026" / "processed"
OUT_DIR = ROOT / "output"
REGISTRY_PATH = ROOT / "configs" / "domain_registry.json"
DIM = 512.0


def load_registry() -> dict:
    if REGISTRY_PATH.exists():
        try:
            return json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
        except Exception:
            pass
    return {"domains": []}


def domain_class_ids(domain_id: str = "palm") -> tuple[str, ...]:
    """Return annotation class ids from domain_registry.json (e.g. heart_line)."""
    reg = load_registry()
    for d in reg.get("domains", []):
        if d.get("id") == domain_id:
            ids = tuple(c["id"] for c in d.get("classes", []) if c.get("id"))
            if ids:
                return ids
    if domain_id == "flood":
        return ("waterline", "flood_boundary", "river_centerline", "bank_line")
    return ("heart_line", "head_line", "life_line", "fate_line")


def short_label(class_id: str) -> str:
    return class_id.replace("_line", "") if class_id.endswith("_line") else class_id


# Palm remains the default dashboard domain; --domain can switch.
ACTIVE_DOMAIN = "palm"
CLASS_IDS: tuple[str, ...] = domain_class_ids(ACTIVE_DOMAIN)
# Short display names (heart, head, ...) kept for HTML template compatibility.
LINES: tuple[str, ...] = tuple(short_label(c) for c in CLASS_IDS)
AUTO_KEY = {short_label(c): c for c in CLASS_IDS}
# Roots scanned for evidence / human truth / photos (mobile gold + bootstrap).
SCAN_DIRS = (FIXTURE_DIR, PALM_LABELS_DIR, PALM_LABELS_RAW)


def configure_domain(domain_id: str) -> None:
    """Rebind module-level class lists + scan dirs from the registry."""
    global ACTIVE_DOMAIN, CLASS_IDS, LINES, AUTO_KEY, SCAN_DIRS, DIM
    ACTIVE_DOMAIN = domain_id
    CLASS_IDS = domain_class_ids(domain_id)
    LINES = tuple(short_label(c) for c in CLASS_IDS)
    AUTO_KEY = {short_label(c): c for c in CLASS_IDS}
    if domain_id == "flood":
        SCAN_DIRS = (FLOOD_PROCESSED, FLOOD_LABELS_DIR, FLOOD_LABELS_RAW)
        DIM = 1.0  # flood labels are in pixel space; mean_err stays in px
    else:
        SCAN_DIRS = (FIXTURE_DIR, PALM_LABELS_DIR, PALM_LABELS_RAW)
        DIM = 512.0


def dist(a: list, b: list) -> float:
    return math.hypot(a[0] - b[0], a[1] - b[1])


def poly_arc(poly: list) -> float:
    return sum(dist(poly[i - 1], poly[i]) for i in range(1, len(poly)))


def human_polys(human: dict, name: str) -> list:
    for key in (name, AUTO_KEY[name]):
        val = human.get(key)
        if isinstance(val, list) and val:
            # Accept [[[x,y]..]] (list of polys) or [[x,y]..] (single poly).
            if val and isinstance(val[0], list) and val[0] and isinstance(val[0][0], (int, float)):
                return [val]
            return [p for p in val if isinstance(p, list) and len(p) >= 2]
    return []


def compare_line(auto_info: dict, hpolys: list) -> dict:
    auto_present = bool(auto_info.get("present"))
    human_present = any(len(p) >= 2 for p in hpolys)
    tp = int(auto_present and human_present)
    fp = int(auto_present and not human_present)
    fn = int(not auto_present and human_present)
    tn = int(not auto_present and not human_present)
    err_px, len_ratio = None, None
    if auto_present and human_present:
        apts = auto_info.get("points_canonical_norm") or []
        hpts = [pt for p in hpolys for pt in p]
        if apts and hpts:
            err_px = sum(min(dist(a, h) for h in hpts) for a in apts) / len(apts) * DIM
        auto_arc = float(auto_info.get("arc_length_px") or 0.0)
        human_arc = sum(poly_arc(p) for p in hpolys) * DIM
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
        "auto_conf": float(auto_info.get("confidence") or 0.0),
        "auto_arc_px": float(auto_info.get("arc_length_px") or 0.0),
    }


def find_human_label(stem: str) -> Path | None:
    hits: list[Path] = []
    for d in SCAN_DIRS:
        if d.exists():
            hits.extend(sorted(d.glob(f"{stem}_USER_CORRECTED*.json")))
    return hits[-1] if hits else None


def find_evidence_path(stem: str) -> Path | None:
    for d in SCAN_DIRS:
        p = d / f"{stem}_evidence.json"
        if p.exists():
            return p
    return None


def label_reviewed(human: dict, has_file: bool) -> bool:
    """Gold mobile labels without schema count as reviewed; imports need reviewed:true."""
    if not has_file:
        return False
    if "reviewed" in human:
        return bool(human.get("reviewed"))
    # Legacy USER_CORRECTED without schema (phone gold) = reviewed.
    return True


def collection_for(stem: str) -> str:
    if ACTIVE_DOMAIN == "flood":
        if (FLOOD_LABELS_RAW / f"{stem}.jpg").exists() or stem.startswith("rf_"):
            return "flood_labels"
        return "flood_nepal"
    if (PALM_LABELS_RAW / f"{stem}.jpg").exists() or (PALM_LABELS_DIR / f"{stem}_USER_CORRECTED.json").exists():
        if stem.startswith("rf_") or (PALM_LABELS_RAW / f"{stem}.jpg").exists():
            return "palm_labels"
    return "mobile_captures"


def load_cases() -> tuple[list, list]:
    """Load cases from mobile_captures + data/palm_labels.

    - Evidence-backed stems enter the confusion matrix when human-reviewed.
    - Label-only imports (no evidence yet) appear in the queue / case list so the
      training tab can track reviewed vs imported.
    """
    cases, queue = [], []
    stems: set[str] = set()

    for d in SCAN_DIRS:
        if not d.exists():
            continue
        for path in d.glob("*_evidence.json"):
            if "USER_CORRECTED" in path.name:
                continue
            stems.add(path.name.replace("_evidence.json", ""))
        for path in d.glob("*_USER_CORRECTED*.json"):
            name = path.name
            for suf in ("_USER_CORRECTED_v2.json", "_USER_CORRECTED.json"):
                if name.endswith(suf):
                    stems.add(name[: -len(suf)])
                    break
        if d == PALM_LABELS_RAW or d == FIXTURE_DIR:
            for path in d.iterdir():
                if path.suffix.lower() in {".jpg", ".jpeg", ".png"}:
                    n = path.name.lower()
                    if "overlay" in n or "canonical" in n or "user_corrected" in n:
                        continue
                    stems.add(path.stem)

    for stem in sorted(stems):
        ev_path = find_evidence_path(stem)
        lines, fate = {}, {}
        if ev_path:
            data = json.loads(ev_path.read_text())
            payload = data.get("domain_payload") or {}
            lines = payload.get("major_lines") or {}
            fate = payload.get("fate_analysis") or {}
            # Flood / generic: promote evidence items into a class_id → info map
            if not lines and "evidence" in data:
                class_map = {}
                for d in load_registry().get("domains", []):
                    if d.get("id") == ACTIVE_DOMAIN:
                        class_map = d.get("inference_class_map") or {}
                        break
                for ev in data.get("evidence", []):
                    raw = ev.get("class_id") or ev.get("type") or (ev.get("metadata") or {}).get("feature_class") or ""
                    mapped = class_map.get(raw, raw)
                    pts = []
                    for p in (ev.get("geometry") or {}).get("points", []) or []:
                        if isinstance(p, dict) and "x" in p:
                            pts.append([p["x"], p["y"]])
                    if mapped and pts:
                        lines[mapped] = {
                            "present": True,
                            "confidence": ev.get("confidence", 0.8),
                            "points_canonical_norm": pts,
                            "arc_length_px": (ev.get("geometry") or {}).get("total_length", 0),
                        }
        hpath = find_human_label(stem)
        human = json.loads(hpath.read_text()) if hpath else {}
        reviewed = label_reviewed(human, bool(hpath))
        # Queue: needs human review (no file, or imported with reviewed:false).
        if not reviewed:
            queue.append(stem)
        per_line = {n: compare_line(lines.get(AUTO_KEY[n]) or lines.get(n) or {}, human_polys(human, n)) for n in LINES}
        cases.append({
            "stem": stem,
            "reviewed": reviewed,
            "imported": bool(human.get("source") and "roboflow" in str(human.get("source"))),
            "collection": collection_for(stem),
            "has_evidence": bool(ev_path),
            "per_line": per_line,
            "fate_analysis": fate,
        })
    return cases, queue


def aggregate(cases: list) -> dict:
    # Matrix covers reviewed fixtures with auto evidence only; unreviewed live in the queue.
    reviewed = [c for c in cases if c["reviewed"] and c.get("has_evidence", True)]
    agg = {}
    for name in LINES:
        rows = [c["per_line"][name] for c in reviewed]
        tp = sum(r["tp"] for r in rows)
        fp = sum(r["fp"] for r in rows)
        fn = sum(r["fn"] for r in rows)
        tn = sum(r["tn"] for r in rows)
        errs = [r["mean_err_px"] for r in rows if r["mean_err_px"] is not None]
        prec = tp / (tp + fp) if (tp + fp) else None
        rec = tp / (tp + fn) if (tp + fn) else None
        f1 = 2 * prec * rec / (prec + rec) if prec and rec else None
        agg[name] = {
            "n": len(rows),
            "tp": tp,
            "fp": fp,
            "fn": fn,
            "tn": tn,
            "precision": prec,
            "recall": rec,
            "f1": f1,
            "mean_err_px": sum(errs) / len(errs) if errs else None,
        }
    return agg


def fmt(v, nd: int = 2) -> str:
    return "—" if v is None else f"{v:.{nd}f}"


def markdown(cases: list, agg: dict, queue: list) -> str:
    out = ["| Line | n | TP | FP | FN | TN | Prec | Rec | F1 | mean err px |",
           "| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for name in LINES:
        a = agg[name]
        out.append(f"| {name} | {a['n']} | {a['tp']} | {a['fp']} | {a['fn']} | {a['tn']} | "
                   f"{fmt(a['precision'])} | {fmt(a['recall'])} | {fmt(a['f1'])} | {fmt(a['mean_err_px'], 1)} |")
    out.append("")
    out.append(f"Validation queue ({len(queue)}): " + (", ".join(queue) if queue else "empty — all reviewed"))
    return "\n".join(out)


LINE_COLORS = {
    "heart": "#F43F5E",
    "head": "#06B6D4",
    "life": "#10B981",
    "fate": "#8B5CF6",
    "waterline": "#3B82F6",
    "flood_boundary": "#EF4444",
    "river_centerline": "#10B981",
    "bank_line": "#F59E0B",
}


def line_color(name: str) -> str:
    if name in LINE_COLORS:
        return LINE_COLORS[name]
    for d in load_registry().get("domains", []):
        for c in d.get("classes", []):
            cid = c.get("id", "")
            if cid == name or short_label(cid) == name:
                return c.get("color") or "#94A3B8"
    return "#94A3B8"


def trend_svg(history: list) -> str:
    if len(history) < 1:
        return '<p class="muted">No history yet — run the script after each tuning pass.</p>'
    W, H, PAD, PADB = 620, 200, 30, 34
    n = max(len(history) - 1, 1)

    def xy(i: int, v: float) -> tuple:
        return (PAD + i / n * (W - PAD - 14), H - PADB - min(max(v, 0), 1) * (H - PADB - 16))

    grid = "".join(
        f'<line x1="{PAD}" y1="{y:.0f}" x2="{W - 14}" y2="{y:.0f}" stroke="#26262b"/>'
        f'<text x="{PAD - 5}" y="{y + 4:.0f}" fill="#71717a" font-size="10" text-anchor="end">{lab}</text>'
        for lab, frac in (("1.0", 1.0), ("0.5", 0.5), ("0.0", 0.0))
        for y in [H - PADB - frac * (H - PADB - 16)])
    series = ""
    for key in LINES:
        vals = []
        for i, h in enumerate(history):
            row = (h.get("agg") or {}).get(key) or {}
            if row.get("f1") is not None:
                vals.append((i, row["f1"]))
        if not vals:
            continue
        coords = " ".join(f"{xy(i, v)[0]:.1f},{xy(i, v)[1]:.1f}" for i, v in vals)
        dots = "".join(
            f'<circle cx="{xy(i, v)[0]:.1f}" cy="{xy(i, v)[1]:.1f}" r="3.5" fill="{line_color(key)}">'
            f"<title>{key} F1={v:.2f} · run {i + 1} · {history[i].get('ts', '')}</title></circle>"
            for i, v in vals)
        series += (f'<polyline points="{coords}" fill="none" stroke="{line_color(key)}" stroke-width="2"/>'
                   + dots)
    xticks = " ".join(
        f'<text x="{xy(i, 0)[0]:.0f}" y="{H - 12}" fill="#71717a" font-size="10" text-anchor="middle">run {i + 1}</text>'
        for i in range(len(history)))
    legend = " ".join(f'<span class="legend" style="color:{line_color(k)}">● {k}</span>' for k in LINES)
    return (f'<div class="chart"><svg viewBox="0 0 {W} {H}" role="img" aria-label="F1 trend">{grid}{series}{xticks}</svg>'
            f'<div class="chart-legend">F1 per run · {legend}</div></div>')


def err_class(v) -> str:
    if v is None:
        return "na"
    return "good" if v < 10 else ("warn" if v < 60 else "bad")


def bar(frac, color: str) -> str:
    pct = 0 if frac is None else min(max(frac, 0), 1) * 100
    return f'<span class="bar"><span style="width:{pct:.0f}%;background:{color}"></span></span>'


def find_image(stem: str) -> str | None:
    # Prefer small files for thumbnails; full overlays are ~18MB.
    for suffix in ("_canonical.png", ".jpg", ".jpeg", ".png", "_canonical_overlay.png", "_full_overlay.png"):
        for d, rel_prefix in (
            (FIXTURE_DIR, "../data/samples/mobile_captures"),
            (PALM_LABELS_RAW, "../data/palm_labels/raw"),
            (PALM_LABELS_DIR, "../data/palm_labels"),
        ):
            cand = d / f"{stem}{suffix}"
            if cand.exists():
                return f"{rel_prefix}/{cand.name}"
    return None


SUFFIXES = ("_USER_CORRECTED_v2.json", "_USER_CORRECTED.json", "_evidence_landmarks.json",
            "_evidence.json", "_prompt.json", "_full_overlay.png",
            "_canonical_overlay.png", "_canonical.png", "_USER_CORRECTED.png")

KIND_LABEL = {"photo": "photo", "evidence": "evidence", "landmarks": "landmarks",
              "prompt": "prompt", "canonical": "canonical", "overlay": "overlay",
              "truth": "human truth", "other": "other"}


def stem_of(name: str) -> str:
    for suf in SUFFIXES:
        if name.endswith(suf):
            return name[: -len(suf)]
    return Path(name).stem


def kind_of(name: str) -> str:
    if "USER_CORRECTED" in name:
        return "truth"
    if name.endswith("_evidence_landmarks.json"):
        return "landmarks"
    if name.endswith("_evidence.json"):
        return "evidence"
    if name.endswith("_prompt.json"):
        return "prompt"
    if name.endswith("_canonical.png"):
        return "canonical"
    if name.endswith(("_full_overlay.png", "_canonical_overlay.png")):
        return "overlay"
    if name.endswith((".jpg", ".jpeg", ".png")):
        return "photo"
    return "other"


def size_fmt(n: int) -> str:
    if n < 1024:
        return f"{n} B"
    if n < 1024 ** 2:
        return f"{n / 1024:.1f} KB"
    return f"{n / 1024 ** 2:.1f} MB"


def inventory() -> dict:
    """All files in mobile_captures + palm_labels, grouped by hand stem."""
    inv: dict = {}
    roots = (
        (FIXTURE_DIR, "data/samples/mobile_captures"),
        (PALM_LABELS_DIR, "data/palm_labels"),
        (PALM_LABELS_RAW, "data/palm_labels/raw"),
    )
    for d, repo_prefix in roots:
        if not d.exists():
            continue
        for p in sorted(d.iterdir()):
            if not p.is_file() or p.name.startswith(".") or p.suffix.lower() == ".md":
                continue
            if p.name in {"manifest.json"}:
                continue
            stem = stem_of(p.name)
            inv.setdefault(stem, []).append({
                "name": p.name,
                "repo": f"{repo_prefix}/{p.name}",
                "href": f"../{repo_prefix}/{p.name}",
                "kind": kind_of(p.name),
                "bytes": p.stat().st_size,
            })
    return inv


def inspect_cmd(stem: str) -> str:
    img = None
    for d, rel in (
        (FIXTURE_DIR, "data/samples/mobile_captures"),
        (PALM_LABELS_RAW, "data/palm_labels/raw"),
        (PALM_LABELS_DIR, "data/palm_labels"),
    ):
        for ext in (".jpg", ".jpeg", ".png"):
            if (d / f"{stem}{ext}").exists():
                img = f"{rel}/{stem}{ext}"
                break
        if img:
            break
    if not img:
        img = f"data/samples/mobile_captures/{stem}.jpg"
    return (f"./build/tools/hand_inspector_cli --image {img} "
            f"--auto-landmarks --out-json output/palm_{stem}.json "
            f"--out-full-overlay output/palm_{stem}_full.png")


MENU_GROUPS = ("palm", "flood", "core", "labeling", "mobile", "quality")

NODE_DUMP = """
const out = [];
function flat(items, prefix) {
  return items.flatMap((it) => {
    const label = prefix ? prefix + ' \\u203a ' + it.label : it.label;
    if (it.menu) return flat(it.menu, label);
    return [{ label, kind: it.ask ? 'ask' : (typeof it.run === 'string' ? 'run' : (it.run ? 'fn' : (it.note ? 'note' : '?'))),
              cmd: typeof it.run === 'string' ? it.run : null, ask: it.ask || null, note: it.note || null }];
  });
}
for (const g of %s) {
  const m = await import('./scripts/menu/' + g + '.mjs');
  out.push({ slug: g, title: m.title, items: flat(m.items, '') });
}
console.log(JSON.stringify(out));
""" % (list(MENU_GROUPS),)


def menu_tasks() -> list:
    """Same step-by-step tasks as `pnpm dev`, read live from scripts/menu/*.mjs."""
    import subprocess
    try:
        r = subprocess.run(["node", "--input-type=module", "-e", NODE_DUMP],
                           cwd=ROOT, capture_output=True, text=True, timeout=20)
        groups = json.loads(r.stdout)
        return groups if isinstance(groups, list) else []
    except Exception:
        return []


SHELL = """<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="Curv Admin: palm-line prediction health — detection quality vs human truth, asset manager, training queue, runnable task index, architecture map.">
<meta name="theme-color" content="#0b0b0d">
<title>Curv Admin — palm prediction health</title>
<style>
:root{--bg:#0b0b0d;--panel:#141417;--panel2:#1a1a1f;--line:#2a2a31;--txt:#f4f4f5;--mut:#a1a1aa;
--green:#34d399;--amber:#fbbf24;--red:#f87171;--blue:#60a5fa;--violet:#a78bfa;--radius:14px}
*{box-sizing:border-box}body{background:var(--bg);color:var(--txt);font-family:system-ui,-apple-system,sans-serif;margin:0}
.wrap{max-width:1180px;margin:0 auto;padding:20px 20px 60px}
.topbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between}
.topbar h1{font-size:22px;margin:0}.topbar h1 small{display:block;font-size:12px;color:var(--mut);font-weight:400;margin-top:4px}
.btns{display:flex;gap:8px;flex-wrap:wrap}
button,.abtn{background:var(--panel2);color:var(--txt);border:1px solid var(--line);border-radius:10px;
padding:8px 14px;font-size:13px;cursor:pointer;text-decoration:none;display:inline-block}
button:hover,.abtn:hover{border-color:var(--blue)}button.primary{background:#2563eb;border-color:#2563eb}
button.approved{border-color:var(--green);color:var(--green)}button.flagged{border-color:var(--red);color:var(--red)}
nav.tabs{position:sticky;top:0;z-index:5;background:rgba(11,11,13,.95);backdrop-filter:blur(6px);
display:flex;gap:6px;padding:12px 0;margin-top:14px;border-bottom:1px solid var(--line)}
nav.tabs button{border-radius:999px}.ct{font-size:11px;background:#33333c;border-radius:999px;padding:1px 8px;margin-left:6px}
nav.tabs button.active{background:#2563eb;border-color:#2563eb}
.tab{display:none}.tab.active{display:block}
h2{font-size:17px;margin:26px 0 12px}h3{font-size:14px;margin:18px 0 8px}
.kpis{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin-top:16px}
.kpi{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);padding:14px}
.kpi .v{font-size:26px;font-weight:700}.kpi .l{font-size:12px;color:var(--mut);margin-top:2px}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:12px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);padding:14px}
.card h3{margin:0 0 8px;font-size:15px}.dot{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:6px}
.meter{display:grid;grid-template-columns:64px 1fr 44px;gap:8px;align-items:center;font-size:12px;margin:5px 0}
.bar{display:block;height:8px;background:#26262b;border-radius:99px;overflow:hidden}
.bar span{display:block;height:100%;border-radius:99px}
.nums{display:flex;gap:10px;font-size:12px;color:var(--mut);margin-top:8px;flex-wrap:wrap}
.err{font-weight:700}.err.good{color:var(--green)}.err.warn{color:var(--amber)}.err.bad{color:var(--red)}.err.na{color:var(--mut)}
table{width:100%;border-collapse:collapse;font-size:13px;background:var(--panel);border-radius:var(--radius);overflow:hidden}
td,th{border-bottom:1px solid var(--line);padding:8px 10px;text-align:left;vertical-align:top}
th{background:var(--panel2);font-size:12px;color:var(--mut);font-weight:600}
tr:last-child td{border-bottom:none}
.badge{display:inline-block;font-size:11px;padding:2px 9px;border-radius:999px;border:1px solid var(--line);white-space:nowrap}
.badge.green{color:var(--green);border-color:var(--green)}.badge.amber{color:var(--amber);border-color:var(--amber)}
.badge.blue{color:var(--blue);border-color:var(--blue)}.badge.violet{color:var(--violet);border-color:var(--violet)}
.badge.grey{color:var(--mut)}
.chip{display:inline-block;font-size:11px;background:#26262b;border-radius:6px;padding:2px 7px;margin:1px 2px;white-space:nowrap}
.stem-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(330px,1fr));gap:12px}
.thumb{width:100%;border-radius:10px;display:block;background:#000}
.cmd{font-family:ui-monospace,monospace;font-size:12px;background:#000;border:1px solid var(--line);
border-radius:8px;padding:8px 10px;overflow-x:auto;white-space:pre;margin:8px 0;color:#d4f4d4}
.toolbar{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin:12px 0}
.toolbar input[type=search]{background:var(--panel2);border:1px solid var(--line);color:var(--txt);
border-radius:10px;padding:8px 12px;font-size:13px;min-width:220px}
.seg{display:flex;gap:4px}.seg button.on{background:#2563eb;border-color:#2563eb}
.muted{color:var(--mut);font-size:13px}a{color:var(--blue)}
.chart svg{width:100%;height:auto;background:#101013;border:1px solid var(--line);border-radius:12px}
.chart-legend{font-size:12px;color:var(--mut);margin-top:6px}.legend{margin-right:10px}
.steps{counter-reset:st}.step{background:var(--panel);border:1px solid var(--line);border-radius:var(--radius);
padding:12px 14px;margin:8px 0;font-size:13px}.step b{color:var(--blue)}
.note{width:100%;background:var(--panel2);border:1px solid var(--line);color:var(--txt);border-radius:8px;
padding:7px 10px;font-size:12px;margin-top:8px}
footer{margin-top:34px;font-size:12px;color:var(--mut);border-top:1px solid var(--line);padding-top:14px}
.skip{position:absolute;left:-999px;top:0;background:#2563eb;color:#fff;padding:10px 16px;border-radius:0 0 10px 0;z-index:99}
.skip:focus{left:0}
.vh{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap}
button:focus-visible,.abtn:focus-visible,a:focus-visible,input:focus-visible{outline:2px solid var(--blue);outline-offset:2px}
nav.tabs button[aria-selected="true"]{background:#2563eb;border-color:#2563eb}
.task{display:flex;gap:10px;align-items:flex-start;background:var(--panel);border:1px solid var(--line);
border-radius:12px;padding:10px 12px;margin:8px 0}
.task .n{flex:none;width:26px;height:26px;border-radius:50%;background:#26262b;color:var(--mut);
font-size:12px;display:flex;align-items:center;justify-content:center}
.task .t{flex:1;min-width:0}.task .t b{font-size:13px;font-weight:600}
.task .t .cmd{margin:6px 0 0}.task .t .hint{font-size:12px;color:var(--mut);margin-top:4px}
.task button{flex:none;align-self:center}
.flow{width:100%;height:auto;background:#101013;border:1px solid var(--line);border-radius:12px}
ol.howto{font-size:13px;line-height:1.7;color:var(--txt);padding-left:20px}
ol.howto code,.muted code,p code,li code{background:#26262b;border-radius:6px;padding:1px 6px;font-size:12px}
@media(max-width:640px){.kpis{grid-template-columns:1fr 1fr}}
@media(prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
</style></head><body><a class="skip" href="#main">Skip to content</a><div class="wrap">
<header class="topbar"><h1>Curv Admin — palm prediction health<small>Generated @@TS@@ · auto vs human truth · source <code>scripts/admin_metrics.py</code></small></h1>
<div class="btns"><a class="abtn primary" href="../apps/annotation_studio/index.html" style="background:#2563eb;border-color:#2563eb;color:#fff;font-weight:600">🎨 Launch CurvStudio</a>
<a class="abtn" href="admin_metrics.json" download>Snapshot JSON</a>
<a class="abtn" href="admin_history.jsonl" download>History</a>
<button id="exportReview" type="button">Export review log</button>
<a class="abtn" href="../docs/admin_dashboard.md">Method doc</a></div></header>
<nav class="tabs" id="tabs" aria-label="Dashboard sections"><div role="tablist" aria-label="Dashboard sections" style="display:flex;gap:6px;flex-wrap:wrap">
<button role="tab" id="tabbtn-overview" aria-controls="tab-overview" aria-selected="true" data-tab="overview">Overview</button>
<button role="tab" id="tabbtn-tasks" aria-controls="tab-tasks" aria-selected="false" data-tab="tasks" tabindex="-1">Tasks <span class="ct">@@NTASKS@@</span></button>
<button role="tab" id="tabbtn-assets" aria-controls="tab-assets" aria-selected="false" data-tab="assets" tabindex="-1">Assets <span class="ct">@@NFILES@@</span></button>
<button role="tab" id="tabbtn-training" aria-controls="tab-training" aria-selected="false" data-tab="training" tabindex="-1">Training data <span class="ct">@@NQUEUE@@</span></button>
<button role="tab" id="tabbtn-results" aria-controls="tab-results" aria-selected="false" data-tab="results" tabindex="-1">Results <span class="ct">@@NCASES@@</span></button>
<button role="tab" id="tabbtn-arch" aria-controls="tab-arch" aria-selected="false" data-tab="arch" tabindex="-1">Architecture</button>
</div></nav>
<main id="main">
<section class="tab active" id="tab-overview" role="tabpanel" aria-labelledby="tabbtn-overview" tabindex="0">@@OVERVIEW@@</section>
<section class="tab" id="tab-tasks" role="tabpanel" aria-labelledby="tabbtn-tasks" tabindex="0">@@TASKS@@</section>
<section class="tab" id="tab-assets" role="tabpanel" aria-labelledby="tabbtn-assets" tabindex="0">@@ASSETS@@</section>
<section class="tab" id="tab-training" role="tabpanel" aria-labelledby="tabbtn-training" tabindex="0">@@TRAINING@@</section>
<section class="tab" id="tab-results" role="tabpanel" aria-labelledby="tabbtn-results" tabindex="0">@@RESULTS@@</section>
<section class="tab" id="tab-arch" role="tabpanel" aria-labelledby="tabbtn-arch" tabindex="0">@@ARCH@@</section>
</main>
<footer>Presence TP/FP/FN/TN: did we claim a line the human says is / isn't there. Geometry error is separate — a TP can still be off. Human truth: <code>*_USER_CORRECTED*.json</code> in <code>data/samples/mobile_captures/</code> and <code>data/palm_labels/</code> (imports need <code>reviewed:true</code> from the annotator). Review approvals live in this browser (localStorage) — export the log and keep it with the run.</footer>
</div>
<script>
const tabs=[...document.querySelectorAll('#tabs [role="tab"]')];
function selectTab(b,focus){tabs.forEach(x=>{x.classList.remove("active");x.setAttribute("aria-selected","false");x.tabIndex=-1});
b.classList.add("active");b.setAttribute("aria-selected","true");b.tabIndex=0;
document.querySelectorAll(".tab").forEach(s=>s.classList.remove("active"));
document.getElementById("tab-"+b.dataset.tab).classList.add("active");if(focus)b.focus()}
tabs.forEach((b,i)=>{b.onclick=()=>selectTab(b,false);
b.onkeydown=e=>{let j=null;if(e.key==="ArrowRight")j=(i+1)%tabs.length;else if(e.key==="ArrowLeft")j=(i-1+tabs.length)%tabs.length;
else if(e.key==="Home")j=0;else if(e.key==="End")j=tabs.length-1;
if(j!==null){e.preventDefault();selectTab(tabs[j],true)}}});
function copyText(t,btn){const done=()=>{const o=btn.textContent;btn.textContent="copied ✓";setTimeout(()=>btn.textContent=o,1200)};
if(navigator.clipboard&&navigator.clipboard.writeText){navigator.clipboard.writeText(t).then(done).catch(()=>fallback())}else fallback();
function fallback(){const ta=document.createElement("textarea");ta.value=t;document.body.appendChild(ta);ta.select();
try{document.execCommand("copy");done()}catch(e){}ta.remove()}}
document.querySelectorAll("[data-copy]").forEach(b=>b.onclick=()=>copyText(b.dataset.copy,b));
const q=document.getElementById("q"),seg=[...document.querySelectorAll(".seg button")];let f="all";
function applyFilter(){document.querySelectorAll("#fixtureRows tr").forEach(tr=>{
const okF=f==="all"||tr.dataset.status===f, okQ=!q||tr.dataset.stem.includes(q.value.trim().toLowerCase());
tr.style.display=okF&&okQ?"":"none"})}
if(q)q.oninput=applyFilter;seg.forEach(b=>b.onclick=()=>{seg.forEach(x=>x.classList.remove("on"));
b.classList.add("on");f=b.dataset.f;applyFilter()});
const LS="curvReviewV1";let reviews={};try{reviews=JSON.parse(localStorage.getItem(LS)||"{}")}catch(e){reviews={}}
function saveReviews(){localStorage.setItem(LS,JSON.stringify(reviews))}
function paintReview(stem){const card=document.querySelector(`[data-review="${stem}"]`);if(!card)return;
const r=reviews[stem]||{};card.querySelector(".bApprove").classList.toggle("approved",r.status==="approved");
card.querySelector(".bFlag").classList.toggle("flagged",r.status==="flagged");
const n=card.querySelector(".note");if(n&&r.note!==undefined&&document.activeElement!==n)n.value=r.note||"";
const s=card.querySelector(".rstate");if(s)s.textContent=r.status?`${r.status} · ${r.ts||""}`:"not reviewed yet"}
document.querySelectorAll("[data-review]").forEach(card=>{const stem=card.dataset.review;
card.querySelector(".bApprove").onclick=e=>{const r=reviews[stem]||{};
reviews[stem]={status:r.status==="approved"?null:"approved",note:card.querySelector(".note").value,ts:new Date().toISOString().slice(0,16)+"Z"};
saveReviews();paintReview(stem)};
card.querySelector(".bFlag").onclick=e=>{const r=reviews[stem]||{};
reviews[stem]={status:r.status==="flagged"?null:"flagged",note:card.querySelector(".note").value,ts:new Date().toISOString().slice(0,16)+"Z"};
saveReviews();paintReview(stem)};
card.querySelector(".note").onchange=e=>{const r=reviews[stem]||{};r.note=e.target.value;reviews[stem]=r;saveReviews()};
paintReview(stem)});
document.getElementById("exportReview").onclick=()=>{const blob=new Blob([JSON.stringify(
{exported:new Date().toISOString(),reviews},null,2)],{type:"application/json"});
const a=document.createElement("a");a.href=URL.createObjectURL(blob);a.download="review_log.json";a.click();
setTimeout(()=>URL.revokeObjectURL(a.href),2000)};
</script></body></html>"""


def fate_chips(c: dict) -> str:
    fa = c["fate_analysis"]
    if not fa:
        return '<span class="muted">—</span>'
    out = []
    for k in ("present", "broken", "gap_count", "stops_at_head", "stops_at_heart", "age_at_end"):
        v = fa.get(k, "—")
        cls = "amber" if (k == "broken" and v) else ("green" if v is True else "grey")
        out.append(f'<span class="badge {cls}">{k}={v}</span>')
    return " ".join(out)


def line_cell(info: dict) -> str:
    auto = "Y" if info["auto_present"] else "·"
    hum = "Y" if info["human_present"] else "·"
    err = info["mean_err_px"]
    es = "" if err is None else f' <span class="err {err_class(err)}">{err:.1f}px</span>'
    return f"{auto}/{hum}{es}"


def overview_html(cases: list, agg: dict, queue: list, history: list) -> str:
    reviewed = [c for c in cases if c["reviewed"]]
    scored = [c for c in reviewed if c.get("has_evidence", True)]
    imported = [c for c in cases if c.get("imported") and not c["reviewed"]]
    f1s = [agg[n]["f1"] for n in LINES if agg[n]["f1"] is not None]
    mean_f1 = sum(f1s) / len(f1s) if f1s else None
    worst = min(LINES, key=lambda n: agg[n]["f1"] if agg[n]["f1"] is not None else 2)
    kpis = (
        f'<div class="kpi"><div class="v">{len(reviewed)}/{len(cases)}</div><div class="l">hands reviewed</div></div>'
        f'<div class="kpi"><div class="v">{len(queue)}</div><div class="l">need labeling</div></div>'
        f'<div class="kpi"><div class="v">{len(imported)}</div><div class="l">imported (unreviewed)</div></div>'
        f'<div class="kpi"><div class="v">{fmt(mean_f1)}</div><div class="l">mean F1 (scored n={len(scored)})</div></div>'
        f'<div class="kpi"><div class="v" style="color:{line_color(worst)}">{worst}</div>'
        f'<div class="l">weakest line · F1 {fmt(agg[worst]["f1"])}</div></div>')
    cards = ""
    for n in LINES:
        a = agg[n]
        cards += (
            f'<div class="card"><h3><span class="dot" style="background:{line_color(n)}"></span>{n}</h3>'
            f'<div class="meter"><span>precision</span>{bar(a["precision"], line_color(n))}<span>{fmt(a["precision"])}</span></div>'
            f'<div class="meter"><span>recall</span>{bar(a["recall"], line_color(n))}<span>{fmt(a["recall"])}</span></div>'
            f'<div class="meter"><span>F1</span>{bar(a["f1"], line_color(n))}<span>{fmt(a["f1"])}</span></div>'
            f'<div class="nums"><span>TP {a["tp"]} · FP {a["fp"]} · FN {a["fn"]} · TN {a["tn"]}</span>'
            f'<span>n={a["n"]}</span><span>err <span class="err {err_class(a["mean_err_px"])}">'
            f'{fmt(a["mean_err_px"], 1)}px</span></span></div></div>')
    qprev = (" ".join(f'<span class="badge amber">{s}</span>' for s in queue[:40])
             + (f' <span class="muted">+{len(queue)-40} more</span>' if len(queue) > 40 else "")
             if queue else '<span class="badge green">empty — all reviewed</span>')
    return (f"<h2>Health at a glance</h2><div class='kpis'>{kpis}</div>"
            f"<h2>Per-line quality (reviewed + evidence only)</h2><div class='cards'>{cards}</div>"
            f"<h2>Trend — F1 per run</h2>{trend_svg(history)}"
            f"<h2>Validation queue ({len(queue)})</h2><p>{qprev}</p>"
            f"<p class='muted'>Label queued hands in the browser annotator "
            f"(<code>python3 tools/palm_annotator/server.py</code>), then re-run "
            f"<code>python3 scripts/admin_metrics.py</code>.</p>")


def assets_html(inv: dict, cases: list) -> str:
    reviewed = {c["stem"] for c in cases if c["reviewed"]}
    stems = sorted(inv)
    total = sum(len(v) for v in inv.values())
    out = [f"<h2>Asset manager — {total} files in <code>mobile_captures/</code> + <code>palm_labels/</code></h2>",
           "<p class='muted'>Every file behind the metrics. Open to preview, copy the repo path for scripts, "
           "download to share. Truth files (<span class='badge violet'>human truth</span>) are what the matrix scores against. "
           "Imported Roboflow labels stay <span class='badge amber'>needs label</span> until Save in the annotator.</p>",
           '<div class="stem-grid">']
    for stem in stems:
        files = inv[stem]
        img = find_image(stem)
        thumb = (f'<a href="{img}"><img class="thumb" src="{img}" loading="lazy" alt="{stem}"></a>' if img else "")
        status = ('<span class="badge green">reviewed</span>' if stem in reviewed
                  else '<span class="badge amber">needs label</span>')
        rows = "".join(
            f"<tr><td><code>{fl['name']}</code></td>"
            f"<td><span class='badge {'violet' if fl['kind'] == 'truth' else 'blue' if fl['kind'] == 'photo' else 'grey'}'>"
            f"{KIND_LABEL[fl['kind']]}</span></td><td>{size_fmt(fl['bytes'])}</td>"
            f"<td><a href=\"{fl['href']}\">open</a> · <a href=\"{fl['href']}\" download>save</a> · "
            f"<a href='#' data-copy=\"{fl['repo']}\" onclick='return false'>copy path</a></td></tr>"
            for fl in files)
        out.append(
            f"<div class='card'><h3>{stem} {status}</h3>{thumb}"
            f"<table><caption class='vh'>Files for this hand</caption><tr><th scope='col'>file</th>"
            f"<th scope='col'>kind</th><th scope='col'>size</th><th scope='col'>"
            f"<span class='vh'>actions</span></th></tr>{rows}</table></div>")
    out.append("</div>")
    return "".join(out)


def training_html(cases: list, queue: list) -> str:
    guide = (
        "<h2>Add / label hands</h2><div class='steps'>"
        "<div class='step'><b>A. Bootstrap (~100 CC-BY)</b> — "
        "<code>export ROBOFLOW_API_KEY=…</code> then "
        "<code>python3 scripts/download_palm_labels.py --limit 100</code>. "
        "Imported keypoints land in <code>data/palm_labels/</code> with <code>reviewed:false</code>.</div>"
        "<div class='step'><b>B. Draw / correct</b> — "
        "<code>python3 tools/palm_annotator/server.py</code> → "
        "<a href='http://127.0.0.1:8765'>http://127.0.0.1:8765</a>. "
        "Color tags 1–4 Heart/Head/Life/Fate, Save sets <code>reviewed:true</code>.</div>"
        "<div class='step'><b>C. Phone gold</b> — drop <code>&lt;stem&gt;.jpg</code> into "
        "<code>data/samples/mobile_captures/</code>, run detect, then label in the same annotator.</div>"
        "<div class='step'><b>D. Compare</b> — re-run this dashboard. "
        "Do <b>not</b> train U-Net until ~50–100 labels are reviewed "
        "(see <code>data/palm_labels/README.md</code>).</div>"
        "</div>")
    qcards = ""
    if queue:
        qcards = f"<h2>Needs labeling ({len(queue)})</h2><div class='stem-grid'>"
        for stem in queue[:60]:
            c = next((x for x in cases if x["stem"] == stem), {})
            img = find_image(stem)
            thumb = (f'<a href="{img}"><img class="thumb" src="{img}" loading="lazy" alt="{stem}"></a>' if img else "")
            kind = "imported" if c.get("imported") else "unlabeled"
            badge = f"<span class='badge amber'>{kind}</span>"
            if c.get("collection") == "palm_labels":
                badge += " <span class='badge violet'>palm_labels</span>"
            cmd = inspect_cmd(stem).replace("&", "&amp;").replace("<", "&lt;")
            qcards += (
                f"<div class='card'><h3>{stem} {badge}</h3>{thumb}"
                f"<p class='muted'>Annotate at <code>http://127.0.0.1:8765</code> or run inspect:</p>"
                f"<div class='cmd'>{cmd}</div>"
                f"<button data-copy=\"{inspect_cmd(stem)}\">Copy inspect command</button> "
                f"<button data-copy=\"{stem}_USER_CORRECTED.json\">Copy truth filename</button></div>")
        if len(queue) > 60:
            qcards += f"<p class='muted'>Showing 60 of {len(queue)} — open the annotator gallery for the rest.</p>"
        qcards += "</div>"
    rcards = "<h2>Reviewed — confirm or flag</h2><div class='stem-grid'>"
    for c in cases:
        if not c["reviewed"]:
            continue
        img = find_image(c["stem"])
        thumb = (f'<a href="{img}"><img class="thumb" src="{img}" loading="lazy" alt="{c["stem"]}"></a>' if img else "")
        chips = " ".join(
            f"<span class='chip'>{n}: auto {'Y' if c['per_line'][n]['auto_present'] else '·'} / "
            f"human {'Y' if c['per_line'][n]['human_present'] else '·'}"
            + (f" · {c['per_line'][n]['mean_err_px']:.0f}px" if c['per_line'][n]['mean_err_px'] is not None else "")
            + "</span>" for n in LINES)
        coll = f" <span class='badge grey'>{c.get('collection','')}</span>" if c.get("collection") else ""
        rcards += (
            f"<div class='card' data-review=\"{c['stem']}\"><h3>{c['stem']} "
            f"<span class='badge green'>reviewed</span>{coll} <span class='muted rstate'></span></h3>{thumb}"
            f"<p>{chips}</p><button class='bApprove'>Approve ✓</button> <button class='bFlag'>Flag ✗</button>"
            f"<input class='note' placeholder='review note (stored in this browser)…'></div>")
    rcards += "</div>"
    return guide + qcards + rcards


def tasks_html(groups: list) -> str:
    from html import escape as esc
    if not groups:
        return ("<h2>Runnable tasks</h2><p class='muted'>Menu modules unreadable — "
                "run <code>pnpm dev</code> in a terminal for the interactive menu.</p>")
    out = ["<h2>Runnable tasks — same steps as <code>pnpm dev</code></h2>",
           "<p class='muted'>A static page cannot run shell commands — <b>Copy</b>, paste in a "
           "repo-root terminal, Enter. Numbering matches the <code>pnpm dev</code> menu. "
           "Items marked <span class='badge grey'>input</span> need the shown value; "
           "<span class='badge grey'>guided</span> opens a submenu or note — use "
           "<code>pnpm dev</code> for those.</p>"]
    for gi, g in enumerate(groups, 1):
        out.append(f"<h3>{gi}. {esc(g['title'])}</h3>")
        for ti, it in enumerate(g["items"], 1):
            label = esc(it["label"])
            if it["kind"] == "run":
                cmd = esc(it["cmd"])
                out.append(
                    f"<div class='task'><span class='n' aria-hidden='true'>{ti}</span>"
                    f"<div class='t'><b>{label}</b><div class='cmd'>{cmd}</div></div>"
                    f"<button type='button' data-copy=\"{esc(it['cmd'], quote=True)}\">Copy</button></div>")
            elif it["kind"] == "ask":
                ask = esc(it["ask"] or "input")
                if it["cmd"]:
                    body = f"<div class='cmd'>{esc(it['cmd'])} &lt;{ask}&gt;</div>"
                    btn = f"<button type='button' data-copy=\"{esc(it['cmd'], quote=True)}\">Copy</button>"
                else:
                    body = (f"<div class='hint'>needs your {ask} to build the command — "
                            f"run via <code>pnpm dev</code>, or copy the inspect command from the Training data tab</div>")
                    btn = ""
                out.append(
                    f"<div class='task'><span class='n' aria-hidden='true'>{ti}</span>"
                    f"<div class='t'><b>{label}</b> <span class='badge grey'>input</span>{body}</div>{btn}</div>")
            else:
                hint = esc(it["note"]) if it.get("note") else "guided step — run via <code>pnpm dev</code>"
                out.append(
                    f"<div class='task'><span class='n' aria-hidden='true'>{ti}</span>"
                    f"<div class='t'><b>{label}</b> <span class='badge grey'>guided</span>"
                    f"<div class='hint'>{hint}</div></div></div>")
    return "".join(out)


ARCH_ROLES = [
    ("core/", "Domain-agnostic vision engine (C++). Knows pixels, not palms."),
    ("domains/palm/", "Palm product logic: crease detection, fate taxonomy, report payload."),
    ("domains/flood/", "Flood product logic: river morphology, delta, registration."),
    ("apps/mobile_overlay/", "Shared camera overlay (iOS + Android). Renders only — never decides."),
    ("apps/ios_palm_app/", "Shippable iOS app (PalmScanner)."),
    ("apps/android_palm_app/", "Shippable Android app + native .so packaging."),
    ("tools/", "Desktop CLIs: hand_inspector, overlay debugger, demos."),
    ("scripts/", "Python ops: metrics, sweeps, pipeline helpers. Menu groups in scripts/menu/."),
    ("data/samples/mobile_captures/", "Phone gold hands + evidence + human truth."),
    ("data/palm_labels/", "Roboflow bootstrap + annotator labels (raw JPEGs gitignored)."),
    ("tools/palm_annotator/", "Local browser polyline editor (color-tagged Heart/Head/Life/Fate)."),
    ("output/", "Gitignored build artifacts: dashboard, snapshots, overlays."),
    ("docs/", "Method, vision, roadmaps. Start at product_vision.md."),
]

FLOW_STEPS = [
    ("Mobile capture", "photo"),
    ("hand_inspector_cli", "detect"),
    ("*_evidence.json", "auto lines"),
    ("Human correction", "you label"),
    ("*_USER_CORRECTED", "truth"),
    ("admin_metrics.py", "scores"),
    ("This dashboard", "decide"),
    ("Prediction report", "user value"),
]


def arch_html() -> str:
    import os
    boxes, arrows = [], []
    W, BW, BH, GAP = 1180, 128, 54, 22
    per_row = 4
    for i, (title, sub) in enumerate(FLOW_STEPS):
        r, k = divmod(i, per_row)
        x = 10 + k * (BW + GAP)
        y = 10 + r * (BH + 44)
        fill = "#1c2b1f" if "Human" in title or "TRUTH" in title.upper() else ("#1a2340" if "dashboard" in title else "#16161c")
        stroke = "#34d399" if "Human" in title or "TRUTH" in title.upper() else ("#60a5fa" if "dashboard" in title else "#3a3a44")
        boxes.append(
            f'<rect x="{x}" y="{y}" width="{BW}" height="{BH}" rx="10" fill="{fill}" stroke="{stroke}"/>'
            f'<text x="{x + BW / 2}" y="{y + 23}" fill="#f4f4f5" font-size="11" font-weight="bold" text-anchor="middle">{title}</text>'
            f'<text x="{x + BW / 2}" y="{y + 39}" fill="#a1a1aa" font-size="10" text-anchor="middle">{sub}</text>')
        if k < per_row - 1 and i + 1 < len(FLOW_STEPS) and (i + 1) % per_row:
            arrows.append(f'<line x1="{x + BW}" y1="{y + BH / 2}" x2="{x + BW + GAP}" y2="{y + BH / 2}" stroke="#52525b" marker-end="url(#ah)"/>')
        elif (i + 1) < len(FLOW_STEPS):
            nx = 10
            arrows.append(f'<line x1="{x + BW / 2}" y1="{y + BH}" x2="{nx + BW / 2}" y2="{y + BH + 44}" stroke="#52525b" marker-end="url(#ah)"/>')
    rows = max(1, -(-len(FLOW_STEPS) // per_row))
    H = rows * (BH + 44) + 10
    flow = (f'<svg class="flow" viewBox="0 0 {W} {H}" role="img" aria-label="Data flow from capture to prediction report">'
            '<defs><marker id="ah" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">'
            '<path d="M0,0 L8,4 L0,8" fill="none" stroke="#52525b"/></marker></defs>'
            + "".join(arrows + boxes) + "</svg>")
    tre = []
    for d, role in ARCH_ROLES:
        p = ROOT / d.rstrip("/")
        n = sum(1 for _, _, fs in os.walk(p) for f in fs if not f.startswith(".")) if p.exists() else 0
        tre.append(f"<tr><td><code>{d}</code></td><td>{role}</td><td>{n} files</td></tr>")
    return (
        "<h2>How the code fits together</h2>"
        "<p class='muted'>Green = human input, blue = this page. The loop: detect → you correct → "
        "scores update → predictions improve. Overlays never decide; only confirmed geometry reaches reports.</p>"
        + flow +
        "<h2>Repo map (live file counts)</h2>"
        "<table><caption class='vh'>Repository directories and their roles</caption>"
        "<tr><th scope='col'>directory</th><th scope='col'>role</th><th scope='col'>size</th></tr>"
        + "".join(tre) + "</table>"
        "<h2>Working agreements</h2><ol class='howto'>"
        "<li><code>core/</code> stays domain-agnostic — palm/flood logic lives in <code>domains/</code>.</li>"
        "<li>Overlays render; they never classify. Evidence JSON is the contract between layers.</li>"
        "<li>Every tuning pass ends with <code>python3 scripts/admin_metrics.py</code> so the trend chart moves.</li>"
        "<li>Human corrections are first-class training labels, not side files.</li></ol>")


def results_html(cases: list, agg: dict) -> str:
    mrows = "".join(
        f"<tr><td><span class='dot' style='background:{line_color(n)}'></span>{n}</td><td>{agg[n]['n']}</td>"
        f"<td>{agg[n]['tp']}</td><td>{agg[n]['fp']}</td><td>{agg[n]['fn']}</td><td>{agg[n]['tn']}</td>"
        f"<td>{fmt(agg[n]['precision'])}</td><td>{fmt(agg[n]['recall'])}</td><td>{fmt(agg[n]['f1'])}</td>"
        f"<td><span class='err {err_class(agg[n]['mean_err_px'])}'>{fmt(agg[n]['mean_err_px'], 1)}px</span></td></tr>"
        for n in LINES)
    def fixture_row(c: dict) -> str:
        status = "reviewed" if c["reviewed"] else "queued"
        if c["reviewed"]:
            badge = "<span class='badge green'>reviewed</span>"
        elif c.get("imported"):
            badge = "<span class='badge amber'>imported</span>"
        else:
            badge = "<span class='badge amber'>needs label</span>"
        if c.get("collection") == "palm_labels":
            badge += " <span class='badge violet'>palm_labels</span>"
        if not c.get("has_evidence", True):
            badge += " <span class='badge grey'>no auto</span>"
        cells = "".join(f"<td>{line_cell(c['per_line'][n])}</td>" for n in LINES)
        fate_td = f"<td>{fate_chips(c)}</td>" if ACTIVE_DOMAIN == "palm" else ""
        return (f"<tr data-stem=\"{c['stem']}\" data-status=\"{status}\">"
                f"<td><b>{c['stem']}</b><br>{badge}</td>{cells}{fate_td}</tr>")

    frows = "".join(fixture_row(c) for c in cases)
    line_headers = "".join(
        f"<th scope='col'>{n}<br><small>auto/human</small></th>" for n in LINES
    )
    fate_header = "<th scope='col'>fate analysis</th>" if ACTIVE_DOMAIN == "palm" else ""
    return (
        "<h2>Confusion matrix per line</h2>"
        f"<table><caption class='vh'>Presence confusion matrix per {ACTIVE_DOMAIN} class</caption>"
        "<tr><th scope='col'>line</th><th scope='col'>n</th><th scope='col'>TP</th><th scope='col'>FP</th>"
        "<th scope='col'>FN</th><th scope='col'>TN</th>"
        "<th scope='col'>prec</th><th scope='col'>rec</th><th scope='col'>F1</th><th scope='col'>err</th></tr>"
        + mrows + "</table>"
        "<h2>Per-fixture detail</h2>"
        "<div class='toolbar'><div class='seg'><button data-f='all' class='on'>All</button>"
        "<button data-f='reviewed'>Reviewed</button><button data-f='queued'>Needs label</button></div>"
        "<input type='search' id='q' placeholder='filter by hand name…'></div>"
        f"<table><caption class='vh'>Per-fixture auto vs human presence and geometry error</caption>"
        f"<tr><th scope='col'>fixture</th>{line_headers}{fate_header}</tr>"
        f"<tbody id='fixtureRows'>{frows}</tbody></table>")


def html(ts: str, cases: list, agg: dict, queue: list, history: list, inv: dict, groups: list) -> str:
    page = SHELL.replace("@@TS@@", ts)
    page = page.replace("@@NFILES@@", str(sum(len(v) for v in inv.values())))
    page = page.replace("@@NQUEUE@@", str(len(queue)))
    page = page.replace("@@NCASES@@", str(len(cases)))
    page = page.replace("@@NTASKS@@", str(sum(len(g["items"]) for g in groups)))
    page = page.replace("@@OVERVIEW@@", overview_html(cases, agg, queue, history))
    page = page.replace("@@TASKS@@", tasks_html(groups))
    page = page.replace("@@ASSETS@@", assets_html(inv, cases))
    page = page.replace("@@TRAINING@@", training_html(cases, queue))
    page = page.replace("@@RESULTS@@", results_html(cases, agg))
    page = page.replace("@@ARCH@@", arch_html())
    return page


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--markdown", action="store_true")
    ap.add_argument("--no-history", action="store_true")
    ap.add_argument(
        "--domain",
        default="palm",
        help="Domain id from configs/domain_registry.json (default: palm)",
    )
    args = ap.parse_args()

    configure_domain(args.domain)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    ts = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    cases, queue = load_cases()
    agg = aggregate(cases)

    inv = inventory()
    snap = {"ts": ts, "domain": ACTIVE_DOMAIN, "class_ids": list(CLASS_IDS), "agg": agg, "cases": cases, "queue": queue, "inventory": inv}
    (OUT_DIR / "admin_metrics.json").write_text(json.dumps(snap, indent=2))
    hist_path = OUT_DIR / "admin_history.jsonl"
    history = []
    if hist_path.exists():
        history = [json.loads(line) for line in hist_path.read_text().splitlines() if line.strip()]
    if not args.no_history:
        with hist_path.open("a") as f:
            f.write(json.dumps({"ts": ts, "domain": ACTIVE_DOMAIN, "agg": agg,
                                "n_cases": len(cases), "queue": queue}) + "\n")
        history.append({"ts": ts, "agg": agg})
    groups = menu_tasks()
    (OUT_DIR / "admin_dashboard.html").write_text(html(ts, cases, agg, queue, history, inv, groups))

    if args.markdown:
        print(markdown(cases, agg, queue))
    else:
        print(f"domain={ACTIVE_DOMAIN} cases={len(cases)} queue={len(queue)} -> output/admin_metrics.json + output/admin_dashboard.html")


if __name__ == "__main__":
    main()
