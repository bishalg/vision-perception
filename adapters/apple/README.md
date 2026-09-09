# Apple Adapter (Swift)

Source for the iOS/macOS integration of the C-ABI:

- [`CurvEngine.swift`](CurvEngine.swift) — Swift wrapper over the flat C-ABI
  (`Result<String, CurvError>`), BGRA pixel format default. Supports both
  **landmarks ROI** (palm) and **quad ROI** (vastu / floor-plan).

Real-time capture guidance and Liquid Glass HUD live in
[`apps/mobile_overlay/ios`](../../apps/mobile_overlay/ios) (`CurvCameraOverlay`),
not in this adapter.

Status: source-complete; SwiftPM package + `CurvEngine.xcframework` packaging
(device + simulator slices) via [`scripts/build_ios_xcframework.sh`](../../scripts/build_ios_xcframework.sh).
