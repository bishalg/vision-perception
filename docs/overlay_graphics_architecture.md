# Overlay Graphics Architecture

Central spec for every custom graphic drawn over camera output in Curv mobile
products. Authority order: this doc states principles; numbers live in
`apps/mobile_overlay/shared/shapes/*.json` (checked by
`scripts/check_shared_shapes.py`, gated in CI); process lives in
`docs/mobile_parity.md`.

## 1. The one rule

**Real-time landmark drawing stays fully native. Static/tutorial art is shared
geometry data rendered by native code.** No WebViews, no SVG renderers on
device — native Canvas keeps 60fps, zero bridge latency, offline operation,
and the animation control we already have.

| Tier | Contents | Renders with | Source of geometry |
| :--- | :--- | :--- | :--- |
| **Live** (per-frame) | `RegionOverlayView` mounts/corridors, `HandOutlineOverlay` skeleton, HUD + tilt rails, crease editor + loupe, camera preview | SwiftUI Canvas / Compose Canvas, per-frame | 21 live tracking points (MediaPipe order); math in `PalmRegionGeometry` (ports must stay fraction-identical) |
| **Shared static** | Searching silhouette, Vastu mandala, theme tokens | Native Canvas, drawn on state change only | `mobile_overlay/shared/shapes/*.json`, parsed once |
| **Native chrome** | Sheets, toggles, TopAppBar/toolbar, icons | SwiftUI / Material3 | Platform idiom (may differ; behavior must not) |

## 2. Shared spec contracts (`mobile_overlay/shared/shapes/`)

- `silhouette.json` — tutorial hand. Normalized 0..1 (x right, y down).
  Thumb-on-viewer-left base (matches `01_left_bishal.jpg` and mirrored
  selfies); mirror x for the opposite presentation, never mirror labels.
- `vastu_mandala.json` — 3×3 zone identity (`key/name/element`); layout
  metrics (insets, radii, type) stay platform-native on purpose.
- `overlay_theme.json` — canonical `#RRGGBB` colors, alphas, timings, gates.

Binding rules:

1. **Fill once, stroke per finger.** `closed_loop_order` chains end-to-start
   (gate-verified); fill the whole loop a single time, then stroke
   `fingers.*` / `thumb` / `base.*` for draw-in and pulse. Per-finger fills
   double-blend translucent alphas into seams.
2. **Dash lengths are iOS points; Android px is exactly 2×.** Density
   scaling, never drift — do not "fix" it.
3. **Theme hexes are law.** Resolved drifts live in the JSON's `_resolved`
   list; align code to JSON, never JSON to drifting code.
4. **Coordinate mapping is identical both sides:** `px = x * rect.width`,
   `py = y * rect.height`; mirror as `1 - x` on geometry only.
5. **Parsers stay thin:** segment move + cubic curves + hex colors only. No
   feature may require extending the JSON schema without updating the gate,
   both loaders (`GuideSpec.swift`, `GuidePaths.kt`), and this doc.

## 3. What stays duplicated on purpose (ports, not sharing)

`PalmRegionGeometry`, `ImageCoordinateMapper`, Catmull-Rom splines, HUD
behavior: near line-for-line Swift/Kotlin ports, locked by
`check_mobile_parity.py` + `PalmRegionGeometryTests` (iOS) with a JVM
golden-vector mirror still to do (Android has no test source set yet).
KMP stays deferred: the ports are in sync and tested; revisit only if drift
bites twice.

## 4. Test requirements for overlay changes

- iOS: `xcodebuild test -scheme CurvCameraOverlay-Package` (simulator;
  plain `swift test` cannot build the UIKit-gated targets on macOS).
- `./scripts/check_shared_shapes.py` and `./scripts/check_mobile_parity.py`
  must pass; new shared tokens need gate coverage in the same change.
- Visual changes to the silhouette need a before/after render check against
  `data/samples/mobile_captures/01_left_bishal.jpg` / `02_right_bishal.jpg`.

## 5. Adoption status

- Web CurvStudio hint preview: separate 220×250 artwork (`palmGuide.ts`),
  evidence-matched to captures, tested. Intentionally not unified with the
  mobile silhouette (different proportions/audience); shared principles
  (zoning, handedness, highlight) apply.
- Mobile silhouette + mandala: spec + loaders landed; views still draw
  hardcoded paths (identical numbers — the gate now pins them). Rewiring
  views to parse the spec is the tracked next step, both platforms in one
  change.
- Live region geometry: solid, no changes planned.
