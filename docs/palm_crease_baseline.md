# Palm crease baseline & progress log

> **Audience**: Future AI / engineers tuning Heart / Head / Life / Fate  
> **Rule**: Improve on **desktop** (`palm_overlay_debug` + fixtures) before trusting phone overlays.  
> **Do not** redo iOS capture UX / white landmark dots — those are done.

---

## 1. How to use this file

1. **Read the latest baseline snapshot** (§3) before changing `CreaseChainLinker`.
2. After a tuning pass, re-run the refresh commands (§4), fill a **new dated row** in §3, and note what changed in §5.
3. Judge success on `*_full_overlay.png` (photo space), not only the 512 crop.
4. Phone apps must consume `points_image_norm` from JSON — never invent new landmark-only guides as the product path.

Shared evidence contract (under `domain_payload.major_lines.<heart_line|head_line|life_line|fate_line>`):

```json
{
  "name": "heart_line",
  "present": true,
  "confidence": 0.9,
  "segment_ids": [12, 44],
  "arc_length_px": 173.0,
  "points_canonical_norm": [[0.2, 0.3]],
  "points_image_norm": [[0.41, 0.22]]
}
```

Normals are **0…1**, top-left origin (same as Vision / ML Kit landmarks). Prefer **`points_image_norm`** for drawing on the captured photo. Fate may be `present: false`.

---

## 2. Visual QA rubric (pass / fail)

On each fixture’s `*_full_overlay.png`:

| Line | Pass | Fail |
| :--- | :--- | :--- |
| **Heart** | Follows upper transverse crease below MCP row; meaningful pinky↔index span | On basal finger creases, Head band, or a short stub |
| **Head** | Mid-palm transverse crease; spans most of palm width | Truncated half-palm, or swapped with Heart |
| **Life** | Curves around thenar (thumb mound) toward wrist | Flat transverse fragment, or only a lower stub |
| **Fate** | Optional. Vertical center column if present | Claiming Life/Head fragments; short noise OK to drop |

**Target:** visually ~90% on both fixtures before treating mobile native overlays as shippable.

---

## 3. Baseline snapshots (append-only)

### 2026-09-09 — post bidirectional linker + scoring (current)

| Field | Value |
| :--- | :--- |
| **Label** | `baseline_2026-09-09_linker_v2` |
| **Fixtures** | `01_left_bishal`, `02_right_bishal` |
| **Artifacts** | `data/samples/mobile_captures/*_{full_overlay,canonical_overlay,evidence}.json` |
| **Code focus** | `CreaseChainLinker`: bidirectional `linkSegments`, PCA orientation, separated Heart/Head bands, geometry-aware chain pick |

**Quantitative (from evidence `points_canonical_norm` / metrics):**

| Fixture | Line | present | arc_px | segs | n_img | canon span_x | canon y range |
| :--- | :--- | :---: | ---: | ---: | ---: | ---: | :--- |
| 01_left | heart | yes | 195.6 | 2 | 86 | 0.320 | 0.355–0.524 |
| 01_left | head | yes | 170.9 | 4 | 91 | 0.376 | 0.640–0.722 |
| 01_left | life | yes | 135.6 | 4 | 78 | 0.192 | 0.413–0.688 |
| 01_left | fate | yes | 94.2 | 2 | 96 | 0.057 | 0.596–0.786 |
| 02_right | heart | yes | 110.8 | 2 | 110 | 0.229 | 0.553–0.631 |
| 02_right | head | yes | 171.1 | 3 | 66 | 0.346 | 0.635–0.744 |
| 02_right | life | yes | 216.5 | 6 | 75 | 0.110 | 0.587–0.999 |
| 02_right | fate | yes | 231.9 | 3 | 74 | 0.131 | 0.578–0.949 |

**vs pre-fix (same day, before bidirectional growth)** — approximate:

| Fixture / line | Before | After | Notes |
| :--- | :--- | :--- | :--- |
| 01 head span_x | ~0.17 | **0.38** | Mid-seed truncation fixed |
| 01 life | lower stub (~0.70–0.88 y) | arc ~0.41–0.69 y | Better toward MCP |
| 02 life arc_px | ~121 | **216** | More thenar coverage |
| 02 heart | shallower short | still shorter span than ideal | Watch ulnar/radial completeness |

**Honest visual grade:** Better than box heuristics / uni-directional chaining; **not yet ~90%**. Continue desktop iteration.

**Mobile status at this baseline:**

| Platform | Overlay source | Native engine |
| :--- | :--- | :--- |
| iOS | Prefers `points_image_norm`; landmark fallback gated; **Demo fixture + Import JSON + drag/reorder/reassign** for testing without XCFramework | XCFramework blocked on **OpenCV-for-iOS** |
| Android | Still landmark-guess guides (`generateSplinesFromAnalysis`) | AAR path exists; same JSON contract when native linked |

**iOS test path (no XCFramework):** open PalmScanner → **Desktop demo** (bundled Steger lines) or **Import** `*_evidence.json` after capture → **Edit** lines → **Use these lines for reading**. Details: [`apps/ios_palm_app/README.md`](../apps/ios_palm_app/README.md).

---

### Template for next snapshot

```markdown
### YYYY-MM-DD — <short label>

| Field | Value |
| :--- | :--- |
| **Label** | `baseline_YYYY-MM-DD_<slug>` |
| **What changed** | 1–3 bullets (linker / Steger / fixtures) |
| **Visual grade** | pass / partial / fail vs §2 rubric |

| Fixture | Line | present | arc_px | segs | n_img | canon span_x | canon y range |
| ... | ... | ... | ... | ... | ... | ... | ... |

**Delta vs previous baseline:** …
```

Or regenerate the metrics table:

```bash
python3 scripts/palm_crease_baseline_metrics.py
```

---

## 4. Refresh commands (golden path)

```bash
cmake --build build --target hand_inspector_cli palm_overlay_debug test_crease_chain_linker
./build/tests/test_crease_chain_linker

./build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg
# Keys: r re-run, d all ridges, s save; trackbars = gap / collinearity (press r after)

./build/tools/hand_inspector_cli \
  --image data/samples/mobile_captures/01_left_bishal.jpg \
  --auto-landmarks \
  --out-warp data/samples/mobile_captures/01_left_bishal_canonical.png \
  --out-overlay data/samples/mobile_captures/01_left_bishal_canonical_overlay.png \
  --out-full-overlay data/samples/mobile_captures/01_left_bishal_full_overlay.png \
  --out-json data/samples/mobile_captures/01_left_bishal_evidence.json

# Repeat for 02_right_bishal.jpg
python3 scripts/palm_crease_baseline_metrics.py
```

Add more **raw** phone stills under `data/samples/mobile_captures/` (see folder README). Do not commit UI screenshots.

---

## 5. Change log (engineering)

| Date | Change | Why |
| :--- | :--- | :--- |
| 2026-09-09 | Bidirectional `linkSegments` | Mid-palm seeds truncated Heart/Head/Life |
| 2026-09-09 | PCA orientation in `makeStats` | Endpoint `atan2(|dy|,|dx|)` mis-scored curved Life |
| 2026-09-09 | Separated Heart/Head depth bands + span scoring | Exclusive assignment theft / weak transverse span |
| 2026-09-09 | Geometry-aware chain selection | Longest chain ≠ correct crease |
| 2026-09-09 | C API: landmarks + mask/disc + `points_image_norm` | Mobile JSON was empty / lacked priors |
| 2026-09-09 | iOS: parse polylines; gate landmark fallback | Stop shipping anatomy guesses as detection |
| 2026-09-09 | iOS: Desktop demo + Import evidence + edit/reorder/reassign | Test Steger lines on device before XCFramework; prediction uses adjusted lines |
| 2026-09-09 | Tests: bidirectional, Life≠transverse, Fate gate | Lock regressions |

Tune primarily in [`domains/palm/src/CreaseChainLinker.cpp`](../domains/palm/src/CreaseChainLinker.cpp). Optional Steger sigma: [`PalmParameters.hpp`](../domains/palm/include/curv/domain/PalmParameters.hpp). **No Canny / U-Net. No palm vocabulary in `core/`.**

---

## 6. Platform roadmap (iOS now, Android later)

### Recommendation: **do Android after**, not in full parallel

| Approach | When | Pros | Cons |
| :--- | :--- | :--- | :--- |
| **Desktop + iOS first** (recommended) | Now | One quality loop; one JSON consumer to debug; Android copies a known-good path | Android stays on landmark guides longer |
| **Light parallel** (docs only) | Now | Android checklist ready; no duplicate bugs | No user-visible Android gain yet |
| **Full parallel code** | Only if two people | Faster calendar time | Double the polyline/coord bugs while desktop still moves |

**Why later is easier:** both apps already call the same C-ABI shape. Once desktop overlays pass §2 and iOS draws `points_image_norm` correctly, Android is mostly:

1. Parse `points_image_norm` on `MajorLineInfo` (keys `heart_line`, not `heart`).
2. Build `CreaseSpline` from those points; gate `generateSplinesFromAnalysis`.
3. Ensure JNI/AAR emits enriched JSON (same C API path as iOS).
4. Aspect-fit overlay mapping equivalent to iOS `ImageCoordinateMapper`.

### Android parity checklist (do not start until desktop baseline looks right)

- [ ] Confirm `./scripts/build_android_aar.sh` links `curv_capi` and returns non-empty `points_image_norm`
- [ ] Extend [`PalmAnalysisResult.kt`](../apps/android_palm_app/app/src/main/java/org/curv/palmapp/model/PalmAnalysisResult.kt) with `pointsImageNorm`
- [ ] Replace landmark geometry in [`PalmAnalysisViewModel.kt`](../apps/android_palm_app/app/src/main/java/org/curv/palmapp/controller/PalmAnalysisViewModel.kt) `generateSplinesFromAnalysis`
- [ ] Map `heart_line` / `head_line` / `life_line` / `fate_line` → `CreaseLineType`
- [ ] Draw in photo aspect-fit rect (match white landmark coords)
- [ ] Re-run same two fixtures via desktop; compare Android screenshot only as a smoke check — detector QA stays on `*_full_overlay.png`

### Blockers to clear first

1. Desktop visual ~90% on fixtures (§2).
2. iOS: OpenCV-for-iOS / `OpenCV_DIR` so `scripts/build_ios_xcframework.sh` succeeds (see [`adapters/apple/README.md`](../adapters/apple/README.md)).
3. Then Android AAR parity (usually simpler if Linux/Android OpenCV NDK build already works).

---

## 7. Related docs

| Doc | Role |
| :--- | :--- |
| [`handover_palm_crease_ios.md`](handover_palm_crease_ios.md) | Original iOS handover (capture done; creases were fake) |
| [`research/palm_next_phase.md`](research/palm_next_phase.md) | Product analysis phase (features / VLM); do not replace Steger |
| [`../data/samples/mobile_captures/README.md`](../data/samples/mobile_captures/README.md) | Fixture folder how-to |
| This file | **Living baseline + progress** for crease identity quality |
