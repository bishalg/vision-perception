# Mobile / desktop palm crease fixtures

Real palm photos used to iterate Heart / Head / Life / Fate overlays on desktop
**before** trusting phone overlays.

**Living baseline (metrics + rubric + Android-later plan):**
[`docs/palm_crease_baseline.md`](../../docs/palm_crease_baseline.md)

```bash
python3 scripts/palm_crease_baseline_metrics.py --markdown
```

## Contents

| File | Notes |
| :--- | :--- |
| `01_left_bishal.jpg` | Left palm capture |
| `02_right_bishal.jpg` | Right palm capture |
| `*_canonical.png` | 512×512 upright warp |
| `*_canonical_overlay.png` | Named crease overlay on warp |
| `*_full_overlay.png` | Overlay on original photo (**judge success here**) |
| `*_evidence.json` | Includes `points_canonical_norm` + `points_image_norm` |
| `*_evidence_landmarks.json` | 21 MediaPipe-order landmarks used for the run |

## How to refresh overlays

```bash
cmake --build build --target hand_inspector_cli palm_overlay_debug

./build/tools/hand_inspector_cli \
  --image data/samples/mobile_captures/01_left_bishal.jpg \
  --auto-landmarks \
  --out-warp data/samples/mobile_captures/01_left_bishal_canonical.png \
  --out-overlay data/samples/mobile_captures/01_left_bishal_canonical_overlay.png \
  --out-full-overlay data/samples/mobile_captures/01_left_bishal_full_overlay.png \
  --out-json data/samples/mobile_captures/01_left_bishal_evidence.json

# Interactive HighGUI debugger (trackbars + keys o/l/r/d/h/s)
./build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg

# After refresh: append a dated snapshot in docs/palm_crease_baseline.md
python3 scripts/palm_crease_baseline_metrics.py --markdown
```

## Adding more phone captures

1. Export the upright still from the PalmScanner thumbnail / Files (or Android equivalent).
2. Drop as `0N_description.jpg` in this folder (aim for 3–6 photos).
3. Prefer also dumping 21 landmarks JSON from a desktop `--auto-landmarks` run.
4. Judge success on `*_full_overlay.png` (original photo), not only the 512 crop.

Do **not** check in UI screenshots of the result screen — the detector needs a raw palm photo.
