# Admin Dashboard — Prediction Engine Health + Human Validation

> Purpose: see at a glance whether the prediction engine is moving in the
> right direction, where it is wrong (false matrix), and what humans still
> need to validate. One page, regenerated from fixtures — no backend.

---

## 1. What it answers

| Question | View | Source |
| :--- | :--- | :--- |
| Is detection improving over time? | Trend: per-line `F1`, mean point error (px), report agreement per run | `output/admin_history.jsonl` |
| Where are the false calls? | Confusion matrix per line: TP / FP / FN / TN on **presence** | auto `*_evidence.json` vs human `*_USER_CORRECTED*.json` |
| Is geometry close, not just present? | Mean min-distance auto→human (px in 512 space) + length ratio | `points_canonical_norm` vs human polyline |
| Is the report right? | `fate_analysis` spot values (present/broken/stops/age) + reading agreement | `domain_payload.fate_analysis` |
| What needs human review? | Validation queue: fixtures with evidence but no human correction | missing `*_USER_CORRECTED*.json` |
| Flood side same? | Surge/width + controls over time (hooks defined, wired when flood holdout lands) | `flood_delta.json` series |

False matrix is about **presence** (did we claim a line the human says isn't
there and vice versa). Geometry error is separate: a TP can still be
wild-off, which is why both are tracked.

## 2. How to regenerate

```bash
python3 scripts/admin_metrics.py            # snapshot + history append
python3 scripts/admin_metrics.py --markdown # table for docs/PROGRESS.md
open output/admin_dashboard.html            # the view (self-contained, no server)
```

- Snapshot: `output/admin_metrics.json` (one run, timestamped).
- History: `output/admin_history.jsonl` (one line per run — the trend source).
- View: `output/admin_dashboard.html` (tables + trend SVG, inline data).
- `output/` is gitignored build artifact; paste `--markdown` tables into
  `docs/PROGRESS.md` when a run is worth keeping.

## 3. Metric definitions (frozen v1)

- `human_present`: human polyline exists with ≥2 points (any sub-poly).
- `auto_present`: `major_lines.<line>.present == true`.
- TP/FP/FN/TN from those two booleans. `precision = TP/(TP+FP)`,
  `recall = TP/(TP+FN)`, `F1` likewise; `n=0` renders as `—`.
- `mean_err_px`: mean over auto points of min point-to-point distance to
  human polyline, ×512. Approximation (dense polylines); v2 uses
  point-to-segment.
- `len_ratio`: `auto arc_length_px / human arc length px` (human arc from
  polyline ×512). `—` when either side absent.
- Fate spot-checks read straight from `fate_analysis` (no human fate truth
  required beyond the polyline).

## 4. Human validation flow

1. Dashboard **queue** lists captures needing correction.
2. Human draws corrections (today: Markup on canonical warp) → saved as
   `data/samples/mobile_captures/<stem>_USER_CORRECTED*.json` (+`.png`).
3. Re-run the script: queue shrinks, matrix + trends update.
4. Prediction layer consumes only confirmed geometry
   (`predictionReadySplines`), so every validated item directly improves
   the next report. See [`docs/product_vision.md`](product_vision.md).
