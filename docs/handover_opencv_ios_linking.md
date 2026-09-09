# Handover — Linking the native C++ CurvEngine (+ OpenCV) into the iOS app

**Date:** 2026-09-09
**Branch:** `dev` (uncommitted working-tree changes)
**Goal of this task:** Make live palm capture in `apps/ios_palm_app` use the real C++
Steger crease pipeline instead of the Vision-landmark fallback. Root cause: the C++
engine (and OpenCV) is never linked into the app.

This is **Phase 1 only**: get OpenCV-for-iOS built, make the CMake tree cross-compile
for iOS, package `CurvEngine.xcframework` + `opencv2.xcframework`, and link both into the
app so `cv::*` symbols resolve. **Runtime quality** (crease line accuracy, quality-gate
tuning for phone stills, orientation, JSON buffer sizing) is explicitly **Phase 2** and
not touched here.

Full design/rationale: `/Users/bishalghimire/.claude/plans/start-with-the-opencv-parsed-prism.md`

---

## Decisions already made (confirmed with user)

| Decision | Choice |
| :-- | :-- |
| OpenCV version for iOS | **4.11.0** (matches desktop Homebrew 4.x) |
| `dnn` module on iOS | **Dropped.** Original plan was "keep it", but OpenCV 4.11's dnn FP16 NEON kernels (`conv_winograd_f63.simd.hpp`, `vfmaq_f16`) fail to compile under the Xcode 26.6 clang. iOS uses Apple Vision for landmarks, so `dnn` + `HandLandmarkDetector.cpp` are excluded on iOS (see `adapters/CMakeLists.txt`). Desktop still builds dnn. |
| Storing `opencv2.xcframework` | **Generated + gitignored**, built on demand by `scripts/build_opencv_ios.sh`; not committed |
| Two XCFrameworks vs one merged | **Two separate** — `CurvEngine.xcframework` (thin C API `.a`) + `opencv2.xcframework` alongside |

---

## What has been DONE (code changes, all in working tree, NOT committed)

1. **`scripts/build_opencv_ios.sh`** (new, executable)
   Idempotent wrapper: clones OpenCV `4.11.0`, runs
   `platforms/apple/build_xcframework.py` for `iphoneos` + `iphonesimulator` (arm64
   only), modules kept = core/imgproc/imgcodecs/dnn, dropped = videoio/highgui/gapi/
   objc/python/java/ts. Publishes to `third_party/ios/opencv2.xcframework`.

2. **`CMakeLists.txt`**
   - Homebrew `CMAKE_PREFIX_PATH` entries now wrapped in `if(NOT CMAKE_SYSTEM_NAME
     STREQUAL "iOS")` — stops host macOS OpenCV/spdlog leaking into an iOS configure.
   - `find_package(CURL REQUIRED)` moved inside the non-iOS branch.
   - `enable_testing()` + `add_subdirectory(apps/desktop_pipeline_demo|tools|tests)`
     now guarded `if(NOT ... iOS)`. `core`/`adapters`/`domains` stay unconditional.

3. **`adapters/CMakeLists.txt`**
   - `curv_adapters` source list built via `${CURV_ADAPTERS_SOURCES}`; excluded on iOS:
     `HandLandmarkDetector.cpp` (dnn), `CctvStreamAdapter.cpp`, `CctvVideoSource.cpp`
     (videoio), `CctvHttpSnapshot.cpp` (curl). All still built on desktop.
   - `CURL::libcurl` link moved to a non-iOS `if` block.

4. **`scripts/build_ios_xcframework.sh`**
   - Calls `scripts/build_opencv_ios.sh` first.
   - Locates the per-slice build-tree `OpenCVConfig.cmake` under
     `third_party/ios/opencv-build-4.11.0/{iphoneos,iphonesimulator}/…` and passes it as
     `-DOpenCV_DIR=` for each slice (device vs simulator). Honors an explicit
     `OpenCV_DIR` env override.
   - After `xcodebuild -create-xcframework`, copies `opencv2.xcframework` into `build/`
     next to `CurvEngine.xcframework`.

5. **`adapters/apple/Package.swift`**
   - Adds a second `.binaryTarget` `opencv2` → `../../build/opencv2.xcframework`.
   - Native path gated on **both** frameworks existing (`nativeAvailable`); `CurvApple`
     depends on `["curv_capi", "opencv2"]` when available.

6. **`apps/ios_palm_app/project.yml`**
   - `PalmScanner` target gains `OTHER_LDFLAGS: "-lc++ -framework Accelerate"` and
     `DEAD_CODE_STRIPPING: YES` (OpenCV static core needs libc++ + Accelerate explicit).

7. **`adapters/capi/include/module.modulemap`** — `git add`-ed (was untracked; required
   for `import curv_capi` in `adapters/apple/CurvEngine.swift`).

8. **`.gitignore`** — ignores `third_party/ios/opencv2.xcframework/`, `opencv-src-*/`,
   `opencv-build-*/`.

9. **`third_party/SOURCE_PROVENANCE.md`** — OpenCV 4.11.0 (Apache-2.0) entry added.

---

## CURRENT STATE (where the next agent picks up)

**OpenCV build: root cause found and fixed in the script; rebuild not yet re-run to
completion.**

- Host has **Xcode 26.6 (build 17F113)**, far newer than OpenCV 4.11.0 (Dec 2024).
- Two failed runs, both `xcodebuild ... ALL_BUILD` **exit 65**. Real error (captured):
  ```
  modules/dnn/src/layers/cpu_kernels/conv_winograd_f63.simd.hpp:1397:
  error: always_inline function 'vfmaq_f16' requires target feature 'fullfp16',
  but would be inlined into function 'impl_accum_F16' that is compiled without
  support for 'fullfp16'
  ```
  i.e. **the `dnn` module** does not build. `IPHONEOS_DEPLOYMENT_TARGET=9.0` was a
  red herring; bumping to 13.0 did not change this.
- **Fix already applied** to `scripts/build_opencv_ios.sh`: added `--without dnn` and
  `--iphoneos_deployment_target 13.0`; and the CMake/adapters changes above now drop
  `dnn` + `HandLandmarkDetector.cpp` on iOS. Verified no iOS-compiled TU includes
  `<opencv2/dnn.hpp>` or `HandLandmarkDetector.hpp`.

### Next steps for the OpenCV build

1. `rm -rf third_party/ios/opencv-build-4.11.0` and re-run `scripts/build_opencv_ios.sh`
   (source clone under `third_party/ios/opencv-src-4.11.0` can be reused).
2. If a *different* module now fails under Xcode 26 (possible: `imgcodecs` 3rdparty,
   `core` NEON): either `--without <module>` if non-essential, or switch the clone from
   tag `4.11.0` to the `4.x` branch HEAD (newer-Xcode fixes) — edit `OPENCV_VERSION` /
   the `git clone` in `scripts/build_opencv_ios.sh`. We truly only need
   **core + imgproc + imgcodecs**.
3. On success verify:
   `plutil -p third_party/ios/opencv2.xcframework/Info.plist` shows `ios-arm64` and
   `ios-arm64-simulator`.
   Scratchpad logs:
   `/private/tmp/claude-501/-Users-bishalghimire-Documents-WORK-Code-vision-perception/a150b2be-ed1b-4105-b550-cc1d728111ef/scratchpad/opencv_dev.log`

### Then, the rest of Phase 1 (untested — blocked on OpenCV)

4. Run `scripts/build_ios_xcframework.sh`. Watch for:
   - Configure log must say `OpenCV: Version 4.11.0` with include dirs under
     `third_party/ios/…` **not** `/opt/homebrew`.
   - No `find_package(CURL)` on the iOS configure; no descent into `tests/ tools/
     apps/desktop_pipeline_demo`.
   - The `find … -name OpenCVConfig.cmake` in the script may need path adjustment
     depending on where `build_framework.py` actually leaves the per-slice build tree
     (expected: `…/opencv-build-4.11.0/iphoneos/build/build-arm64-iphoneos/`). If it
     can't find it, fall back to writing `cmake/ios_opencv.cmake` that hand-sets
     `OpenCV_INCLUDE_DIRS` / `OpenCV_LIBS` from the framework slice (see plan file §1
     fallback).
5. `--target curv_capi_static` must compile clean for both slices (no `videoio.hpp` /
   `curl/curl.h` / `cv::dnn` errors). Fix any remaining iOS-unsafe transitive include.
6. `xcodebuild -create-xcframework` → `build/CurvEngine.xcframework` +
   `build/opencv2.xcframework` both present.
7. App link:
   `cd apps/ios_palm_app && xcodegen generate && xcodebuild -scheme PalmScanner
   -destination 'generic/platform=iOS' build`
   — expect first-time `Undefined symbol: cv::…` or duplicate-symbol issues; resolve by
   adjusting link order / `OTHER_LDFLAGS` / making sure only ONE OpenCV copy is linked.
   Fallback if SPM nested binaryTarget misbehaves under XcodeGen: vendor both
   `build/*.xcframework` directly as `framework:` deps (`embed: false`) in
   `project.yml` and drop the `opencv2` target from `Package.swift` (plan §4).
8. Device smoke test: launch PalmScanner → `CurvEngine` init must not throw
   `.nativeUnavailable`; on capture, `CaptureResultView` subtitle switches from the
   landmark-fallback label to the native engine and `CreaseSplineOverlayView` draws
   `points_image_norm` polylines (not `fallbackLandmarkCreases`). **Line quality is
   Phase 2 — only confirm the native path executes.**
9. Update `docs/handover_palm_crease_ios.md` §3 to mark the XCFramework blocker
   resolved.

---

## Key files

| File | Role |
| :-- | :-- |
| `scripts/build_opencv_ios.sh` | builds `third_party/ios/opencv2.xcframework` (NEW) |
| `scripts/build_ios_xcframework.sh` | builds `build/CurvEngine.xcframework`, copies opencv2 alongside |
| `CMakeLists.txt` | iOS branch guards (prefix path, CURL, subdirs) |
| `adapters/CMakeLists.txt` | iOS-conditional CCTV sources + CURL link |
| `adapters/apple/Package.swift` | SPM binary targets for both frameworks |
| `apps/ios_palm_app/project.yml` | XcodeGen; app link settings |
| `adapters/capi/src/curv_capi.cpp` | the C API the Swift app calls (`curv_pipeline_process_landmarks_roi`) |
| `adapters/apple/CurvEngine.swift` | Swift wrapper; `#if canImport(curv_capi)` |
| `apps/ios_palm_app/Sources/Controllers/PalmAnalysisViewModel.swift` | `finishCapture` → native vs `fallbackLandmarkCreases` |
| `docs/handover_palm_crease_ios.md` | prior handover; documents the original blocker |
| `~/.claude/plans/start-with-the-opencv-parsed-prism.md` | full plan w/ rationale + fallbacks |

## Verification quick-reference

```bash
# 1. OpenCV framework
scripts/build_opencv_ios.sh
plutil -p third_party/ios/opencv2.xcframework/Info.plist   # expect ios-arm64 + ios-arm64-simulator

# 2. Engine framework (also runs step 1 if needed)
scripts/build_ios_xcframework.sh
ls build/CurvEngine.xcframework build/opencv2.xcframework

# 3. App
cd apps/ios_palm_app && xcodegen generate
xcodebuild -scheme PalmScanner -destination 'generic/platform=iOS' build   # zero Undefined cv:: symbols
```

## Do NOT

- Rewrite Steger in Swift/Metal. The C++ pipeline is done.
- Merge the two XCFrameworks.
- Commit `third_party/ios/` build outputs.
- Tune crease quality / quality gates here — that's Phase 2.
