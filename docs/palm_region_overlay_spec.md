# Live Palm Region Overlay — Spec v1 (P0)

> Real-time sci-fi region view on the iOS camera: dotted mount outlines +
> line corridors, dim and filtered. Live is UI feel only; analytic value
> lands post-capture (PDF-report print media, P1b).

## 1. Non-goals

- No line *detection* live. Corridors are landmark-anchored guide bands, not detections.
- No C++ on the live path. Geometry is pure Swift off the 21 Vision landmarks.
- No text labels live. Names/symbols appear only in the post-capture result view.
- No behavior change to capture gating, guidance, or analysis.

## 2. Geometry (norm landmark space, y-down, origin top-left)

Input: 21 points, MediaPipe order (wrist=0, thumb 1–4, index 5–8, middle 9–12,
ring 13–16, pinky 17–20). Helpers: `lerp(a,b,t)`, `palmDepth(mcp,t)=lerp(mcp,wrist,t)`.

### 2.1 Mounts (7 closed polygons, drawn dotted)

| Mount | Anchor construction |
| :--- | :--- |
| Jupiter | quad: indexMCP(5) → mid(indexMCP,middleMCP) top edge; depth to `palmDepth` t=0.22 |
| Saturn | quad: mid(indexMCP,middleMCP) → mid(middleMCP,ringMCP); depth t=0.22 |
| Apollo | quad: mid(middleMCP,ringMCP) → mid(ringMCP,pinkyMCP); depth t=0.22 |
| Mercury | quad: mid(ringMCP,pinkyMCP) → pinkyMCP(17) extended 0.5× finger gap outward; depth t=0.22 |
| Venus | ellipse-ish closed curve: thumbCMC(1) → thumbMCP(2) → lerp(thumbMCP,wrist,0.35) → lerp(wrist,indexMCP,0.35) → lerp(indexMCP,thumbMCP,0.45) → close |
| Luna | quad: lower-ulnar: lerp(wrist,pinkyMCP,0.25) → pinkyMCP side edge → down to wrist line → close |
| Mars (±, one band) | quad between Venus edge and Luna edge across mid-palm: `palmDepth` t=0.35…0.55, ulnar half |

Top edge of the four finger mounts = basal-crease line (MCP row). All
fractions are in unit palm-depth so geometry scales with hand size and works
mirrored (view layer mirrors for left hand via existing `mirror` logic).

### 2.2 Corridors (3 open dashed polylines, guide bands, not detections)

- Heart: pinkyMCP→ringMCP→middleMCP→indexMCP at `palmDepth` t≈0.08–0.12.
- Head: from `lerp(lifeOrigin,wrist,0.18)` across `palmDepth` t≈0.30–0.36.
- Life: wrap curve around Venus edge (same anchors as `fallbackLandmarkCreases` life).
- Corridor = center polyline; renderer draws it dashed + two faint parallel
  echoes at ±40% mount-depth offset (cheap: scale offsets toward wrist).

## 3. Look (sci-fi, dim, filtered)

- Stroke: 1pt, `dash:[2,5]`, round caps. Mount fill: ≤8% opacity same hue.
- Hue: `OverlayPalette.emerald` when `guidanceState.isReady`, else white 25–40%.
- Corridor echoes: 0.6pt, `dash:[1,6]`, half opacity of center line.
- Renderer: SwiftUI `Canvas` (no per-frame view churn), `allowsHitTesting(false)`.
- Anti-shimmer: EMA smoothing α=0.35 on landmark points inside the view;
  redraw follows the existing ~15 Hz metrics throttle cadence.

## 4. LOD + toggle

| Guidance | Overlay |
| :--- | :--- |
| `.searching` (no hand) | silhouette only (unchanged) |
| hand found, not ready | 7 mount outlines |
| `.ready` / countdown | mounts + corridors (if toggle on) |

- Corridor toggle: HUD pill, default **OFF** (`UserDefaults` `showPalmCorridors`,
  `@AppStorage`), persists across launches. One-line flip to default-on later.
- Toggle visible only while a hand is detected (no dead UI while searching).

## 5. Files (P0)

- NEW `apps/mobile_overlay/ios/Sources/CurvPalmOverlay/PalmRegionGeometry.swift`
  — pure geometry, no SwiftUI: `PalmRegions` struct (7 named polys + 3 corridors),
  `PalmRegionGeometry.regions(from:)`, `+scaffold` (simulator, no camera).
- NEW `apps/mobile_overlay/ios/Sources/CurvPalmOverlay/RegionOverlayView.swift`
  — Canvas renderer + EMA + LOD props.
- EDIT `apps/ios_palm_app/Sources/Views/MainScannerView.swift` — layer
  `RegionOverlayView` at zIndex 1 beside `HandOutlineOverlay`; add toggle pill.
- Mirror: geometry built right-hand; view applies `scaleEffect(x:-1)` when
  `detectedIsLeft` (same as `PalmSilhouetteView`).

## 6. Later (out of P0 scope)

- P1: post-capture analytic regions in `CaptureResultView` + region field in
  `predictionPayloadJSON` + mount labels/symbols.
- P1b: PDF-report print layout (multiple overlays + reading summary, share sheet).
- P2: Android Compose Canvas parity; C++ `PalmRegionMap` canonical polygons for
  constrained line scoring; dashboard evidence export.

## 7. Verify P0

1. `node --check` n/a (Swift) — `xcodebuild build` clean, zero warnings target.
2. `./scripts/run_ios_sim.sh` — scaffold regions render (mounts dotted, corridors
   toggle on/off works, no hand needed).
3. Device run (iPhone 17 Pro) — LOD transitions searching→found→ready; no
   shimmer; toggle persists across relaunch; capture flow unchanged.
4. Screenshots: sim scaffold + device live, attached to progress note.
