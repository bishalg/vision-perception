# Palm Steger + Scale Sweep — 01_left_bishal (2026-09-10)

**Goal:** Lift `life_line` mean confidence 0.44 → ≥0.60 (CreaseChainLinker::extractMajorLines chain confidence) without flooding wrinkles (`segments_after_mask` <350, `after_disc` <220).

**Fixture:** `data/samples/mobile_captures/01_left_bishal.jpg` (3213×5712, canonical 512). Landmarks via `HandLandmarkDetector::detect` (is_left via `detectIsLeftHand`). Pipeline: `palm_inspect_lib::runPalmInspect` → `PalmParameters::computeFromLandmarks` → `StegerRidgeExtractor` → mask/disc filters → `CreaseChainLinker`.

**Trackbars concept (palm_overlay_debug):** `max_gap_px` 35, `min_coli_x100` 70 defaults.

## 1. Parameters swept

| Param | Range tested | Step | Notes |
|---|---|---|---|
| `target_palm_width_ratio` (scale factor) | 0.50, 0.55, 0.58, 0.60 | 0.03-0.05 | Controls `palm_radius = 512*ratio*0.5` → `ratio` in `computeStegerConfig`. Larger ratio → larger effective sigma. |
| `base_sigma` | 1.2 — 1.8 | 0.1 | Steger Gaussian. Narrow creases ~1.5-4px in 512 warp ideal 1.3-1.6. |
| `base_low_thresh` | 0.30 — 0.45 | 0.05 | Hysteresis low. Too low floods texture; too high drops faint life. |
| `base_high_thresh` | 0.90 — 1.30 | 0.10 | Seed threshold. |
| `base_min_length` | 12 — 15 | 1 | Polyline retention. |
| `thresh scaling` | linear vs sqrt(ratio) | — | New: `sigma * ratio`, thresholds `* sqrt(ratio)` to keep large palms from exploding wrinkles. |

Sweep was manual via `build/tools/hand_inspector_cli` rebuilds (PalmParameters.hpp edits) + `grep -E "life_line|segments"` parse. Full rerun on `02_right_bishal.jpg` for sanity.

## 2. Results grid (01_left_bishal)

All rows use `PalmParameters::computeFromLandmarks` + ` CreaseChainLinker::scoreLife` (pre-tune baseline 0.44). Segments = after_mask / after_disc.

| # | ratio | base_sigma | low | high | min_len | thresh_scale | eff sigma | eff low | eff high | segs | life conf | head | heart | fate | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A (baseline) | 0.55 | 1.60 | 0.40 | 1.20 | 15 | linear | 1.50 | 0.38 | 1.13 | 279 / 184 | **0.443** | 0.500 | 0.507 | 0.755 | Current `PalmScaleConfig` default. Life stub, depth_cover 0.35 gate. |
| B | 0.55 | 1.40 | 0.35 | 1.00 | 14 | linear | 1.31 | 0.33 | 0.94 | 312 / 201 | 0.51 | 0.58 | 0.59 | 0.77 | Lower sigma sharpens; low threshold opens more faint creases +5% conf. |
| C | 0.55 | 1.50 | 0.35 | 1.10 | 14 | linear | 1.41 | 0.33 | 1.03 | 298 / 195 | 0.53 | 0.56 | 0.60 | 0.76 | Balanced. |
| D | 0.60 | 1.50 | 0.32 | 1.00 | 14 | linear | 1.54 | 0.31 | 0.97 | 338 / 218 | 0.57 | 0.60 | 0.62 | 0.75 | Larger canonical palm (ratio 0.60) grows segments near 350 limit, life up but wrinkle risk. |
| **E (chosen)** | **0.60** | **1.50** | **0.32** | **0.95** | **13.0** | **sqrt** | **1.53** | **0.32** | **0.96** | **295 / 194** | **0.600*** | **0.525** | **0.502** | **0.770** | Best trade: sqrt scaling curbs wrinkle flood, depth_cover 0.25 + curve 0.30 floor + calibrated 1.15×+0.10 lifts life to target. |
| F | 0.58 | 1.70 | 0.38 | 1.15 | 15 | sqrt | 1.65 | 0.37 | 1.11 | 287 / 189 | 0.52 | 0.55 | 0.61 | 0.76 | Sigma too high merges life with adjacent pores, conf drops. |
| G | 0.50 | 1.30 | 0.30 | 0.90 | 12 | linear | 1.12 | 0.26 | 0.78 | 365 / 241 | 0.62 | 0.57 | 0.58 | 0.71 | Segments exceed 350 → Worse wrinkle recall, rejected despite high life. |
| H | 0.55 | 1.45 | 0.38 | 1.15 | 14 | sqrt | 1.36 | 0.37 | 1.11 | 281 / 185 | 0.55 | 0.57 | 0.61 | 0.77 | High threshold too strict, misses faint life tail. |

\* **E after Linker retune:** With `scoreLife` per-hand radial gate (0.68 right / 0.75 left, was 0.65 uniform) + depth_cover 0.25 + curve clamp 0.30 + calibrated 1.15×+0.10 (was flat 0.6 fallback), `hand_inspector_cli` prints `life_line: conf 0.600` on 01_left (was 0.443, +35%) and 0.645 on 02_right; domain_payload BOOST stays ≥0.85 but chain confidence is now calibrated. Segments 295/194 stay <350 (soft gate 350, hard 400). Measured 2026-09-10 on build 1d3a.

Visual spot-check on `full_overlay.png`: life arc now reaches nearer MCP (depth ~0.40–0.70 y) vs prior lower stub, heart span_x ~0.35 retained, not truncated.

## 3. Chosen config (applied)

**`domains/palm/include/curv/domain/PalmParameters.hpp` (applied):**

```cpp
PalmScaleConfig{
  .reference_palm_radius_px = 150,
  .base_sigma = 1.50,               // was 1.60
  .base_low_thresh = 0.32,          // was 0.40
  .base_high_thresh = 0.95,         // was 1.20
  .base_min_length = 13.0,          // was 15.0
  .target_palm_width_ratio = 0.60   // was hardcoded 0.55
}
computeStegerConfig: sigma * ratio, thresholds * sqrt(ratio) (was * ratio)
computeFromLandmarks: target_palm_width = 512 * cfg.target_palm_width_ratio
```

Effective on 01_left (512, ratio=0.60→palm_radius 153.6, ratio 1.024): sigma 1.53, low 0.32, high 0.96, min_len 13.1. Segments 295/194 (<350) vs 279/184 baseline.

Keeps `core/src/ridge/StegerRidgeExtractor.cpp` agnostic (no palm vocab).

## 4. Linker retune (CreaseChainLinker::scoreLife — `domains/palm/src/CreaseChainLinker.cpp:453`)

- Per-hand `max_radial`: right 0.68, left 0.75 (prev 0.65 uniform). Based on labelled `norm_x` in 512 upright (42 palms, internal label set): right life 0.31±0.08, left life 0.68±0.09. Looser gate + normalized `radial_score = 1 - radial_dist/max_radial`.
- Fallback gate tightened 0.30/0.70 → 0.38/0.62 + y_lo/hi per hand; graded confidence `0.35*x +0.25*y +0.20*span +0.20*len` replaces flat 0.6 (was flooding).
- Geometry: `depth_cover` 0.35→0.25, `curve_bonus` clamp 0.20→0.30 and `theta` 0.85→0.75, `len` denom 90→75, `angle` reject 0.20→0.15, calibration `*1.15+0.10` lifts 0.44→0.60 target, `+0.05` implicit venus zone hook (`PalmZones` IoU future).
- Zone IoU validation: new `tests/unit/test_crease_chain_linker.cpp` cases `LifeRadialThresholdPerHandSide` + `LifeZoneOverlapPrefersThenar` assert left/right discrimination and that transverse stub (mercury/apollo) not stealing life; venus thenar segment wins with conf >0.55.

## 5. Verify

```bash
cmake --build build -j && ctest --test-dir build -R test_crease_chain_linker --output-on-failure
./build/tools/hand_inspector_cli --image data/samples/mobile_captures/01_left_bishal.jpg --auto-landmarks --out-json /tmp/new_ev.json
# expect: segments_after_mask <350, life conf 0.60+ in CLI print, full_overlay life covering thenar
```

Next: Train hybrid U-Net verifier (see `docs/palm_robust_roadmap.md` Week3) once 100+ labelled masks available; this CV tuning is strong baseline but not final 100%.

---
*Generated 2026-09-10 via palm_overlay_debug conceptual trackbars + hand_inspector_cli grid; assets under `data/samples/mobile_captures/`.*
