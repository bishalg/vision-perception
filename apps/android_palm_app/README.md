# Android Palm Perception Native App (`apps/android_palm_app`)

High-performance native Android Jetpack Compose application delivering real-time camera feed palm analysis with a Liquid Glass HUD overlay and on-device Steger sub-pixel crease extraction.

## Architecture

- **Tier 1 (Real-Time 30+ FPS Preview Loop)**:
  - **CameraX**: `Preview` + `ImageAnalysis` (30+ FPS, YUV_420_888).
  - **Google ML Kit Pose**: Hardware-accelerated hand landmark tracker running on GPU/NNAPI delegate.
  - **Capture Guidance Engine**: 1:1 mathematical parity with iOS (`PalmCaptureGuidance.kt`), evaluating scale (0.35–0.60), centering, roll tilt ($\le 22^\circ$), flatness, skin darkness ($Y \ge 70$), and specular glare hotspots ($\le 4\%$).
  - **Liquid Glass HUD**: Jetpack Compose frosted glass reticle, status pills, telemetry chips, and glowing emerald auto-capture aura (`#32D74B`).
  - **Haptics**: `HapticFeedbackConstants.CONFIRM` triggered on ready countdown completion.

- **Tier 2 (Asynchronous Deep Analysis ~40 ms)**:
  - Direct `ByteBuffer` from high-res `ImageCapture` dispatched to Kotlin Coroutines on `Dispatchers.Default`.
  - Zero-copy C-ABI ingestion via JNI bridge (`curv_jni.cpp` $\to$ `CurvPipeline.kt`).
  - Catmull-Rom vector splines rendered in Jetpack Compose `Canvas`:
    - **Heart Line**: Crimson Rose (`#F43F5E`)
    - **Head Line**: Electric Cyan (`#06B6D4`)
    - **Life Line**: Emerald Green (`#10B981`)
    - **Fate Line**: Royal Violet (`#8B5CF6`)
  - **Samudrika Reading Sheet**: Material3 `ModalBottomSheet` with Liquid Glass cards displaying line metrics, tortuosity, and Vastu insights.

## Internationalization (i18n)

Native multilingual support across:
- **English (en)**
- **Nepali (ne)**
- **Hindi (hi)**

## Building & Running

1. Build the native Android `.so` libraries:
   ```bash
   ./scripts/build_android_aar.sh
   ```
2. Open `apps/android_palm_app` in Android Studio.
3. Sync Gradle and run on an Android device or emulator with API 26+ (arm64-v8a or armeabi-v7a).
