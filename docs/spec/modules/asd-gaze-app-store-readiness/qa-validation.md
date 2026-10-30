# ASD Gaze App, App Store Readiness - QA Validation

## Test strategy

Test-first per work package: each package lists the test files it adds or changes;
the implementer writes the failing test, then the code. Three layers:

1. Kit unit tests (`domains/asd_screening/ios/Tests/CurvAsdGazeKitTests`), run with
   `swift test` on macOS (the package declares `.macOS(.v14)`; ARKit code is behind
   `#if canImport(ARKit) && !os(macOS)`).
2. App unit tests (`apps/ios_asd_gaze_app/Tests`, target `AsdGazeCollectorTests`,
   hosted by the app), run with `xcodebuild test` on an iOS Simulator.
3. Static gates: claims denylist, privacy-API audit, secrets grep, metadata lengths.

Device-only behaviour (camera permission, interruption, idle timer, TrueDepth) is
verified by a manual checklist on an iPhone; the checklist is `acceptance.md`
scenarios 1, 10-13, 19.

## Commands (never build inside the repository)

```bash
# Kit
cd ~/Documents/WORK/Code/vision-perception/domains/asd_screening/ios
swift test --scratch-path /private/tmp/asd-kit
swift test --scratch-path /private/tmp/asd-kit-tsan --sanitize=thread
swift build --scratch-path /private/tmp/asd-kit -Xswiftc -strict-concurrency=complete -Xswiftc -warnings-as-errors

# App (regenerate the project first; schemes are not shared, XcodeGen creates them)
cd ~/Documents/WORK/Code/vision-perception
xcodegen generate --spec apps/ios_asd_gaze_app/project.yml
xcodebuild test \
  -project apps/ios_asd_gaze_app/AsdGazeCollector.xcodeproj \
  -scheme AsdGazeCollector \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
  -derivedDataPath /private/tmp/asd-app-dd \
  -parallel-testing-enabled NO \
  -only-testing:AsdGazeCollectorTests \
  CODE_SIGNING_ALLOWED=NO

# Unsigned Release build (proves Debug-only flags are really off)
xcodebuild build \
  -project apps/ios_asd_gaze_app/AsdGazeCollector.xcodeproj \
  -scheme AsdGazeCollector -configuration Release \
  -destination 'generic/platform=iOS' \
  -derivedDataPath /private/tmp/asd-app-rel \
  CODE_SIGNING_ALLOWED=NO

# Static gates
python3 scripts/check_asd_claims.py
bash scripts/check_privacy_api_use.sh
git grep -n -E "9802030896|bishal\.ghimire@gmail|binodprasadghimire" -- . ':!docs/spec' && exit 1 || true
```

The simulator name must exist locally (`xcrun simctl list devices available`); CI
uses the image's default iPhone.

## Test matrix (identifier -> proof)

| Spec id | Test or check | File |
| --- | --- | --- |
| SIM-1, SIM-4 | `CaptureSource` round-trips through `SessionHeader`, `GazeSessionReportSummary`, `GazeSessionRecord` JSON; v1.5 fixture decodes with `legacyProvenance == true` | kit `CaptureSourceTests.swift` (new); app `GazeSessionRecordVersioningTests.swift` |
| SIM-2 | On macOS (`isSupported` unavailable) `startSession()` leaves `isRecording == false`, `trackingStage == .unavailable(.faceTrackingUnsupported)` | kit `GazeSessionTests.swift` |
| SIM-3 | `tickSimulatedFrame` without `startSimulatedSession` appends nothing; with it, `captureSource == .simulated` | kit `GazeSessionTests.swift` |
| SIM-5 | mapper returns `nil` for simulated record; history store ignores; CareKit outcome ignored; `NormCollectionGate` closed | app `ClinicalInteroperabilityTests.swift`, `GazeSessionRecordVersioningTests.swift`, `CareKitClinicalStoreTests.swift`, `ResearchKitStudyTests.swift` |
| SIM-5 (vault) | filename suffix `.sim.jsonl.enc` | kit `GazeSessionTests.swift` |
| SIM-6, CLM-3, CNS-2 | Release build compiles and `nm`/`strings` of the Release binary contains no "Switch to Simulation", "Elevated Risk", "BlendShapeML" | `scripts/check_release_symbols.sh` (new, runs after the unsigned Release build) |
| SIM-7 | generated Info.plist has `UIRequiredDeviceCapabilities` with `front-facing-camera` and without `arkit` | app `BuildConfigurationTests.swift` (new, reads `Bundle.main.infoDictionary`) |
| PLC-1, PLC-2 | summary from a session with zero face hits has `nil` dwell ratios and `nil` latency | kit `GazeSessionTests.swift` |
| PLC-3 | FHIR Observation and OMOP rows omit absent metrics | app `ClinicalInteroperabilityTests.swift` |
| PLC-4 | baseline inference with `nil` latency re-normalises weights; `primaryContributingFeature` lacks "CoreML" when heuristic | kit `BiomarkerClassificationEngineTests.swift` |
| PLC-5 | 30 fps synthetic timestamps produce duration from timestamps, dwell capped at 100 ms | kit `GazeProcessingActorTests.swift` |
| CLM-1, CLM-2, CI-5 | denylist script fails on seeded fixture, passes on repo | `scripts/check_asd_claims.py` with self-test `--selftest` |
| CLM-4 | `MCHATRiskLevel.displayName` strings; raw values unchanged | app `ResearchKitStudyTests.swift` |
| CLM-5 | result view model / PDF contain the two mandatory sentences | app `DoctorReportGeneratorTests.swift` (text extraction from PDF data) |
| CNS-1 | `ConsentToken` cannot be constructed without `hasConsentedPIA`; app session wrapper refuses without token | app `ConsentGatingTests.swift` (new) |
| CNS-2 | logger not present in Release (symbol check); in Debug with flag, no write when `normCollectionOptIn == false` | app `BlendShapeLoggerTests.swift` (new, Debug only) |
| CNS-3, CNS-4 | written file has `.complete` protection attribute and parent dir `isExcludedFromBackup` | kit `GazeSessionTests.swift`; app `ResearchKitSyncServiceTests.swift` (simulator supports `FileProtectionType` attributes; assert via `FileManager.attributesOfItem`) |
| CNS-5 | revoke purges queue/dead-letter/quarantine | app `ResearchKitSyncServiceTests.swift` |
| CNS-6 | `DataDeletionService.deleteAll()` empties every store; CareKit untouched | app `DataDeletionServiceTests.swift` (new) |
| CNS-7 | record built from the stimulus flow uses the token; `childName` absent from encoded JSON | app `GazeSessionRecordVersioningTests.swift` |
| CNS-8 | estimator receives no frames unless enabled | kit `AgeBandEstimationTests.swift` |
| CNS-9 | missing/HTTP config -> `ResearchStudyConfiguration.load()` is `nil`; `ResearchKitSyncService.init` throws/fails closed | app `ResearchStudyConfigurationTests.swift` (new) |
| CAM-2 | failure/interruption delegate calls set `.unavailable`, summary counts interruption, verdict capped | kit `GazeProcessingActorTests.swift` |
| CAM-1, CAM-3 | `CameraAccessCoordinator` state machine; idle-timer toggled through an injected protocol | app `CameraAccessCoordinatorTests.swift` (new) |
| CONC-1..4 | TSAN run; ordering test with 1000 snapshots and a stop in the middle | kit `GazeProcessingActorTests.swift`, `TrackingPipelineTests.swift` |
| PRV-1, PRV-2 | manifest matches compiled features; API-use audit script | `scripts/check_privacy_api_use.sh`; app `BuildConfigurationTests.swift` |
| FHIR-1..4, FHIR-7 | bundle builder output, reference resolution, canonical URL, linkIds | app `FHIRBundleBuilderTests.swift` (new), `ClinicalInteroperabilityTests.swift` |
| FHIR-5 | care mapper output | app `FHIRCareMapperTests.swift` (new) |
| FHIR-6 | OMOP rows have no reference ranges; unverifiable concept ids are 0 with source value | app `ClinicalInteroperabilityTests.swift` |
| CI-1..3 | workflow file lint (`actionlint` optional) and a dry run on a branch | `.github/workflows/ios.yml` |
| CI-4 | secrets grep; `fastlane` files contain no personal identifiers | `scripts/check_asd_claims.py --secrets` |

## Synthetic data rules

- Tests use `GazeSessionReportSummary.fixture(...)` in the test target only.
- Frame fixtures are generated from deterministic sine functions as `tickSimulatedFrame`
  does today; no recorded child telemetry may be committed.
- Participant tokens in fixtures are `child-TEST0000`; names never appear.
- FHIR fixtures use `urn:uuid:` with fixed UUIDs for reproducible snapshots.

## CI hooks

- `ios.yml` runs the four build/test steps and the static gates on every PR touching
  the iOS paths; it is required for merge to `dev`.
- The existing C++ `ci.yml` is untouched except for adding the claims-script job,
  which is cheap and path-independent.

## Non-functional checks

- Main-thread hitching: with CONC-1 in place, Instruments "Hangs" shows no hang > 250 ms
  during a 60 s session on an iPhone 12.
- Memory: vault write of a 60 s session (3600 frames) stays under 50 MB peak.
- Battery/thermal: idle timer disabled only during recording; no session longer than
  the playlist duration plus 5 s.

## TODO

- Decide whether to vendor a FHIR validator binary for CI once the organisation's
  FHIR server (if any) is chosen; until then structural checks only (FHIR-7).
