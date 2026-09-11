# CurvStudio Operator Guide

> **Lost?** Draw in studio → Save Truth → Compare → Retune engine → Repeat.
> Do **not** train a neural net until ~50–100 labels have `reviewed: true`.

CurvStudio is the labeling + comparison desk for every domain (palm creases,
flood lines, and future packs). Same drawing tools; different class names and
defaults come from [`configs/domain_registry.json`](../configs/domain_registry.json).

---

## Cheatsheet

| Step | Action | Command / UI |
| ---: | --- | --- |
| 1 | Open studio | `pnpm studio` → http://localhost:8080/apps/annotation_studio/ |
| 2 | Pick domain | Header tabs: Palm or Flood |
| 3 | Pick sample | Filmstrip: `gold` = phone/satellite truth set; `bootstrap` = imported |
| 4 | See proposal | Dashed lines = engine. Empty? open **Test Bench** → Run |
| 5 | Correct | Polyline / polygon / brush / wand / Absent (`A`). **Save Truth** (`Cmd+S`) |
| 6 | Compare | Header **Compare**: Source \| Engine \| Yours (synced zoom) |
| 7 | Fix engine | Many FN/FP → retune Steger/linker, re-run inference — do not redraw everything |
| 8 | Split | **Dataset Split** when a domain has enough reviewed labels → `manifest.json` |
| 9 | Train ML | Only after the reviewed-count gate (see `docs/palm_robust_roadmap.md`) |

---

## Step-by-step

### 1. Open studio

```bash
pnpm studio
# optional hot reload for React UI:
pnpm studio:vite
```

Legacy palm annotator (`python3 tools/palm_annotator/server.py`, port 8765) is
**deprecated**. Use CurvStudio only.

### 2. Pick domain

Same app, different classes:

- **Palm** — Heart / Head / Life / Fate (`norm_512` warp space)
- **Flood** — Waterline / Flood Boundary / River Centerline / Bank (`pixel` space)

Flood defaults polygon for inundation boundary and polyline for centerlines;
palm stays polyline-first. Tool engine is shared.

### 3. Pick a sample

Filmstrip badges:

| Badge | Meaning |
| --- | --- |
| green check | Human truth saved (`*_USER_CORRECTED.json`) |
| `gold` | Primary dataset dir (phone gold / Nepal processed) |
| `bootstrap` | Extra import dirs (Roboflow raw) |
| `T` / `V` / `E` | Train / Valid / Test from `manifest.json` |

### 4. Look at the engine proposal

- **Draw** mode: AI ridges are dashed overlays on the same canvas.
- Layers panel: toggle Source / AI / Ground Truth + TP/FP/FN metrics.
- If no AI lines: **Test Bench** runs the domain’s `inference_cli` from the registry
  (palm → `hand_inspector_cli`, flood → `curv_cli --domain geospatial`).

### 5. Correct and save

1. Select a class (keys `1`–`9`).
2. Draw with the class default tool, or switch tools (`L` polyline, `P` polygon,
   `B` brush, `S` wand snap, `R` bbox, `H`/Space pan).
3. Mark missing features with **Absent** (`A`).
4. **Save Truth** (`Cmd+S`) writes `*_USER_CORRECTED.json` with `reviewed: true`.

### 6. Compare (side-by-side)

Toggle **Draw | Compare** in the header.

| Pane | Shows |
| --- | --- |
| **Source** | Photo (optional faint import ghost); Flood may offer pre/post Sentinel |
| **Engine** | Photo + dashed AI ridges (“what the pipeline proposes”) |
| **Yours** | Photo + solid human polylines (“what you confirmed”) |

Pan/zoom stay locked across panes. Metrics under the panes reuse the same
TP/FP/FN / mean error / length ratio as the Layers panel.

After Test Bench inference on the current sample, studio jumps to **Compare**
so you can judge Source vs Engine vs Yours immediately.

### 7. Fix the engine, not the JSON

If Life line (or waterline) is systematically FN/FP across many samples:

1. Retune Steger / linker — palm: [`docs/palm_sweep.md`](palm_sweep.md);
   flood: geospatial Steger configs.
2. Re-run inference (Test Bench or CLI).
3. Re-save human labels only when the **human** line was wrong.

Batch health: `python3 scripts/admin_metrics.py` → open
`output/admin_dashboard.html`.

### 8. Split for ML (when ready)

**Dataset Split** → Train / Valid / Test percentages → writes `manifest.json`
under the domain dataset dir. Prefer **labeled only** once you have reviewed volume.

### 9. Train ML (deferred)

Do **not** start U-Net / MLP export until ~50–100 labels have `reviewed: true`.
See [`docs/training_data.md`](training_data.md) and
[`docs/palm_robust_roadmap.md`](palm_robust_roadmap.md) Week 3.

---

## How you gradually get better

```text
label 5–10 gold samples in Compare
  → admin_metrics.py
  → systematic miss? retune CV
  → wrong labels? fix in studio + Save
  → reviewed ≥ ~50–100? split → then ML
```

Palm and flood share this loop. Only class names, coordinate space, and
default tools differ — plug future domains via the registry + optional
`IDomainPack`, never a second studio.

---

## Related docs

- Product loop: [`docs/product_vision.md`](product_vision.md)
- Training strategy: [`docs/training_data.md`](training_data.md)
- Domain registry: [`configs/domain_registry.json`](../configs/domain_registry.json)
- Admin dashboard: [`docs/admin_dashboard.md`](admin_dashboard.md)
