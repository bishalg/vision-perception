# Android Adapter (JNI)

Source for the Android integration of the C-ABI. Two pieces:

- [`curv_jni.cpp`](curv_jni.cpp) — JNI glue (`Java_org_curv_CurvPipeline_native*`).
  Zero-copy ingestion of CameraX direct `ByteBuffer`s (`GetDirectBufferAddress`),
  with up-front validation of pixel geometry and buffer capacity
  (`invalid_frame_geometry` / `buffer_too_small` error strings) before the raw
  pointer reaches the engine.
- [`CurvPipeline.kt`](CurvPipeline.kt) — Kotlin wrapper `org.curv.CurvPipeline`
  (`AutoCloseable`), loading `libcurv_capi.so`, returning the evidence JSON string
  or an `{"error": ...}` payload.

Status: source-complete and covered by the native C-ABI test suite; Gradle/NDK
project packaging (`platforms/android/`) is the next planned milestone (Phase C).
The NDK build uses [`cmake/toolchains/android-ndk.cmake`](../../cmake/toolchains/android-ndk.cmake)
(arm64-v8a, API 26, `c++_static`).
