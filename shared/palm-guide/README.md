# Shared Palm Guide Spec

Canonical palm-crease reference geometry. Single source of truth lives in
TypeScript (`apps/annotation_studio/...`, see below); this folder's JSON is
the generated artifact for native consumers.

Adoption status (honest):
- Web CurvStudio hint preview: adopted (imports the TS modules directly).
- Mobile searching silhouette (`mobile_overlay/.../PalmSilhouetteView.swift`,
  `PalmSilhouette.kt`): NOT yet consuming this spec — still hand-authored per
  platform. Unifying it is tracked work (per-finger paths, fill-once +
  per-finger strokes).
- Mobile live region overlay (`PalmRegionGeometry`): intentionally separate —
  it is landmark-driven (computed from the 21 live tracking points), not
  static art, and stays that way.

## Single source of truth

The artwork lives in TypeScript, not in this folder:

- Creases + highlight mapping — `apps/annotation_studio/src/core/palmGuide.ts`
- Hand segments (outline, fingers, thumb, contours, labels) — `apps/annotation_studio/src/components/Sidebar/Panels/guide/`

`palm_guide.json` in this folder is **generated**, never hand-edited:

```bash
cd apps/annotation_studio
pnpm guide:export   # == node export_guide.mjs
```

CI expectation: re-running the export produces no diff. If you retune a
crease or finger, run the export and commit the JSON alongside.

## Spec layout (`palm_guide.json`)

| Key | Contents |
|---|---|
| `viewBox` | `{width: 220, height: 250}` — divide x by 220, y by 250 for 0..1 landmark-style coordinates |
| `base_handedness` | `"right"` (thumb right, pinky left). Mirror x for left hands |
| `zones` | `finger_zone_max_y` (128), `crease_zone_min_y` (130), `palm_region_x` — creases must stay below/inside |
| `silhouette_path` | Closed hand outline incl. thumb — use as the **clip boundary** for crease rendering |
| `outline_upper_path` / `outline_lower_path` | Visible palm strokes (the gap between them is bridged by the thumb) |
| `thumb` | `{outerEdgeD, innerEdgeD, jointTick}` |
| `fingers` | Per finger `{id, label, labelAt, jointTicks}` — Mercury/Apollo/Saturn/Jupiter |
| `contours` | Hypothenar echo + wrist bracelets |
| `mount_labels` | Venus / Moon / Mars anchors |
| `creases` | Heart / head / life / fate `{id, label, description, path, labelAt}` |
| `fallback_colors` | Class colors when a domain pack omits them |

Anatomy contract: heart/head run near-horizontal with a slight tilt below the
MCP joints; life is a C-curve around the Venus mount (mirrors with handedness);
fate runs near-vertical, wrist → Saturn mount.

View default: the Studio preview opens thumb-left (`handedness="left"`) to
match canonical captures (`canonical_palm.png`) and mirrored front-camera
selfie previews. Use `handedness="right"` for true camera-facing right palms.

## Consuming on Android (Compose Canvas)

```kotlin
// Parse once (androidx.core:core-ktx PathParser), draw in normalized space.
val silhouette = PathParser.createPathFromPathData(spec.silhouette_path)
// Map viewBox px -> canvas: Offsets scaled by (canvas.w / 220, canvas.h / 250).
// Clip creases: canvas.clipPath(silhouetteScaled) before drawing crease paths.
// Left hand: scale(-1f, 1f) + translate(-width, 0f) around geometry only.
```

See `apps/android_palm_app/.../view/HandOutlineOverlay.kt` (landmark skeleton
overlay) — the static reference art slots in behind it using the same mapper.

## Consuming on iOS (SwiftUI)

```swift
// SwiftUI has no SVG-path loader; add the ~40-line M/L/C/Q/Z parser once
// (see CreaseSplineOverlayView.swift for the existing spline renderer),
// then: Path(spec.silhouette_path) as .clipShape() for the crease layer.
// Normalize with x/220, y/250 to match VisionHandTracker landmark space.
// Left hand: .scaleEffect(x: -1) on geometry layers, never on Text labels.
```

## Consuming on web

Import the TS modules directly (no JSON round-trip needed):

```tsx
import { PalmGuide } from './Panels/PalmGuide';
<PalmGuide highlightId={activeClassId} colorFor={colorFor} handedness="left" />
```
