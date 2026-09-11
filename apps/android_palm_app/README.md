# Android Palm Perception Native App (`apps/android_palm_app`)

Palm analysis app on the shared **`:overlay`** library plus palm plugin.

> Helper, not oracle: same contract as iOS — propose lines, human corrects
> fast, only confirmed geometry feeds the reading; corrections become labels.
> Principle: [`docs/product_vision.md`](../../docs/product_vision.md).

## Modules

- `:overlay` → `apps/mobile_overlay/android` (camera, HUD, guidance)
- `:app` → palm ViewModel, ML Kit tracker, crease canvas, reading sheet

## Building

```bash
./scripts/build_android_aar.sh
# Open apps/android_palm_app in Android Studio (API 26+)
```

## Crease overlays — status & when to touch

**Today:** colored Heart/Head/Life/Fate are still **landmark-guess guides** in
`PalmAnalysisViewModel.generateSplinesFromAnalysis` (same class of fake as old
iOS `generateLandmarkCreases`). Metrics keys `"heart"` / `"head"` do not match
engine keys `heart_line` / `head_line`.

**Do not rework Android creases in parallel with desktop tuning.** Desktop
quality + iOS native polyline path come first. Shared contract and checklist:

- [`docs/palm_crease_baseline.md`](../../docs/palm_crease_baseline.md) §6  
- [`docs/handover_palm_crease_ios.md`](../../docs/handover_palm_crease_ios.md)

**Later parity (copy iOS once proven):**

1. Ensure AAR/JNI JSON includes non-empty `points_image_norm` (C API already
   enriches on the landmarks-ROI path).
2. Parse `points_image_norm` on `MajorLineInfo`; map `heart_line` → `HEART`, etc.
3. Build `CreaseSpline` from those polylines; gate landmark geometry as fallback only.
4. Keep detector QA on desktop `*_full_overlay.png`, not Android UI screenshots.
