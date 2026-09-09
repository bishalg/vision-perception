# Handover: Palm crease overlays → iOS (next agent)

> **Date**: 2026-09-09 (updated same day after linker v2 + iOS polyline wiring)  
> **Branch**: `dev` (ahead of `origin/dev`; **desktop + iOS work mostly uncommitted**)  
> **Audience**: Next AI / engineer continuing palm line work  
> **Living baseline**: [`docs/palm_crease_baseline.md`](palm_crease_baseline.md) — metrics, rubric, change log, Android-later plan.  
> **Do not** restart capture UX or landmark dots — those are done.

---

## 1. One-sentence status

**White hand landmarks work on iOS (live + post-capture).** Desktop Steger → chain → polylines are improved (bidirectional linker) but **not yet ~90%**. iOS **code** prefers `points_image_norm` and gates landmark fallback; **native XCFramework still blocked** on OpenCV-for-iOS, so the device may still fall back to landmark guides until that links. Android still uses landmark guesses — **port later** (see baseline §6).

---

## 2. What is already done (do not redo)

### iOS capture / landmarks (working)

| Piece | Path | Notes |
| :--- | :--- | :--- |
| Live camera + cm framing coaching | `apps/mobile_overlay/ios/`, `PalmAnalysisViewModel` | Wider scale band; “Move ~N cm closer/farther” |
| Live white dots | `MainScannerView` + `HandOutlineOverlay` | Same skeleton as result view |
| Post-capture re-detect | `PalmAnalysisViewModel.finishCapture` | Re-runs Vision on upright still (not video landmarks) |
| Upright bake | `uprightImage(from:)` | Rotates landscape stills into upright bitmap |
| Aspect-fit overlay mapping | `ImageCoordinateMapper.swift`, `CaptureResultView` | Dots/lines map into photo rect, not full view |
| Shared overlay kit | `apps/mobile_overlay/` | Camera HUD reusable; palm/vastu apps plug in |
| Polyline consumer (code) | `PalmAnalysisResult.creaseSplinesFromPolylines`, gated `fallbackLandmarkCreases` | Needs XCFramework for real data |

**Committed earlier:** `62e290c` — shared overlay kit + palm/vastu apps.  
**Uncommitted (must keep):** iOS capture/coordinate fixes + desktop Phase 1/2 + C API enrichment (see §7).

### Desktop Phase 1–2 (landed in working tree)

| Piece | Path | Notes |
| :--- | :--- | :--- |
| Reusable pipeline | `tools/lib/curv/tools/palm_inspect_lib.*` | `runPalmInspect()` |
| CLI | `hand_inspector_cli` → thin wrapper | Writes evidence + landmarks JSON |
| Debug GUI | `tools/palm_overlay_debug` | HighGUI; keys `o/l/r/d/h/s`; gap/coli trackbars |
| Landmark-relative naming | `CreaseChainLinker::extractMajorLines` | MCP→wrist depth; exclusive Heart/Head/Life/Fate |
| Bidirectional chaining + PCA scoring | `CreaseChainLinker.cpp` | Fixes mid-seed truncation; see baseline snapshot |
| Palm-disc filter | `filterGraphByPalmDisc` | Inflated disc; drops finger noise |
| Polyline JSON | `PalmDomainPack` + C API `enrichImageSpacePoints` | `points_canonical_norm`, `points_image_norm` |
| Fixtures + baseline metrics | `data/samples/mobile_captures/`, `scripts/palm_crease_baseline_metrics.py` | Append snapshots in baseline doc |

**Honest visual quality:** Better than uni-directional / box heuristics, **not yet ~90%** on real creases. Keep iterating on desktop before trusting phone overlays.

---

## 3. Root cause of “random lines” on iOS (read this)

1. `CurvEngine.xcframework` is **not linked** (needs OpenCV-for-iOS) → `CurvEngine` init throws `.nativeUnavailable` → landmark fallback.
2. Historically, `generateLandmarkCreases` drew anatomy guesses between MCPs. That path is now **`fallbackLandmarkCreases`** and must not silently ship as detection when native JSON has polylines.
3. Desktop good overlays = Steger dark valleys + `CreaseChainLinker` + `OverlayRenderer`.

```text
photo → 21 landmarks → 512 canonical warp
     → mask + palm-disc filter → Steger (extract_dark_lines)
     → extractMajorLines (landmark priors)
     → points_image_norm → iOS CreaseSpline (when XCFramework linked)
```

---

## 4. Recommended order of work (next thread)

### A. Finish desktop quality (before more mobile)

Goal: Heart/Head/Life sit on skin creases for `data/samples/mobile_captures/*` when judged on `*_full_overlay.png`.

Follow [`docs/palm_crease_baseline.md`](palm_crease_baseline.md) §2–4: tune, refresh fixtures, append a new baseline row.

```bash
cmake --build build --target hand_inspector_cli palm_overlay_debug test_crease_chain_linker
./build/tests/test_crease_chain_linker
./build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg
python3 scripts/palm_crease_baseline_metrics.py --markdown
```

Tune primarily in:

- [`domains/palm/src/CreaseChainLinker.cpp`](../domains/palm/src/CreaseChainLinker.cpp) — scorers + chaining  
- Optionally [`PalmParameters`](../domains/palm/include/curv/domain/PalmParameters.hpp) Steger sigma (**no Canny / U-Net**)

### B. Then iOS: ship real polylines on device

1. Provide OpenCV-for-iOS; run [`scripts/build_ios_xcframework.sh`](../scripts/build_ios_xcframework.sh) (`OpenCV_DIR=...`).
2. Confirm `Package.swift` picks up `build/CurvEngine.xcframework` and `usedNativeEngine` is true with non-empty polylines.
3. Keep `ImageCoordinateMapper` + aspect-fit. Do **not** invent new landmark-only guides.

### C. Android — **later** (not full parallel)

Same JSON contract. Mirror iOS polyline consumer once desktop + iOS native are proven. Checklist in baseline §6. Light parallel = keep that checklist updated only.

### D. Explicitly out of scope until A is good

- Reworking camera distance / countdown (done)  
- Vastu app  
- VLM prompt rewrite  
- Training a U-Net  
- Full Android crease rewiring before desktop QA passes  

Architecture: **no palm vocabulary in `core/`**.

---

## 5. Key file map

| Role | File |
| :--- | :--- |
| Living baseline | `docs/palm_crease_baseline.md` |
| Metrics dump | `scripts/palm_crease_baseline_metrics.py` |
| iOS capture + crease path | `apps/ios_palm_app/Sources/Controllers/PalmAnalysisViewModel.swift` |
| Vision 21 pts | `apps/ios_palm_app/Sources/Controllers/VisionHandTracker.swift` |
| Result UI | `CaptureResultView.swift`, `CreaseSplineOverlayView.swift`, `ImageCoordinateMapper.swift` |
| Swift Curv wrapper | `adapters/apple/CurvEngine.swift` (`#if canImport(curv_capi)`) |
| Android (later parity) | `apps/android_palm_app/.../PalmAnalysisViewModel.kt` |
| Desktop inspect | `tools/lib/curv/tools/palm_inspect_lib.*` |
| Debug GUI | `tools/palm_overlay_debug.cpp`, `palm_overlay_debug_lib.cpp` |
| Line identity | `domains/palm/src/CreaseChainLinker.cpp` |
| Domain + C API JSON | `PalmDomainPack.cpp`, `adapters/capi/src/curv_capi.cpp` |
| Product plan (older) | `docs/research/palm_next_phase.md` |
| This handover | `docs/handover_palm_crease_ios.md` |

---

## 6. Evidence JSON contract (iOS must parse)

Under `domain_payload.major_lines.<heart_line|head_line|life_line|fate_line>`:

```json
{
  "name": "heart_line",
  "present": true,
  "confidence": 0.9,
  "segment_ids": [12, 44],
  "arc_length_px": 173.0,
  "points_canonical_norm": [[0.2, 0.3], "..."],
  "points_image_norm": [[0.41, 0.22], "..."]
}
```

- Normals are **0…1**, top-left origin (same as Vision landmarks on iOS).  
- Prefer **`points_image_norm`** for drawing on the captured photo.  
- Fate may be `present: false` — that is OK.

`DomainInput` has optional `landmarks` pointer ([`IDomainPack.hpp`](../core/include/curv/interfaces/IDomainPack.hpp)) so naming uses MCP priors in canonical space. C API now passes landmarks, filters palm disc, and fills `points_image_norm`.

---

## 7. Git / workspace warning

At handover time, **desktop Phase 1–2 + iOS capture/polyline wiring are uncommitted** (and `dev` was already ahead by the overlay-kit commit). Next agent should:

1. `git status` / `git diff` — review before committing.  
2. Prefer **two commits** if user asks to commit: (a) desktop crease tooling + linker, (b) iOS capture/coordinate + polyline consumer — or one clear message if bundling.  
3. Large PNG overlays under `data/samples/mobile_captures/` — OK for fixtures; don’t commit UI screenshots.  
4. **Do not push** unless asked.

---

## 8. Success criteria for “ready for iOS polish”

- [ ] On `01_left_bishal` / `02_right_bishal` full overlays: Heart ≈ upper transverse crease, Head ≈ mid crease, Life hugs thenar (Fate optional).  
- [ ] Evidence JSON has non-empty `points_image_norm` for present lines.  
- [ ] New row appended in [`palm_crease_baseline.md`](palm_crease_baseline.md) after each meaningful tune.  
- [ ] `palm_overlay_debug` is the day-to-day tuning loop.  
- [ ] iOS result view draws **those** points; subtitle shows native engine when XCFramework linked.  
- [ ] Landmark fallback cannot silently ship as “detection.”  
- [ ] (Later) Android checklist in baseline §6 completed.

---

## 9. Suggested first prompt for next chat

> Continue from `docs/palm_crease_baseline.md` and `docs/handover_palm_crease_ios.md`. Improve desktop Heart/Head/Life on `data/samples/mobile_captures` via `palm_overlay_debug` / `CreaseChainLinker` until visual rubric passes; append a new baseline snapshot. Do not change iOS capture. Do not start Android crease wiring until desktop QA passes. When overlays look right, finish OpenCV-iOS XCFramework so iOS consumes live `points_image_norm`.

---

## 10. Quick commands cheat sheet

```bash
# Desktop
cmake --build build --target hand_inspector_cli palm_overlay_debug test_crease_chain_linker
./build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg
python3 scripts/palm_crease_baseline_metrics.py --markdown

# iOS app (XcodeGen)
cd apps/ios_palm_app && xcodegen generate
open PalmScanner.xcodeproj
# Team 5Q924U6M6X; run on device for camera
# Native: OpenCV_DIR=<ios-opencv> ./scripts/build_ios_xcframework.sh
```
