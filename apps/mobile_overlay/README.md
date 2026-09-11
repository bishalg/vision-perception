# Shared Mobile Camera Overlay Kit (`apps/mobile_overlay`)

Domain-agnostic real-time camera guidance + Liquid Glass HUD used by every Curv
mobile product (palm, vastu, and future apps).

## Layout

```text
apps/mobile_overlay/
├── ios/                          # SwiftPM package
│   ├── Package.swift
│   └── Sources/
│       ├── CurvCameraOverlay/    # Camera, guidance, HUD, OverlayProfile
│       ├── CurvPalmOverlay/      # Palm silhouette + landmark → SubjectPose
│       └── CurvVastuOverlay/     # Mandala reticle + floor_plan profile
└── android/                      # Gradle Android library (`org.curv.overlay`)
    └── src/main/java/org/curv/overlay/
        ├── camera/               # CameraX session + preview
        ├── guidance/             # CaptureGuidance, SubjectPose, photometrics
        ├── hud/                  # LiquidGlassHUD + CameraOverlayHost
        ├── profile/              # OverlayProfile / GuidanceCopy
        ├── palm/                 # Palm plugin
        └── vastu/                # Vastu plugin
```

## Contract

1. **Core overlay** never imports palm anatomy, Vastu zone names, or CurvEngine.
2. Apps supply an `OverlayProfile` + reticle content (`PalmSilhouette` / `VastuMandala`).
3. Apps map tracker output → `SubjectPose` (or use `SubjectPose.reticleFill` for document capture).
4. Deep perception stays in the app: landmarks ROI (palm) or quad ROI (vastu) via C-ABI.
5. **Overlay never decides.** It guides capture and renders proposals; the human
   edits and confirms in the app, and only confirmed geometry feeds prediction
   ([`docs/product_vision.md`](../../docs/product_vision.md)).

## Consuming apps

| App | Overlay plugin | Domain id |
| :--- | :--- | :--- |
| `apps/ios_palm_app`, `apps/android_palm_app` | Palm | `palm` |
| `apps/ios_vastu_app`, `apps/android_vastu_app` | Vastu | `floor_plan` |
