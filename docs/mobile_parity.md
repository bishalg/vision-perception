# Mobile parity: iOS → Android porting system

> **Status**: active. iOS leads, Android mirrors in parallel.
> **Machine gate**: `./scripts/check_mobile_parity.py` (CI job `mobile-parity`).
> This doc is the map and the process; the script is the enforcement.
> Related: [`handover_palm_crease_ios.md`](handover_palm_crease_ios.md) (older;
> its "Android — later, not full parallel" stance in §4C is superseded by this doc).

## 1. Why this exists

iOS fixes were landing with no record of what Android still had to copy, so
ports happened from memory and drift was invisible until screenshots diverged.
The fix is two cheap things:

1. **A correspondence map** (§2) — for any iOS file/behavior, exactly where
   the Android counterpart lives. No hunting.
2. **An automated contract** (§3) — the machine-checkable parts of parity
   (line identity/colors, tool + coaching sets, languages, hand side, file
   counterparts, per-screen strings). CI fails if a fix lands on one side
   only. What the script cannot check goes through the port workflow (§4).

Prose rots; the script doesn't. When they disagree, fix the code first, then
this doc.

## 2. File correspondence map

Same row = same feature. Names differ where the platform idiom differs
(e.g. sheets, trackers); behavior must still match.

### Palm app — screens

| iOS (`apps/ios_palm_app/Sources/`) | Android (`.../org/curv/palmapp/`) | Notes |
| :--- | :--- | :--- |
| `Views/MainScannerView.swift` | `view/MainScannerScreen.kt` | Live camera + overlays + thumbnail |
| `Views/CaptureResultView.swift` | `view/CaptureResultView.kt` | Review + crease list (also hosts iOS `HandOutlineOverlay`) |
| `Views/SamudrikaReadingSheet.swift` | `view/ReadingBottomSheet.kt` | Reading sheet |
| `Views/CreaseLineEditorView.swift` | `view/CreaseLineEditorScreen.kt` | Precision editor |
| `Views/CreaseSplineOverlayView.swift` | `view/CreaseSplineOverlayView.kt` | Spline rendering |
| `Views/ImageCoordinateMapper.swift` | `view/ImageCoordinateMapper.kt` | Aspect-fit/fill mapping |
| `Views/MagnifyingLoupeView.swift` | *(inline in `CreaseLineEditorScreen.kt`)* | Verified by checker |
| `Models/PalmAnalysisResult.swift` | `model/PalmAnalysisResult.kt` | Incl. `CreaseLineType` colors |
| `Models/SimplePalmReading.swift` | `model/SimplePalmReading.kt` | Rule-based reading text |
| `Models/LocalizedStrings.swift` | `model/LocalizedStrings.kt` | Both load the shared JSON below |
| `Controllers/PalmAnalysisViewModel.swift` | `controller/PalmAnalysisViewModel.kt` | Capture + analysis flow |
| `Controllers/VisionHandTracker.swift` | `controller/MLKitHandTracker.kt` | Platform tracker (Vision vs ML Kit) |

### Shared overlay kit (`apps/mobile_overlay/`)

Mirrors 1:1 by name except the aliases below. iOS: `ios/Sources/CurvCameraOverlay|CurvPalmOverlay|CurvVastuOverlay`;
Android: `android/.../org/curv/overlay/`.

| iOS | Android | Notes |
| :--- | :--- | :--- |
| `PalmOverlayProfile` | `PalmOverlay` | Profile constants |
| `DeviceTiltProvider` | `DeviceTiltMonitor` | Platform sensor API |
| `VastuOverlayProfile` | `VastuOverlay` | Zone info + profile |
| `VastuMandalaView` | `VastuMandala` | Strict `View` suffix is iOS-only |
| `CameraPreviewView` | `CameraPreview` | Same |
| `HandSideToggleView` | `HandSideToggle` | Same |
| `PalmSilhouetteView` | `PalmSilhouette` | Same |
| `TiltRailView` | `TiltRail` | Same |

Android-only by design (no iOS counterpart expected): `Theme.kt`,
`MainActivity.kt` (app shell / Material theme).

### Single sources of truth (edit once, both apps get it)

| Artifact | Path | Consumed by |
| :--- | :--- | :--- |
| UI strings (en/ne/hi) | `apps/mobile_overlay/shared/strings/palm_strings.json` | Both `LocalizedStrings` loaders; validated by `check_shared_strings.py` |
| Overlay shapes + theme | `apps/mobile_overlay/shared/shapes/*.json` | `GuideSpec.swift` / `GuidePaths.kt` loaders; validated by `check_shared_shapes.py` (loop chaining, code cross-check, theme hexes) |

If a fix introduces a user-visible string, color, or threshold that both apps
need, prefer adding it to a shared artifact over duplicating literals.

## 3. Parity contract

**Enforced by `scripts/check_mobile_parity.py` (CI fails on violation):**

1. **Line identity**: `CreaseLineType` name + hex color + display-key match
   exactly on both sides (heart/head/life/fate).
2. **Editor tools**: the `editor.tool*` key set referenced in both editor
   files is identical (5 tools).
3. **Editor coaching**: the `editor.coaching*` key set is identical.
4. **Languages**: `AppLanguage` codes + display names match (en/ne/hi).
5. **Hand side**: `HandSide` cases match (auto/left/right).
6. **File counterparts**: every palm app/overlay file has a counterpart per
   §2 (aliases + verified inline components included).
7. **Screen strings**: per-screen `tr()` key sets match for the scanner,
   result, and sheet pairs (modulo documented divergences below).

**Manual (reviewer checks during the port):** gesture behavior, thresholds,
timings, haptics, layout on small screens, and anything the script reports
as a WARNING — confirm it, then port it or record it below.

## 4. Port workflow (iOS fix → Android copy)

1. **Land the iOS fix** with a behavior-first commit message (what the user
   sees/feels, not just what moved). Reference this doc if Android work
   remains.
2. **Mirror on Android** using the §2 map — same behavior, native
   expression (e.g. toolbar ↔ TopAppBar, sheet detents ↔ ModalBottomSheet,
   SF Symbol ↔ Material icon). Native chrome may differ; behavior must not.
3. **Run both gates** before commit:
   ```bash
   ./scripts/check_shared_strings.py
   ./scripts/check_mobile_parity.py
   ```
4. **If the checker complains**: either complete the port, or — for a
   legitimate platform difference — add it to "Documented divergences" (§6)
   with a reason. Never silence the checker without a doc entry.
5. **Update the port queue** (§5): move the item to Done with the verifying
   evidence (checker output counts).

## 5. Port queue

Seeded 2026-09-12 from recent iOS commits; statuses verified against code
(states "verified" where the checker now locks it).

### Done (locked by checker)

| iOS change | Android status |
| :--- | :--- |
| Distinct line colors — heart `#FF2D55`, head `#007AFF`, life `#34C759`, fate `#AF52DE` (`ab245d8`) | Done — identical enum entries (verified: `line_types`) |
| Unified 5-tool editor bar: Adjust/Draw/Undo/Reset/Label (`8ffde95`, `ab245d8`) | Done — same 5 tool keys (verified: `editor_tools`) |
| State-aware coaching pill, 6 coaching strings (`8ffde95`, `ab245d8`) | Done — same 6 keys (verified: `editor_coaching`) |
| 4-point control splines + freehand redraw (`f242087`) | Done — `resampleToControlPoints(..., 4)` + freehand + smoothing both sides |
| AUTO hand detection + dynamic indicator (`1a110d1`) | Done — `HandSide.AUTO` + toggle + fallback selection both sides (verified: `handside`) |
| No internal telemetry on user screens (`9cd6265`) | Done — both show only confidence % + point counts, no tortuosity/arc-length |
| Shared en/ne/hi strings, single JSON | Done — both loaders read `palm_strings.json` (verified: `languages` + `check_shared_strings.py`) |
| iOS 16 traits instead of iOS 17 `.isToggle` (`66d2ddf`) | N/A (iOS API-level fix; deployment target is still iOS 16.3) |

### In flight / to verify

- Uncommitted work in both apps (camera session, view models, result/editor
  views, `palm_strings.json`, manifests, app icons): land, then re-run both
  gates before commit.
- `result.editPoints`, `result.manualBanner*`, `result.dragHint`,
  `result.selectedLine` and similar keys flagged unused by
  `check_shared_strings.py`: confirm dead and prune, or wire up.

## 6. Documented divergences (intentional, do not "fix")

| Divergence | Reason |
| :--- | :--- |
| iOS review toolbar uses Cancel **text** (`editor.cancel`); Android TopAppBar uses a **Close icon** | Platform idiom. Checker exception recorded in script. |
| Tracker implementations (Vision vs ML Kit) and file names | Platform API; 21-point contract is what must match. |
| Sheet presentation (detents vs ModalBottomSheet), nav chrome (toolbar vs TopAppBar), icons (SF vs Material) | Native expression of the same UX. |
| Android `Theme.kt` / `MainActivity.kt` have no iOS counterparts | App-shell/theming layer, not a feature. |
| `HandOutlineOverlay` is a separate file on Android, inline in iOS `CaptureResultView.swift`; `MagnifyingLoupeView` is a separate file on iOS, inline in Android's editor | Verified by checker (needle must remain in host file). |

## 7. Quick commands

```bash
./scripts/check_shared_strings.py   # shared JSON schema + key references
./scripts/check_mobile_parity.py    # iOS/Android contract + drift
```
