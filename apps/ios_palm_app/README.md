# iOS Palm Scanner (`apps/ios_palm_app`)

> **Maintenance Mode**: This component is in maintenance mode since 2026-10-27. No active feature development; hygiene and security fixes only. See [`docs/roadmap_2026q4/06_palm_maintenance_roadmap.md`](../../docs/roadmap_2026q4/06_palm_maintenance_roadmap.md).
>
> Helper, not oracle: the app proposes lines, you correct them fast
> (**Edit** → drag/add/remove/relabel), then **Use these lines for reading**.
> Only confirmed geometry feeds the prediction. Corrections become training
> labels (`*_USER_CORRECTED.*`) so proposals improve over time.
> Principle: [`docs/product_vision.md`](../../docs/product_vision.md).

## Why you still saw "random" lines

`CurvEngine.xcframework` is **not linked** yet (needs OpenCV-for-iOS). Without it the
app falls back to landmark-guess creases. That is expected until native links.

## Edit lines for reading

On the result screen:

| Action | How |
| :--- | :--- |
| Reshape | **Edit** → drag vertex handles on the selected line |
| Reorder | Drag list rows (edit mode) |
| Reassign Heart/Head/Life/Fate | Tag menu on a row |
| Lock for reading | **Use these lines for reading** → `predictionReadySplines` / `predictionPayloadJSON()` |

Adjusted geometry is what future VLM/reading should consume — not the raw landmark guesses.

## Native engine (later)

```bash
# Requires iOS OpenCV install
OpenCV_DIR=/path/to/ios-opencv ./scripts/build_ios_xcframework.sh
# Package.swift auto-picks build/CurvEngine.xcframework
```

See [`docs/palm_crease_baseline.md`](../../docs/palm_crease_baseline.md) and
[`docs/handover_palm_crease_ios.md`](../../docs/handover_palm_crease_ios.md).

## Capture modes

Top capsule switches protocols live (`PalmScanMode`, `Sources/Models/PalmScanMode.swift`):

| Mode | What happens |
| :--- | :--- |
| **Still** | Classic single photo → crease analysis → reading. |
| **3D Orbit** | Guided 6-angle hand orbit → mount topography (see below). Shutter hides; keyframes auto-capture. |
| **Close-Up** | Macro-focus mosaic tiles for deep creases. |
| **Fingertips** | Macro-focus fingertip captures for dermatoglyphic patterns. |

## 3D Orbit Scan (mount topography)

Guided 6-bin orbit — Flat → Tilt Left → Tilt Right → Thumb In → Thumb Out →
Fingers Up — that builds a per-mount height map for the Samudrika reading.

**How to scan:** prop the phone up or hold it steady — only the hand moves.
Show your open palm and hold ~1 s per step until the ring turns green. The
phone's own tilt is never measured, so left and right hands work identically.

**Hand-measured gates** (`OrbitCaptureGuidance`, shared overlay kit): tilt
bins compare the hand's MCP-axis rotation against the flat reference (≥ 8°);
thumb/fingertip bins compare fingertip excursion. The rigidity reference
refreshes every bin, and setup blockers coach specifically (`fitHand`,
`moveCloser`, `addLight`) instead of a silent 0%.

**Depth sourcing, automatic fallback:**
1. `orbit_depth_v1` — hardware depth (LiDAR/disparity `AVDepthData`) sampled
   at mount anchors on keyframe stills only (≤ 6/scan, 256 px, never streamed).
2. `orbit_metric_v1` — multi-view parallax in millimetres (80 mm metacarpal
   scale prior, gravity-measured viewpoint rotation).
3. `orbit_tilt_proxy_v1` — legacy tilt-exposure fallback, no millimetres.

**Engine path:** `OrbitCaptureSession` → `curv_orbit_session_v1` JSON →
`curv_analyze_orbit_session_json` (C-ABI) → `MountReliefAnalyzer` (parallax via
`core/surface/SurfaceRelief::estimateMetricRelief`) → per-mount
`height_mm` / prominence (`elevated` / `average` / `deficient`).

**Orbit UI** (`Sources/Views/MainScannerView.swift`): mode capsule + direction
chip at top (below the notch, outside the non-interactive HUD reticle);
center arrow + live badge; bottom scan card with progress, angle pips,
one plain instruction at a time, and Restart. The Lux/Glare/Tilt telemetry
row is unchanged.

**Tests:** `apps/mobile_overlay/ios/Tests/CurvPalmOverlayTests/OrbitCaptureTests.swift`
(steady-phone tilt fill, gate strictness, depth sampler, metric/proxy paths).
