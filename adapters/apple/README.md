# Apple Adapter (Swift)

Source for the iOS/macOS integration of the C-ABI:

- [`CurvEngine.swift`](CurvEngine.swift) — Swift wrapper over the flat C-ABI
  (`Result<String, CurvError>`), BGRA pixel format default. Supports both
  **landmarks ROI** (palm) and **quad ROI** (vastu / floor-plan).

Real-time capture guidance and Liquid Glass HUD live in
[`apps/mobile_overlay/ios`](../../apps/mobile_overlay/ios) (`CurvCameraOverlay`),
not in this adapter.

## Native XCFramework

```bash
# From repo root — builds device + simulator slices
./scripts/build_ios_xcframework.sh
# → build/CurvEngine.xcframework
```

**Prerequisite:** an OpenCV build/install that CMake can find when
`CMAKE_SYSTEM_NAME=iOS` (Homebrew macOS OpenCV is **not** enough). Until that
exists, CurvApple still compiles in Swift-only mode and the palm app uses
landmark crease fallbacks.

[`Package.swift`](Package.swift) auto-detects `build/CurvEngine.xcframework` and
adds a `curv_capi` binary target so `#if canImport(curv_capi)` compiles against
the C ABI. Without the XCFramework, CurvApple still builds in Swift-only mode
(`CurvEngine` throws `.nativeUnavailable`).

Palm major-line JSON includes `points_image_norm` (0…1, top-left) for overlays
on the captured photo — prefer those over landmark-guess guides.

Status: source-complete; SwiftPM package + `CurvEngine.xcframework` packaging
(device + simulator slices) via [`scripts/build_ios_xcframework.sh`](../../scripts/build_ios_xcframework.sh).
