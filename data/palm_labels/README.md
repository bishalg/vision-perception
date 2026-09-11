# Palm Labels — Bootstrap + Human Review

Scale directory for Heart / Head / Life / Fate polyline ground truth.
Phone gold fixtures still live in [`../samples/mobile_captures/`](../samples/mobile_captures/);
this folder holds the Roboflow bootstrap set and anything labeled in the
browser annotator.

## Layout

| Path | Committed? | Role |
| :--- | :---: | :--- |
| `raw/*.jpg` | no | Downloaded / dropped palm photos |
| `*_USER_CORRECTED.json` | yes (JSON only) | Imported or human-reviewed polylines |
| `*_evidence.json` | optional | Auto proposals from `hand_inspector_cli` |
| `manifest.json` | yes | Download provenance + sample index |
| `_download/` | no | Temporary Roboflow zip extract |

## Schema (`palm_label_v1`)

Compatible with `scripts/admin_metrics.py` `human_polys()`:

```json
{
  "schema": "palm_label_v1",
  "space": "image_norm",
  "source": "roboflow_import",
  "reviewed": false,
  "heart": [[[0.46, 0.32], [0.47, 0.33]]],
  "head": [],
  "life": [],
  "fate": [],
  "hand_mask": []
}
```

- Coordinates are **0…1**, top-left origin.
- `space`: `image_norm` (full photo) or `canonical_norm` (512 warp).
- Empty array = line absent.
- `reviewed: true` is set **only** when you Save from the drawing tool.
- Imported Roboflow keypoints are **proposals**, not gold — review before training.

## Download (~100 CC-BY samples)

```bash
export ROBOFLOW_API_KEY=rf_xxxxx   # never commit
python3 scripts/download_palm_labels.py --limit 100
```

Requires a Roboflow Universe API key. The script refuses non-CC-BY licenses
(CC-BY-NC is blocked by `docs/license_policy.md`). Images stay gitignored;
provenance is logged in `manifest.json` and `third_party/SOURCE_PROVENANCE.md`.

## Draw / correct

```bash
python3 tools/palm_annotator/server.py
# open http://127.0.0.1:8765
```

Keys: `1–4` line tags, `5` hand mask, click to place vertices, Backspace undo,
`A` mark absent, `S` save, `N`/`P` next/prev.

## Compare

```bash
python3 scripts/admin_metrics.py && open output/admin_dashboard.html
```

## Learning (deferred)

Do **not** train U-Net / MLP until ~50–100 labels have `reviewed: true`.
Heatmap export (`tools/palm_label_export.py`) is the next step after that gate —
see `docs/palm_robust_roadmap.md` Week 3 and `docs/training_data.md`.
