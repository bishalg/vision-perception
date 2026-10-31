# ASD Gaze Tracking Robustness - Work Packages

Companion to `technical.md` (spec ids), `acceptance.md` and ADR-0007. The live status
table is `docs/roadmap_2026q4/04_asd_phased_work_packages.md §0`; this file holds the
detail. Inside a wave, packages share no files and can run in parallel; they still
share one working tree, so repo-wide gates (`scripts/rust_test.sh`,
`scripts/ios_test.sh`, `pnpm -C apps/curv_console lint test e2e`) run in the main
session after integration. Build outputs go under `/private/tmp/asd-*` or `build/`.

Tier legend: `S` = `senior-implementer` (Opus); `I` = `implementer` (Sonnet).
Every package: write the listed tests first, then the code; report what was skipped.
Only `rust/Cargo.lock` and `rust/crates/asd-core/src/lib.rs` (one `pub mod` line each)
may be touched by more than one package; the main session resolves them at integration.

External dependencies: **WP-R.5** (asd-ffi + xcframework) gates WP-G.7; **WP-A.11**
(kit consumes Rust, golden parity harness) gates WP-G.9. WP-G.0, G.1–G.6, G.8, G.11,
G.12 do not wait for either.

## Wave 0 (start now, parallel)

### WP-G.0 Swift capture hygiene, no new maths - `I`

Goal: TRK-1 (record untracked frames), CAL-1 (no cross-child reuse), CAL-3 (no
fabricated target point), GEO-3/GEO-4 groundwork (orientation lock). Native-only
changes; no new arithmetic in Swift.

Files to touch:
`domains/asd_screening/ios/Sources/CurvAsdGazeKit/GazeProcessingActor.swift` (the
`guard snapshot.isTracked` branch buffers a frame with `isValid = false`,
`qualityVerdict = .rejected`, `confidence = 0`, the last valid point, and the
measured interval, when recording),
`apps/ios_asd_gaze_app/Sources/Views/BubbleCalibrationView.swift` (a target with zero
collected samples is re-shown once, then the calibration fails visibly with a "try
again" state; no `?? targetPoints[i]` fallback; layout ignores safe areas, no
navigation bar during capture),
`apps/ios_asd_gaze_app/Sources/Views/GazeScreeningWizardView.swift` (remove the "skip,
use saved calibration" path), `apps/ios_asd_gaze_app/Sources/UI/AppLocalizer.swift`
(remove `wizard_skip_calibrate`), the capture views (`GazePositioningStepView.swift`,
`BubbleCalibrationView.swift`, `GazeStimulusScreeningView.swift`) lock the interface
orientation from positioning until results (portrait on iPhone; on iPad the orientation
in effect at positioning).

Files not to touch: `ScreenRaycaster.swift`, `GazeCalibrationProfile.swift`,
`OneEuroFilter.swift` (they move to Rust; fixing their maths here is wasted work),
anything under `rust/`, `apps/curv_console/`.

Tests first (`domains/asd_screening/ios/Tests/CurvAsdGazeKitTests/`):
`GazeProcessingActorTests` — 60 tracked + 30 untracked snapshots → 90 buffered frames,
`validRatio` 60/90 (within rounding), untracked frames have `isValid == false`; an
interruption still buffers nothing (CAM-2 unchanged). App tests: wizard has no skip
action; calibration with one target starved of samples does not call
`applyAndPersist`.

Commands: `scripts/ios_test.sh`.
Acceptance: acceptance scenario 1.
Depends on: nothing. Parallel with: WP-G.1. Note for the owner: lowers `validRatio` on
sessions with track loss (H-G1); default plan is to land it **after** the 2026-11-10
demo unless the owner says otherwise.

### WP-G.1 Rust tracking types, raw log format, device geometry, raycast, angles - `S`

Goal: GEO-1..GEO-6, PIPE-2, QS-1 types, REP-1 format, the module tree for everything
after it. Everything later compiles against these types.

Files to touch (all new unless stated):
`rust/crates/asd-core/src/gaze/mod.rs` and **every** module file of the technical.md
API sketch as a stub with an `//! owner: WP-G.x` line (`geometry`, `raycast`,
`calibration`, `filter`, `track_state`, `confidence`, `gapfill`, `aoi_space`,
`validation`, `drift`, `pipeline`, `summary`); `gaze/geometry.rs`, `gaze/raycast.rs`
implemented; `rust/crates/asd-core/src/types/tracking.rs` (`RawTrackingSample`,
`RawLogHeader`, `Orientation`, `TrackState`, `RayStatus`, `ProcessedSample`,
`TargetKind`, `TargetResult`, `CalModelKind`, `CalibrationQuality`, `TrackingQuality`
and its sub-structs, `TrackingQualityParams` with every default from technical.md and
`provisional: true`, `RecalAdvice`, `RecalReason`); `types/mod.rs` (one `pub mod`
line); `rust/crates/asd-core/resources/device_geometry.json` (only entries with a
cited `ref`: start with the two idiom defaults as `legacy-default` plus models whose
resolution and ppi Apple publishes, `cameraOffsetMm` marked `unmeasured` and set to
the idiom default offset); `asd-core/src/lib.rs` (one `pub mod gaze;` line);
`asd-core/tests/gaze_geometry.rs`, `asd-core/tests/gaze_raycast.rs`,
`asd-core/tests/tracking_types_roundtrip.rs`.

Files not to touch: existing `asd-core` modules (`quality.rs`, `metrics.rs`, ...),
`asd-contract`, `asd-ffi`, Swift.

Tests first: geometry lookup (known id, unknown id → idiom default + source), rotation
for all four orientations (screen dims swap, camera offset rotates), `norm_to_mm`, visual
angle against hand-computed values (0°, 1° at 57.3 cm, 10°, the exact `atan2` form);
raycast analytic cases (GEO-6) including `t > 2.5` → `outOfRange`, backward ray,
grazing ray, missing eye transforms → face centre + 3 cm; off-screen hit returned
unclamped (x = −0.2 stays −0.2); round-trip of every type through JSON (camelCase,
optional fields absent not null).

Commands: `cargo test --manifest-path rust/Cargo.toml -p asd-core`,
`scripts/rust_test.sh`.
Acceptance: scenario 2.
Depends on: WP-R.0 (done). Parallel with: WP-G.0.

## Wave 1 (after WP-G.1, parallel)

### WP-G.2 Calibration models, robust target aggregation, quality, selection - `S`

Goal: CAL-2..CAL-7 maths, `CalibrationSession` state machine of CAL-8 (timing and
sample acceptance only; no UI).
Files: `asd-core/src/gaze/calibration.rs`, `asd-core/tests/gaze_calibration.rs`,
`asd-core/tests/golden/gaze/calibration/*.json`.
Not to touch: `types/tracking.rs` (ask the main session for a type change), other gaze
modules.
Tests first: `screenAffine` reproduces every case of `GazeCalibrationProfileTests.swift`
(offset, scale+offset, too few points, mismatched counts, degenerate points) except
clamping, which is intentionally gone; `angularAffine` recovers a synthetic yaw/pitch
bias and gain exactly with noise-free input and keeps validation error < 0.3° when the
head moves 10 cm after calibration while `screenAffine` exceeds 1°; homography (Hartley
normalisation, exact on 4 points), poly2 (exact on 6+ points); eligibility (`minPoints
+ 2`); LOO-CV and residual in degrees; quality bands at each threshold edge; MAD
rejection drops a planted outlier; a target below `minSamplesPerTarget` → `NeedsRetry`
then `Missing`; `autoSelect` picks the simpler model when the gain is < 0.3°; condition
number guard.
Commands: `cargo test -p asd-core`, `scripts/rust_test.sh`.
Depends on: G.1.

### WP-G.4 Validation, drift monitor, re-calibration advice, session summary - `S`

Goal: VAL-2..VAL-4, DRF-1..DRF-3, CAL-9 advice, QS-1, QS-2, QS-4.
Files: `asd-core/src/gaze/{validation,drift,summary}.rs`,
`asd-core/tests/gaze_{validation,drift,summary}.rs`,
`asd-core/tests/golden/gaze/{validation,drift,summary}/*.json`.
Not to touch: `types/tracking.rs`, `calibration.rs`, `pipeline.rs`.
Tests first: per-target accuracy (median of post-settle samples), RMS-S2S and SD on
hand-built sequences; `missed` below the sample minimum; `complete` with 3 of 4;
drift relative to the mean validation offset (constant bias → drift 0); DRF-3 both
rules and a non-consecutive warn pair that must not fire; pose-shift advice suppressed
for `angularAffine`; advice ceiling (`maxRecalibrationsPerSession`, 1 inside the battery);
QS-2 for each of the four availability cases; QS-4 verdict at each boundary, including
the `idiomDefault` and `skipped` caps; absent fields serialise as absent keys.
Commands: `cargo test -p asd-core`, `scripts/rust_test.sh`.
Depends on: G.1. Parallel with G.2 (validation takes processed samples, not models).

### WP-G.6 Contract 1.7 and `TrackingQualityParams` in the scoring profile - `S`

Goal: SCH-1, SCH-3, QS-3 and QS-5 on the wire.
Files: `rust/crates/asd-contract/src/session_v1_7.rs` (new; reuses the 1.6 structs where
unchanged, typed `tracking_quality: Option<TrackingQuality>` on the record,
`calibrated_distance_cm: Option<f64>`, per-clip fields), `asd-contract/src/lib.rs`,
`asd-contract/src/config.rs` (`ScoringProfile.tracking_quality` with `serde(default)`,
`SCORING_PROFILE_SCHEMA_VERSION = "1.1.0"`), `asd-contract/src/mirror.rs` (schema
mirrors for the new core types), `asd-contract/src/schema.rs` (export 1.7 next to 1.6),
`asd-contract/tests/session_v1_7.rs`, `asd-contract/tests/config_defaults.rs`,
`docs/spec/schemas/asd/session-v1.7.json` (new, generated),
`docs/spec/schemas/asd/config-bundle-v1.json` (regenerated),
`apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json` (new fixture, hand-made
from the 1.6 one plus a full `trackingQuality`).
Not to touch: `session_v1_6.rs` behaviour (1.6 must keep validating the 1.6 fixture),
`asd-backend`, `api_v1.rs` (that is G.11).
Tests first: the 1.7 fixture validates and round-trips through JCS; the 1.6 fixture
still validates against 1.6 and is rejected by nothing in 1.7 decoding; `trackingQuality`
absent is valid; a 1.7 payload with `calibratedDistanceCm` absent validates; an old
bundle without `trackingQuality` decodes to defaults; the exported default
`TrackingQualityParams` equals the asd-core defaults (single source).
Commands: `cargo test -p asd-contract`, `cargo run -p asd-cli -- schema check`,
`scripts/rust_test.sh`.
Depends on: G.1.

### WP-G.8 Bench capture build (iOS, Debug-only) - `I`

Goal: BEN-2, REP-1 writer. Gives the bench raw data early, independent of the Rust
pipeline integration.
Files: `apps/ios_asd_gaze_app/Sources/Views/BenchCaptureView.swift` (new),
`apps/ios_asd_gaze_app/Sources/Models/BenchLogWriter.swift` (new; writes the
`asd-raw-tracking/1` JSONL defined in G.1 field for field, plus a target log),
`domains/asd_screening/ios/Sources/CurvAsdGazeKit/ARSessionBridge.swift` (an optional raw
sample tap compiled only under `ASD_BENCH_CAPTURE`, fed on the AR delegate queue, also
carrying `ARFrame.lightEstimate` and the interface orientation),
`apps/ios_asd_gaze_app/project.yml` (`ASD_BENCH_CAPTURE` in Debug only),
`scripts/check_release_symbols.sh` (assert absent from Release and DemoRelease), entry
row in `SettingsView.swift` under the flag, and the 13-point grid + 5/9-point target
sequences (fixed timings, no gaze feedback).
Not to touch: `GazeProcessingActor.swift` (G.0 owns it in wave 0), Rust, console.
Tests first: `BuildConfigurationTests` asserts the flag is absent in Release and
DemoRelease; a unit test encodes one sample and checks every key name and the header
against a committed sample line from G.1 (`asd-core/tests/fixtures/raw_tracking_sample.jsonl`);
`asd-cli gaze replay --check` accepts a 10-second log recorded on a device (manual, in
the report).
Commands: `scripts/ios_test.sh`.
Depends on: G.1 (format), G.0 (merged first, to avoid touching capture views while they
change).

## Wave 2

### WP-G.3 Per-sample pipeline: track state, confidence, filter, gap fill, AOI space - `S`

Goal: TRK-1, TRK-2, CNF-1, CNF-3, CNF-4, GAP-1..GAP-3, GEO-7, PIPE-1, PIPE-3, PIPE-5.
Files: `asd-core/src/gaze/{track_state,confidence,filter,gapfill,aoi_space,pipeline}.rs`,
`asd-core/tests/gaze_{track_state,confidence,filter,gapfill,aoi_space,pipeline}.rs`,
`asd-core/tests/golden/gaze/{filter,pipeline}/*.json`.
Not to touch: `calibration.rs`, `validation.rs`, `drift.rs`, `summary.rs` (call them),
`types/tracking.rs`, `metrics.rs` (SCR-3 is G.13).
Tests first: 1-Euro output equals `OneEuroFilterTests.swift` cases to 1e-12; blink
recovery is 50 ms at both 30 and 60 Hz; each confidence factor is exactly 0.5 at its hard
limit and the min-composition reproduces the CORE-7 pass/fail on the existing
`golden/quality` inputs; gap of 60 ms / 0.5° filled, 60 ms / 2° not filled, 200 ms not
filled; filled samples do not change `validRatio` or dwell; aspect-fill mapping of a
1080×1920 video on 393×852 and 820×1180 screens (hand-computed crop); pipeline order
(calibration before filter, filter only on `tracked`); determinism (two runs,
byte-identical JCS).
Commands: `cargo test -p asd-core`, `scripts/rust_test.sh`.
Depends on: G.1, G.2 (`CalModel::apply`), G.4 (`begin_target`/`end_target` use
`validation::target_result`).

### WP-G.11 Backend accepts 1.7, tracking columns, review rule, API fields - `S`

Goal: SCH-2, SCH-4.
Files: `rust/crates/asd-backend/src/ingest/{validate,derive,auto_review}.rs`,
`rust/crates/asd-backend/migrations/0006_tracking_quality.sql` (next free number at
the time; columns of SCH-4, drop and recreate the `review_items.reason` CHECK with
`low_tracking_quality`), `asd-backend/src/read/sessions.rs`,
`asd-backend/src/routes/sessions.rs` (filter `tracking=`), `rust/crates/asd-contract/src/api_v1.rs`
(list item and detail fields), `docs/spec/schemas/asd/api-v1.json` (regenerated),
`rust/.sqlx/` (regenerated), `asd-backend/tests/{sessions,console_read,review}.rs`.
Not to touch: `session_v1_6.rs`, `session_v1_7.rs` (G.6), `config_docs/`.
Tests first: 1.6 post still 201 with NULL tracking columns; 1.7 post 201 with derived
columns; header 1.7 + body 1.6 → 400 `unsupported-schema-version`; verdict `poor` →
`low_tracking_quality` review item in the same transaction; list filter; detail returns
`tracking_quality` snake_case; idempotent replay of a 1.7 body unchanged (CON-17).
Commands: `scripts/rust_test.sh` (Postgres service), `cargo sqlx prepare --check --workspace`.
Depends on: G.6.

## Wave 3

### WP-G.5 Synthetic tracking generator, replay, `asd-cli gaze`, end-to-end goldens - `I`

Goal: TV-1, TV-2, REP-1 check, BEN-5 tool.
Files: `asd-core/src/synth_tracking.rs` (feature `synth`), `asd-core/src/lib.rs` (one
line), `asd-core/tests/gaze_scenarios.rs`, `asd-core/tests/golden/gaze/scenarios/*.json`
(the TV-2 list), `asd-core/tests/fixtures/raw_tracking_sample.jsonl` (if G.1 did not
create it), `rust/crates/asd-cli/src/cmd/gaze.rs` (`gaze synth`, `gaze replay [--check]`,
`gaze bench-report`), `asd-cli/src/cmd/mod.rs`, `asd-cli/tests/gaze_cli.rs`.
Not to touch: any `gaze/*.rs` logic (report defects to the main session).
Tests first: generator determinism (same seed → identical bytes); every TV-2 scenario's
stated tolerance; `replay --check` rejects a log with a missing header, a non-monotonic
`t_sec`, and a wrong format version; `bench-report` over three synthetic "devices"
prints the per-device × condition × model table and ranks models.
Commands: `cargo test -p asd-core --features synth`, `cargo test -p asd-cli`,
`scripts/rust_test.sh`.
Depends on: G.3, G.4 (end-to-end), G.2.

### WP-G.7 FFI: `GazePipelineHandle` and calibration/validation entry points - `S`

Goal: ADR-0007 D2, PIPE-1 across UniFFI.
Files: `rust/crates/asd-ffi/src/gaze.rs` (new), `asd-ffi/src/lib.rs` (one `mod` line),
`asd-ffi/tests/gaze_boundary.rs`.
Not to touch: `asd-core` logic, the xcframework script (R.5 owns it), Swift.
Tests first: flat records round-trip every field; malformed config JSON → `AsdError`, no
panic; a 10-minute synthetic stream through the handle equals the same stream through
`GazePipeline` directly; `summary()` JSON equals `asd-core` output; a Criterion bench
(or a timed test, ignored in CI) reports per-call cost on the host.
Commands: `cargo test -p asd-ffi`, `scripts/build_asd_xcframework.sh`.
Depends on: WP-R.5, G.3, G.4.

### WP-G.13 Scoring uses tracking quality (opt-in, versioned) - `S` (owner gate H-G2)

Goal: SCR-1..SCR-4.
Files: `asd-core/src/{scoring,metrics}.rs` (new rule set behind
`ScoringProfile.trackingQuality.scoringRules = "v2"`, v1 unchanged and default),
`asd-core/src/types/scoring.rs` (`scoringRulesVersion`, `belowResolution` row flag),
`asd-core/tests/golden/{scoring,metrics}/v2_*.json` (new files only),
`asd-core/tests/golden_scoring.rs`, `golden_metrics.rs`.
Not to touch: existing golden files (FFI-4), `flag.rs` (flag rules stay; they see more
low-confidence clips), Swift.
Tests first: v1 goldens unchanged; v2: clip low-confidence when accuracy > cap; eye row
`belowResolution` at 2.7° AOI and 2.0° accuracy, kept at 6.8°; I-VT at 30°/s gives the same
fixation count for the same angular movement on iPhone and iPad geometry (fails on v1,
proving G-F15).
Commands: `cargo test -p asd-core`, `scripts/rust_test.sh`; after WP-A.11 also the kit's
`GoldenParityTests`.
Depends on: G.4, G.6, owner approval.

### WP-G.14 Bench execution, device matrix, threshold and model decision - owner + `I`

Goal: BEN-1, BEN-3..BEN-6, DEV-2, DEV-3.
Files: `docs/spec/modules/asd-gaze-robustness/technical.md` (device matrix table,
measured thresholds, BEN-6 decisions), `docs/spec/adr/ADR-0007-gaze-tracking-robustness.md`
(D6 outcome), `rust/crates/asd-core/resources/device_geometry.json` (measured camera
offsets), the `TrackingQualityParams` defaults in `types/tracking.rs` (flip
`provisional` to false only for values the bench measured), a bench report under
`docs/spec/modules/asd-gaze-robustness/bench/<date>-report.md` (tables only, no raw data).
Not to touch: any logic; raw logs (BEN-8, never committed).
Tests: `asd-cli gaze bench-report` reproduces the committed tables from the logs (hash
listed); `cargo test -p asd-core` still green with the new defaults.
Depends on: G.8 (capture), G.5 (report tool), owner time and volunteers (H-G4, H-G5).

## Wave 4

### WP-G.9 Kit integration: the actor runs the Rust pipeline - `S`

Goal: PIPE-1 on device, PIPE-4, CNF-2 capture, QS-1 in the record, SCH-2 (1.7 writer),
GEO-3 orientation input.
Files: `domains/asd_screening/ios/Sources/CurvAsdGazeKit/GazeProcessingActor.swift`
(owns one `GazePipelineHandle`; per frame builds a `RawSampleRecord`, stores the
`ProcessedSampleRecord` into `GazeFrame`), `ARSessionBridge.swift` (light estimate,
orientation, model identifier from `utsname`), `GazeSessionController.swift`
(calibration/validation/check target API for the views: `beginTarget`, `endTarget`,
`advice`), `GazeTypes.swift` (`track_state`, `gap_filled` on frames, encoded
snake_case), `GazeCalibrationProfile.swift` (becomes a Codable wrapper of the Rust
calibration JSON, bound to `participantToken` and session; the `UserDefaults` store is
removed), `ScreenRaycaster.swift` and `OneEuroFilter.swift` (bodies move to
`Tests/CurvAsdGazeKitTests/Support/LegacyReference/`),
`apps/ios_asd_gaze_app/Sources/Models/{GazeSessionRecord,SessionSyncClient}.swift`
(schema 1.7, `trackingQuality`, optional `calibratedDistanceCm`, `X-Schema-Version: 1.7`),
their tests, `Tests/CurvAsdGazeKitTests/GazePipelineParityTests.swift`.
Not to touch: views (G.10), Rust crates (golden JSON files only), console.
Tests first: replay of the committed synthetic scenario logs through the Swift actor
gives the same `TrackingQuality` JSON as `cargo test` (JCS compare); untracked, blink and
offscreen frames carry the right `track_state`; a 1.7 record validates with
`asd-cli schema validate session-v1.7.json`; per-frame cost measured with `XCTMetric`
on device and reported (PIPE-4); the sync client sends `1.7`; TSan and strict
concurrency clean.
Commands: `scripts/ios_test.sh`.
Depends on: G.7, WP-A.11, G.0, G.6.

### WP-G.12 Console Quality tab, filters, review reason - `I`

Goal: CSL-1..CSL-3.
Files: `apps/curv_console/src/components/QualityPanel.tsx`, new
`apps/curv_console/src/components/{TrackingQuality,DriftChart,LossBar}.tsx`,
`apps/curv_console/src/lib/api/{types,mock}.ts`, `src/components/{SessionsTable,SessionFilters}.tsx`,
`src/messages/en.ts`, `src/lib/api/contract.test.ts` (new fields), component tests,
one Playwright step.
Not to touch: Rust, iOS.
Tests first: zod accepts `api-v1.json` examples with and without `tracking_quality`; 1.6
session renders "not recorded"; drift chart draws warn/hard lines from the profile values
in the payload; distance row shows "-" when `calibrated_distance_cm` is null.
Commands: `pnpm -C apps/curv_console lint test e2e`.
Depends on: G.11. Can run in wave 3 once G.11 has merged.

## Wave 5

### WP-G.10 App UI: child-friendly calibration, validation, drift checks, re-calibration - `I`

Goal: CAL-7, CAL-8, VAL-1, DRF-1, CAL-9 prompts, CNF-2 operator `glasses` question,
GEO-4 layouts.
Files: `apps/ios_asd_gaze_app/Sources/Views/BubbleCalibrationView.swift` (rewritten on
top of `beginTarget/endTarget`; shrinking character targets, sound, pop; no gaze-proximity
gate; quality result screen with "try again" on `poor`/`failed`), new
`ValidationStepView.swift`, new `DriftCheckTargetView.swift`,
`GazeStimulusScreeningView.swift` / `StimulusScreeningView.swift` (insert a check target
between clips and after the last clip; act on `advice()` only at clip boundaries),
`GazePositioningStepView.swift` (optional glasses question, light hint from the recorded
ambient level), `GazeScreeningWizardView.swift` (calibration → validation → battery),
`AppLocalizer.swift` (en/ne/hi strings), Maestro flows under
`apps/ios_asd_gaze_app/maestro/`, UI tests.
Not to touch: kit sources (G.9), Rust, console.
Tests first: wizard order; a `failed` calibration cannot proceed without a retry or an
explicit "continue anyway" that the record shows; a check target appears between every
pair of clips; advice `recalibrate` mid-clip is deferred to the boundary; claims scanner
passes on all new strings (`python3 scripts/check_asd_claims.py --swift --secrets`).
Commands: `scripts/ios_test.sh`, Maestro BDD suite.
Depends on: G.9.

## Parallelisation summary

| Wave | Packages in parallel | Tier | Shared files |
| --- | --- | --- | --- |
| 0 | G.0, G.1 | I, S | none |
| 1 | G.2, G.4, G.6, G.8 | S, S, S, I | `rust/Cargo.lock` only |
| 2 | G.3, G.11 | S, S | none (`api_v1.rs` is G.11 only) |
| 3 | G.5, G.7 (after R.5), G.13 (after H-G2), G.14 (owner), G.12 | I, S, S, owner+I, I | `asd-core/src/lib.rs` one line (G.5), main session merges |
| 4 | G.9 (after A.11) | S | none |
| 5 | G.10 | I | none |

Recommended first launch: WP-G.1 (`senior-implementer`) now; WP-G.0 (`implementer`)
after the demo (or now if the owner accepts the `validRatio` change before it). When
G.1 merges, launch G.2, G.4, G.6 (`senior-implementer` ×3) and G.8 (`implementer`) together.

## Traps for implementers

1. `GazeCalibrationProfile.apply` clamps to [0,1] and the raycaster clamps before it.
   The Rust port must not clamp anywhere in the stored path; clamping is a drawing
   concern. Parity tests against `GazeCalibrationProfileTests.testApplyClampsToValidRange`
   are expected to differ: drop that case and say so.
2. The Swift tests for the raycaster (`ScreenRaycasterTests.test3DCameraSpaceRaycast*`)
   pin the defective behaviour. Do not port them as goldens; use the analytic GEO-6 cases.
3. ARKit's face and camera coordinate conventions do not follow the UI orientation.
   Do not assume "x right, y up" in landscape; G.8's bench build must record a corners
   check in every orientation, and G.1's rotation tests are only as good as that check.
   If the bench shows a different convention, it is a G.1 fix, not a Swift fix.
4. `lookAtPoint` is in face-anchor space and is a convergence point, not a direction of
   unit length; `angularAffine` needs the direction from the eye midpoint (face space) to
   the look-at point, then yaw = atan2(x, z) and pitch = atan2(y, √(x²+z²)) in that frame.
   The sign of z for "towards the camera" differs between face and camera space.
5. `GazeTelemetrySample` encodes snake_case with `valid` for `isValid` (see
   `asd-core/src/types/frame.rs`). New per-frame keys (`track_state`, `gap_filled`) follow
   the same snake_case rule; frame-level keys stay camelCase. Two casings in one file is
   the existing contract (CON-12), not a bug to fix here.
6. Untracked frames have no head pose. Store the pose fields as the last known values with
   `track_state = lost`, or make them optional in the frame type; do not write zeros, which
   look like a perfect frontal pose to every downstream consumer.
7. Blink recovery and every window in this module are in milliseconds of frame time.
   ARKit drops to 30 Hz under thermal pressure; a frame count is always wrong somewhere.
8. `validRatio` changes meaning after G.0 (denominator includes lost frames). Any test
   that hard-codes a `validRatio` from a fixture with gaps will move; update the expected
   value with a comment citing TRK-1, never loosen the assertion.
9. Gap-filled samples must not reach dwell or AOI latency. The easiest way to get this
   wrong is to feed the gap-filled sequence into the existing `metrics::extract`; feed it
   only to the fixation run detector.
10. UniFFI objects must be `Send + Sync`. Wrap `GazePipeline` in a `Mutex` inside the handle;
    do not make the core type `Sync` with interior mutability.
11. The video is 9:16 and aspect-filled; the catalog boxes are currently interpreted as
    screen boxes. Until the catalog declares `aoiSpace`, keep the existing interpretation
    (default `"screen"` for catalog schema ≤ current) so scores do not move silently; switch
    the default to `"video"` only with a catalog version bump and new golden files.
12. Calibration and validation must use the same eye position and geometry as the session;
    computing angles with the idiom default while the session uses the table (or the other
    way round) yields self-consistent but wrong numbers that no test catches. One
    `PipelineConfig` per session, passed everywhere.
13. `.gitignore` already ignores `*.pdf` and some binaries; bench reports are Markdown. Raw
    logs (`*.jsonl`) must not be added under `docs/` or `rust/`; add an ignore rule for
    `**/bench-logs/` if a package creates such a folder locally.
14. Other agents are editing `rust/` and `apps/curv_console/` concurrently. Rebase on `dev`
    before starting a package and keep to the listed files; migrations take the next free
    number at integration time, not the number written here.
