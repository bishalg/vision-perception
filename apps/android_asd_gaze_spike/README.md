# android_asd_gaze_spike: Android front-camera gaze feasibility spike (WP-A.14, AND-4)

> **Throw-away.** This app exists only to produce the AND-4 number sheet for owner
> decision 1 (clinic iPad vs Android phones, ADR-0011). It is not product code, it is never
> released, and it is excluded from every gate (no CI job, lint, claims scan or coverage
> scope covers this directory). Delete it once the decision is made.
>
> **Adults only. No child data.** Adult volunteers with written consent (BEN-1). Never run
> it with a child.
>
> **Logs are never committed.** Raw tracking logs are biometric data (ADR-0007 D13, BEN-8).
> They stay on the phone until copied to an encrypted disk, outside git (`.gitignore` covers
> `logs/` and every log suffix as a backstop). Delete them from the phone after copying.

Protocol, number sheet and go/no-go rule:
`docs/research/android_gaze_spike/2026-11-03-protocol-and-sheet.md`.

## What it does

A setup screen (back-end, BEN-4 condition from Rust's closed list, participant number 1-20,
measured distance) and a run screen:

1. **Positioning.** Live frame rate and detected-frame ratio at the top; a small preview
   for MediaPipe.
2. **Calibration.** 2 s lead-in, then the 9-point layout (`calibration.layout9`), one white
   dot at a time, fixed timing (`settleMs + collectMs`), no gaze feedback.
3. **Validation.** The 13-point bench grid (`validation.benchLayout13`, VAL-5).
4. **Free viewing.** 60 s of slowly moving shapes; frames carry `clip_media_time_sec` and
   `stimulus_id = spike-free-view`.
5. **Files.** Four files in `/sdcard/Android/data/org.curv.asd.spike/files/spike_logs/`:

| File | Content | Written by |
| --- | --- | --- |
| `<base>.jsonl` | `asd-raw-tracking/1` raw log (REP-1) | Rust `gazeRawLogDocument` (the iOS bench encoder) |
| `<base>.targets.json` | target schedule | Rust `gazeScheduledTargetsJson` |
| `<base>.landmarks.jsonl` | per-frame tracker output, uninterpreted (sidecar) | app |
| `<base>.meta.json` | device, display dpi, camera intrinsics, frame clock, fps, detected ratio, CPU, thermal | app |

Layouts and target windows come from Rust (`gazeDefaultConfigJson`), never from Kotlin.
The live numbers are counts: **fps** = tracker results per second over the last 2 s;
**detected** = frames with everything the back-end can give for gaze (MediaPipe: a face
with the 10 iris landmarks; ARCore: a tracked face) / all frames of the run.

## Back-ends and licences (checked against `docs/license_policy.md`)

| Back-end | Library | Licence | Policy |
| --- | --- | --- | --- |
| MediaPipe Face Landmarker (478 landmarks incl. iris, 52 ARKit-named blend shapes, face transform) on CameraX | `com.google.mediapipe:tasks-vision:0.10.21`; model `face_landmarker.task` float16/1 | Apache-2.0 (POM); model Apache-2.0 (Google AI Edge model card) | Greenlist; the policy names MediaPipe landmark models explicitly. Model is 3.6 MB, downloaded at build time with a SHA-256 check, not committed. |
| CameraX | `androidx.camera:*:1.4.2` | Apache-2.0 | Greenlist |
| ARCore Augmented Faces (468-vertex mesh, centre and region poses; **no eyes, iris or blinks**) | `com.google.ar:core:1.57.0` + Google Play Services for AR on the phone | "ARCore Additional Terms of Service" (POM licence), proprietary runtime | **Not greenlist.** Acceptable for a throw-away spike that ships nowhere; any product use needs an owner licence review. Only on devices on Google's ARCore supported list. |
| ASD core AAR | `apps/android_asd_core` (`org.curv.asd:core`) + JNA 5.17.0 | first party; JNA under Apache-2.0 | already listed in policy section 4.3 |

MediaPipe is pinned to 0.10.21 because it is the last release with `armeabi-v7a`
libraries (0.10.26 and later ship `arm64-v8a` only), and cheap phones may run a 32-bit
Android Go userspace.

## Binding gap (reported, not patched here)

The Rust pipeline (`asd-core::gaze`) needs, per frame, an ARKit-shaped `RawTrackingSample`:
`face_transform` (face pose in the camera frame, metres, ARKit axes) and `look_at`
(eye-in-head gaze point in the face frame), optionally both eye transforms. Neither
back-end produces those: MediaPipe gives image-normalised landmarks, iris points, blend
shapes and a facial transformation matrix in its own convention; ARCore gives poses in its
own camera space and no eye data at all. Turning them into `face_transform`/`look_at` is
gaze maths (axis conventions, metric scale from intrinsics, eye-in-head direction from
iris versus eye corners), which belongs in Rust (project `CLAUDE.md`, ADR-0007 D1, D12).

Missing in Rust / the AAR today:

1. `TrackingSource` has only `arkitFace`; the header written by `gazeRawLogDocument` says
   `"source": "arkitFace"` for these logs (the back-end is in `appVersion` and the meta).
   Needed: `mediapipeFaceLandmarker` and `arcoreAugmentedFace` sources with their
   `CAMERA_AXES` (PLT-3).
2. No function that maps a back-end's landmarks (plus camera intrinsics) to a
   `RawSampleRecord`'s `face_transform`, `look_at` and eye transforms. Needed: e.g.
   `gaze_raw_sample_from_mediapipe(landmarks, blendshapes, face_matrix, intrinsics, image size)`
   in `asd-core::gaze` + `asd-ffi`, and an `asd-cli gaze convert-android <landmarks.jsonl>`
   that rewrites a spike log so it can be replayed.
3. No Android device geometry: `PipelineConfig::for_model(Build.MODEL)` falls back to the
   phone idiom default (an iPhone size). Needed: Android entries in
   `resources/device_geometry.json`, or a header input that takes measured screen
   millimetres and camera offset (the meta records the OEM dpi; the protocol asks for
   caliper measurements).

Consequence: the raw log written now carries `t_sec`, `face_tracked`, the ARKit-named
`eye_blink_left/right` blend shapes (MediaPipe), orientation and clip time, and the full
schedule. `asd-cli gaze replay --check` accepts it; a full `asd-cli gaze replay` runs but
every target ends `missed` (no gaze ray), so **accuracy in degrees needs the Rust package
above**. The sidecar keeps the raw landmarks, matrices and intrinsics, so the volunteer
sessions do not have to be repeated: once the converter exists, convert the logs and replay
them. Frame rate, detected-frame ratio, CPU and thermal numbers are measured now. The Rust
package is a separate work package (WP-A.14 rule: "if a Rust change is needed, it is a
separate package").

## Build and install

Prerequisites: JDK 17 (Gradle 8.14 does not run on newer JDKs), Android SDK (platform 36),
NDK r27+ and Rust for the core.

```bash
# 1. The ASD core AAR (once, and after Rust changes), from the repository root:
scripts/build_asd_android.sh

# 2. The spike app:
cd apps/android_asd_gaze_spike
echo "sdk.dir=$HOME/Library/Android/sdk" > local.properties   # or set ANDROID_HOME
JAVA_HOME=$(/usr/libexec/java_home -v 17) ./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

`-PasdCoreAar=/path/to/core-release.aar` uses another AAR. The first build downloads the
MediaPipe model (network needed once).

## Copy the logs off the phone

```bash
adb pull /sdcard/Android/data/org.curv.asd.spike/files/spike_logs/ ~/asd-spike-logs/
```

Keep them on an encrypted disk outside the repository, then use **Delete all logs on this
phone** in the app. Never attach a raw log to a ticket, chat or commit; the sheet cites logs
by SHA-256 (`shasum -a 256`).
