# iOS Palm Perception Native App (`apps/ios_palm_app`)

High-performance native iOS application delivering real-time camera feed palm analysis with a Liquid Glass HUD overlay and on-device Steger sub-pixel crease extraction.

## Architecture

- **Tier 1 (Real-Time 60 FPS Preview Loop)**:
  - **AVFoundation**: `AVCaptureSession` capturing 60 FPS 1080p frames (`kCVPixelFormatType_32BGRA`).
  - **Apple Vision Framework**: `VNDetectHumanHandPoseRequest` running directly on the Apple Neural Engine (ANE) with 0 MB added model overhead.
  - **Capture Guidance Engine**: 5-gate evaluator checking scale ($0.35 - 0.60$), centering, roll tilt ($\le 22^\circ$), flatness, skin darkness ($Y \ge 70$), and glare hotspot ratio ($\le 4\%$).
  - **Liquid Glass HUD**: SwiftUI `.ultraThinMaterial` overlay with status pills, dynamic telemetry gauges, and glowing emerald auto-capture aura (`#32D74B`).
  - **Haptics**: `UIImpactFeedbackGenerator` providing physical feedback on ready alignment.

- **Tier 2 (Asynchronous Deep Analysis ~40 ms)**:
  - High-res photo buffer dispatched to `DispatchQueue.global(qos: .userInitiated)`.
  - Ingested by `CurvEngine` C-ABI flat interface (`curv_pipeline_process_landmarks_roi`).
  - Catmull-Rom vector splines rendered in SwiftUI `Canvas`:
    - **Heart Line**: Crimson Rose (`#F43F5E`)
    - **Head Line**: Electric Cyan (`#06B6D4`)
    - **Life Line**: Emerald Green (`#10B981`)
    - **Fate Line**: Royal Violet (`#8B5CF6`)
  - **Samudrika Reading Sheet**: Liquid Glass modal displaying line metrics, tortuosity, and Vastu palm insights.

## Internationalization (i18n)

Native multilingual support across:
- **English (en)**
- **Nepali (ne)**
- **Hindi (hi)**

## Building & Integrating

1. Build the universal Apple XCFramework:
   ```bash
   ./scripts/build_ios_xcframework.sh
   ```
2. Open `apps/ios_palm_app` in Xcode or import as a Swift Package.
3. Link `CurvEngine.xcframework` into your target.
