# Android Adapter (JNI)

Source for the Android integration of the C-ABI. Two pieces:

- [`curv_jni.cpp`](curv_jni.cpp) — JNI glue (`Java_org_curv_CurvPipeline_native*`).
  Zero-copy ingestion of CameraX direct `ByteBuffer`s. Exposes landmarks ROI and
  **quad ROI** for vastu / floor-plan apps.
- [`CurvPipeline.kt`](CurvPipeline.kt) — Kotlin wrapper `org.curv.CurvPipeline`
  (`AutoCloseable`), loading `libcurv_capi.so`.

Real-time capture guidance and Liquid Glass HUD live in
[`apps/mobile_overlay/android`](../../apps/mobile_overlay/android) (`org.curv.overlay`),
not in this adapter.

Status: source-complete; NDK packaging via [`scripts/build_android_aar.sh`](../../scripts/build_android_aar.sh).
