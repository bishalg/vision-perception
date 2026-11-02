# android_asd_core: the ASD Rust core for Android (AAR `org.curv.asd:core`)

Gradle library module (Kotlin, `minSdk 26`, `compileSdk 36`, AGP 8.13) that packages the
UniFFI Kotlin bindings of `rust/crates/asd-ffi` and `libasd_ffi.so` for `arm64-v8a`,
`armeabi-v7a` and `x86_64` (spec AND-1..3, ADR-0011 §7). It holds **no logic**: scoring,
metrics, quality, sync protocol and validation live in Rust (project `CLAUDE.md`,
ADR-0006); Android code calls the generated functions and never re-implements them.

## Build

```bash
scripts/build_asd_android.sh                # .so files + bindings + AAR
scripts/build_asd_android.sh --test         # ... + JVM golden parity test
scripts/build_asd_android.sh --host --test  # no NDK: host library + bindings + test
```

Prerequisites: Rust (`rust/rust-toolchain.toml`), `cargo-ndk`, Android NDK r27+, SDK
(`ANDROID_HOME` or `local.properties`), JDK 17, Gradle 8.14+ (`GRADLE=` if not on `PATH`).
The script writes the generated sources and libraries to `build/asd/android/` (git-ignored;
`-PasdAndroidOut=` points Gradle elsewhere) and the AAR to
`core/build/outputs/aar/core-release.aar`. Running Gradle directly works once the script
has written that directory.

## Use

```kotlin
import org.curv.asd.core.*

val version = coreVersion()
val score = scoreSessionJson(metricSetsJson, baselineJson, ageMonths, framesJson, scoredAt)
```

Documents cross as JSON in the Swift `Codable` wire format; fallible calls throw
`AsdException` (`InvalidJson`, `InvalidArgument`, `NotRegistered`, `UploadRefused`,
`InvalidUrl`). The AAR depends on JNA (`net.java.dev.jna:jna:5.17.0@aar`, Apache-2.0,
`docs/license_policy.md`); `consumer-rules.pro` keeps JNA and the bindings under R8.

## Test

`core/src/test/kotlin/org/curv/asd/core/GoldenParityTest.kt` (JVM, host library through
JNA) mirrors the Swift kit's `GoldenParityTests`: the asd-core golden vectors
(`rust/crates/asd-core/tests/golden`) for scoring, flag, quality, metrics and aggregate to
1e-9; `coreVersion()` against asd-core's version; the backoff schedule `[1,2,4,8,60]`; and
the request body for `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.8.json`
byte for byte against its JCS form in `core/src/test/resources/golden/`. CI runs it in the
`android-bindings` job of `.github/workflows/rust.yml`.
