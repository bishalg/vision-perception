# Apple Adapter (Swift)

Source for the iOS/macOS integration of the C-ABI:

- [`CurvEngine.swift`](CurvEngine.swift) — Swift wrapper over the flat C-ABI
  (`Result<String, CurvError>`), BGRA pixel format default, mirroring the JNI
  semantics of the Android adapter.

Status: source-complete; SwiftPM package + `CurvEngine.xcframework` packaging
(device + simulator slices, via [`cmake/toolchains/apple-clang.cmake`](../../cmake/toolchains/apple-clang.cmake))
is the next planned milestone (Phase C).
