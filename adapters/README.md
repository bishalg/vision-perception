# Hardware & Technology Adapters

Concrete implementations of the core abstraction interfaces, plus the stable
foreign-function boundary. Everything here is swappable; the core depends only
on `core/include/curv/interfaces/`.

## Implemented

- **`capi/`**: the stable C-ABI shared library (`libcurv_capi`) — the single
  contract consumed by JNI, Swift, and the CLI tools. See its README for the
  lifecycle, status codes, and buffer rules.
- **`opencv/`**: anti-aliased sub-pixel overlay renderer + SVG vector export.
- **`opencv_dnn/`**: 21-landmark ONNX hand landmark detector (`cv::dnn`) with an
  anatomical heuristic fallback when no model is loaded.
- **`android/`**: JNI glue (`curv_jni.cpp`, zero-copy CameraX direct buffers) and
  the Kotlin wrapper `org.curv.CurvPipeline`. Gradle/NDK packaging planned (Phase C).
- **`apple/`**: Swift wrapper `CurvEngine.swift` over the C-ABI.
  SwiftPM/xcframework packaging planned (Phase C).

## Planned (placeholder directories)

- **`onnxruntime/`**, **`tflite/`**, **`mediapipe/`**: alternative inference backends.
- **`android_camera/`**, **`ios_camera/`**: native platform camera ingestion.
- **`file_image_source/`**: static image / directory batch frame ingestion.

## Milestone Status & Next Phase

- **Current Version**: `0.1.0`
- **Completed**:
  - Stable C-ABI (`libcurv_capi`) with exception-isolation, bounded input checks, and atomic frame tracking
  - OpenCV sub-pixel overlay renderer (`OverlayRenderer`) with 16x anti-aliasing and SVG vector exporter
  - OpenCV-DNN ONNX landmark detector (`HandLandmarkDetector`) with anatomical fallback
  - Android JNI native bindings (`curv_jni.cpp`) + Kotlin wrapper (`CurvPipeline.kt`)
  - Apple Swift native wrapper (`CurvEngine.swift`)
- **Next Phase (Milestone 7 / Phase C)**: Android Gradle/NDK packaging (`platforms/android/`), CameraX zero-copy demo app, and iOS SwiftPM / `CurvEngine.xcframework` distribution.

