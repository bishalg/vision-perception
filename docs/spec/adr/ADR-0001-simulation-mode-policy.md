# ADR-0001: Simulation mode is a build-time capability with mandatory data provenance

Status: Proposed, 2026-10-22
Spec ids: SIM-1..8, PLC-1..5

## Context

`GazeSessionController.startSession` (`domains/asd_screening/ios/Sources/CurvAsdGazeKit/GazeSessionController.swift:278-299`)
starts an `ARSession` when `ARFaceTrackingConfiguration.isSupported`, otherwise sets
`isSimulationMode = true` and returns. The views then call `tickSimulatedFrame` on a
60 Hz timer, producing deterministic sine-wave gaze and affect data (`:446-552`).
Nothing downstream distinguishes this from live data: `GazeSessionReportSummary`,
`SessionHeader`, `GazeSessionRecord` and the FHIR Observation (`status: final`,
`FHIRInteroperabilityMapper.swift:153`) are identical. A menu item lets any user
toggle simulation at runtime (`MainGazeScannerView.swift:644-651`), and bubble
calibration accepts every target while simulating (`BubbleCalibrationView.swift:570`).
Only `NormCollectionGate` (`ResearchKitStudyModels.swift:516-526`) checks the flag.

An App Store reviewer on a device without face tracking would therefore see a
working "screening" that invents results. A parent on such a device would get
fabricated engagement metrics in the child's history, CareKit outcomes and
exported clinical records.

## Decisions

1. Simulation is a compile-time capability (`ASD_SIMULATION_ENABLED`, Debug only).
   The Release binary contains no simulation entry point, no toggle, and no
   simulated frame ticker.
2. Live capture never falls back. Unsupported hardware, denied permission, session
   failure and interruption are explicit `TrackingStage.unavailable(reason:)` states
   the UI must render.
3. Provenance is a required, non-defaulted field (`CaptureSource`) on every data
   structure that leaves the controller: summary, vault header, history record. A
   default would reintroduce the silent path.
4. Simulated data is quarantined by type at every sink: FHIR, OMOP, history,
   CareKit, ResearchKit, norm collection all refuse it; the vault keeps it with a
   `.sim` suffix for developer debugging.
5. Placeholder measurements are removed rather than labelled: first-look latency
   becomes optional and absent, dwell ratios are optional and absent when no face
   hit occurred, durations come from timestamps. An absent metric is omitted from
   every export; nothing substitutes a default.
6. The Release build declares `UIRequiredDeviceCapabilities: [arkit, front-facing-camera]`
   so the store does not offer the app to devices that cannot run it; the runtime
   check remains as defence in depth.

## Alternatives rejected

- Keep runtime simulation but mark records `isSimulated`. Rejected: a Bool that
  defaults to `false` is exactly the bug we have; and a reviewer-visible toggle
  invites the question of what else is simulated.
- Allow simulation in Release for reviewers via a hidden gesture or demo account.
  Rejected: App Review Guideline 2.3 (accurate metadata) and 1.4.1 risk; the demo
  path must be a real session on a supported device, which the review notes describe.
- Label the 480 ms latency as "estimated". Rejected: an estimate that never varies is
  a constant; exporting it under LOINC 8692-2 with a reference range is a false
  measurement.

## Consequences

- Devices without ARKit face tracking cannot use the gaze feature at all; the diary,
  milestones and medication features still work. This is the honest product.
- The kit's public initialisers lose their defaults; tests move to a fixture helper.
  Existing tests in both targets need updates (listed in WP-1.1 and WP-1.2).
- `GazeSessionRecord` schema bumps to v1.6 with a legacy-provenance marker.
- The classifier heuristic loses its latency input; because it is Debug-only after
  ADR-0004 this is acceptable.
