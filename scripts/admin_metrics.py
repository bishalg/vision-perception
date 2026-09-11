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


LINE_COLORS = {"heart": "#F43F5E", "head": "#06B6D4", "life": "#10B981", "fate": "#8B5CF6"}


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
        vals = [(i, h["agg"][key]["f1"]) for i, h in enumerate(history) if h["agg"][key]["f1"] is not None]
        if not vals:
            continue
        coords = " ".join(f"{xy(i, v)[0]:.1f},{xy(i, v)[1]:.1f}" for i, v in vals)
        dots = "".join(
            f'<circle cx="{xy(i, v)[0]:.1f}" cy="{xy(i, v)[1]:.1f}" r="3.5" fill="{LINE_COLORS[key]}">'
            f"<title>{key} F1={v:.2f} · run {i + 1} · {history[i].get('ts', '')}</title></circle>"
            for i, v in vals)
        series += (f'<polyline points="{coords}" fill="none" stroke="{LINE_COLORS[key]}" stroke-width="2"/>'
                   + dots)
    xticks = " ".join(
        f'<text x="{xy(i, 0)[0]:.0f}" y="{H - 12}" fill="#71717a" font-size="10" text-anchor="middle">run {i + 1}</text>'
        for i in range(len(history)))
    legend = " ".join(f'<span class="legend" style="color:{c}">● {k}</span>' for k, c in LINE_COLORS.items())
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
    for suffix in ("_canonical.png", ".jpg", "_canonical_overlay.png", ".png", "_full_overlay.png"):
        for cand in (FIXTURE_DIR / f"{stem}{suffix}",):
            if cand.exists():
                return f"../data/samples/mobile_captures/{cand.name}"
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
    """All files in the fixture dir, grouped by hand stem."""
    inv: dict = {}
    for p in sorted(FIXTURE_DIR.iterdir()):
        if not p.is_file() or p.name.startswith("."):
            continue
        stem = stem_of(p.name)
        inv.setdefault(stem, []).append({
            "name": p.name,
            "repo": f"data/samples/mobile_captures/{p.name}",
            "href": f"../data/samples/mobile_captures/{p.name}",
            "kind": kind_of(p.name),
            "bytes": p.stat().st_size,
        })
    return inv


def inspect_cmd(stem: str) -> str:
    return (f"./build/tools/hand_inspector_cli --image data/samples/mobile_captures/{stem}.jpg "
            f"--auto-landmarks --out-json output/palm_{stem}.json "
            f"--out-full-overlay output/palm_{stem}_full.png")


SHELL = """<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
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
@media(max-width:640px){.kpis{grid-template-columns:1fr 1fr}}
</style></head><body><div class="wrap">
<div class="topbar"><h1>Curv Admin — palm prediction health<small>Generated @@TS@@ · auto vs human truth · source <code>scripts/admin_metrics.py</code></small></h1>
<div class="btns"><a class="abtn" href="admin_metrics.json" download>Snapshot JSON</a>
<a class="abtn" href="admin_history.jsonl" download>History</a>
<button id="exportReview">Export review log</button>
<a class="abtn" href="../docs/admin_dashboard.md">Method doc</a></div></div>
<nav class="tabs" id="tabs">
<button data-tab="overview" class="active">Overview</button>
<button data-tab="assets">Assets <span class="ct">@@NFILES@@</span></button>
<button data-tab="training">Training data <span class="ct">@@NQUEUE@@</span></button>
<button data-tab="results">Results <span class="ct">@@NCASES@@</span></button>
</nav>
<section class="tab active" id="tab-overview">@@OVERVIEW@@</section>
<section class="tab" id="tab-assets">@@ASSETS@@</section>
<section class="tab" id="tab-training">@@TRAINING@@</section>
<section class="tab" id="tab-results">@@RESULTS@@</section>
<footer>Presence TP/FP/FN/TN: did we claim a line the human says is / isn't there. Geometry error is separate — a TP can still be off. Human truth files: <code>*_USER_CORRECTED*.json</code> in <code>data/samples/mobile_captures/</code>. Review approvals live in this browser (localStorage) — export the log and keep it with the run.</footer>
</div>
<script>
const tabs=[...document.querySelectorAll("#tabs button")];
tabs.forEach(b=>b.onclick=()=>{tabs.forEach(x=>x.classList.remove("active"));b.classList.add("active");
document.querySelectorAll(".tab").forEach(s=>s.classList.remove("active"));
document.getElementById("tab-"+b.dataset.tab).classList.add("active")});
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
    f1s = [agg[n]["f1"] for n in LINES if agg[n]["f1"] is not None]
    mean_f1 = sum(f1s) / len(f1s) if f1s else None
    worst = min(LINES, key=lambda n: agg[n]["f1"] if agg[n]["f1"] is not None else 2)
    kpis = (
        f'<div class="kpi"><div class="v">{len(reviewed)}/{len(cases)}</div><div class="l">hands reviewed</div></div>'
        f'<div class="kpi"><div class="v">{len(queue)}</div><div class="l">need labeling</div></div>'
        f'<div class="kpi"><div class="v">{fmt(mean_f1)}</div><div class="l">mean F1 (presence)</div></div>'
        f'<div class="kpi"><div class="v" style="color:{LINE_COLORS[worst]}">{worst}</div>'
        f'<div class="l">weakest line · F1 {fmt(agg[worst]["f1"])}</div></div>')
    cards = ""
    for n in LINES:
        a = agg[n]
        cards += (
            f'<div class="card"><h3><span class="dot" style="background:{LINE_COLORS[n]}"></span>{n}</h3>'
            f'<div class="meter"><span>precision</span>{bar(a["precision"], LINE_COLORS[n])}<span>{fmt(a["precision"])}</span></div>'
            f'<div class="meter"><span>recall</span>{bar(a["recall"], LINE_COLORS[n])}<span>{fmt(a["recall"])}</span></div>'
            f'<div class="meter"><span>F1</span>{bar(a["f1"], LINE_COLORS[n])}<span>{fmt(a["f1"])}</span></div>'
            f'<div class="nums"><span>TP {a["tp"]} · FP {a["fp"]} · FN {a["fn"]} · TN {a["tn"]}</span>'
            f'<span>n={a["n"]}</span><span>err <span class="err {err_class(a["mean_err_px"])}">'
            f'{fmt(a["mean_err_px"], 1)}px</span></span></div></div>')
    qprev = (" ".join(f'<span class="badge amber">{s}</span>' for s in queue)
             if queue else '<span class="badge green">empty — all reviewed</span>')
    return (f"<h2>Health at a glance</h2><div class='kpis'>{kpis}</div>"
            f"<h2>Per-line quality (reviewed hands only)</h2><div class='cards'>{cards}</div>"
            f"<h2>Trend — F1 per run</h2>{trend_svg(history)}"
            f"<h2>Validation queue ({len(queue)})</h2><p>{qprev}</p>"
            f"<p class='muted'>Label the queued hands in the Training data tab, then re-run "
            f"<code>python3 scripts/admin_metrics.py</code> (or menu 4 → 7).</p>")


def assets_html(inv: dict, cases: list) -> str:
    reviewed = {c["stem"] for c in cases if c["reviewed"]}
    stems = sorted(inv)
    total = sum(len(v) for v in inv.values())
    out = [f"<h2>Asset manager — {total} files in <code>data/samples/mobile_captures/</code></h2>",
           "<p class='muted'>Every file behind the metrics. Open to preview, copy the repo path for scripts, "
           "download to share. Truth files (<span class='badge violet'>human truth</span>) are what the matrix scores against.</p>",
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
            f"<table><tr><th>file</th><th>kind</th><th>size</th><th></th></tr>{rows}</table></div>")
    out.append("</div>")
    return "".join(out)


def training_html(cases: list, queue: list) -> str:
    guide = (
        "<h2>Add a new hand (3 steps)</h2><div class='steps'>"
        "<div class='step'><b>1. Drop the photo</b> into <code>data/samples/mobile_captures/&lt;stem&gt;.jpg</code> "
        "(e.g. <code>03_left_mom.jpg</code>).</div>"
        "<div class='step'><b>2. Run detection</b> — copy, paste in terminal:<div class='cmd'>./build/tools/hand_inspector_cli "
        "--image data/samples/mobile_captures/&lt;stem&gt;.jpg --auto-landmarks "
        "--out-json output/palm_&lt;stem&gt;.json --out-full-overlay output/palm_&lt;stem&gt;_full.png</div></div>"
        "<div class='step'><b>3. Correct the lines</b> (Markup on the canonical warp, or edit the overlay debugger output) "
        "and save as <code>&lt;stem&gt;_USER_CORRECTED.json</code> next to the photo. Then menu 4 → 7 to rebuild this page.</div>"
        "</div>")
    qcards = ""
    if queue:
        qcards = "<h2>Needs labeling</h2><div class='stem-grid'>"
        for stem in queue:
            img = find_image(stem)
            thumb = (f'<a href="{img}"><img class="thumb" src="{img}" loading="lazy" alt="{stem}"></a>' if img else "")
            cmd = inspect_cmd(stem).replace("&", "&amp;").replace("<", "&lt;")
            qcards += (
                f"<div class='card'><h3>{stem} <span class='badge amber'>needs label</span></h3>{thumb}"
                f"<p class='muted'>Run, correct, save truth as <code>{stem}_USER_CORRECTED.json</code>:</p>"
                f"<div class='cmd'>{cmd}</div>"
                f"<button data-copy=\"{inspect_cmd(stem)}\">Copy inspect command</button> "
                f"<button data-copy=\"{stem}_USER_CORRECTED.json\">Copy truth filename</button></div>")
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
        rcards += (
            f"<div class='card' data-review=\"{c['stem']}\"><h3>{c['stem']} "
            f"<span class='badge green'>reviewed</span> <span class='muted rstate'></span></h3>{thumb}"
            f"<p>{chips}</p><button class='bApprove'>Approve ✓</button> <button class='bFlag'>Flag ✗</button>"
            f"<input class='note' placeholder='review note (stored in this browser)…'></div>")
    rcards += "</div>"
    return guide + qcards + rcards


def results_html(cases: list, agg: dict) -> str:
    mrows = "".join(
        f"<tr><td><span class='dot' style='background:{LINE_COLORS[n]}'></span>{n}</td><td>{agg[n]['n']}</td>"
        f"<td>{agg[n]['tp']}</td><td>{agg[n]['fp']}</td><td>{agg[n]['fn']}</td><td>{agg[n]['tn']}</td>"
        f"<td>{fmt(agg[n]['precision'])}</td><td>{fmt(agg[n]['recall'])}</td><td>{fmt(agg[n]['f1'])}</td>"
        f"<td><span class='err {err_class(agg[n]['mean_err_px'])}'>{fmt(agg[n]['mean_err_px'], 1)}px</span></td></tr>"
        for n in LINES)
    def fixture_row(c: dict) -> str:
        status = "reviewed" if c["reviewed"] else "queued"
        badge = ("<span class='badge green'>reviewed</span>" if c["reviewed"]
                 else "<span class='badge amber'>needs label</span>")
        cells = "".join(f"<td>{line_cell(c['per_line'][n])}</td>" for n in LINES)
        return (f"<tr data-stem=\"{c['stem']}\" data-status=\"{status}\">"
                f"<td><b>{c['stem']}</b><br>{badge}</td>{cells}<td>{fate_chips(c)}</td></tr>")

    frows = "".join(fixture_row(c) for c in cases)
    return (
        "<h2>Confusion matrix per line</h2>"
        "<table><tr><th>line</th><th>n</th><th>TP</th><th>FP</th><th>FN</th><th>TN</th>"
        "<th>prec</th><th>rec</th><th>F1</th><th>err</th></tr>" + mrows + "</table>"
        "<h2>Per-fixture detail</h2>"
        "<div class='toolbar'><div class='seg'><button data-f='all' class='on'>All</button>"
        "<button data-f='reviewed'>Reviewed</button><button data-f='queued'>Needs label</button></div>"
        "<input type='search' id='q' placeholder='filter by hand name…'></div>"
        "<table><tr><th>fixture</th><th>heart<br><small>auto/human</small></th>"
        "<th>head<br><small>auto/human</small></th><th>life<br><small>auto/human</small></th>"
        "<th>fate<br><small>auto/human</small></th><th>fate analysis</th></tr>"
        f"<tbody id='fixtureRows'>{frows}</tbody></table>")


def html(ts: str, cases: list, agg: dict, queue: list, history: list, inv: dict) -> str:
    page = SHELL.replace("@@TS@@", ts)
    page = page.replace("@@NFILES@@", str(sum(len(v) for v in inv.values())))
    page = page.replace("@@NQUEUE@@", str(len(queue)))
    page = page.replace("@@NCASES@@", str(len(cases)))
    page = page.replace("@@OVERVIEW@@", overview_html(cases, agg, queue, history))
    page = page.replace("@@ASSETS@@", assets_html(inv, cases))
    page = page.replace("@@TRAINING@@", training_html(cases, queue))
    page = page.replace("@@RESULTS@@", results_html(cases, agg))
    return page


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--markdown", action="store_true")
    ap.add_argument("--no-history", action="store_true")
    args = ap.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    ts = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    cases, queue = load_cases()
    agg = aggregate(cases)

    inv = inventory()
    snap = {"ts": ts, "agg": agg, "cases": cases, "queue": queue, "inventory": inv}
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
    (OUT_DIR / "admin_dashboard.html").write_text(html(ts, cases, agg, queue, history, inv))

    if args.markdown:
        print(markdown(cases, agg, queue))
    else:
        print(f"cases={len(cases)} queue={len(queue)} -> output/admin_metrics.json + output/admin_dashboard.html")


if __name__ == "__main__":
    main()
