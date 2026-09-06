# CMake Modules & Toolchains

- `compiler_warnings.cmake` — `vision_set_compiler_warnings(<target>)`: strict
  diagnostic set (`-Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion ...`),
  plus `-Werror`/`/WX` when the `VISION_WERROR` option is ON.
- `dependencies.cmake` — shared dependency discovery (OpenCV, nlohmann_json,
  spdlog; `find_package` first, FetchContent fallback).
- `toolchains/android-ndk.cmake` — Android NDK cross toolchain (arm64-v8a, API 26, `c++_static`).
- `toolchains/apple-clang.cmake` — Apple platform toolchain (iOS/macOS builds).

Root build options: `VISION_WERROR`, `VISION_SANITIZERS` (e.g. `address,undefined`,
`thread`), `VISION_COVERAGE` (llvm-cov instrumentation), `VISION_BUILD_BENCHMARKS`.
See `CMakePresets.json` for the `release` / `debug` / `asan` / `tsan` / `coverage` presets.
