# Palm Line Robust 100% Roadmap — CV → Hybrid ML

**Date:** 2026-09-10
**Status:** Draft for review — implements user request: document phase-by-phase, day-by-day for 100% main-line detection before coding.
**Guiding principle:** [`product_vision.md`](product_vision.md) — detection serves the reading; human edits are first-class and become training labels.
**Context:** Desktop `hand_inspector_cli` on `data/samples/mobile_captures/01_left_bishal.jpg` is accurate for fingertip dots + hand outline (`adapters/opencv/src/HandBoundaryDetector.cpp:1`), but Heart/Head/Life/Fate crease polylines are wild-off (avg `conf 0.44-0.50` in `/tmp/test_evidence.json` vs `0.75` for fate). Outline uses solid binary mask; creases are low-contrast, variable-width valleys not ideal Steger ridges.

---

## 1. Diagnosis: Why CV-only is at its ceiling

| Component | What it does | Why it works for outline/dots | Why main lines drift |
|---|---|---|---|
| `core/src/ridge/StegerRidgeExtractor.cpp` | Hessian eigen-decomposition, sub-pixel centreline `sigma=1.5` | Fingers are high-contrast cylindrical ridges | Palm creases are **wide valleys** (width 1.5-4px in `512` warp), shallow gradient, skin texture adds false ridges; Steger `low_threshold 0.5` either misses faint life lines or floods with wrinkles |
| `domains/palm/src/PalmZones.cpp:5` + `CreaseChainLinker.cpp:520` `scoreHeart/Life` | Zones + hand-side radial test `is_left ? norm_x>0.30 : <0.70` | Zones correct for anatomy | Scoring is heuristic (length/tort/angle) without learned shape prior; a long wrinkle in Jupiter zone outscores a short true heart segment |
| `core/src/geometry/RoiWarper.cpp:11` `warpToCanonical` | `p0-p9` up vector + `p5-p17` width → `512` | Stable for `scale` | `0.55*target_dim` width is estimate; distance error → sigma mismatch on phone `~0.9m` vs desktop `0.4m` |
| `adapters/capi/src/curv_capi.cpp:483` palm path `nullptr` quality gate | Always emits domain payload even when `blur<25` | Keeps phone capture from hard fail | Lets 0.44-confidence lines surface as solid instead of dashed fallback (`CaptureResultView.swift:104`) |

**Verdict:** Keep CV for **outline + candidates**, but main-line **classification needs learned shape/appearance prior**. Pure CV tuning will not reach 100% on 12 phone fixtures — need ML segmenter or learned linker.

---

## 2. Target architecture: Hybrid (CV candidates → ML verifier)

```
YUV 8MP ──► Vision 21 landmarks (already) ──► RoiWarper 512 warp
          └─► Steger candidates (200-400 segments) ──┐
                                                     ├──► CreaseChainLinker (keep) but replace scorer with MLP/GNN
                                                     │    trained on labelled major-line masks
                                                     └──► OR: U-Net segmentation `palm_crease_unet.onnx` that outputs 4 heatmaps directly on 512 warp

Warp + mask stays in C++ (fast, explainable); major-line identity becomes ML.
Fallback dashed scaffold remains when ML conf <0.55 (so UI never empty, `PalmAnalysisViewModel.swift:397`).
```

**Why hybrid, not pure ML:** Outline/dots already `>95%` with CV+Vision; ML only for 4 lines keeps data needs modest (500-800 labelled palms) vs end-to-end hand detector (`HandLandmarkDetector.cpp:80` `is_left_hand`).

---

## 3. Success criteria for “100%”

* **Geometric:** On `data/samples/mobile_captures` + 20-phone holdout, all 4 lines rendered within `±8px` in `512` warp (`IoU>0.85` vs hand-labelled polyline) on `>95%` of frames; `gapCount<=1`.
* **Confidence calibration:** `PalmDomainPack` `confidence` correlates with `IoU` (reliability diagram slope `0.9-1.1`), not forced `1.0`. UI shows `≥0.70` as solid, `<0.55` dashed.
* **Latency:** `512` warp + Steger + ML <150ms on iPhone 17 Pro (`adapters/apple/CurvEngine.swift`).

---

## 4. Phase-by-phase, day-by-day (4 weeks)

### Week 1 — Ground truth & harness (no model yet)
* **D1:** Create `data/palm_labels/README.md` + `labelme` schema: 4 polylines `heart|head|life|fate` + `hand_mask` polygon. Label `01_left/02_right` + 10 new phone captures (3 distances × light). One annotator, second reviewer.
* **D2:** Add `tools/palm_label_export.py` → COCO + `512` warp masks; add `tests/unit/test_palm_label_io.cpp` round-trip.
* **D3:** Freeze metric `scripts/palm_crease_baseline_metrics.py --holdout` computes `IoU/chord error/conf` per line; commit `docs/palm_crease_baseline.md` snapshot (current `0.44` baseline).
* **D4:** Refactor `PalmAnalysisViewModel.swift:397` `fallbackLandmarkCreases` to log `is_left, scale, blur` to `os_log` for telemetry; add `test_cli_lib` smoke that `hand_inspector_cli --image 01_left --auto-landmarks` must emit 4 lines (even if dashed).
* **D5:** Review with you: confirm label quality, sign off on IoU target. **Exit gate:** `make label-check` passes.

### Week 2 — Steger + link tuning as strong baseline (still CV)
* **D8-9:** Sweep `PalmParameters::computeFromLandmarks` sigma `1.1-1.8` + `StegerConfig low 0.3-0.7` on holdout via `palm_overlay_debug` trackbars; grid search logs to `docs/palm_sweep.md`. Keep best that lifts avg `conf 0.50→0.60` without >350 segments.
* **D10:** Retune `CreaseChainLinker::scoreLife` radial threshold per hand side using labelled `norm_x` distribution; add zone IoU test `test_crease_chain_linker.cpp:27`.
* **D11:** Fix quality gate: re-enable `LaplacianQualityAnalyzer` but with `min_blur 18` for phone + annotate `confidence*=0.7` when `is_usable=false` (`curv_capi.cpp:483`).
* **D12:** Device smoke: `xcodebuild -project apps/ios_palm_app/PalmScanner.xcodeproj -scheme PalmScanner -destination 'platform=iOS,id=00008150...'` + `devicectl … --console` 20 captures; log `gapCount`. **Exit gate:** `life/head >0.60` median.

### Week 3 — ML verifier (core innovation)
* **D15-16:** Train `palm_crease_unet` (MobileNetV3 encoder, 4-channel head) on 70% labels (augment: warp scale/blur/tilt from `DeviceTiltProvider`). Or if data <300, train **MLP linker**: 11-dim segment features → 4-class `heart|head|life|other` (distilled from `CreaseChainLinker` features). Keep `core/` agnostic, put model in `domains/palm/` + `models/palm_unet.onnx`.
* **D17:** Export ONNX → CoreML `palm_crease.mlpackage` for iOS, `opencv_dnn` for desktop/android. Wire in `adapters/apple/Package.swift` as `palm_ml` product, `PalmDomainPack.cpp:89` loads via `HandLandmarkDetector` path.
* **D18:** Hybrid inference: Steger candidates → ML scorer replaces heuristic; fallback to heuristic if `max conf <0.55`. Benchmark `512` latency; quantize to `fp16` if >150ms.
* **D19:** Review: compare U-Net heatmap vs heuristic on holdout `IoU`; pick winner. **Exit gate:** `IoU>0.85` on `80%` holdout.

### Week 4 — Integration, E2E, hardening to 100%
* **D22:** Wire `SimplePalmReading.swift:1` to use ML `confidence` for template tiers; wire `CaptureResultView.swift:185` debug panel to show `IoU` estimate.
* **D23:** Add `tests/unit/test_palm_ml Linker` + `test_hand_inspector_lib` asserts 4 lines `conf>0.60` on `data/samples/mobile_captures`; add `ci.yml` job `hand_inspector_cli` smoke.
* **D24:** Field: 50 captures across 5 volunteers (light/tilt via `TiltRailView.swift:1`), log `gap/fork`; fix mask over-filter (`HandBoundaryDetector.cpp:96`).
* **D25:** Polish: `PalmSilhouetteView.swift:8` final beauty pass, `HandSide` auto vs manual toggle telemetry.
* **D26:** Final audit like `2026-09-10` fresh audit: `ctest 29/29`, `format --check`, `lint`, `coverage` (linux runner), device `BUILD SUCCEEDED` + `simctl/devicectl` stay alive `>9s`, `palm_crease_baseline_metrics.py --markdown` shows `conf 0.70+` median. Document in `docs/palm_robust_roadmap.md` and tag `palm-v1.0`.

---

## 5. Risks & mitigations

* **Data scarcity (<200 labels):** Use MLP linker not U-Net; augment via synthetic `synth_generator` curves (`tests/synth_lib.cpp:1`).
* **Latency on iPhone 17 Pro:** Keep Steger candidates <250 via `handBoundaryDetector` purge `95` already; quantize U-Net to 224×224 if needed.
* **Overfitting to Bishal hands:** Holdout must include 3+ donors from week 4 field.

---

## 6. How to track

* Kanban: `D1-D26` as GitHub issues with `palm-robust` label, linked to `docs/palm_robust_roadmap.md`.
* Daily `hand_inspector_cli` artifact: `canonical_overlay.png` + `evidence.json` committed to `data/samples/mobile_captures` for visual diff.
* Weekly demo to you: phone `devicectl ... --console` + desktop `overlay_palm.png` side-by-side.

Next step: I will commit this doc as `docs/palm_robust_roadmap.md` and open `Week1 D1` label task — approve to proceed?
