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
OUT_DIR = ROOT / "output"
LINES = ("heart", "head", "life", "fate")
AUTO_KEY = {name: f"{name}_line" for name in LINES}
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


def load_cases() -> tuple[list, list]:
    cases, queue = [], []
    for path in sorted(FIXTURE_DIR.glob("*_evidence.json")):
        if "USER_CORRECTED" in path.name:
            continue
        stem = path.name.replace("_evidence.json", "")
        data = json.loads(path.read_text())
        payload = data.get("domain_payload") or {}
        lines = payload.get("major_lines") or {}
        hpaths = sorted(FIXTURE_DIR.glob(f"{stem}_USER_CORRECTED*.json"))
        human = json.loads(hpaths[-1].read_text()) if hpaths else {}
        reviewed = bool(hpaths)
        if not reviewed:
            queue.append(stem)
        per_line = {n: compare_line(lines.get(AUTO_KEY[n]) or {}, human_polys(human, n)) for n in LINES}
        cases.append({"stem": stem, "reviewed": reviewed,
                      "per_line": per_line, "fate_analysis": payload.get("fate_analysis") or {}})
    return cases, queue


def aggregate(cases: list) -> dict:
    # Matrix covers reviewed fixtures only; unreviewed live in the queue, not as FPs.
    reviewed = [c for c in cases if c["reviewed"]]
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


def trend_svg(history: list) -> str:
    if len(history) < 1:
        return "<p>No history yet — run the script after each tuning pass.</p>"
    W, H, PAD = 560, 180, 28
    colors = {"heart": "#F43F5E", "head": "#06B6D4", "life": "#10B981", "fate": "#8B5CF6"}

    def pts(key: str, sub: str) -> str:
        vals = [(i, h["agg"][key][sub]) for i, h in enumerate(history) if h["agg"][key][sub] is not None]
        if not vals:
            return ""
        n = max(len(history) - 1, 1)
        coords = " ".join(f"{PAD + i / n * (W - 2 * PAD):.1f},{H - PAD - min(max(v, 0), 1) * (H - 2 * PAD):.1f}"
                          for i, v in vals)
        return f'<polyline points="{coords}" fill="none" stroke="{colors[key]}" stroke-width="2"/>'

    f1 = "".join(pts(k, "f1") for k in LINES)
    legend = " ".join(f'<span style="color:{c}">{k}</span>' for k, c in colors.items())
    return (f"<p>F1 over time (per run): {legend}</p>"
            f'<svg width="{W}" height="{H}" style="background:#111;border-radius:8px">'
            f'<line x1="{PAD}" y1="{H - PAD}" x2="{W - PAD}" y2="{H - PAD}" stroke="#444"/>'
            f'<line x1="{PAD}" y1="{PAD}" x2="{PAD}" y2="{H - PAD}" stroke="#444"/>{f1}</svg>')


def html(ts: str, cases: list, agg: dict, queue: list, history: list) -> str:
    def row(name: str) -> str:
        a = agg[name]
        return (f"<tr><td>{name}</td><td>{a['n']}</td><td>{a['tp']}</td><td>{a['fp']}</td>"
                f"<td>{a['fn']}</td><td>{a['tn']}</td><td>{fmt(a['precision'])}</td>"
                f"<td>{fmt(a['recall'])}</td><td>{fmt(a['f1'])}</td><td>{fmt(a['mean_err_px'], 1)}</td></tr>")

    def fate(c: dict) -> str:
        fa = c["fate_analysis"]
        if not fa:
            return "—"
        keys = ("present", "broken", "gap_count", "stops_at_head", "stops_at_heart", "age_at_end")
        return ", ".join(f"{k}={fa.get(k, '—')}" for k in keys)

    detail = "".join(
        f"<tr><td>{c['stem']}</td>" + "".join(
            f"<td>{'Y' if c['per_line'][n]['auto_present'] else '·'}/"
            f"{'Y' if c['per_line'][n]['human_present'] else '·'}"
            + (f" {c['per_line'][n]['mean_err_px']:.1f}px" if c['per_line'][n]['mean_err_px'] is not None else "")
            + "</td>" for n in LINES) + f"<td><small>{fate(c)}</small></td></tr>"
        for c in cases)
    q = ", ".join(queue) if queue else "empty — all reviewed"
    return f"""<!doctype html><html><head><meta charset="utf-8"><title>Curv Admin — prediction health</title>
<style>body{{background:#0b0b0d;color:#eee;font-family:system-ui,sans-serif;margin:24px}}table{{border-collapse:collapse;margin:12px 0}}td,th{{border:1px solid #333;padding:6px 10px;font-size:13px}}th{{background:#1c1c1e}}small{{color:#aaa}}a{{color:#6cf}}</style>
</head><body><h1>Prediction engine health</h1><p><small>Generated {ts} · presence TP/FP/FN/TN + geometry error vs human truth · source: <code>scripts/admin_metrics.py</code></small></p>
<h2>Confusion matrix per line</h2><table><tr><th>line</th><th>n</th><th>TP</th><th>FP</th><th>FN</th><th>TN</th><th>prec</th><th>rec</th><th>F1</th><th>err px</th></tr>{"".join(row(n) for n in LINES)}</table>
<h2>Trend</h2>{trend_svg(history)}
<h2>Per-fixture (auto/human + err) + fate spot-check</h2><table><tr><th>fixture</th><th>heart</th><th>head</th><th>life</th><th>fate</th><th>fate_analysis</th></tr>{detail}</table>
<h2>Validation queue ({len(queue)})</h2><p>{q}</p>
<p><small>Method: <a href="../docs/admin_dashboard.md">docs/admin_dashboard.md</a> · human truth <code>*_USER_CORRECTED*.json</code> · history <code>output/admin_history.jsonl</code></small></p>
</body></html>"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--markdown", action="store_true")
    ap.add_argument("--no-history", action="store_true")
    args = ap.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    ts = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    cases, queue = load_cases()
    agg = aggregate(cases)

    snap = {"ts": ts, "agg": agg, "cases": cases, "queue": queue}
    (OUT_DIR / "admin_metrics.json").write_text(json.dumps(snap, indent=2))
    hist_path = OUT_DIR / "admin_history.jsonl"
    history = []
    if hist_path.exists():
        history = [json.loads(line) for line in hist_path.read_text().splitlines() if line.strip()]
    if not args.no_history:
        with hist_path.open("a") as f:
            f.write(json.dumps({"ts": ts, "agg": agg,
                                "n_cases": len(cases), "queue": queue}) + "\n")
        history.append({"ts": ts, "agg": agg})
    (OUT_DIR / "admin_dashboard.html").write_text(html(ts, cases, agg, queue, history))

    if args.markdown:
        print(markdown(cases, agg, queue))
    else:
        print(f"cases={len(cases)} queue={len(queue)} -> output/admin_metrics.json + output/admin_dashboard.html")


if __name__ == "__main__":
    main()
