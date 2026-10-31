# ASD Gaze Tracking Robustness - Technical Spec

Status: Proposed (2026-10-31). Snapshot reviewed: `dev` at `1f43026`.
Decision record: `docs/spec/adr/ADR-0007-gaze-tracking-robustness.md`.
Work packages: `work-packages.md` (WP-G.0 … WP-G.14). Acceptance: `acceptance.md`.
Builds on: ADR-0006 and `docs/spec/modules/asd-rust-core-and-backend/technical.md` (CON-*,
CORE-*, FFI-*), the store-readiness spec (PLC-5, CAM-2, SIM-*, CONC-*).

## Purpose and scope

Today the app measures where a child looks, but it does not measure how well it
measures it. No session records its own accuracy, calibration is never scored,
drift is not detected, and several geometry defects (below) bias every gaze sample.
This module makes the tracking measure and report its own quality, in degrees of
visual angle, per device and per session. It also moves the per-frame gaze maths into
Rust (`asd-core::gaze`), as `CLAUDE.md` requires.

In scope: device screen geometry, raycast, calibration models and calibration quality,
the validation step, drift checks between clips, re-calibration triggers, per-sample and
per-fixation confidence, track-state and gap-fill rules, mapping AOIs from video space to
screen space, the session `trackingQuality` record (schema 1.7), backend columns and
review rule, the console Quality tab, the bench protocol, the device matrix, and
synthetic and replay test vectors.

Out of scope (deliberately): post-hoc drift correction of recorded samples (needs AOI
hit testing at finalize; listed as a follow-up in ADR-0007), appearance-based (RGB/CNN)
gaze models, Android capture (WP-A.12 and later), visionOS capture (not possible, see
PLT-1), normalising session JSON casing (still CON-12, 1.7 does not do it), and any
change to the norms or to the traffic-light thresholds.

## Verified current state this spec builds on

Paths are relative to `domains/asd_screening/ios/Sources/CurvAsdGazeKit/` unless they
start with `apps/` or `rust/`.

| # | Finding | Evidence |
| --- | --- | --- |
| G-F1 | The real pipeline order is raycast → clamp to [0,1] → affine calibration → 1-Euro → AOI hit test. The raycaster clamps the raw point **before** calibration, so a raw estimate that falls off the glass (common: the code comment says raw error is 3–6 cm on a 7.1 cm wide phone) cannot be corrected and gets pinned to the edge. | `ScreenRaycaster.swift:118`, `GazeProcessingActor.swift:379` |
| G-F2 | Sample validity (`isWithinScreen`) is decided on the **uncalibrated** raw point, not on the calibrated point that is stored and hit-tested. | `GazeProcessingActor.swift:450` |
| G-F3 | When the ray parameter `t > 2.5`, the raycaster sets `validForwardRay = true` but uses the 3D look-at point (not a point on the screen plane) as the hit. That sample can be marked as on-screen at a wrong position. | `ScreenRaycaster.swift:96-101` |
| G-F4 | Screen size is two hard-coded idiom defaults (iPhone 0.071×0.154 m, iPad 0.178×0.247 m) with the camera assumed horizontally centred and 8 mm above the active area, portrait only. Every other model (Pro Max, iPad Pro 13", the M4 iPad Pro whose camera sits on the long edge) gets a wrong geometry. | `ScreenRaycaster.swift:20-40,115` |
| G-F5 | The app allows landscape on iPhone and all four orientations on iPad, but nothing in the gaze path knows the interface orientation. | `apps/ios_asd_gaze_app/project.yml:75-76`; no orientation handling in `Sources/` |
| G-F6 | Per-sample `confidence` is hard-coded `0.95` (or `0.0` during a blink), so the quality gate's `minConfidence` check never fires on real data. | `GazeProcessingActor.swift:457` |
| G-F7 | Frames where the face is not tracked are **not buffered** (early return before recording). Track loss therefore does not lower `validRatio`; it only shows up as timestamp gaps. `validRatio` overstates quality whenever the child turns away. | `GazeProcessingActor.swift:348-365` |
| G-F8 | Blink recovery is a frame count (3 frames), not a time, so it is 50 ms at 60 Hz and 100 ms at 30 Hz. | `GazeProcessingActor.swift:166` |
| G-F9 | Calibration samples come from `controller.currentRawGazePoint`, which is the **1-Euro filtered** raw point, read by a main-thread 60 Hz `Timer` that is not synchronised with ARKit frames. The fit is then applied to the **unfiltered** raw point. | `apps/ios_asd_gaze_app/Sources/Views/BubbleCalibrationView.swift:578-620` |
| G-F10 | Calibration samples are collected only while the raw gaze is within 160 pt (220 pt for the centre) of the target plus a centre bias. This selects the samples whose raw error is already small and biases the fit. | `BubbleCalibrationView.swift:607-621` |
| G-F11 | A target with no collected samples falls back to the target position itself ("zero assumed error"), which silently drags the fit towards identity at that point. The fit is always applied; it has no residual, no quality score and no rejection. | `BubbleCalibrationView.swift:670`, `:697-699` |
| G-F12 | Calibration targets span only x,y ∈ [0.22, 0.78]; the GeoPref AOIs run to the screen edges (x = 0 and x = 1), so edge gaze is extrapolated. | `BubbleCalibrationView.swift:117-123`; `StimulusAOITracker.swift` |
| G-F13 | One calibration profile per device in `UserDefaults`, reused across sessions and children; the wizard offers "Skip (Use Saved Calibration)". A session can therefore run on another child's calibration. | `GazeCalibrationProfile.swift:132-150`; `apps/ios_asd_gaze_app/Sources/UI/AppLocalizer.swift:223` |
| G-F14 | `calibratedDistanceCm` is never measured: every record gets the initialiser default `35.0`, which the console shows as the session's distance. This contradicts CON-14 ("no default measurements"). | `apps/ios_asd_gaze_app/Sources/Models/GazeSessionRecord.swift:120`; `apps/curv_console/src/components/QualityPanel.tsx` |
| G-F15 | I-VT uses 5.0 **normalised screen units per second**, not degrees per second. At 45 cm that is about 45°/s on an iPhone (7.1 cm wide) and about 110°/s on an iPad (17.8 cm wide), so fixation counts and durations are not comparable across devices. The Rust port copied the same constant. | `PerVideoMetrics.swift:80-86`; `rust/crates/asd-core/src/metrics.rs` |
| G-F16 | Stimulus videos are 1080×1920 and are shown with `.resizeAspectFill`, which crops about 9 % per side horizontally on a 19.5:9 iPhone and about 12 % per side vertically on an iPad in portrait. The catalog does not say whether AOI boxes are in video or screen coordinates; the pipeline hit-tests them as screen coordinates. | `apps/ios_asd_gaze_app/Sources/Views/StimulusVideoPlayerView.swift:38`; `stimulus_catalog.json`; `ffprobe` of `Resources/Videos/*.mp4` |
| G-F17 | The calibration view lays targets out in a `GeometryReader` inside a `NavigationStack` with padding; the stimulus view ignores safe areas; the raycaster normalises to the physical full screen. The three coordinate frames differ. | `BubbleCalibrationView.swift:150-200`; `GazeStimulusScreeningView.swift:118-160` |
| G-F18 | Rust `asd-core` already has `quality::evaluate_sample` (CORE-7) and `QualityGate`; `ScoringProfile.qualityGate` in `asd-contract::config` carries the same six parameters. `GazeSessionRecord.extra` keeps unknown `rawPayload` keys. `asd-ffi` is still an empty crate (WP-R.5 todo). | `rust/crates/asd-core/src/quality.rs`; `rust/crates/asd-contract/src/{config,session_v1_6}.rs`; `rust/crates/asd-ffi/src/lib.rs` |
| G-F19 | Size check: the catalog `Eye_Region` is 0.30 × 0.18 of the frame. On an iPhone at 45 cm that is about 2.1 × 2.8 cm, about 2.7° × 3.5°; on an iPad in portrait about 6.8° × 5.6°. Eye-versus-mouth metrics on a phone are near or below plausible calibrated ARKit accuracy. | arithmetic from `stimulus_catalog.json` and G-F4 |

## Definitions

- **Visual angle** between screen points P and Q for an eye at E (all in mm, screen
  frame): `θ = atan2(|(P−E)×(Q−E)|, (P−E)·(Q−E))` in degrees. Never the small-angle
  approximation for accuracy figures.
- **Accuracy** (per target): visual angle between the target and the **median** of the
  target's valid samples after the settle window. Session and validation accuracy are
  medians over targets (Holmqvist et al. 2012 use the mean offset; we use the median
  because of child data, and we report it as such).
- **Precision**: `RMS-S2S` = root mean square of the visual angles between consecutive
  valid samples; `SD` = √(σx² + σy²) of the sample positions in degrees around their
  centroid. Both are reported on the calibrated, filtered signal (what analysis uses)
  and RMS-S2S also on the unfiltered calibrated signal, because the filter lowers it
  artificially.
- **Data loss**: share of frame time in a window that is not `tracked` (see GAP-1).
- **Screen-normalised space**: x,y ∈ [0,1] over the full physical display in the
  current interface orientation, origin top-left, y down. Every target, every gaze point
  and every AOI box handed to the pipeline is in this space (GEO-4).

## 1. Device geometry and raycast (GEO)

- **GEO-1** `asd-core::gaze::geometry` MUST hold a device geometry table, compiled in from
  `rust/crates/asd-core/resources/device_geometry.json`, with one entry per hardware
  model: `modelIdentifiers` (for example `"iPhone17,1"`), `displayName`, `nativePixels
  [w,h]` and `screenWidthMm`, `screenHeightMm` (active area, portrait native frame),
  `cameraOffsetMm {x, y}` (TrueDepth aperture centre relative to the active area's
  top-left, portrait native frame, y down, negative y = above the glass), and `source ∈
  spec | measured`. Millimetres derived from published resolution and ppi are `spec`;
  the camera offset is `measured` only after the bench (BEN-3). Implementers MUST NOT
  type in values from memory; every entry cites its source in a `ref` string.
- **GEO-2** Lookup by model identifier. Unknown model → idiom default (today's G-F4
  constants) with `geometrySource = "idiomDefault"`; the session `trackingQuality.verdict`
  is then capped at `fair` (QS-4). A later bundle may add entries (DEV-4).
- **GEO-3** The pipeline MUST take the interface orientation (`portrait`,
  `portraitUpsideDown`, `landscapeLeft`, `landscapeRight`) and rotate screen size and
  camera offset accordingly. An orientation change during a recording ends the current
  calibration's validity (CAL-9).
- **GEO-4** Every capture screen (positioning, calibration, validation, drift checks,
  stimulus) MUST lay out in screen-normalised space over the full display (no safe-area
  inset, no navigation bar), so target coordinates and gaze coordinates share one frame.
- **GEO-5** `gaze::raycast` MUST intersect the ray from the eye origin (midpoint of the
  two eye transforms in camera space; face centre + 3 cm up only when the eye transforms
  are absent) through the look-at point with the screen plane, and return the hit in
  screen-normalised space **without clamping**, plus `eyePosMm` (eye origin in the screen
  frame) and a `rayStatus ∈ ok | backward | grazing | outOfRange`. Only `ok` produces a
  gaze point. This fixes G-F1 and G-F3: there is no fallback to the 3D look-at point.
- **GEO-6** Raycast correctness is proved by analytic cases (eye at known positions,
  look-at towards the four corners and the centre, every orientation, two geometries);
  expected values are computed by hand in the test file, not taken from the Swift
  implementation, because the Swift behaviour is defective (G-F1, G-F3).
- **GEO-7** `gaze::aoi_space::video_to_screen(aoi, video_aspect, screen_aspect, gravity)`
  MUST map an AOI box from video-normalised to screen-normalised space for
  `aspectFill` and `aspectFit`, dropping parts that fall outside the visible area. The
  catalog gains `aoiSpace: "video" | "screen"` per clip (default `"video"` once the
  catalog is versioned, G-F16), and `videoAspect`. Hit testing uses screen-space boxes.

## 2. Calibration (CAL)

- **CAL-1** Calibration is per session and per participant. A stored profile MUST NOT
  be applied to a session for a different `participantToken`, and the "skip, use saved
  calibration" path MUST be removed (G-F13). A session without its own calibration
  records `calibration.quality = "skipped"`.
- **CAL-2** Sample collection for a target MUST be gated on tracking quality (face
  tracked, track state `tracked`, sample confidence ≥ `minConfidence`, head angular
  speed below `maxHeadSpeedDegPerSec`), never on the distance between raw gaze and the
  target (G-F10). Samples are taken from the pipeline's per-frame output (unfiltered raw
  screen point, frame timestamp), not from a UI timer (G-F9).
- **CAL-3** Per target: drop samples in the settle window (`settleMs`, default 400),
  then reject samples farther than 3 × MAD from the per-axis median; the target's raw
  point is the median of the rest. A target needs ≥ `minSamplesPerTarget` (default 20,
  about 330 ms at 60 Hz). A target that does not reach it is **re-shown once**; if it
  fails again it is marked missing and is not part of the fit. There is no fallback to
  the target position (G-F11).
- **CAL-4** `gaze::calibration` MUST implement four models behind one enum, each with
  `fit(points) -> Result<Model, CalError>` and `apply(raw) -> point` (no clamping):
  - `screenAffine` (6 parameters; today's model; parity with `GazeCalibrationProfile.fit`
    on its existing test cases);
  - `angularAffine` (6 parameters): affine correction of the eye-in-head gaze direction
    (yaw, pitch of the look-at vector in the face frame) before the raycast, so the
    correction stays valid when the head translates or the distance changes;
  - `screenHomography` (8 parameters, DLT with Hartley normalisation);
  - `screenPoly2` (12 parameters, `1, x, y, xy, x², y²` per axis).
  Minimum usable points: affine 3, homography 4, poly2 6; a model is **eligible** only
  with at least `minPoints + 2` usable points (so 5 points allow only the affine models,
  9 points allow all four).
- **CAL-5** Calibration quality MUST be computed for the fitted model: `residualDeg`
  (median visual-angle residual on the calibration points), `looCvDeg` (median
  leave-one-out error: fit on n−1, measure the left-out point), `conditionNumber` of the
  normalised design matrix, `usablePoints`, `missingPoints`. Quality band from `looCvDeg`
  with thresholds in `TrackingQualityParams.calibration` (provisional defaults until
  BEN-6: `good ≤ 2.5°`, `fair ≤ 3.5°`, `poor ≤ 5.0°`, else `failed`; also `failed` when
  `usablePoints < 4` or `conditionNumber > 1e6`).
- **CAL-6** Model selection: the configured `defaultModel` is fitted always; when
  `autoSelect` is true, every eligible model is fitted and the one with the lowest
  `looCvDeg` wins, unless it beats the simplest eligible model by less than
  `minImprovementDeg` (default 0.3°), in which case the simpler one is used. Default
  `defaultModel = screenAffine`, `autoSelect = false` until BEN-6 decides (ADR-0007 D6).
- **CAL-7** Targets: default 5 calibration points at (0.5,0.5), (0.12,0.12), (0.88,0.12),
  (0.12,0.88), (0.88,0.88); optional 9-point layout adds (0.5,0.12), (0.12,0.5),
  (0.88,0.5), (0.5,0.88). Positions are config (`TrackingQualityParams.calibration.layout5|layout9`),
  inset so the target's visual radius stays on the glass.
- **CAL-8** Child-friendly target: an animated character that appears large and shrinks
  to a small (≤ 1° at 45 cm) bright centre over the window, with a short sound and a
  "pop" at the end; one target window = `settleMs + collectMs` (defaults 400 + 1200 ms);
  the window pauses (not resets) while tracking is lost and resumes for up to
  `maxTargetWaitMs` (default 4000 ms). The UI is native (WP-G.10); timing and acceptance
  of samples are decided by Rust (`CalibrationSession` state machine in `gaze::calibration`).
- **CAL-9** Re-calibration triggers (evaluated in Rust, `gaze::drift::advise`), acted on
  only at a clip boundary, never mid-clip:
  1. calibration quality `poor` or `failed` → offer re-calibration before the battery;
  2. validation accuracy worse than `validation.maxAccuracyDeg` (VAL-4);
  3. drift (DRF-3);
  4. head pose moved away from the calibration pose (median over the last 3 s:
     distance changed by more than `poseShiftDistancePct`, default 20 %, or yaw/pitch by
     more than `poseShiftAngleDeg`, default 15°) — screen models only, because
     `angularAffine` is designed to absorb this;
  5. interface orientation changed;
  6. calibration older than `maxCalibrationAgeSec` (default 600).
  At most `maxRecalibrationsPerSession` (default 2, of which at most 1 inside the
  battery). Each re-calibration is recorded with its reason (QS-1).

## 3. Validation (VAL)

- **VAL-1** After every calibration, a validation step MUST show `validation.layout`
  targets (default 4: (0.30,0.30), (0.70,0.30), (0.30,0.70), (0.70,0.70), deliberately
  not calibration positions) with the same child-friendly target as CAL-8 but a
  different character, `settleMs` 400 and `collectMs` 1000.
- **VAL-2** Per target Rust computes accuracy, RMS-S2S, SD, data loss and the number of
  valid samples; a target with fewer than `validation.minSamplesPerTarget` (default 12)
  valid samples is `missed`. Validation is `complete` with ≥ 3 of 4 targets hit, else
  `incomplete` (still recorded).
- **VAL-3** Validation accuracy = median accuracy over hit targets; precision = median
  RMS-S2S over hit targets.
- **VAL-4** Validation accuracy > `validation.maxAccuracyDeg` (default 5.0°) → advise
  re-calibration (CAL-9.2). Above `validation.warnAccuracyDeg` (default 3.5°) the
  operator sees a warning but may continue. A validation shown with no hit target
  advises re-calibration (reason `validationAccuracy`); an incomplete validation whose
  accuracy is within limits gives a warning with the same reason (audit 2026-10-31).
- **VAL-5** A bench-only 13-point validation grid (3×3 at 0.1/0.5/0.9 plus 4 at
  0.3/0.7) exists behind the bench build flag (BEN-2); it never ships in Release.

## 4. Drift (DRF)

- **DRF-1** Between consecutive clips of a battery and once after the last clip, the
  app MUST show one check target (`drift.checkLayout`, cycling through (0.5,0.5),
  (0.3,0.3), (0.7,0.3), (0.3,0.7), (0.7,0.7)), `settleMs` 300, `collectMs` 900, same
  sample rules as VAL-2 (`minSamples` 10).
- **DRF-2** Each check yields `offsetDeg` (accuracy at that target), `offsetVector` in
  degrees (x, y) and the session time. `driftDeg` for a check = visual angle between its
  offset vector and the mean validation offset vector (so a constant validation bias is
  not counted as drift).
- **DRF-3** Drift trigger: `driftDeg > drift.warnDeg` (default 1.5°) on two consecutive
  checks, or `> drift.hardDeg` (default 3.0°) on one check → advise re-calibration.
- **DRF-4** Drift is detected and recorded; recorded samples are **not** corrected in
  this version (ADR-0007 D9).

## 5. Per-sample state, confidence and gaps (TRK, CNF, GAP)

- **TRK-1** Every ARKit frame during a recording MUST produce a buffered frame,
  including frames without a tracked face (fixes G-F7). Each frame carries
  `track_state ∈ tracked | blink | lost | offscreen | artifact`:
  `lost` = no face anchor or `isTracked == false`; `blink` = either eye openness below
  `blinkThreshold`, plus a post-blink recovery of `blinkRecoveryMs` (default 50, time
  based, fixes G-F8); `artifact` = implausible eye-in-head angular speed
  (> `maxEyeSpeedDegPerSec`, default 1000) or `rayStatus != ok`; `offscreen` = calibrated
  point outside the screen by more than `offscreenMarginNorm` (default 0.02); otherwise
  `tracked`.
- **TRK-2** Validity is decided on the calibrated point (fixes G-F2). The display point
  may be clamped for drawing only; stored samples are not clamped.
- **TRK-3** Interruptions (CAM-2) keep their current accounting; no frames exist during
  them and they are not counted as track loss.
- **CNF-1** `gaze::confidence::sample(features, params) -> f64` in [0,1] is the
  **minimum** of these factors, each a piecewise-linear ramp that is 1 inside a comfort
  zone, 0.5 exactly at the existing hard limit, and 0 beyond it:
  eye openness (comfort ≥ 0.70, limit `blinkThreshold`), |yaw| and |pitch| (comfort
  ≤ 0.6 × limit, limit `maxYawRad`/`maxPitchRad`), distance (comfort 40–60 cm, limits
  `minDistanceCm`/`maxDistanceCm`), head angular speed (comfort ≤ 30°/s, limit
  `maxHeadSpeedDegPerSec` default 120°/s), and, for screen models only, distance shift
  from the calibration pose (comfort ≤ 10 %, limit `poseShiftDistancePct`). Track state
  other than `tracked` → 0. Because the factors meet 0.5 at the existing limits, the
  existing gate (`minConfidence = 0.50`, CORE-7) keeps the same pass/fail boundary for
  distance, pose and blink, and newly rejects only motion and artefact samples.
- **CNF-2** Ambient light (`ARLightEstimate.ambientIntensity`) and, when available, the
  directional light estimate are recorded per frame and summarised per session
  (QS-1 `conditions`); light is **not** a confidence factor until the bench shows it
  matters (BEN-5). TrueDepth tracking is infrared-assisted, so visible light is expected
  to matter less than for RGB trackers.
- **CNF-3** Per-fixation confidence = mean sample confidence over the fixation's
  samples; a fixation below `minConfidence` is `lowConfidence`. Per clip:
  `meanSampleConfidence` and `meanFixationConfidence`.
- **CNF-4** The pipeline writes the computed confidence into
  `GazeTelemetrySample.confidence` (replacing the constant, G-F6), so the existing
  CORE-7 gate and frame files carry it.
- **GAP-1** A gap is a run of non-`tracked` frames between two `tracked` frames. A gap
  is filled by linear interpolation when its duration ≤ `gapFill.maxMs` (default 75 ms,
  Olsen 2012) **and** the visual angle between the bracketing samples ≤
  `gapFill.maxDeg` (default 1.0°). Filled samples are marked `gap_filled = true`.
- **GAP-2** Filled samples are used only for fixation segmentation. They MUST NOT add
  dwell, MUST NOT count as valid in `validRatio`, and MUST NOT be hit-tested for AOI
  entry or latency. (PLC-5 already credits a valid frame with up to 100 ms of dwell
  across a following gap; filling for dwell would count it twice.)
- **GAP-3** Blinks longer than `gapFill.maxMs` are never filled and end a fixation.
  Loss is accounted by state: `blinkSec`, `trackLossSec`, `offscreenSec`, `artifactSec`,
  `gapFilledSec`, `longestGapSec` (frame-interval sums, each interval capped at 100 ms
  like PLC-5, plus the uncapped `longestGapSec`).

## 6. Pipeline shape (PIPE)

- **PIPE-1** `asd-core::gaze::pipeline::GazePipeline` is a stateful, single-owner object:
  `new(config: PipelineConfig)` (geometry, orientation, `TrackingQualityParams`,
  calibration), `push(raw: RawTrackingSample) -> ProcessedSample`, `set_calibration`,
  `set_orientation`, `begin_target(kind, target)` / `end_target() -> TargetResult`
  (calibration, validation and check targets share this), `advice() -> RecalAdvice`,
  `summary() -> TrackingQuality`. Order inside `push`: track state → raycast (GEO-5)
  → calibration apply → confidence → 1-Euro filter (on `tracked` samples only, sample
  time) → AOI-space hit test → output.
- **PIPE-2** `RawTrackingSample` (also the bench and replay log line, REP-1):
  `t_sec` (frame clock), `face_tracked`, `face_transform [16]`, `look_at [3]`,
  `left_eye_transform [16]`, `right_eye_transform [16]`, `eye_blink_left`,
  `eye_blink_right`, `ambient_intensity?`, `ambient_color_temp?`, `light_direction [3]?`,
  `orientation`, `clip_media_time_sec?`, `stimulus_id?`. Blend shapes other than the
  two blink values stay native (affect is out of this module).
- **PIPE-3** The 1-Euro filter is a Rust port of `OneEuroFilter.swift` with identical
  output on that file's test cases (parameters 1.2 / 0.015 / 1.0 become config).
- **PIPE-4** Per-sample cost of `push` MUST be ≤ 0.2 ms at p99 on the slowest supported
  device (DEV-2), measured through the FFI in WP-G.9.
- **PIPE-5** Determinism: same config and same input sequence → byte-identical outputs
  after JCS canonicalisation (golden tests rely on it). No wall clock, no randomness.

## 7. Session tracking-quality record (QS)

- **QS-1** `asd-core::types::tracking::TrackingQuality` (camelCase), produced by
  `GazePipeline::summary()`:

  ```text
  trackingQuality {
    pipelineVersion        "asd-gaze/1.0.0"        // asd-core gaze maths version
    paramsVersion          string                   // TrackingQualityParams schemaVersion
    device { modelIdentifier, geometrySource: table|idiomDefault, orientation,
             screenWidthMm, screenHeightMm, sampleRateHz }   // median frame interval
    calibration { model, layoutPoints, usablePoints, missingPoints, residualDeg,
                  looCvDeg, conditionNumber, quality: good|fair|poor|failed|skipped,
                  pose { distanceCm, yawDeg, pitchDeg }, ageSecAtStart, attempts }
    validation? { complete, targets: [{ xNorm, yNorm, accuracyDeg?, precisionRmsDeg?,
                  sdDeg?, dataLoss, validSamples, missed }],
                  accuracyDeg?, precisionRmsDeg?, precisionRmsRawDeg?, dataLoss }
    driftChecks [{ atSec, xNorm, yNorm, offsetDeg?, driftDeg?, missed }]
    maxDriftDeg?
    recalibrations [{ atSec, reason, newQuality }]
    conditions { distanceCmMedian, distanceCmIqr, headSpeedDegPerSecP90,
                 ambientIntensityMedian?, lightAsymmetry?, glasses?: bool }
    loss { trackLossSec, blinkSec, offscreenSec, artifactSec, gapFilledSec, longestGapSec }
    confidence { meanSample, meanFixation?, lowConfidenceSampleRatio }
    estimatedAccuracyDeg?                           // QS-2
    verdict: good|fair|poor|unusable                // QS-4
  }
  ```

  Optional (`?`) fields are absent when not measured, never defaulted (CON-14).
  `glasses` is an operator answer at the positioning step, absent when not asked.
- **QS-2** `estimatedAccuracyDeg` = `max(validation.accuracyDeg, median(driftChecks.offsetDeg))`
  when both exist; `validation.accuracyDeg` when no check was hit; median of check
  offsets when there is no validation and ≥ 2 checks were hit; otherwise absent.
- **QS-3** Per clip (`perVideo[].details`, 1.7): `meanSampleConfidence`,
  `meanFixationConfidence?`, `nearestCheckOffsetDeg?` (the check right after the clip),
  `aoiMinSizeDeg?` (smallest dimension of the clip's role-bearing AOIs in degrees at the
  session's median distance).
- **QS-4** Verdict: `unusable` when `estimatedAccuracyDeg` is absent or above the
  `failed` band, or `validRatio < 0.50`; `poor` above the `fair` band or data loss
  > 40 %; `fair` above the `good` band or data loss > 20 % or `geometrySource =
  idiomDefault` or `calibration.quality = skipped` or no complete validation (accuracy
  from drift checks alone never earns `good`, audit 2026-10-31); else `good`. The existing
  `qualityVerdict` (validRatio based) is unchanged.
- **QS-5** `calibratedDistanceCm` = `trackingQuality.calibration.pose.distanceCm` when a
  calibration ran in the session; absent otherwise (fixes G-F14; needs 1.7, SCH-2).

## 8. Contract and config (SCH)

- **SCH-1** Session schema **1.7** = 1.6 plus: typed optional `rawPayload.trackingQuality`
  (QS-1), the QS-3 fields inside `perVideo[].details` and inside
  `rawPayload.perVideo[]`, `calibratedDistanceCm` optional in `rawPayload` (already
  optional at top level), per-frame `track_state`, `gap_filled` in the frames file
  sample. No casing change (CON-12 stays deferred). `asd-cli schema export` writes
  `docs/spec/schemas/asd/session-v1.7.json`; `session-v1.6.json` stays committed and
  checked.
- **SCH-2** The app writes 1.7 from the first build that runs the Rust pipeline
  (WP-G.9). Backend MUST accept `X-Schema-Version` ∈ {`1.6`, `1.7`} with the body
  version equal to the header (CON-21 becomes a set); 1.6 sessions get `NULL` tracking
  columns.
- **SCH-3** `ScoringProfile` gains `trackingQuality: TrackingQualityParams` with
  `#[serde(default)]`; `SCORING_PROFILE_SCHEMA_VERSION` becomes `1.1.0`;
  `config-bundle-v1.json` is regenerated (additive). Every threshold named in this spec
  lives there, with the defaults stated here, so the bench outcome (BEN-6) is a config
  change, not a code change.
- **SCH-4** Backend: migration adds `sessions.tracking_verdict`,
  `tracking_accuracy_deg`, `calibration_quality`, `calibration_model`,
  `tracking_pipeline_version`, `device_model` (all nullable, derived from
  `rawPayload.trackingQuality` at ingest like ING-2), and the review reason
  `low_tracking_quality` (verdict `poor` or `unusable`) added to the `review_items.reason`
  CHECK constraint. API: session list items gain `tracking_verdict`,
  `tracking_accuracy_deg`; list filter `tracking=good|fair|poor|unusable`; session detail
  gains `tracking_quality` (the QS-1 object, snake_case per `/v1`).

## 9. Scoring coupling (SCR) - opt-in, owner-approved (WP-G.13)

- **SCR-1** A clip is low-confidence (existing `lowConfidence` flag, so CORE-4 flag rules
  apply unchanged) when, in addition to today's `validSampleRatio < 0.60`:
  `estimatedAccuracyDeg > maxAccuracyDegForScoring` (default 5.0°), or the session verdict
  is `unusable`.
- **SCR-2** AOI resolution guard: a metric row whose target AOI has `aoiMinSizeDeg <
  aoiResolutionFactor × estimatedAccuracyDeg` (default factor 2.0, i.e. a margin of at
  least one accuracy on each side, after Orquin & Holmqvist 2018) is marked
  `belowResolution` and excluded from the clip subscore like an imputed row.
- **SCR-3** I-VT in degrees per second (default 30°/s, Olsen 2012) using the per-sample
  eye position and device geometry, replacing G-F15's normalised threshold; fixation
  segmentation uses gap-filled samples (GAP-2).
- **SCR-4** These change scoring outputs, so they ship as a versioned change: new golden
  files (FFI-4: new names, never edits), `PerVideoSessionScore` gains
  `scoringRulesVersion`, and the console shows it. Off until the owner approves (H-G2).

## 10. Devices (DEV)

- **DEV-1** Capture requires TrueDepth face tracking and iOS 18+ (unchanged). Devices
  with TrueDepth that run iOS 18 are iPhone XS / XR and later (no SE model has
  TrueDepth) and iPad Pro 11" (all generations) and 12.9" (3rd generation) and later,
  including the 2024 13" iPad Pro whose camera sits on the long edge; no iPad Air or
  mini has TrueDepth. The runtime check stays `ARFaceTrackingConfiguration.isSupported`;
  this list is for the matrix, verify each entry against Apple's spec pages in WP-G.14.
- **DEV-2** Support tiers, kept as a table in this file (filled by WP-G.14):
  `A` = geometry measured and bench-qualified (BEN-4 pass), recommended for clinic use;
  `B` = geometry from spec, accuracy known only from in-session validation;
  `C` = unknown model (idiom default), sessions capped at `fair`.
- **DEV-3** The table records per model: tier, geometry source, bench median accuracy
  and precision per condition, max face-tracking frame rate observed, and whether
  eye/mouth AOIs pass SCR-2 at 45 cm at the bench accuracy.
- **DEV-4** Geometry entries may later be delivered in the signed bundle (a
  `device_geometry` document kind) so a new device does not need an app release; not
  in this version (the table is compiled in).

### Device matrix (to be filled by WP-G.14)

| Model | Identifiers | Tier | Geometry source | Accuracy baseline (°) | Accuracy distance ±10 cm (°) | RMS-S2S (°) | Eye/mouth AOI resolvable |
| --- | --- | --- | --- | --- | --- | --- | --- |
| (bench pending) | | | | | | | |

## 11. Bench protocol (BEN)

- **BEN-1** Participants: adult volunteers with written consent (n ≥ 8, at least 2 with
  glasses, mixed eye colour), 2 sessions each per device. Children only under an ethics
  approval (H-G4); none in this version.
- **BEN-2** Bench build: Debug-only flag `ASD_BENCH_CAPTURE` (never in Release or
  DemoRelease; `scripts/check_release_symbols.sh` asserts it) adds a bench screen that
  runs calibration (5 and 9 point), the 13-point grid (VAL-5) and a free-viewing clip,
  and writes `RawTrackingSample` JSONL (REP-1) plus a target log to the app's
  Documents directory for manual export. No face images, no video.
- **BEN-3** Setup: device on a fixed stand, screen vertical, at 40, 50 and 60 cm measured
  with a ruler from the glass to the outer eye corner; camera offset of each device
  measured with calipers (feeds GEO-1 `measured`).
- **BEN-4** Condition matrix per device: `baseline` (50 cm, office light ≈ 300–500 lux,
  head free but still); `distance` (calibrate at 50, validate at 40 and 60); `head`
  (validate with head turned ±15° yaw and ±10° pitch); `motion` (participant follows a
  slowly moving target while swaying); `glasses`; `lowLight` (≈ 50 lux);
  `sideLight` (single lamp at 90°); `occlusion` (hand covering the mouth; one eye
  partly covered by hair); `blinks` (instructed blinks every 2 s). Tier A pass:
  median baseline accuracy ≤ 2.5°, `distance` ≤ 3.5°, filtered RMS-S2S ≤ 0.5°, baseline
  data loss ≤ 10 %.
- **BEN-5** Analysis is `asd-cli gaze bench-report --logs <dir> --models all` (WP-G.5):
  replays every log through the Rust pipeline with each calibration model and each
  layout, and prints accuracy/precision/loss per device × condition × model, plus
  whether light level predicts error (decides CNF-2).
- **BEN-6** Decisions taken from the report, recorded in this file and in ADR-0007 D6:
  the default calibration model and `autoSelect`, the calibration bands (CAL-5), the
  validation and drift thresholds, and the DEV-2 tiers. They change config defaults
  (SCH-3), not code.
- **BEN-7** Optional reference tracker (H-G5): a wearable tracker with screen-surface
  mapping (for example Pupil Labs Neon with AprilTag markers around the device, or
  Tobii Pro Glasses 3), synchronised by an on-screen flash at start and end. Compared:
  per-sample angular difference during the 13-point grid and AOI dwell agreement
  (ICC(2,1)) on the free-viewing clip. Not a gate for Tier A.
- **BEN-8** Bench logs are personal biometric data: stored outside git (encrypted
  bucket or encrypted disk), referenced by SHA-256 in the bench report; never committed.

## 12. Test vectors (REP, TV)

- **REP-1** The `RawTrackingSample` JSONL format (PIPE-2, one JSON object per line, a
  header line with `format: "asd-raw-tracking/1"`, device model, orientation, geometry
  and target schedule) is defined in Rust; `asd-cli gaze replay --check <file>`
  validates a log.
- **TV-1** `asd-core::synth_tracking` (feature `synth`) generates raw logs from a known
  gaze script with seeded noise: per-user angular bias and gain, head translation and
  rotation paths, distance changes, linear drift, blinks (100–400 ms), track-loss runs,
  artefact spikes, 30 Hz and 60 Hz frame clocks, every orientation. Ground truth is in
  the header.
- **TV-2** Golden scenario files under `rust/crates/asd-core/tests/golden/gaze/` (each
  `{input, expected}`, new file per change): at least `ideal`, `bias_only`,
  `bias_gain`, `distance_shift_after_cal`, `head_yaw_15`, `drift_3deg_linear`,
  `blinks_and_loss`, `gap_60ms_vs_200ms`, `artifact_spike`, `orientation_landscape_ipad`,
  `fps_30`, `missing_target`. Expected values include accuracy recovered within stated
  tolerances (for example `bias_only`: validation accuracy ≤ 0.2° after calibration;
  `drift_3deg_linear`: DRF-3 fires at or before the second check after drift exceeds
  1.5°; `gap_60ms_vs_200ms`: the first gap filled, the second not).
- **TV-3** Synthetic vectors only are committed. Real bench logs may become goldens only
  with the owner's written consent decision (H-G6), and then only adult volunteer logs.

## 13. Console (CSL)

- **CSL-1** Session detail Quality tab gains a "Tracking" section: verdict badge,
  estimated accuracy (°), validation accuracy and precision, calibration model + quality
  + LOO-CV, device model and geometry source, pipeline version; a drift chart (check
  offsets over session time with the warn/hard lines); a loss bar (tracked / blink /
  lost / offscreen / artefact share); conditions (distance median and IQR, head speed,
  light, glasses). 1.6 sessions show "not recorded (app version before tracking
  quality)". The "distance" row shows `calibrated_distance_cm` only when present.
- **CSL-2** Sessions table gains a tracking verdict column and filter; review queue
  shows reason `low_tracking_quality`.
- **CSL-3** zod schemas follow `api-v1.json` (API-3 contract test covers the new fields).
- **CSL-4** An org-level tracking summary per device model (median estimated accuracy,
  share of `poor`/`unusable`) is listed for WP-C.6 and is not part of this module.

## 14. Platforms (PLT)

- **PLT-1** visionOS: apps get no eye-gaze data (gaze drives system hover only) and no
  ARKit face tracking, so visionOS is not a capture platform. `CaptureEntryGate`
  already blocks it (SIM-7). A future visionOS build may show stimuli or the clinician
  views only.
- **PLT-2** Android: no TrueDepth equivalent; ARCore Augmented Faces gives a face mesh
  but no eye gaze. An Android capture front-end would produce `RawTrackingSample`s from
  an RGB face/iris landmark model; everything from raycast onward (geometry table entry,
  calibration, validation, drift, confidence, `TrackingQuality`) is reused unchanged via
  the same UniFFI definitions (WP-A.12). It needs its own bench qualification, its own
  `captureSource` value (a schema change) and is expected to be less accurate.
- **PLT-3** Nothing in `asd-core::gaze` may depend on ARKit conventions beyond the
  `RawTrackingSample` fields; the face-frame axis convention is a documented constant
  per `source`.

## Public Rust API sketch (load-bearing names only)

```rust
// asd-core
pub mod gaze {
    pub mod geometry;     // DeviceGeometry, lookup(model_id), rotate(orientation), norm_to_mm, angle_deg
    pub mod raycast;      // raycast(sample, geometry) -> RayHit { point_norm, eye_pos_mm, status }
    pub mod calibration;  // CalModelKind, CalModel::{fit, apply}, quality(points, model, geo), CalibrationSession
    pub mod filter;       // OneEuro, OneEuro2D (port of OneEuroFilter.swift)
    pub mod track_state;  // TrackStateMachine (time-based blink recovery)
    pub mod confidence;   // sample(features, params) -> f64, fixation(..)
    pub mod gapfill;      // fill(samples, params) -> Vec<FilledSample>
    pub mod aoi_space;    // video_to_screen(aoi, video_aspect, screen_aspect, gravity)
    pub mod validation;   // target_result(samples, target, geo), summarize(targets)
    pub mod drift;        // DriftMonitor, advise(state) -> RecalAdvice
    pub mod pipeline;     // GazePipeline
    pub mod summary;      // tracking_quality(..) -> TrackingQuality, verdict(..)
}
pub mod types { pub mod tracking; } // RawTrackingSample, ProcessedSample, TrackState, TrackingQuality, TrackingQualityParams, ...
#[cfg(feature = "synth")] pub mod synth_tracking;

// asd-ffi (WP-G.7): #[uniffi::Object] GazePipelineHandle wrapping gaze::pipeline::GazePipeline,
// flat #[uniffi::Record] RawSampleRecord / ProcessedSampleRecord for the per-frame path,
// JSON strings for config in and TrackingQuality out (ADR-0006 D4).
```

## Evidence used for the calibration and quality decisions

Cited for the reasoning in ADR-0007; implementers do not need them to build.

- Holmqvist, Nyström & Mulvey (2012), "Eye tracker data quality: what it is and how to
  measure it", ETRA: accuracy/precision definitions, RMS-S2S vs SD.
- Cerrolaza, Villanueva & Cabeza (2012), "Study of polynomial mapping functions in
  video-oculography eye trackers", ACM TOCHI: higher-order mapping terms give little
  benefit; 2nd order with about 9 points is the usual ceiling.
- Hansen & Ji (2010), "In the eye of the beholder: a survey of models for eyes and
  gaze", IEEE TPAMI: 2D regression mappings degrade with head movement; geometric
  (3D) models with per-user angular offsets are head-pose tolerant. Basis for
  `angularAffine`.
- Niehorster, Cornelissen, Holmqvist, Hooge & Hessels (2018), "What to expect from your
  remote eye-tracker when participants are unrestrained", Behavior Research Methods:
  data loss and accuracy under head movement; supports the `head`, `motion` bench
  conditions.
- Wass, Forssman & Leppänen (2014), "Robustness and precision: how data quality may
  influence key dependent variables in infant eye-tracker analyses", Infancy: low
  precision and flicker bias fixation durations and AOI measures in young children;
  supports per-session quality reporting and gap rules.
- Dalrymple, Manner, Harmelink, Teska & Elison (2018), "An examination of recording
  accuracy and precision from eye tracking data from toddlerhood to adulthood",
  Frontiers in Psychology: toddler data quality is worse than adult data; supports
  measuring it per session instead of assuming bench numbers.
- Olsen (2012), "The Tobii I-VT fixation filter" (Tobii technical report): 75 ms
  gap fill, 30°/s velocity threshold.
- Orquin & Holmqvist (2018), "Threats to the validity of eye-movement research in
  psychology", Behavior Research Methods: AOI size and margins relative to accuracy.
- Krafka et al. (2016), "Eye Tracking for Everyone", CVPR, and Valliappan et al. (2020),
  Nature Communications: phone-based gaze reaches about 1–2 cm (roughly 1–3°) only with
  per-user calibration; supports mandatory per-session calibration and validation.

Our own evidence, which overrides the literature for defaults: the WP-G.14 bench report
(BEN-5, BEN-6). Until it exists, every threshold above is provisional and labelled so in
`TrackingQualityParams` (`provisional: true`).
