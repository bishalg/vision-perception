# Handover: Palm label annotator + training-data loop

> **Date**: 2026-09-11  
> **Audience**: Next AI / engineer continuing labeling → compare → (later) ML  
> **Plan**: Palm Label Annotator (local web draw tool + Roboflow bootstrap)  
> **Do not** train U-Net / MLP yet — wait until ~50–100 labels have `reviewed:true`.

---

## 1. One-sentence status

**Download → draw color-tagged Heart/Head/Life/Fate lines → save `*_USER_CORRECTED.json` → admin dashboard compares auto vs human is built.** Raw images are gitignored; ML training is explicitly deferred. User still needs a `ROBOFLOW_API_KEY` to pull the ~100 bootstrap photos.

---

## 2. What was done (do not redo)

| Piece | Path | Notes |
| :--- | :--- | :--- |
| Download script | [`scripts/download_palm_labels.py`](../scripts/download_palm_labels.py) | `--limit 100`, CC-BY check (refuses NC), COCO keypoints → `palm_label_v1` |
| Label store | [`data/palm_labels/`](../data/palm_labels/) | `README.md`, `manifest.json`, `raw/` (gitignored JPEGs) |
| Provenance | [`third_party/SOURCE_PROVENANCE.md`](../third_party/SOURCE_PROVENANCE.md) | Roboflow dataset entry |
| Browser annotator | [`tools/palm_annotator/server.py`](../tools/palm_annotator/server.py) + [`static/index.html`](../tools/palm_annotator/static/index.html) | Local `http://127.0.0.1:8765` |
| Metrics scan | [`scripts/admin_metrics.py`](../scripts/admin_metrics.py) | Scans `mobile_captures` **and** `palm_labels`; imported vs reviewed queue |
| Menu | [`scripts/menu/labeling.mjs`](../scripts/menu/labeling.mjs) | Download / annotator / dashboard entries |
| Docs | [`docs/training_data.md`](training_data.md), [`docs/admin_dashboard.md`](admin_dashboard.md), [`data/palm_labels/README.md`](../data/palm_labels/README.md) | Loop + defer-ML notes |

### Annotator UX (already implemented)

- Tags `1–4` = Heart / Head / Life / Fate (colors `#F43F5E` / `#06B6D4` / `#10B981` / `#8B5CF6`); `5` = hand mask
- Click vertices, drag handles, Backspace undo, `A` absent, `S` save, `N`/`P` next/prev
- Overlays: imported/current solid + auto `*_evidence.json` dashed
- Save writes `reviewed:true` next to the photo (`palm_labels/` or `mobile_captures/`)

### Label schema (`palm_label_v1`)

```json
{
  "schema": "palm_label_v1",
  "space": "image_norm",
  "source": "roboflow_import|annotator",
  "reviewed": false,
  "heart": [[[x,y], ...]],
  "head": [], "life": [], "fate": [],
  "hand_mask": []
}
```

- Coords **0…1**, top-left. Empty array = line absent.
- Legacy phone gold (`01_left_bishal_USER_CORRECTED.json` without `reviewed`) counts as reviewed.
- Imports with `reviewed:false` stay in the **queue** until Save in the annotator.

---

## 3. What already existed (context — do not rebuild)

- CV pipeline: landmarks → 512 warp → Steger → `CreaseChainLinker` → 4 colored lines → `*_evidence.json`
- Phone gold: 2 photos in `data/samples/mobile_captures/`; **1** human-labeled (`01_left`); `02_right` still unlabeled
- iOS post-capture **edit** UI exists but does **not** export `USER_CORRECTED.json`
- Roadmap for ML: [`docs/palm_robust_roadmap.md`](palm_robust_roadmap.md) Week 3 (after labels)

---

## 4. How to run (next agent / user)

```bash
# 1. Bootstrap (~100 CC-BY images) — needs key, never commit JPEGs
export ROBOFLOW_API_KEY=rf_xxxxx
python3 scripts/download_palm_labels.py --limit 100
# If Universe license metadata is fuzzy after manual CC BY 4.0 check:
#   python3 scripts/download_palm_labels.py --limit 100 --force-license 'CC BY 4.0'

# 2. Draw / correct
python3 tools/palm_annotator/server.py
# open http://127.0.0.1:8765

# 3. Compare
python3 scripts/admin_metrics.py && open output/admin_dashboard.html

# Or via menu: pnpm dev → [LABELING]
```

Default Roboflow target: `palms-lf79v` / `palm-lines-recognition-lbedg` v1 (override with `--workspace` / `--project` / `--version`).

---

## 5. What to do next (priority order)

1. **User run download** with a real API key; confirm license is CC BY on the Universe page.
2. **Human-review** imported keypoints in the annotator (community labels are noisy — treat as proposals).
3. **Label `02_right_bishal`** + any new phone drops in `mobile_captures/` (gold for *this* camera).
4. Optional: batch-run `hand_inspector_cli --auto-landmarks` on `data/palm_labels/raw/*.jpg` so the annotator shows our auto lines too (write `*_evidence.json` beside labels).
5. After **~50–100 `reviewed:true`**: implement `tools/palm_label_export.py` (4-channel 512 heatmaps) → then MLP linker or U-Net per roadmap — **not before**.

---

## 6. Do not

- Commit `data/palm_labels/raw/` JPEGs or Roboflow zips
- Download CC-BY-NC / academic-only sets into the commercial tree
- Train U-Net on unreviewed imports
- Rebuild iOS annotation as the primary labeling path (desktop browser is intentional for volume)
- Edit the plan file under `.cursor/plans/` unless the user asks

---

## 7. Quick file map

```text
scripts/download_palm_labels.py     # ingest
tools/palm_annotator/               # draw/tag/save
data/palm_labels/                   # bootstrap labels + README
data/samples/mobile_captures/       # phone gold
scripts/admin_metrics.py            # compare + dashboard
scripts/menu/labeling.mjs           # pnpm dev entries
docs/training_data.md               # strategy
docs/palm_robust_roadmap.md         # later ML phases
```

---

## 8. Smoke check (already passed once)

- `python3 scripts/admin_metrics.py --markdown` → left hand scored; `02_right_bishal` in queue
- Annotator `/api/items` lists both fixtures; legacy left = `reviewed:true`; save API works
- Download `--help` works; actual Roboflow pull **not** run in-session (no API key in env)
