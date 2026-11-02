# Android front-camera gaze spike: protocol and AND-4 number sheet (WP-A.14)

Status: **protocol ready, numbers not yet measured** (no Android device was available when
the app was built). Owner decision this feeds: ADR-0011 open question 1 / product vision
"Capture device" (clinic iPad with TrueDepth vs Android phones).

App: `apps/android_asd_gaze_spike/` (throw-away, adults only, no child data, logs never
committed; read its `README.md` first, especially "Binding gap").
Spec: `docs/spec/modules/asd-emr-lite-and-sharing/technical.md` AND-4,
`work-packages.md` WP-A.14; method: ADR-0007 and `docs/spec/modules/asd-gaze-robustness/technical.md`
(CAL, VAL, BEN).

## 0. What can and cannot be measured today

| Column | Measurable now? | Source |
| --- | --- | --- |
| Frame rate | yes | `<base>.meta.json` `live.meanFps` (tracker results per second) |
| Detected-frame ratio | yes | `live.detectedRatio` (MediaPipe: face + 10 iris landmarks; ARCore: tracked face) |
| CPU / thermal | yes | `live.processCpuCores`, `thermalStart`/`thermalEnd` (`thermalStatus` 0 none .. 3 severe .. 6 shutdown) |
| `asd-cli gaze replay --check` | yes | the `.jsonl` raw log (encoded by Rust; verified on the emulator, 22 targets, exit 0) |
| Accuracy in degrees (median, p90), pipeline valid-frame ratio | **no, binding gap** | needs a Rust package that turns MediaPipe/ARCore landmarks into `face_transform` + `look_at` (README "Binding gap"); then the sidecar `.landmarks.jsonl` is converted and replayed, with no new volunteer sessions |

So run the volunteer sessions now; fill the accuracy columns when the Rust converter
lands (a separate work package, as WP-A.14 requires for any Rust change).

## 1. Devices

Two phones from the owner's list, one per price class:

| Slot | Class | Model (fill) | Price USD, date, source (fill) | Must check before buying |
| --- | --- | --- | --- | --- |
| D1 | ~USD 120 | | | front camera; `arm64-v8a` or `armeabi-v7a`; listed at developers.google.com/ar/devices **with front-camera / Augmented Faces support** (else ARCore is n/a on it) |
| D2 | ~USD 250 | | | same |

For each phone, before the first session: Android version and `Build.MODEL` (the meta
records both), caliper measurement of the display active area (width x height mm) and of
the front-camera centre relative to the active area's top-left corner (mm). These go in the
sheet because the Rust geometry table has no Android entries yet (README gap 3).

Reference: the iPad the pilot uses (model, iPadOS version) for the comparison in section 6.

## 2. Volunteers

- Adults (18+) only, written consent, n >= 6 (BEN-1 asks n >= 8 for the full bench; 6 is
  the spike minimum), at least 2 wearing glasses, mixed eye colour, and if possible mixed
  skin tone (front-camera RGB trackers are sensitive to it; TrueDepth less so).
- **No children, ever.** No names in the app: participant number 1-20 maps to the paper
  consent sheet only.
- Raw logs are biometric data: copied to an encrypted disk, never committed, never mailed.

## 3. Set-up

- Phone on a fixed stand, **portrait**, screen vertical, front camera at the top, screen
  centre at the volunteer's eye height. Do-not-disturb on, brightness 80 %, adaptive
  brightness off, battery > 50 %, not charging.
- **Distance 35-65 cm**, measured with a ruler from the glass to the outer eye corner;
  baseline at **50 cm**. Enter the measured value in the app.
- **Lighting:** baseline office light 300-500 lux at the face (light-meter app or meter),
  no window or lamp behind the volunteer; `lowLight` about 50 lux; `sideLight` one lamp at
  90 degrees.
- The volunteer keeps the head still but not fixed (no chin rest), looks at each dot until
  it disappears, and then watches the moving shapes freely for 60 s.

## 4. Steps (per device)

1. Build and install (README "Build and install"); open the app, confirm `asd-core x.y.z`
   shows (the Rust core loaded). For ARCore, start one ARCore run without a volunteer: if
   the app says "not on the ARCore supported-devices list" or "Play Services for AR is not
   installed" and it cannot be installed, mark every ARCore cell of that device **n/a** and
   note why.
2. Per volunteer, in this order (each run is about 100 s: 9-point calibration, 13-point
   validation grid, 60 s free viewing):

   | # | Back-end | Condition | Distance | Repeats |
   | --- | --- | --- | --- | --- |
   | 1 | MediaPipe, CPU | baseline | 50 | 2 |
   | 2 | MediaPipe, GPU delegate | baseline | 50 | 1 |
   | 3 | ARCore | baseline | 50 | 2 |
   | 4 | MediaPipe (faster delegate from 1-2) | distance | 35 | 1 |
   | 5 | MediaPipe (same) | distance | 65 | 1 |
   | 6 | MediaPipe (same) | lowLight | 50 | 1 |
   | 7 | MediaPipe (same) | glasses (only volunteers with glasses) | 50 | 1 |

   Note: unlike BEN-4 `distance` (calibrate at 50, validate at 40/60), rows 4-5 calibrate
   and validate at the same distance; they test whether tracking holds at the range ends.
3. In positioning, wait until "detected" stays near 100 %, then **Start run**. Do not leave
   the screen during a run (the app discards an interrupted run). Triple-tap cancels.
4. **Thermal soak**, once per device with one volunteer: six MediaPipe baseline runs back
   to back (about 10 min). Record `thermalStatus` and fps of the first and last run.
5. Copy and clean up:
   ```bash
   adb pull /sdcard/Android/data/org.curv.asd.spike/files/spike_logs/ /Volumes/<encrypted>/asd-spike/<device>/
   ```
   then **Delete all logs on this phone** in the app.

## 5. Commands

From the repository root (`rust/` toolchain; logs stay outside the repository):

```bash
# Format check of every raw log (works today):
for f in /Volumes/<encrypted>/asd-spike/<device>/spike_logs/*[0-9].jsonl; do
  cargo run -q --manifest-path rust/Cargo.toml -p asd-cli -- gaze replay --check "$f"
done

# Frame rate, detected ratio, CPU and thermal per run (works today):
jq -r '[.rawLog, .capture.backend, .capture.delegate // "-", .condition, .distanceCm,
        .live.meanFps, .live.detectedRatio, .live.processCpuCores,
        .thermalStart.thermalStatus, .thermalEnd.thermalStatus] | @tsv' \
  /Volumes/<encrypted>/asd-spike/<device>/spike_logs/*.meta.json

# Log identity for the sheet (never the log itself):
shasum -a 256 /Volumes/<encrypted>/asd-spike/<device>/spike_logs/*[0-9].jsonl

# Accuracy, after the Rust converter exists (binding gap): convert, then replay.
#   asd-cli gaze convert-android <base>.landmarks.jsonl ...   (name to be decided by that package)
cargo run -q --manifest-path rust/Cargo.toml -p asd-cli -- gaze replay <converted>.jsonl --model angularAffine \
  | jq '{acc: .summary.validation.accuracyDeg, prec: .summary.validation.precisionRmsDeg,
         loss: .summary.validation.dataLoss, targets: [.targets[] | select(.kind=="validation") | .accuracyDeg]}'
cargo run -q --manifest-path rust/Cargo.toml -p asd-cli -- gaze bench-report --logs <dir-of-converted-logs> --models all
```

Accuracy per cell = median and p90 over all hit `grid13` validation targets of all
baseline runs of that device x back-end (pool targets, not run medians). Today, a full
`gaze replay` of a spike log runs but every target is `missed` and the verdict is
`unusable`; that is the binding gap, not a tracking result.

## 6. AND-4 sheet (fill in)

Baseline condition, 50 cm, office light. Pool all volunteers. `n/a` with a reason where a
back-end cannot run on a device.

| Device | Price (USD) | Back-end | Accuracy median (deg) | Accuracy p90 (deg) | Valid-frame ratio (pipeline) | Detected-frame ratio | fps (mean, min run) | CPU cores / thermal end | Volunteers / runs | Go / no-go |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D1 (~120) | | MediaPipe Face Landmarker | | | | | | | | |
| D1 (~120) | | ARCore Augmented Faces | | | | | | | | |
| D2 (~250) | | MediaPipe Face Landmarker | | | | | | | | |
| D2 (~250) | | ARCore Augmented Faces | | | | | | | | |
| iPad (reference) | | ARKit TrueDepth | G.14 open | G.14 open | G.14 open | - | 60 (ARKit) | | | reference |

Secondary (MediaPipe, faster delegate), per device:

| Device | Delegate used | 35 cm median (deg) | 65 cm median (deg) | lowLight median (deg) | glasses median (deg) | Detected ratio 35 / 65 / lowLight | Soak: fps first -> last, thermalStatus |
| --- | --- | --- | --- | --- | --- | --- | --- |
| D1 | | | | | | | |
| D2 | | | | | | | |

Log SHA-256 list (one line per raw log) and the `--check` output go in an appendix below the
tables; the logs themselves never do.

## 7. Go / no-go rule

**Which iPad numbers:** WP-G.14 (the iPad bench) is still `todo`, so there are no measured
iPad numbers. The rule below therefore uses the **provisional bands implemented in WP-G.2**
(calibration quality, CAL-5: good <= 2.5 deg, fair <= 3.5 deg, poor <= 5.0 deg; validated
only on synthetic logs) and the **BEN-4 Tier A targets** (baseline <= 2.5 deg, distance
<= 3.5 deg, data loss <= 10 %). These are synthetic/provisional reference values, not
iPad measurements; say so in the decision. When G.14 publishes iPad numbers, re-apply the
rule with "within 0.5 deg of the iPad median and within 5 points of its valid-frame ratio"
in place of the band limits.

Per device x back-end (baseline):

- **GO** (an Android capture package is worth planning): accuracy median <= 2.5 deg **and**
  p90 <= 3.5 deg **and** pipeline valid-frame ratio >= 0.90 **and** mean fps >= 25 with no
  run below 20 **and** thermalStatus < 3 (severe) at the end of the soak **and** 35/65 cm
  medians <= 3.5 deg.
- **CONDITIONAL** (report to the owner; Android only for coarse AOIs, never instead of the
  iPad in the pilot): median <= 3.5 deg and valid-frame ratio >= 0.85 and fps >= 20. AOIs
  must then be at least twice the measured accuracy (Orquin & Holmqvist 2018), which the
  current clips may not meet.
- **NO-GO**: anything worse, or accuracy that cannot be measured.
- **Decision:** Android is "go" for the clinic only if **D1 or D2** reaches GO with one
  back-end. Otherwise the pilot stays on iPads (ADR-0011 recommendation), and the Android
  AAR (WP-A.12) stays the non-capture floor (sync, scoring, console parity).

Expectation, so the numbers are read correctly: ARCore Augmented Faces returns no eye,
iris or blink data, so its "gaze" can only be head pose; it is expected to be NO-GO on
accuracy and is measured for fps and tracking stability only. MediaPipe has iris
landmarks but no depth sensor; the literature range for calibrated phone RGB gaze is about
1-3 deg (Krafka et al. 2016; Valliappan et al. 2020), with adults, not toddlers.

## 8. Results

(Empty until the sessions are run. Fill sections 1, 6 and the appendix; add one line of
conclusion for the owner.)

## Appendix: log hashes and `--check` output

(fill)
