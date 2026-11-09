# ASD Gaze App, App Store Readiness - Work Packages

Companion to `technical.md` (spec ids), `acceptance.md`, `qa-validation.md` and
ADR-0001..0005. Phases run in order; inside a phase, packages in the same wave share
no files and can run in parallel across implementer agents. All build paths are under
`/private/tmp/asd-*`, never inside the repository.

Tier legend: `senior` = `senior-implementer` (Opus); `impl` = `implementer` (Sonnet).

## Phase 0 - prerequisites (human, before any agent starts)

| Item | Action |
| --- | --- |
| P0-1 | Commit or stash the uncommitted working tree on `dev` (26 files, about 1000 lines, including `ResearchKitStudyModels.swift`, `ResearchKitSyncService.swift`, `BubbleCalibrationView.swift`, `OnboardingFlowView.swift`, `PerVideoMetrics.swift`, `PerVideoScoringEngine.swift` and four test files). Phase 1 packages own several of these files and must start from a committed baseline. |
| P0-2 | Decide H1 (history rewrite vs. rotate), H2 (name), H3 (M-CHAT licence), H4 (publisher, bundle id, domains), H5 (category), H6 (research study at launch). Defaults in `non-technical.md` apply if undecided; WP-1.5a and WP-1.6 use placeholders that the claims script flags until replaced. |
| P0-3 | Run the baseline once so regressions are attributable: the kit and app test commands in `qa-validation.md`. Record pass/fail counts in the PR description. |

## Phase 1 - blockers

### Wave A (parallel)

#### WP-1.1 Kit provenance, placeholders, vault protection - `senior`

Goal: SIM-1..5 (kit side), PLC-1..4, CNS-3, CNS-8, CLM-3 (kit side), `TrackingStage.unavailable`.

Files owned (all under `domains/asd_screening/ios/`):
`Package.swift` (swiftSettings defines), `Sources/CurvAsdGazeKit/GazeTypes.swift`,
`GazeSessionController.swift`, `GazeProcessingActor.swift` (only `finalizeSession`
aggregation and `processSimulatedFrame` gating; the pipeline rewrite is WP-2.1),
`FacialAffectMetrics.swift` (`SessionHeader.captureSource`), `SessionVaultManager.swift`,
`Inference/BiomarkerClassificationEngine.swift`, `Inference/SubjectAgeEstimator.swift`
(no-op when disabled).
Tests: `Tests/CurvAsdGazeKitTests/CaptureSourceTests.swift` (new), `GazeSessionTests.swift`,
`BiomarkerClassificationEngineTests.swift`, `GazeProcessingActorTests.swift`,
`AgeBandEstimationTests.swift`, `TrackingPipelineTests.swift`, new
`Tests/CurvAsdGazeKitTests/Support/SummaryFixture.swift` (`GazeSessionReportSummary.fixture`).
Do not touch `PerVideoMetrics.swift`, `PerVideoScoringEngine.swift`, `PerVideoMetricsTests.swift`.

Exact changes:
1. `CaptureSource`, `TrackingUnavailableReason`, `TrackingStage.unavailable(reason:)` in `GazeTypes.swift` as in the spec's public API block.
2. `GazeSessionController`: replace `isSimulationMode` storage with `captureSource: CaptureSource?` (keep a computed read-only `isSimulationMode`), delete the fallback at `:298`, add `startSimulatedSession` under `#if ASD_SIMULATION_ENABLED`, make `tickSimulatedFrame` a no-op unless simulated, add `isSubjectAgeEstimationEnabled` and check it at `:617` before calling the estimator, remove `:40-46` defaults, make `firstEyeLatencyMs`, `eyeDwellRatio`, `mouthDwellRatio` optional and `nil` at `:389-390`/`:411`, add `captureSource`, `interruptionCount`, `interruptedSec` to the summary (the last two always 0 until WP-2.1). Custom `Decodable` on the summary tolerates missing keys (vault files written before this change).
3. `SessionHeader`: required `captureSource`; `SessionVaultManager.saveSession` uses `.sim.jsonl.enc` suffix for simulated, writes with `.completeFileProtection`, sets `isExcludedFromBackup` on the directory, and wraps the `classification` block of `makeResearchKitJson` and `makeCareKitJson` in `#if ASD_RESEARCH_CLASSIFIER`.
4. `BiomarkerClassificationEngine`: optional inputs re-normalise weights; `primaryContributingFeature` strings without "CoreML" on the heuristic path; whole file under `#if ASD_RESEARCH_CLASSIFIER` except the `RiskStratification` enum (it is persisted by the app; keep it compiling).
5. `Package.swift`: `.define("ASD_SIMULATION_ENABLED", .when(configuration: .debug))`, `.define("ASD_RESEARCH_CLASSIFIER", .when(configuration: .debug))`.

Tests first: `CaptureSourceTests` (round-trip, decode without key fails for header, summary decode tolerates missing optional metrics), `GazeSessionTests.testStartSessionWithoutFaceTrackingIsUnavailable`, `testSimulatedSessionRequiresExplicitStart`, `testNoFaceHitsGiveNilDwellRatios`, `testVaultSimulatedSuffixAndProtection`, `BiomarkerClassificationEngineTests.testNilLatencyRenormalises`.
Commands that must pass: `swift test --scratch-path /private/tmp/asd-kit` (Debug, flags on) and `swift build -c release --scratch-path /private/tmp/asd-kit-rel` (flags off; proves the `#if` blocks compile both ways).
Depends on: P0-1. Parallel with: WP-1.4, WP-1.5a, WP-1.6.

#### WP-1.4 Consent-gated capture, logger removal, queue protection, deletion - `senior`

Goal: CNS-1, CNS-2, CNS-4, CNS-5, CNS-6, SIM-5 (`ResearchKitStudyStore` refuses simulated).

Files owned (under `apps/ios_asd_gaze_app/`): `Sources/App/AsdGazeApp.swift`,
`Sources/Models/BlendShapeCSVLogger.swift`, `Sources/Models/ResearchKitSyncService.swift`,
`Sources/Models/ResearchKitStudyStore.swift`, `Sources/Models/ChildProfile.swift`
(`ConsentToken`), new `Sources/Models/GazeSessionController+Consent.swift`
(`startConsentedSession(token:battery:stimulusId:)` extension calling the kit's
`startSession`), new `Sources/Models/DataDeletionService.swift`,
`Sources/Views/SettingsView.swift` (delete action and the device-status row text
`settings_simulation_mode` -> "Face tracking unsupported on this device").
Tests: `Tests/ResearchKitSyncServiceTests.swift`, new `Tests/DataDeletionServiceTests.swift`,
`Tests/ConsentGatingTests.swift`, `Tests/BlendShapeLoggerTests.swift` (inside `#if ASD_ML_DATA_EXPORT`).

Exact changes: wrap logger file and the `startListening()` call in `#if ASD_ML_DATA_EXPORT`; logger writes only when `normCollectionOptIn` and recording, with `.completeFileProtection` and backup exclusion. Sync queue moves to Application Support with protection and backup exclusion; `purgeAllQueued()` added and called from `revokeConsent` (the store calls it through an injected closure so the store stays queue-free and testable). `DataDeletionService.deleteAll(includeCarePlans:)` deletes vault dir (`SessionVaultManager().vaultDirectory`), history (`GazeSessionHistoryStore.shared.clear()`), queues, `BlendShapeML` dir if present, calibration (`GazeCalibrationStore.reset()`), M-CHAT results and enrollment. `ConsentToken` is a struct with a `fileprivate` init and `static func mint(from:) -> ConsentToken?`.
Contract for other packages: `ConsentToken.mint(from: ChildProfileStore.shared)`; `controller.startConsentedSession(token:battery:stimulusId:)`.
Commands: app tests command from `qa-validation.md` (will compile only after WP-1.1 lands because `SessionHeader` changed; run the kit tests meanwhile and integrate in Wave B).
Depends on: P0-1 (and WP-1.1 for compile). Parallel with: WP-1.1, WP-1.5a, WP-1.6.

#### WP-1.5a Store metadata rewrite and claims gate - `impl`

Goal: CLM-1, CLM-5 (store text), CLM-6, CI-5 (script side), CI-4 secrets grep.

Files owned: `apps/ios_asd_gaze_app/fastlane/metadata/**` except `review_information/`
(all locales, iOS and Android), new `scripts/check_asd_claims.py` (denylist, length
limits mirrored from the `validate_metadata` lane, `--selftest`, `--secrets`),
`.github/workflows/ci.yml` (one new job `asd-claims` running the script), new
`docs/spec/modules/asd-gaze-app-store-readiness/claims-denylist.md` (human-readable
copy of the list; the script is the source of truth).
Exact changes: name -> "EarlyGaze Observation Diary" (placeholder until H2),
subtitle -> "Engagement and daily notes", keywords without `biomarker`/`screening`,
description rewritten as an observation diary with the two mandatory sentences,
release notes rewritten without FDA/HIPAA/IRB/biomarker/SMART-on-FHIR claims,
`primary_category.txt` -> `HEALTH_AND_FITNESS`, Hindi/Nepali names translated
equivalents of the placeholder, Android title/short/full descriptions aligned.
Tests first: `python3 scripts/check_asd_claims.py --selftest` (seeded failures) then the real run exits 0.
Depends on: nothing. Parallel with: WP-1.1, WP-1.4, WP-1.6.

#### WP-1.6 Secrets and identity out of the tree - `impl`

Goal: CI-4 (files), ADR-0005 decisions 1-2.

Files owned: `apps/ios_asd_gaze_app/fastlane/Appfile`, `Matchfile`, `config.yml`,
`.env.example`, `Fastfile` (env-driven ids and `app_review_information` from env in
`update_metadata`), `fastlane/SIGNING_AND_SECRETS.md`, `.gitignore` (add
`apps/ios_asd_gaze_app/fastlane/metadata/review_information/`), delete
`apps/ios_asd_gaze_app/fastlane/metadata/review_information/*`.
Exact changes: no literal Apple ID, team name, phone, email or password remains; defaults for `APP_IDENTIFIER` stay `org.curv.asd.gazecollector` until H4. Document the `git filter-repo` command for H1 in `fastlane/SIGNING_AND_SECRETS.md` without running it.
Commands: `python3 scripts/check_asd_claims.py --secrets` (after WP-1.5a lands; until then the grep line in `qa-validation.md`), `ruby -c apps/ios_asd_gaze_app/fastlane/Fastfile`.
Depends on: nothing. Parallel with: WP-1.1, WP-1.4, WP-1.5a.

### Wave B (parallel, after Wave A)

#### WP-1.2 App data layer provenance and export refusal - `senior`

Goal: SIM-4, SIM-5 (app sinks), PLC-3, CLM-3 (CareKit fields), CLM-4 (`displayName`), CNS-7 (record side).

Files owned: `Sources/Models/GazeSessionRecord.swift` (v1.6, `captureSource`, optional metrics, `legacyProvenance`, custom decoder; history store ignores simulated), `Sources/Models/FHIRInteroperabilityMapper.swift` (return `nil`/`[]` for simulated; omit absent components and rows), `Sources/Models/ResearchKitStudyModels.swift` (`MCHATRiskLevel.displayName`; `NormCollectionGate` takes `CaptureSource`), `Sources/Models/CareKitClinicalStore.swift` and `Sources/Models/CarePlanModels.swift` (ignore simulated; risk fields under `#if ASD_RESEARCH_CLASSIFIER`), `project.yml` Debug `OTHER_SWIFT_FLAGS` adds `-DASD_SIMULATION_ENABLED -DASD_RESEARCH_CLASSIFIER` (Release unchanged).
Tests: `Tests/GazeSessionRecordVersioningTests.swift`, `Tests/ClinicalInteroperabilityTests.swift`, `Tests/CareKitClinicalStoreTests.swift`, `Tests/CarePlanStoreTests.swift`, `Tests/ResearchKitStudyTests.swift`, new `Tests/Support/RecordFixtures.swift`.
Contract for WP-1.5b and WP-1.3: `GazeSessionRecord.captureSource`, `.legacyProvenance`, `MCHATRiskLevel.displayName`, `GazeSessionRecord(summary:participantToken:...)` (parameter renamed from `participantId` to make the name-passing call sites fail to compile).
Commands: app tests command.
Depends on: WP-1.1. Parallel with: WP-1.5b.

#### WP-1.5b In-app claims copy and result surfaces - `impl`

Goal: CLM-2..5 (views), SIM-8 badge component, SIM-5 (PDF banner), CNS-7 (result view lines 55 and 604).

Files owned: `Sources/Views/BiomarkerRiskStratificationView.swift` (entire file under `#if ASD_RESEARCH_CLASSIFIER`), `Sources/Views/GazeSessionResultView.swift`, `Sources/Views/BiomarkerDetailViews.swift`, `Sources/Views/BiomarkerAffectAndQualityViews.swift`, `Sources/Views/CaregiverDashboardView.swift`, `Sources/Views/MCHATSurveyModalView.swift`, `Sources/Views/PerVideoResultViews.swift`, `Sources/Views/OnboardingFlowView.swift` (copy only), `Sources/Models/DoctorReportGenerator.swift`, `Sources/UI/AppLocalizer.swift`, new `Sources/Views/SimulatedDataBadge.swift`, new `Sources/Views/NotDiagnosticFooter.swift`.
Tests: `Tests/DoctorReportGeneratorTests.swift` (PDF text contains the two sentences and never "Elevated Risk"), claims script on the tree.
Commands: `python3 scripts/check_asd_claims.py`; app tests command.
Depends on: WP-1.2 contract (compile after both land). Parallel with: WP-1.2.

### Wave C (after Wave B)

#### WP-1.3 View wiring for unsupported devices, consent token, flags - `impl`

Goal: SIM-2, SIM-6, SIM-7, SIM-8 (placement), CNS-1 (call sites).

Files owned: `Sources/Views/MainGazeScannerView.swift`, `Sources/Views/GazeStimulusScreeningView.swift` (participant token from enrollment or `child-<id>`; `SessionHeader(captureSource:)`), `Sources/Views/BubbleCalibrationView.swift`, `Sources/Views/GazePositioningStepView.swift`, `Sources/Views/GazeScreeningWizardView.swift` (entry gate), `Sources/Views/SubjectAgeQuickCheckCard.swift` (sets `isSubjectAgeEstimationEnabled = true` for its session only), `Sources/Views/ARFaceCameraView.swift` (no synthetic mesh in Release; show `.unavailable` state), new `Sources/Views/UnsupportedDeviceView.swift`, `project.yml` (`INFOPLIST_KEY_UIRequiredDeviceCapabilities` for `arkit` and `front-facing-camera` - note Apple restricts tightening this key after first release, so it must be right now).
Tests: new `Tests/BuildConfigurationTests.swift` (required capabilities present in `Bundle.main.infoDictionary`), app tests, Release unsigned build, new `scripts/check_release_symbols.sh` (strings of the Release binary must not contain "Switch to Simulation", "Elevated Risk", "BlendShapeML").
Depends on: WP-1.1, WP-1.2, WP-1.4, WP-1.5b.

Phase 1 exit gate (main session): kit tests, app tests, Release build, claims script, secrets grep, manual device scenarios 1-3, 10, 14-17 from `acceptance.md`.

## Phase 2 - high

### Wave A (parallel)

#### WP-2.1 Single-owner concurrency, camera observer, timestamp durations - `senior`

Goal: CONC-1..4, CAM-2, PLC-5, CI-3 (kit warnings as errors).

Files owned: `domains/asd_screening/ios/Sources/CurvAsdGazeKit/GazeSessionController.swift`, `GazeProcessingActor.swift`, `GazeTypes.swift` (if new Sendable snapshot fields are needed), `OneEuroFilter.swift` (only if it must become actor-owned; prefer moving instances into the actor without editing the filter), `Package.swift` (`-warnings-as-errors` via `unsafeFlags` is not allowed for dependents; instead add `swiftSettings: [.enableUpcomingFeature(...)]` as needed and enforce warnings-as-errors in CI with `-Xswiftc -warnings-as-errors`), new `Sources/CurvAsdGazeKit/ARSessionBridge.swift`.
Tests: `GazeProcessingActorTests.swift`, `TrackingPipelineTests.swift`, `GazeSessionTests.swift`, new `ConcurrencyOrderingTests.swift` (1000 snapshots, stop at 500, exactly 500 frames; calibration/AOI/clock updates from a `DispatchQueue.concurrentPerform` while processing), `OneEuroFilterTests.swift` unchanged.
Exact changes: delete the delegate pipeline at `GazeSessionController.swift:695-856`; the bridge builds `FaceAnchorSnapshot` and calls `processFaceAnchor`; controller is `@MainActor`, not `@unchecked Sendable`; `stopSession()` is `async` and awaits `finalizeSession`; implement `didFailWithError`, `sessionWasInterrupted`, `sessionInterruptionEnded` on the bridge; durations and dwell from timestamps with the 100 ms cap; `interruptionCount`/`interruptedSec` filled.
Commands: `swift test --scratch-path /private/tmp/asd-kit`, `swift test --scratch-path /private/tmp/asd-kit-tsan --sanitize=thread`, `swift build --scratch-path /private/tmp/asd-kit -Xswiftc -strict-concurrency=complete -Xswiftc -warnings-as-errors`.
Depends on: Phase 1. Parallel with: WP-2.2, WP-2.3 (under the `stopSession() async` contract; app compiles after WP-2.2 lands).

#### WP-2.2 Camera permission, idle timer, scene phase (app side) - `impl`

Goal: CAM-1, CAM-3, UI for `.unavailable` reasons.

Files owned: new `Sources/Models/CameraAccessCoordinator.swift` (state machine over an injected `CameraAuthorizing` protocol and `IdleTimerControlling` protocol), new `Sources/Views/CameraDeniedView.swift`, `Sources/Views/GazeScreeningWizardView.swift`, `Sources/Views/GazeStimulusScreeningView.swift`, `Sources/Views/MainGazeScannerView.swift`, `Sources/Views/GazePositioningStepView.swift`, `Sources/Views/BubbleCalibrationView.swift`, `Sources/Views/SubjectAgeQuickCheckCard.swift` (all: permission check before start, `await controller.stopSession()`, `scenePhase` handling, idle timer through the coordinator).
Tests: new `Tests/CameraAccessCoordinatorTests.swift`.
Commands: app tests; manual scenarios 11-13.
Depends on: Phase 1. Parallel with: WP-2.1, WP-2.3.

#### WP-2.3 Research configuration, privacy manifests, usage strings - `senior`

Goal: CNS-9, PRV-1..3, CNS-8 disclosure, ADR-0002 decision 5.

Files owned: new `Sources/Models/ResearchStudyConfiguration.swift`, `Sources/Models/ResearchKitStudyModels.swift` (consent document built from configuration; fictional defaults deleted), `Sources/Models/ResearchKitSyncService.swift` (`shared` becomes `static let shared: ResearchKitSyncService?`, https-only), `Sources/Views/CareKitDashboardModals.swift`, `Sources/Views/ResearchStudyConsentView.swift`, `Sources/Views/CaregiverDashboardView.swift` (research card hidden when `nil`), `Sources/Views/SettingsView.swift` (research rows hidden when `nil`), `apps/ios_asd_gaze_app/Resources/PrivacyInfo.xcprivacy`, `domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/PrivacyInfo.xcprivacy`, `project.yml` (usage strings only; no `ASDResearchStudy` key by default), new `scripts/check_privacy_api_use.sh`.
Tests: new `Tests/ResearchStudyConfigurationTests.swift`, `Tests/ResearchKitSyncServiceTests.swift` (inject config), `Tests/ResearchKitStudyTests.swift`, `Tests/BuildConfigurationTests.swift` (manifest has no collected types in the default build).
Commands: app tests; `bash scripts/check_privacy_api_use.sh`.
Depends on: Phase 1. Parallel with: WP-2.1, WP-2.2 (SettingsView was touched in Phase 1 by WP-1.4, which has landed; no Phase 2 package other than this one touches it).

Phase 2 exit gate: all Phase 1 gates plus TSAN run, Instruments hang check on device, scenarios 11-13, 18-21.

## Phase 3 - medium

### Wave A (parallel)

#### WP-3.1 FHIR Bundle, provenance and coding fixes - `senior`

Goal: FHIR-1..4, FHIR-6, FHIR-7.

Files owned: new `Sources/Models/FHIRBundleBuilder.swift`, new `Sources/Models/FHIRNamespaces.swift`, `Sources/Models/FHIRInteroperabilityMapper.swift`, `Sources/Models/ClinicalInteroperabilityStandards.swift` (non-optional LOINC lookup; local CodeSystem codes; OMOP ids verified or zeroed), `Sources/Models/GazeSessionRecord.swift` (replace `toFHIRObservation` with `FHIRBundleBuilder.bundle(for:...)`), `Sources/Views/GazeSessionResultView.swift` and `Sources/Views/PediatricReportView.swift` (export call sites), new `Tests/FHIRBundleBuilderTests.swift`, `Tests/ClinicalInteroperabilityTests.swift`, new `Tests/Fixtures/expected-bundle.json`.
Contract: `FHIRBundleBuilder.bundle(for record: GazeSessionRecord, mchat: (MCHATScoreResult, [Int: Bool])?, extraResources: [[String: AnyCodableValue]], subject: FHIRSubject) -> [String: AnyCodableValue]`.
Commands: app tests.
Parallel with: WP-3.2, WP-3.3, WP-3.4.

#### WP-3.2 Diary, milestones, medications to FHIR - `senior`

Goal: FHIR-5.

Files owned: new `Sources/Models/FHIRCareMapper.swift`, new `Sources/Models/FHIRCareCodings.swift`, new `Tests/FHIRCareMapperTests.swift`. Reads `MoodDiaryModels.swift`, `ClinicalDevelopmentModels.swift`, `CarePlanModels.swift` without editing them.
Contract: `FHIRCareMapper.resources(diary: [MoodDiaryEntry], milestones: [ChildMilestoneRecord], medications: [MedicationPlan], subjectRef: String) -> [[String: AnyCodableValue]]` (type names to be confirmed from the three model files by the implementer; report back if they differ).
Parallel with: WP-3.1, WP-3.3, WP-3.4.

#### WP-3.3 iOS CI - `impl`

Goal: CI-1, CI-2.

Files owned: new `.github/workflows/ios.yml`, new `scripts/ios_test.sh` (the exact commands from `qa-validation.md`, parameterised by `DERIVED_DATA` defaulting to `/private/tmp/asd-app-dd`), `apps/ios_asd_gaze_app/README.md` (test section), `docs/spec/modules/asd-gaze-app-store-readiness/qa-validation.md` (only if commands change).
Commands: `bash -n scripts/ios_test.sh`; a dry run of the workflow on a branch (the main session triggers it).
Parallel with: WP-3.1, WP-3.2, WP-3.4.

#### WP-3.4 Publisher identity, M-CHAT gate, stimulus provenance - `impl`

Goal: CI-4 (project side), CI-5, ADR-0005 decision 3, H3 gate.

Files owned: `project.yml` (bundle ids from `APP_IDENTIFIER`/`WIDGET_IDENTIFIER` env with the current defaults; `-DASD_MCHAT_LICENSED` only when `MCHAT_LICENSED=1` is exported at generate time), `apps/ios_asd_gaze_app/fastlane/*` (final org values once H4 is decided), `fastlane/metadata/en-US/*url.txt`, `Sources/Views/MCHATSurveyModalView.swift` (file-level `#if ASD_MCHAT_LICENSED`), the views that present it (grep `MCHATSurveyModalView(` at implementation time; expected `CaregiverDashboardView.swift`, `GazeScreeningWizardView.swift`, `CareKitDashboardModals.swift`; do not edit `GazeSessionResultView.swift` or `PediatricReportView.swift`, which WP-3.1 owns), `NOTICE.md` (stimulus video provenance and licence).
Commands: claims script; Release build with and without the flag.
Parallel with: WP-3.1, WP-3.2, WP-3.3.

## Phase 4 - low

| Package | Tier | Files | Goal |
| --- | --- | --- | --- |
| WP-4.1 Metadata URLs and locale strings | impl | `fastlane/metadata/**/*url.txt`, `hi/`, `ne-NP/` | CI-5 final values; translated strings for new copy |
| WP-4.2 Swift 6 language mode | senior | `project.yml` (`SWIFT_VERSION: "6.0"`, `SWIFT_TREAT_WARNINGS_AS_ERRORS`), any file that fails to compile | CI-3 |
| WP-4.3 Protected file-backed history store | senior | `Sources/Models/GazeSessionRecord.swift` (store only), new `Tests/GazeSessionHistoryStoreTests.swift` | migrate from `UserDefaults` with one-time import, `.completeFileProtection`, backup excluded |
| WP-4.4 Identifier renames (`Biomarker*` -> `Engagement*`) | impl | kit and app, mechanical | optional, ADR-0004 decision 7 |

WP-4.2 and WP-4.3 share no files; WP-4.4 touches everything and runs alone, last.

## Dependency summary

```
P0 -> [1.1 | 1.4 | 1.5a | 1.6] -> [1.2 | 1.5b] -> 1.3 -> gate
   -> [2.1 | 2.2 | 2.3] -> gate
   -> [3.1 | 3.2 | 3.3 | 3.4] -> gate
   -> [4.1 | 4.2 | 4.3] -> 4.4
```

## Traps for implementers

1. `OTHER_SWIFT_FLAGS` in `project.yml` does not reach the SwiftPM package; kit flags live in `Package.swift` with `.when(configuration: .debug)`. `swift test` builds Debug, so flags are on there; `swift build -c release` proves the off path.
2. `MCHATRiskLevel` raw values are persisted (`Codable`); never change them. Add `displayName`.
3. `GazeSessionReportSummary` is stored inside vault files and `GazeSessionRecord` in `UserDefaults`; new required fields need custom decoders with legacy defaults or every existing user loses history.
4. `SessionHeader` has many parameters; its call sites are in two views (`GazeStimulusScreeningView.swift:559`, `MainGazeScannerView.swift:720`) and kit tests. Adding a required parameter breaks the app until WP-1.3 lands; that is intended.
5. XcodeGen regenerates the project and schemes; there are no shared schemes in git. Run `xcodegen generate` before any `xcodebuild`, and check `xcodebuild -list` shows `AsdGazeCollectorTests` in the scheme.
6. Use `swift test --sanitize=thread`, not `-Xswiftc -sanitize=thread` alone (the linker flag is missing otherwise).
7. `isExcludedFromBackup` is set through `URLResourceValues` on a `var url`; set it after `createDirectory` on every launch (idempotent).
8. File protection attributes are observable on the simulator through `FileManager.attributesOfItem(atPath:)[.protectionKey]`; assert on that, not on the behaviour of a locked device.
9. `ResearchKitSyncService.shared` is created eagerly at type load; making it optional changes several call sites in dashboard modals and settings (WP-2.3 owns them all).
10. IP-literal URLs bypass ATS, so the `http://127.0.0.1` endpoint would really send in Release; fail closed on scheme, do not rely on ATS.
11. `GazeProcessingActor.finalizeSession` and `GazeSessionController.stopSession` currently both aggregate; after WP-2.1 only the actor does. Do not leave both.
12. `UIRequiredDeviceCapabilities` cannot be tightened after the first App Store release; decide the final set in WP-1.3.
13. `Localizer` tables are string dictionaries in `AppLocalizer.swift`; the claims script must scan values in all three languages, and Devanagari text needs the denylist transliterations (the script keeps an explicit list for hi/ne).
14. `BubbleCalibrationView.swift:570` makes calibration trivially pass when simulating; under the Debug flag that is fine, but the Release path must compute the real distance check.
15. CareKit resolves from GitHub on first build; CI must cache SwiftPM (`~/Library/Developer/Xcode/DerivedData/**/SourcePackages` is inside derived data, so cache the derived data path).
16. The uncommitted working tree (P0-1) contains changes to five files that Phase 1 owns; starting without committing will produce unreviewable diffs.

## Working tree at design time

`git status` on `dev` (2026-10-22) showed 26 modified files, uncommitted, including
`apps/ios_asd_gaze_app/Sources/Models/{MoodDiaryModels,MoodDiaryViewModel,OnDemandStimulusManager,ResearchKitStudyModels,ResearchKitStudyStore,ResearchKitSyncService,StimulusPlaylist}.swift`,
`Sources/UI/LiquidGlassTheme.swift`, `Sources/Views/{ARFaceCameraView,BubbleCalibrationView,MoodDiaryView,OnboardingFlowView,PerVideoResultViews,StimulusDownloadGateView}.swift`,
four app test files, `domains/asd_screening/ios/Sources/CurvAsdGazeKit/{PerVideoMetrics,PerVideoScoringEngine}.swift`,
`PerVideoMetricsTests.swift`, `docs/asd_app_sprint_tracker.md`, `scripts/compute_flood_delta.py`,
`tests/test_e2e_hackathon_pipeline.py`. None of these were modified by the design work.

## Phase 6 wave (2026-11-02, ADR-0010) — Swift 6 mode and `Biomarker*` renames

Replaces the phase 4 rows WP-4.2 and WP-4.4 above (tracker names WP-A.6 and WP-A.7).
Backend and console packages of the same batch (WP-B.16, WP-C.17) are in
`docs/spec/modules/asd-rust-core-and-backend/work-packages.md` ("Phase 6 wave"); the
batch summary is tracker `04 §8`. Measured baseline at `03d84a3` (Xcode 27.2 / Swift
6.4.2): kit 0 errors in mode 6 (debug, tests, release); app 8 errors in 7 source files,
widget clean; about 70 test-target diagnostics in 13 files that become errors in mode 6.

Gate for every package here: `scripts/ios_test.sh` (kit tests, TSan, warnings-as-errors
build, xcodegen, app tests, Release + DemoRelease builds, claims, schema, privacy API,
release symbols). Use a private `DERIVED_DATA` under `/private/tmp/asd-*` per agent; the
Swift 6 scratch dirs `/private/tmp/asd-app-dd-swift6` and `/private/tmp/asd-app-dd-kit-swift6`
from the design survey may be deleted.

### Wave 1 (parallel, no shared files; also parallel with WP-B.16 and WP-A.7a)

#### WP-A.6a Kit in Swift 6 mode - `I`

Goal: CONC-5 (kit half), CI-3 (kit step).
Files: `domains/asd_screening/ios/Package.swift` (first line `// swift-tools-version:
6.0`; nothing else), `scripts/ios_test.sh` (step "Kit strict concurrency, warnings as
errors": drop `-Xswiftc -strict-concurrency=complete`, keep `-Xswiftc
-warnings-as-errors`; update the header comment), `domains/asd_screening/ios/README.md`
(one line: Swift 6 language mode, Xcode 16+). Any kit source file **only** if the
tools-version flip surfaces an error the survey did not (none expected; report it).
Not to touch: `apps/ios_asd_gaze_app/**`, `scripts/build_asd_xcframework.sh`, the
generated `build/asd/swift/asd_core.swift` (regenerated by the build script; if it fails
in mode 6 the fix is a UniFFI config or version question, stop and report).
Tests first: none new; the proof is the gate. Add to `CurvAsdGazeKitTests` one compile-time
assertion that `GazeProcessingActor` and `ARSessionBridge` are `Sendable` where the test
expects it (`let _: any Sendable = bridge`), so a later regression to `@unchecked` or a
lost conformance fails to compile.
Commands: `swift build --package-path domains/asd_screening/ios --build-tests
--scratch-path /private/tmp/asd-a6a-kit` (expect no "language mode was overridden"
warning), `swift test …`, `swift test … --sanitize=thread`, `swift build … -c release`,
then `scripts/ios_test.sh --dry-run` and the full gate.
Depends on: nothing. Parallel with: A.6b, A.6c, A.7a, B.16.

#### WP-A.6b App sources in Swift 6 mode - `S`

Goal: CONC-5 (app sources), CI-3 (`project.yml`).
Files: `apps/ios_asd_gaze_app/project.yml` (base `SWIFT_VERSION: "6.0"`; delete the
widget target's `SWIFT_VERSION: "5.9"`; `options.xcodeVersion: "16.0"`),
`Sources/Models/FHIRInteroperabilityMapper.swift` (`isoFormatter`: make it a `let` of a
`Sendable` wrapper or construct per call; `ISO8601DateFormatter` is not `Sendable`),
`Sources/Models/SmartOnFhirAuthService.swift` (`KeychainWrapper.standard`: make
`KeychainWrapper` `Sendable` if it holds no mutable state, else `nonisolated(unsafe) let`
with the justification comment, else an actor), `Sources/Views/GazeScreeningWizardView.swift`
(`captureOrder`: `static let` of a `Sendable` array of a `Sendable` enum — make
`ScreeningWizardStep: Sendable`), `Sources/Models/DataDeletionService.swift` (`:83`, `:91`
read `@MainActor` statics `blendShapeFolder`/`benchLogFolder` from a nonisolated context:
make the folder constants `nonisolated static let` URLs computed from `FileManager`, or
hop to the main actor; do not make deletion main-actor-bound),
`Sources/Models/ARKitMoodFusionController.swift` (`:32` sending `note`: make the note
value type `Sendable` or move the consumer to the controller's isolation),
`Sources/Models/ResearchKitStudyStore.swift` (`:364` sending `value`: same approach),
plus the ~10 surviving warnings in `StimulusVideoPlayerView.swift`,
`ResearchStudyConsentView.swift`, `ObservationQuickLogBar.swift`,
`GazeReplayHeatmapFullScreenView.swift` **only where the fix is a one-liner** (otherwise
leave; they stay warnings).
Not to touch: `apps/ios_asd_gaze_app/Tests/**` (A.6c), `Widget/**` and `CareActivityKit/**`
(clean), `domains/asd_screening/ios/**` (A.6a), any `Biomarker*` file beyond the
concurrency fix itself (A.7b renames them; do not rename here).
Tests first: for each fix a regression test in the **existing** test file of that type is
owned by A.6c; A.6b writes none and instead leaves a list of "behaviour to pin" in its
report (e.g., `DataDeletionService` still deletes the bench folder in Debug). Build proof
is the gate.
Commands: `xcodegen generate --spec apps/ios_asd_gaze_app/project.yml`; `xcodebuild build
-scheme AsdGazeCollector -destination 'platform=iOS Simulator,name=iPhone 18 Pro'
-derivedDataPath /private/tmp/asd-a6b-dd CODE_SIGNING_ALLOWED=NO` (zero errors, expect
only the SDK-callback warnings), the unsigned Release and DemoRelease builds, both
`check_release_symbols.sh` runs; the full gate after A.6c lands.
Depends on: nothing. Parallel with: A.6a, A.6c (A.6c verifies against this package's
`project.yml` only after integration; see its commands), A.7a, B.16.

#### WP-A.6c App tests in Swift 6 mode - `I`

Goal: CONC-5 (test target).
Files: `apps/ios_asd_gaze_app/Tests/**` only. Known hot spots from the survey:
`DataDeletionServiceTests.swift` (24), `CareKitClinicalStoreTests.swift` (24),
`ConsentGatingTests.swift` (5), `MCHATGateTests.swift`, `CaptureHygieneTests.swift`,
`CaptureEntryGateTests.swift` (3 each), `CarePlanStoreTests.swift` (2),
`ResearchStudyConfigurationTests.swift`, `ResearchKitSyncServiceTests.swift`,
`ResearchKitStudyTests.swift`, `LiquidGlassContrastTests.swift`,
`GazeSessionRecordVersioningTests.swift`, `BuildConfigurationTests.swift` (1 each).
Patterns: mark test classes whose subject is `@MainActor` as `@MainActor final class`;
replace `var captured` mutated inside `Sendable` closures with `XCTestExpectation` +
a test-only `final class Box<T>: @unchecked Sendable` in `Tests/Support/Box.swift`
(the one place CONC-5 allows it); `await` actor-isolated properties; main-actor statics
read in `XCTAssert` autoclosures → bind to a local first.
Not to touch: `Sources/**`, `project.yml`, `Widget/**`.
Tests first: these *are* tests; the rule is that every assertion that exists at `03d84a3`
still exists with the same meaning (no `XCTSkip`, no deleted assertions; a diff of
`XCTAssert*` counts per file is part of the report).
Commands: until A.6b is integrated, verify with the current 5.9 project plus
`SWIFT_STRICT_CONCURRENCY=complete` and count warnings in `Tests/` from the build log
(target: zero; the survey showed the same diagnostics that mode 6 makes errors); after
integration, `xcodebuild test … -only-testing:AsdGazeCollectorTests` on the mode-6
project with zero errors and the same number of tests run as before.
Depends on: nothing for the work; A.6b for the final proof. Parallel with: A.6a, A.6b,
A.7a, B.16.

#### WP-A.7a Claims gate: prose literals and kit coverage - `I`

Goal: CLM-7 (scanner side; the gate wiring and the triage are WP-A.7b).
Files: `scripts/check_asd_claims.py` (`KIT_SOURCES` constant; `check_swift(roots)` takes
a list; new `check_prose_literals(roots)` or a `prose=True` branch inside `check_swift`:
on every non-comment line without a `claims-ok:` marker, every literal with whitespace
and `len ≥ 12` runs through `check_text(where, "ui", lit)` minus the four noisy rules; a
new CLI flag `--prose` turns it on so `ios_test.sh` can adopt it in A.7b without breaking
in between; `--selftest` gains the CLM-7 cases), `docs/spec/modules/asd-gaze-app-store-readiness/claims-denylist.md`
(describe the prose mode and the identifier exemption).
Not to touch: any `.swift` file, `scripts/ios_test.sh` (A.7b wires `--prose`).
Tests first: extend `selftest()` *before* changing the scanner: seeded failures
`let banner = "Clinically validated biomarker panel"` (rules `clin-valid`, `biomarker`),
`rec = "High risk (8–20). Seek prompt clinical evaluation."` (`clinical`),
`description = "Autism screening at home"` (`screening`), `label = "Your risk score"`
(`risk-score`) each in a non-UI line; passing cases `let key = "biomarkerKey"`,
`"DigitalBiomarker"`, a `claims-ok` prose line, and a comment line containing prose.
Commands: `python3 scripts/check_asd_claims.py --selftest`; `python3
scripts/check_asd_claims.py --swift` (unchanged result, since `--prose` is off);
`python3 scripts/check_asd_claims.py --swift --prose` prints the current findings (about
24 in 15 files at `03d84a3`) and exits 1; that list goes in the report for A.7b.
Depends on: nothing. Parallel with: everything in wave 1.

### Wave 2 (after wave 1 is integrated; alone among iOS packages, parallel with WP-C.17)

#### WP-A.7b `Biomarker*` → `Engagement*` renames, prose-literal triage, gate wiring - `I`

Goal: CLM-8, CLM-7 (gate on), ADR-0004 d7 closure.
Files (renames by `git mv`, then identifier replacement): kit
`Sources/CurvAsdGazeKit/SessionBiomarkerAggregator.swift` → `SessionEngagementAggregator.swift`,
`Sources/CurvAsdGazeKit/Inference/BiomarkerClassificationEngine.swift` →
`EngagementClassificationEngine.swift`, tests `BiomarkerDetailModelTests.swift`,
`SessionBiomarkerAggregatorTests.swift`, `BiomarkerClassificationEngineTests.swift`,
`Support/LegacyReference/LegacySessionBiomarkerAggregator.swift` (→ `Engagement…`); app
`Sources/Views/{BiomarkerAffectAndQualityViews,BiomarkerRiskStratificationView,BiomarkerDetailViews}.swift`
→ `Engagement…`; identifier edits in `AsdCoreBridge.swift`, `GazeProcessingActor.swift`,
`GazeSessionController.swift`, `AgeBandEstimationEngine.swift`, `SessionVaultManager.swift`
(type names only; JSON keys stay), `GazeScreeningWizardView.swift`,
`GazeSessionResultView.swift`, `GazeStimulusScreeningView.swift`, `MainGazeScannerView.swift`,
`ResearchKitStudyModels.swift` (`Biomarker` in a comment/type only; `digitalBiomarker =
"DigitalBiomarker"` stays), kit tests `AsdCoreBridgeTests`, `GoldenParityTests`,
`LegacyParityTests`; docs naming the types: `docs/ASD_GAZE_TRACKER_ARCHITECTURE.md`,
`docs/asd_app_sprint_tracker.md`, `domains/asd_screening/docs/NORMATIVE_BASELINES.md`,
`apps/ios_asd_gaze_app/README.md`, `docs/pitch_deck/*.md`,
`docs/spec/modules/asd-gaze-robustness/technical.md`,
`docs/spec/modules/asd-gaze-app-store-readiness/{technical,qa-validation}.md` (CLM-3's
file name), `docs/spec/modules/asd-rust-core-and-backend/{technical,work-packages}.md`
(mentions of the Swift names), `rust/crates/asd-core/src/{aggregate,metrics}.rs` **doc
comments only** (append "(now `SessionEngagementAggregator`)" to the historical pointer;
`cargo fmt` must not change anything else); `scripts/ios_test.sh` (claims step becomes
`python3 scripts/check_asd_claims.py --swift --prose --secrets`); the ~15 Swift files with
prose-literal findings (reword parent-facing copy; `// claims-ok: M-CHAT-R/F official
wording` on the instrument's category texts; `// claims-ok: Debug-only research
classifier, compiled out of Release (CLM-3)` inside `#if ASD_RESEARCH_CLASSIFIER` blocks;
`AppModeStore` mode descriptions reworded to "observation" vocabulary).
Not to touch: `docs/research/**`, `docs/spec/adr/ADR-0004*.md` (historical quotes),
`Resources/normative_baselines.json`, every `Tests/Fixtures/**` file, the
`SessionVaultManager` JSON keys `"biomarkers"`/`"biomarker"`, `ClinicalInteroperabilityStandards.swift`
raw values, `fastlane/screenshots/**`, Rust code outside the two doc comments, console.
Tests first: (1) a kit test that encodes a session through `SessionVaultManager` and
asserts the JSON still has `"biomarker"` keys (pins the wire name); (2) the four
`expected-session-v1.*.json` fixtures validated by `asd-cli schema validate` unchanged;
(3) `git grep -c Biomarker` over the four Swift roots equals the allowed set (acceptance
30); (4) `check_asd_claims.py --swift --prose` exits 0 on the tree.
Commands: `xcodegen generate` (picks up the renamed files), the full `scripts/ios_test.sh`,
`cargo fmt --check -p asd-core` (doc-comment-only change), `python3
scripts/check_asd_claims.py --selftest`.
Depends on: A.6a, A.6b, A.6c integrated (same files), A.7a (`--prose`). Parallel with:
WP-C.17 (console only). Not parallel with any other iOS package (ADR-0009 D10).

### Integration order

Wave 1: A.6a ∥ A.6b ∥ A.6c ∥ A.7a (∥ B.16 in the Rust module) → main session: `xcodegen
generate`, full `scripts/ios_test.sh`, `scripts/rust_test.sh`, commit. Wave 2: A.7b ∥
C.17 → `scripts/ios_test.sh` (now with `--prose`), console gates, commit.

### Traps for implementers (phase 6, iOS)

1. Setting the language mode through flags (`OTHER_SWIFT_FLAGS`, `-Xswiftc -swift-version
   6`) makes SwiftPM warn "language mode was overridden by extra Swift flags and may be
   inconsistent with code generated during the build"; and the app's flags never reach
   the package anyway (phase 1 trap 1). Flip `swift-tools-version` and `SWIFT_VERSION`.
2. `// swift-tools-version: 6.0` needs Xcode 16+ to even parse the manifest; CI `macos-15`
   has it, a developer on Xcode 15 cannot resolve the package. `xcodeVersion` in
   `project.yml` is only an XcodeGen hint but bump it so the project file says the truth.
3. In mode 6 the kit's `.when(configuration: .debug)` defines are unchanged; the
   release-symbol gate is the proof that nothing moved. Run both `check_release_symbols.sh`
   calls, not only Release.
4. The 5.9 + `complete` warning count under-reports mode-6 errors by a few: `static let`
   of a non-`Sendable` type is reported in both, but region-isolation "sending … risks
   causing data races" shows up only in mode 6 (two sites in the app). Build in mode 6
   for the final count; use the 5.9 proxy only to work ahead in `Tests/`.
5. `xcodebuild build-for-testing` with app errors reports the test target as "Unable to
   resolve module dependency: 'AsdGazeCollector'"; the test diagnostics are hidden until
   `Sources/` compiles. A.6c cannot see its own errors in mode 6 before A.6b lands.
6. `ISO8601DateFormatter`, `DateFormatter`, `NumberFormatter` are not `Sendable`. A
   `static let` of them is an error. Options in order of preference: per-call instance
   (cheap, these are used in cold paths), a `@MainActor static let` when every reader is
   on the main actor, a `nonisolated(unsafe) let` with a comment that the instance is
   never mutated after init. Never `@unchecked Sendable` on a type in `Sources/`.
7. `DataDeletionService` reads `@MainActor` statics for the BlendShape and bench folders
   from a nonisolated deletion routine. Do not make deletion `@MainActor` (it does file
   IO); make the folder URLs `nonisolated static let` or pass them in.
8. XCTest methods are nonisolated in mode 6; touching a `@MainActor` store from a test
   method is an error even though tests run on the main thread. `@MainActor` on the test
   class is the fix; `MainActor.assumeIsolated` hides the problem and is not allowed.
9. "Mutation of captured var in concurrently-executing code" in tests: the fix is a
   `Box<T>` (test-only `@unchecked Sendable`) or an `XCTestExpectation` whose
   fulfilment carries the value through `XCTWaiter`; do not switch to `Task.detached`
   or `DispatchQueue.sync`, which changes what the test proves about ordering.
10. `GazeScreeningWizardView.captureOrder` is a `static let [ScreeningWizardStep]`; the
    error is because the enum is not `Sendable`. Add the conformance; do not move the
    array into the view body.
11. Rename with `git mv` **before** editing contents so history follows the file; run
    `xcodegen generate` afterwards (no manual pbxproj edits; the tracked pbxproj is
    regenerated and committed by the main session, not the package).
12. `Biomarker` must survive in: `SessionVaultManager` JSON keys, `normative_baselines.json`
    (data and provenance prose), every fixture, `"DigitalBiomarker"`, Rust field names,
    `docs/research/**`. The `provenance` string in the signed config-bundle fixtures
    names `BiomarkerClassificationEngine`; it is signed data, touching it breaks the
    signature tests.
13. `apps/ios_asd_gaze_app/build-sim/` and `build/` hold stale indexes with the old
    names; exclude them from any grep that counts remaining occurrences.
14. The prose-literal scanner is line-based and does not understand `#if`. Strings inside
    `#if ASD_RESEARCH_CLASSIFIER` blocks are reported like any other; mark them
    `claims-ok` with the CLM-3 reason rather than loosening the rule.
15. The existing self-test fixture `let identifier = "biomarkerKey"` must keep passing:
    the prose rule triggers only on literals containing whitespace. A literal such as
    `"risk_score"` or `"clinical-crypto-unavailable"` is therefore not reported, by design.
16. Devanagari strings in `AppLocalizer.swift` are already scanned through the `Localizer`
    UI-call rule; the prose mode re-reports the same lines only if they are not UI calls,
    so expect no new hi/ne findings. If the mode reports a Devanagari literal, it is a
    model string and needs the same triage.
