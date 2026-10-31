# ADR-0007: Gaze tracking measures its own quality, in degrees, with the per-frame maths in Rust

Status: Proposed, 2026-10-31
Spec ids: GEO-*, CAL-*, VAL-*, DRF-*, TRK-*, CNF-*, GAP-*, PIPE-*, QS-*, SCH-*, SCR-*,
DEV-*, BEN-*, REP-*, TV-*, CSL-*, PLT-* in
`docs/spec/modules/asd-gaze-robustness/technical.md`
Related: ADR-0006 (Rust core; this ADR supersedes its decision 6 for gaze maths),
ADR-0001 (simulation provenance), ADR-0004 (non-diagnostic claims).

## Context

The ASD app turns ARKit TrueDepth face tracking into screen gaze: `lookAtPoint` →
`ScreenRaycaster` → affine `GazeCalibrationProfile` → `OneEuroFilter2D` →
`StimulusAOITracker` → I-VT fixations and AOI dwell. All scoring downstream (per-clip
rows, composite, traffic-light flag) assumes these gaze points are good enough to tell
a face from a pattern and an eye region from a mouth region.

Nothing checks that assumption. The review of `dev` at `1f43026` (technical.md
G-F1…G-F19) found:

- geometry defects that bias every sample: clamping before calibration, validity
  decided on the uncalibrated point, a raycast branch that accepts an off-plane point,
  two hard-coded screen sizes, no orientation handling, AOIs hit-tested in the wrong
  space for an aspect-fill video;
- no quality measurement: a constant `confidence` of 0.95, track-loss frames dropped
  before they can lower `validRatio`, a constant `calibratedDistanceCm` of 35;
- a calibration that is biased in how it collects samples, invents a zero-error point
  when a target gets no samples, is never scored, and can be reused for another child;
- a fixation threshold in normalised screen units, so fixation metrics differ between
  iPhone and iPad for the same eye movements;
- an eye AOI that is about 2.7° × 3.5° on a phone, close to what calibrated ARKit gaze can
  resolve at all.

ADR-0006 decision 6 left the real-time filter, raycast and calibration in Swift "for
now". `CLAUDE.md` (2026-10-30) requires gaze and AOI maths in Rust. The fixes above are
almost all maths, so they are the moment to move it.

## Decisions

1. **Per-frame gaze maths moves to Rust** (`asd-core::gaze`: geometry, raycast,
   calibration models and quality, track state, confidence, gap fill, AOI-space
   mapping, 1-Euro filter, validation, drift, session summary). This supersedes ADR-0006
   decision 6 for these items. Swift keeps ARKit capture, the frame-to-record
   copying, orientation lock, timers, target animation, sound and UI. The Swift
   `ScreenRaycaster`, `GazeCalibrationProfile.fit/apply` and `OneEuroFilter` bodies move
   to the test target as parity oracles for one release (same rule as FFI-3), then are
   deleted.
2. **A stateful `GazePipeline` object behind UniFFI, with flat records on the per-frame
   path.** The per-frame call (60 Hz) takes a `RawSampleRecord` and returns a
   `ProcessedSampleRecord` (about 40 numbers each way). This is a narrow, deliberate
   exception to ADR-0006 decision 4 (JSON at the boundary), justified by the call rate;
   configuration in and the session `TrackingQuality` out stay JSON. The Swift
   `GazeProcessingActor` owns the one pipeline instance, so the Rust object is
   uncontended (an internal `Mutex` satisfies UniFFI's `Sync` requirement). Budget:
   ≤ 0.2 ms p99 per call (PIPE-4).
3. **Degrees of visual angle are the unit** for accuracy, precision, drift, offsets,
   AOI size checks and (with SCR-3) saccade velocity. This needs a per-model device
   geometry table (screen millimetres and camera offset) compiled into `asd-core`, the
   interface orientation, and the per-sample eye position from the raycast. Unknown
   models fall back to today's idiom defaults and the session says so.
4. **Every session measures its own accuracy.** After each calibration a 4-target
   validation runs (about 6 s); between clips and after the last clip a single check
   target runs (about 1.2 s each). The session records validation accuracy and precision,
   every check offset, and a conservative `estimatedAccuracyDeg = max(validation, median
   check offset)`. An accuracy that was not measured is absent, never defaulted.
5. **Calibration is per session and per participant, collected on tracking quality, and
   scored.** No reuse across children, no "skip with saved calibration". Samples are
   accepted when the face is tracked and the sample is confident, not when raw gaze is
   already near the target. Per-target robust medians; a target without enough samples is
   re-shown once, then left out; never replaced by its own position. Quality = median
   leave-one-out error in degrees, banded `good|fair|poor|failed`.
6. **Calibration model: the evidence decides, the code supports the candidates.**
   Rust implements `screenAffine` (today), `angularAffine` (affine correction of the
   eye-in-head direction before the raycast), `screenHomography` and `screenPoly2`.
   With the 5-point child layout only the two affine models are eligible (a model needs
   at least two points more than its minimum); the 9-point layout makes all four
   eligible. The default stays `screenAffine` with `autoSelect` off until the WP-G.14
   bench shows another model better by at least 0.3° median across the condition
   matrix. The expected winner is `angularAffine`, because 2D screen-space mappings
   degrade when the head moves or the distance changes and child heads move (Hansen &
   Ji 2010; Niehorster et al. 2018), while per-user angular offsets are the
   head-tolerant model. Higher-order polynomials are not expected to help with 5–9
   points (Cerrolaza et al. 2012). The bench outcome changes a config default, not code.
7. **Per-sample confidence is the minimum of ramped factors that meet 0.5 at today's
   hard limits.** So the existing quality gate (`minConfidence` 0.5) keeps exactly its
   distance, pose and blink boundaries and newly rejects only fast head motion,
   implausible eye speed and bad rays. Per-fixation confidence is the mean over its
   samples. Light is recorded but is not a factor until the bench shows that it matters
   (TrueDepth is infrared-assisted).
8. **Track loss is recorded; gaps are filled only for fixation segmentation.** Every
   ARKit frame in a recording becomes a frame, with a track state. Gaps up to 75 ms and
   1° are interpolated for I-VT only, never for dwell, `validRatio` or AOI entry
   (PLC-5's 100 ms dwell cap already covers short gaps). This lowers `validRatio` for
   sessions with track loss. That is intended (the old number was inflated) and is
   called out to the owner (H-G1).
9. **Detect drift and re-calibrate at clip boundaries; do not correct samples.**
   Drift beyond 1.5° on two consecutive checks, or beyond 3° once, advises a
   re-calibration before the next clip (at most one inside the battery). Post-hoc
   correction of recorded samples is left out: it would need AOI hit testing moved
   to finalize in Rust and a rule for interpolating offsets between checks. It is the
   first follow-up once drift data exists.
10. **Contract: session schema 1.7 is additive.** It adds typed `rawPayload.trackingQuality`,
    per-clip confidence and AOI-size fields, per-frame `track_state` and `gap_filled`, and
    makes `rawPayload.calibratedDistanceCm` optional (it becomes the measured
    calibration distance). The backend accepts 1.6 and 1.7. It does not normalise
    casing (CON-12 stays deferred). All thresholds live in
    `ScoringProfile.trackingQuality` (bundle-delivered, `serde(default)`, profile schema
    1.1.0).
11. **Scoring uses tracking quality only through an opt-in, versioned change** (SCR-1..4,
    WP-G.13): a session accuracy cap and an AOI resolution guard both mark clips or rows
    low-confidence, so the existing flag rules apply unchanged, and I-VT moves to 30°/s.
    New golden files, `scoringRulesVersion` on the score, owner approval first.
12. **Platforms.** visionOS is not a capture platform (no gaze data for apps, no face
    tracking). Android plugs a different native front-end into the same
    `RawTrackingSample` and reuses everything after it; it needs its own bench
    qualification and a new `captureSource` value later.
13. **Bench logs are personal data.** Raw tracking logs from volunteers are numbers, not
    images, but they are biometric. They stay out of git (BEN-8). Committed golden
    vectors are synthetic, from a seeded Rust generator with known ground truth.

## Alternatives rejected

- **Keep the per-frame maths in Swift and fix it there.** It breaks `CLAUDE.md`, gives
  Android a second implementation to write, and leaves the bench analysis (Rust CLI) and
  the device (Swift) computing accuracy with different code.
- **JSON per frame across the FFI.** 60 allocations, encodes and decodes per second for a
  fixed, flat shape; flat UniFFI records cost one copy and keep the contract typed. JSON
  remains for documents, as ADR-0006 intends.
- **Homography or 2nd-order polynomial as the default with 5 points.** Eight or twelve
  parameters fitted from five points leave zero or negative redundancy; leave-one-out error
  cannot even be computed meaningfully. Kept as candidates for the 9-point layout only.
- **Gate calibration sample collection on raw-gaze proximity (current behaviour).** It
  selects the samples that need correcting least and biases the fit; the original
  author already found the hard version of this gate defective.
- **Online drift correction (shift the calibration offset after each check).** Easy to
  write and hard to validate: a child looking at the edge of a check target looks like
  drift. Detection plus a proper re-calibration is safer until drift data exists.
- **Kalman or other model-based smoothing instead of 1-Euro.** No evidence that the
  1-Euro filter limits accuracy; it adds a motion model to tune per age. The filter
  stays, ported, with parameters in config.
- **An appearance-based RGB gaze network.** Needs a model with a shippable licence and
  training data, and is generally less accurate than TrueDepth-assisted tracking.
  Relevant for Android later, not for iOS now.
- **Require a reference tracker for every supported device.** It costs hardware and
  coordination; validation targets with known positions already give accuracy. The
  reference tracker is optional and answers a different question (agreement on free
  viewing).
- **Block the session when accuracy is poor.** Child attention is the scarce resource;
  the session is kept, honestly labelled, and scoring treats it as low-confidence
  (with WP-G.13) rather than discarding it.

## Consequences

- `validRatio` falls for sessions with track loss (G-F7 fix) and a few more samples fail
  the gate (motion, artefacts). The console's `low_quality` review rule (`validRatio <
  0.80`) will fire more often. Norms are literature-seeded today, so no collected
  baselines need recomputing; the norm-collection export must record `pipelineVersion`
  so future collected norms do not mix pipelines.
- Express sessions get longer: validation (about 6 s) plus 4 check targets (about 5 s) on
  top of calibration. The "30–60 s at home" claim covers the stimulus time only; the
  owner decides whether the total is acceptable (H-G3).
- The iOS kit gains a hot-path FFI dependency; WP-G.9 cannot start before WP-R.5
  (xcframework) and WP-A.11 (kit consumes Rust) land.
- Session schema gains a version (1.7); the backend and console must accept both
  versions for as long as 1.6 apps exist.
- The bench needs a person, a stand, a ruler, calipers and a few hours per device;
  thresholds stay `provisional` until it is done.
- Eye-versus-mouth metrics on iPhones are expected to fall below the resolution guard
  often. If the owner enables WP-G.13, those rows become `belowResolution`, and the
  recommended clinic device becomes an iPad Pro (H-G2).
