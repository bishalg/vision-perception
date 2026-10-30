# ASD Gaze App, App Store Readiness - Technical Spec

Status: Proposed (2026-10-22). Branch snapshot reviewed: `dev` at `0c94cdc` plus an
uncommitted working tree (26 files, see work-packages.md, "Working tree").

## Purpose

Bring `apps/ios_asd_gaze_app` (XcodeGen app target `AsdGazeCollector`) and its gaze
engine `domains/asd_screening/ios` (SwiftPM package `CurvAsdGazeKit`) to a state that
can be submitted for App Store review under the Open Spectrum Tracking mission:
parents' daily observations of autistic children become clinically structured HL7
FHIR records, privacy-first, and explicitly not a diagnostic tool.

The app uses the front TrueDepth camera through ARKit face tracking for eye
engagement (gaze on screen areas of interest) and emotion engagement (52
blend-shape coefficients reduced to `FacialAffectMetrics`). It never persists camera
frames; that invariant is kept and this spec makes it verifiable.

Every requirement below cites the code it constrains as `path:line` at the reviewed
snapshot. Identifiers (`SIM-1`, ...) are stable and are referenced from
`acceptance.md`, `qa-validation.md` and `work-packages.md`.

## Scope

In scope: the iOS app, the Swift package, fastlane metadata and configuration,
privacy manifests, CI for the Swift targets, and the FHIR export the app produces.

Out of scope (deliberately): the Python research server (`tools/asd_research_server.py`),
Android metadata beyond removing claims, the palm app, any new clinical feature,
CoreML model training, and the on-device SQLCipher database described in
`domains/asd_screening/docs/DATA_STORAGE_AND_PIPELINE.md` (the app uses an AES-GCM
JSONL vault instead; the doc is aspirational and is not a requirement here).

## Inputs and outputs

Inputs: ARKit `ARFaceAnchor` stream (TrueDepth or A12+ front camera), stimulus
playlist media clock, caregiver questionnaire answers, caregiver diary entries.

Outputs: encrypted session vault files, on-device session history, PDF report,
FHIR R4 Bundle (JSON), ResearchKit/CareKit JSON, optional research upload.

## Current-state findings this spec is built on (verified)

| # | Finding | Evidence |
| --- | --- | --- |
| F1 | `startSession` silently enters simulation when `ARFaceTrackingConfiguration.isSupported` is false; nothing downstream can tell simulated from live data. | `domains/asd_screening/ios/Sources/CurvAsdGazeKit/GazeSessionController.swift:278-299` (fallback at `:298`); simulated frames generated at `:446-552` |
| F2 | A "Switch to Simulation" menu item is compiled into every configuration. | `apps/ios_asd_gaze_app/Sources/Views/MainGazeScannerView.swift:644-651` |
| F3 | Bubble calibration passes any target when simulating. | `apps/ios_asd_gaze_app/Sources/Views/BubbleCalibrationView.swift:570` |
| F4 | FHIR Observation is always `status: final` and has no provenance or capture-source marker. | `apps/ios_asd_gaze_app/Sources/Models/FHIRInteroperabilityMapper.swift:153`, `:172-179` |
| F5 | First-look latency is a constant 480 ms; eye/mouth dwell ratios fall back to 0.28/0.14 when no face hits; the same constants are defaults on the summary and record initialisers. | `GazeSessionController.swift:411`, `:389-390`, `:40-46`; `apps/ios_asd_gaze_app/Sources/Models/GazeSessionRecord.swift:47-55` |
| F6 | Those placeholders are exported as measurements (LOINC 8692-2 component, OMOP row with reference range) and weighted 14 % in the "risk score". | `FHIRInteroperabilityMapper.swift:20-38`, `:243-253`; `domains/asd_screening/ios/Sources/CurvAsdGazeKit/Inference/BiomarkerClassificationEngine.swift:184-186` |
| F7 | Session duration and dwell times assume exactly 60 fps regardless of actual frame timing. | `GazeSessionController.swift:379-399` |
| F8 | Medical claims: store name "EarlyGaze: Autism & ADHD Test"; "first FDA-grade at-home autism screening tool", "COPPA/HIPAA aligned", "IRB Protocol #2026-088" in release notes; "High Risk" raw value shown to parents; "ensuring high diagnostic reliability"; "calibrated Bayesian risk probability score"; keyword "biomarker"; Android short description "screener for Autism & ADHD". | `fastlane/metadata/en-US/name.txt`, `release_notes.txt:3,15-17`, `keywords.txt`, `promotional_text.txt`; `apps/ios_asd_gaze_app/Sources/Models/ResearchKitStudyModels.swift:204-208`; `apps/ios_asd_gaze_app/Sources/Views/BiomarkerRiskStratificationView.swift:241`, `:253`; `fastlane/metadata/android/en-US/short_description.txt` |
| F9 | A heuristic "risk stratification" ("Elevated Risk - Specialist Evaluation Recommended") with a "confidence" is shown to parents and written to CareKit outcomes; the fallback labels itself "Rule-Based Clinical Baseline" but the CoreML path labels its feature "CoreML ANE Multi-Biomarker Synthesis". | `BiomarkerClassificationEngine.swift:7-11`, `:168-243`, `:153`; `apps/ios_asd_gaze_app/Sources/Views/GazeStimulusScreeningView.swift:583-589`; `MainGazeScannerView.swift:739-747` |
| F10 | `BlendShapeCSVLogger.shared.startListening()` runs at app launch, before any consent, and appends the child's 52 blend-shape coefficients to a plain CSV in Application Support with no file protection and no backup exclusion. | `apps/ios_asd_gaze_app/Sources/App/AsdGazeApp.swift:53`; `apps/ios_asd_gaze_app/Sources/Models/BlendShapeCSVLogger.swift:25-31`, `:104-109` |
| F11 | The encrypted vault and the research sync queue are also written without `FileProtectionType` and without backup exclusion; the sync queue is plaintext JSON in Documents. | `domains/asd_screening/ios/Sources/CurvAsdGazeKit/SessionVaultManager.swift:30-37`, `:139-141`; `apps/ios_asd_gaze_app/Sources/Models/ResearchKitSyncService.swift:93-94`, `:143` |
| F12 | Personal reviewer contact data and a demo password are committed (and pushed) under `fastlane/metadata/review_information/`; the Appfile and config carry a personal Yahoo Apple ID and an "Individual" team. | `apps/ios_asd_gaze_app/fastlane/metadata/review_information/{phone_number,email_address,demo_password,notes}.txt`; `fastlane/Appfile:5-7`; `fastlane/config.yml:9-11`; `fastlane/.env.example:17`; commit `307312e` is on `origin/dev` |
| F13 | The child's real name is used as the FHIR subject and as `participantId` of persisted session records in the stimulus flow and the result view. | `GazeStimulusScreeningView.swift:575`; `apps/ios_asd_gaze_app/Sources/Views/GazeSessionResultView.swift:55`, `:604`; flows into `FHIRInteroperabilityMapper.swift:173` (`Patient/<participantId>`) |
| F14 | The research consent document is populated with a fictional IRB id, institution and principal investigator; the sync endpoint is hardcoded to `http://127.0.0.1:8089`. | `ResearchKitStudyModels.swift:124-131`; `ResearchKitSyncService.swift:78` |
| F15 | Data races: `aoiTracker` (var, `:99`, written on main `:245`, `:303`, read on AR queue `:767`), `calibrationProfile` (`@Published`, written on main `:211,223,235`, read on AR queue `:718`), `isRecording` (`:258,327` vs `:828`), `sessionStartTime` (`:252` vs `:757`), `blinkRecoveryFramesRemaining` (`:276` vs `:734-736`), OneEuro filters reset on main (`:213-214,263-264`) while filtered on AR queue (`:821-822`); the class is `@unchecked Sendable` (`:78`). `GazeProcessingActor` exists with an equivalent pipeline (`GazeProcessingActor.swift:219`, `:379`, `:477`) but the AR delegate never calls it. | `GazeSessionController.swift` as cited |
| F16 | No handling of camera permission denial, `ARSession` failure or interruption; no idle-timer disable during a session; app backgrounding does not stop the session. | `GazeSessionController.swift:612-870` implements only `didUpdate frame`, `didUpdate anchors`, `didRemove anchors`; no `AVCaptureDevice.authorizationStatus` call anywhere in `apps/ios_asd_gaze_app/Sources` |
| F17 | The subject-age estimator receives every captured frame, unconditionally; the camera usage string does not mention age estimation. | `GazeSessionController.swift:616-627`; `apps/ios_asd_gaze_app/project.yml:47` |
| F18 | Privacy manifests declare only `Health` and `OtherDataTypes`; nothing about face/body data, research uploads or the HealthKit State of Mind writes. | `apps/ios_asd_gaze_app/Resources/PrivacyInfo.xcprivacy:44-71`; `domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources/PrivacyInfo.xcprivacy` |
| F19 | FHIR coding: invented fallback `80628-1-<qId>` linkIds, SNOMED finding `247540003` used as the code of a numeric ratio, `questionnaire` set to a LOINC URI string, bare `Patient/` references, no Bundle, Provenance, Device or RelatedPerson; mood diary, milestones and medications are not exported. | `FHIRInteroperabilityMapper.swift:196`, `:61-78`, `:214`, `:172-175` |
| F20 | No iOS CI: `.github/workflows/ci.yml` builds only C++ targets. fastlane Appfile is an individual account; bundle ids `org.curv.asd.gazecollector[.carewidget]`. | `.github/workflows/ci.yml`; `project.yml:38,88` |
| F21 | `age_suitability_url.txt` points to `https://astro-fusion.com`; `SWIFT_VERSION` is 5.9 with `SWIFT_STRICT_CONCURRENCY: complete` (warnings only). | `fastlane/metadata/en-US/age_suitability_url.txt`; `project.yml:12,17` |
| F22 | Session history (`GazeSessionHistoryStore`) is persisted in `UserDefaults`. | `GazeSessionRecord.swift:171-215` |
| F23 | Revoking research consent leaves queued payloads on disk; a later re-consent would upload them. | `apps/ios_asd_gaze_app/Sources/Models/ResearchKitStudyStore.swift:73-86`; `ResearchKitSyncService.swift:174-199` |
| F24 | There is no "delete my data" action; Settings shows only an informational vault row. The ethics protocol promises deletion on request. | `apps/ios_asd_gaze_app/Sources/Views/SettingsView.swift:512-540`; `domains/asd_screening/docs/ETHICS_PROTOCOL.md` section 1.2 |

## Requirements

### SIM - Simulation mode and data provenance (ADR-0001)

- SIM-1. `CurvAsdGazeKit` MUST define `public enum CaptureSource: String, Codable, Sendable { case trueDepth = "truedepth"; case simulated = "simulated" }` in `GazeTypes.swift`.
- SIM-2. `GazeSessionController.startSession` MUST NOT fall back to simulation. When face tracking is unsupported it MUST set `trackingStage = .unavailable(reason:)` (new case) and leave `isRecording == false`. The existing Bool `isSimulationMode` (`GazeSessionController.swift:112`) MUST become `public private(set) var captureSource: CaptureSource?` plus a read-only computed `isSimulationMode` kept for one release for source compatibility.
- SIM-3. Simulation MUST only be enterable through `startSimulatedSession(...)`, which MUST be compiled only under `#if ASD_SIMULATION_ENABLED`. Because `OTHER_SWIFT_FLAGS` in `project.yml` does not propagate to SwiftPM package targets, the kit flag MUST be declared in `domains/asd_screening/ios/Package.swift` as `swiftSettings: [.define("ASD_SIMULATION_ENABLED", .when(configuration: .debug))]` on the `CurvAsdGazeKit` target, and the app flag in `project.yml` Debug `OTHER_SWIFT_FLAGS` (the file already uses this pattern for `ASD_APP_STORE_AGE_ASSURANCE` at `project.yml:60-66`). The same rule applies to `ASD_RESEARCH_CLASSIFIER` (kit and app) and `ASD_ML_DATA_EXPORT` (app only). `tickSimulatedFrame` MUST remain available to tests but MUST be a no-op unless `captureSource == .simulated`.
- SIM-4. `GazeSessionReportSummary`, `SessionHeader` (`FacialAffectMetrics.swift:271`) and `GazeSessionRecord` MUST carry a non-optional `captureSource: CaptureSource` without a default value in their designated initialisers. `GazeSessionRecord.schemaVersion` becomes `"v1.6"`; decoding v1.5 and earlier records MUST default `captureSource` to `.trueDepth` only when `zeroRawVideoEnforced == true` and the record predates this version, and such records MUST be marked `legacyProvenance: true` (new optional field) so exports can label them.
- SIM-5. The following MUST refuse simulated input: `FHIRInteroperabilityMapper.makeFHIRObservation` and `makeOMOPMeasurements` (return `nil`/empty with a logged reason), `GazeSessionHistoryStore.record` (ignore), `CareKitClinicalStore.recordScreeningOutcome` and `CarePlanStore.markScreeningCompleted` (ignore), `NormCollectionGate` (already excludes; switch its parameter to `CaptureSource`), `ResearchKitStudyStore` screening results, and `DoctorReportGenerator` (render a "simulated, not for clinical use" banner if ever given one). The vault MAY store simulated sessions; the filename MUST then end in `.sim.jsonl.enc`.
- SIM-6. The "Switch to Simulation" menu entry (`MainGazeScannerView.swift:644-651`), the simulation auto-pass in bubble calibration (`BubbleCalibrationView.swift:570`) and the simulated-frame tickers in `GazePositioningStepView.swift:324`, `GazeStimulusScreeningView.swift:475`, `MainGazeScannerView.swift:762` MUST be wrapped in `#if ASD_SIMULATION_ENABLED`.
- SIM-7. Release builds MUST declare `UIRequiredDeviceCapabilities` containing `front-facing-camera` (in `Config/Info-Additions.plist`, merged by `project.yml`) and MUST NOT require `arkit`, which visionOS does not offer and which would block Apple Vision Pro (App Review, Oct 2026). Face tracking is gated at runtime: `GazeSessionController.startSession` refuses when `ARFaceTrackingConfiguration.isSupported` is false, and the screening entry point MUST show `UnsupportedDeviceView` (new) with a plain-language explanation when `ARFaceTrackingConfiguration.isSupported` is false.
- SIM-8. Any UI that displays a session MUST show a persistent "Simulated data" badge when `captureSource == .simulated`.

### PLC - Placeholder measurements (ADR-0003, decision 4)

- PLC-1. `GazeSessionReportSummary.firstEyeLatencyMs` MUST become `Double?` and MUST be `nil` until an actual first-fixation detector exists. The 480.0 constant at `GazeSessionController.swift:411` and the defaults at `:40-46` and `GazeSessionRecord.swift:47-55` MUST be removed; initialisers MUST require every metric explicitly (tests construct fixtures through a `GazeSessionReportSummary.fixture(...)` helper placed in the test target, not in production code).
- PLC-2. `eyeDwellRatio` and `mouthDwellRatio` MUST be `nil` when no eye-or-mouth AOI hit occurred (`:388-390`), never 0.28/0.14. UI MUST render "Not observed" for `nil`.
- PLC-3. Exports MUST omit a component or row whose value is `nil` (FHIR component for LOINC 8692-2, OMOP row 37048700). Exports MUST NOT substitute a default.
- PLC-4. `BiomarkerClassificationEngine.runClinicalBaselineInference` MUST treat a `nil` metric as absent (re-normalise weights over present metrics) and MUST report `executionDevice = .fallbackHeuristic` and `primaryContributingFeature` without the word "CoreML" unless a CoreML model actually ran (`:153`).
- PLC-5. `durationSec` and dwell seconds MUST be computed from frame timestamps: duration = last `timestampMs` minus first; each frame's dwell contribution = min(interval to next frame, 100 ms). The `1.0/60.0` constants at `:379`, `:399`, `:514`, `:795` MUST go. `aoiDwellMs` on `GazeFrame` MUST hold the measured interval.

### CLM - Claims and naming (ADR-0004)

- CLM-1. The App Store name, subtitle, keywords, promotional text, description and release notes (all locales, iOS and Android) MUST NOT contain: "test" as a noun describing the app, "diagnos*" except inside a sentence that says the app does not diagnose, "FDA", "HIPAA", "IRB", "clinically validated", "biomarker", "ADHD", "autism screening"/"autism test", "risk score", "Bayesian", "probability". The exact denylist is `scripts/check_asd_claims.py` (new) and is the source of truth.
- CLM-2. The same denylist MUST be enforced on user-facing strings in `apps/ios_asd_gaze_app/Sources/**` (string literals inside `Text(`, `Label(`, `String(format:`, `Localizer` tables, and `DoctorReportGenerator` draw calls). Identifiers and comments are exempt; an inline `// claims-ok: <reason>` marker exempts a line for terms that are the official name of a licensed instrument (M-CHAT-R/F risk categories).
- CLM-3. The "risk stratification" UI (`BiomarkerRiskStratificationView.swift`, the stratification badge in `GazeSessionResultView.swift:341-371`, lines `:613`, `:661`), the `riskScore`/`riskStratification` outcome fields written to CareKit (`CareKitClinicalStore.swift:117-127`) and the `classification` block of ResearchKit JSON (`SessionVaultManager.swift:204-212`) MUST be compiled only under `#if ASD_RESEARCH_CLASSIFIER` (Debug-only flag). Release builds MUST present `AttentionFlag` (`domains/asd_screening/ios/Sources/CurvAsdGazeKit/AttentionFlag.swift`), whose copy is already non-diagnostic, as the only interpretation layer.
- CLM-4. `MCHATRiskLevel` raw values (`ResearchKitStudyModels.swift:204-208`) MUST stay unchanged (they are persisted). A `displayName` MUST be added and every `riskLevel.rawValue` UI usage (`CaregiverDashboardView.swift:222`, `:421`; `MCHATSurveyModalView.swift:242`, `:399`; `DoctorReportGenerator.swift:363`; `GazeSessionResultView.swift:668`) MUST use `displayName`, rendered as "M-CHAT-R/F: <level> (follow-up needed / discuss with your pediatrician)" with the instrument name present.
- CLM-5. Every results surface (session result, dashboard card, PDF, FHIR Bundle `Composition.text` if present, store description) MUST include the sentence "This is not a diagnostic tool." and the mission line "Part of Open Spectrum Tracking".
- CLM-6. The primary App Store category MUST be `HEALTH_AND_FITNESS` unless the human maintainer chooses `MEDICAL` in ADR-0005; the review notes MUST state the app is an observation diary, not a medical device.

### CNS - Consent, capture gating and data protection (ADR-0002)

- CNS-1. No `ARSession` MUST be created before `ChildProfileStore.shared.profile.hasConsentedPIA == true` (`ChildProfile.swift:91`). `GazeSessionController.startSession` MUST take a `ConsentToken` (a value type that can only be built by `ChildProfileStore` after consent) as a parameter in the app layer wrapper; the kit API stays consent-agnostic.
- CNS-2. `BlendShapeCSVLogger` MUST be compiled only under `#if ASD_ML_DATA_EXPORT` (never set in Release). When compiled, it MUST only write while `isRecording`, only when `ResearchStudyEnrollment.normCollectionOptIn == true`, and MUST write with `FileProtectionType.complete` into a directory excluded from backup. `AsdGazeApp.swift:53` MUST be inside the same `#if`.
- CNS-3. `SessionVaultManager` MUST create its directory with `isExcludedFromBackup = true` and write files with `.completeFileProtection` (`Data.WritingOptions`). The Keychain key stays `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
- CNS-4. `ResearchKitSyncService` queue, dead-letter and quarantine directories MUST live under Application Support (not Documents), be excluded from backup, and be written with `.completeFileProtectionUntilFirstUserAuthentication`.
- CNS-5. `ResearchKitStudyStore.revokeConsent` MUST purge the sync queue, dead-letter and quarantine folders (through a new `ResearchKitSyncService.purgeAllQueued()`), and MUST cancel any in-flight upload.
- CNS-6. Settings MUST offer "Delete all observation data" which deletes: vault directory, `GazeSessionHistoryStore` records, sync queues, `BlendShapeML` directory if present, calibration profile, M-CHAT results and enrollment. It MUST NOT delete CareKit care plans silently; it MUST ask separately. The action MUST require a confirmation dialog and MUST be reachable within two taps from Settings.
- CNS-7. `participantId` on `GazeSessionRecord` and the FHIR subject MUST be a pseudonymous token (`ResearchStudyEnrollment.participantId` when enrolled, else `child-<sessionId prefix 8>` as `MainGazeScannerView.swift:709-711` already does). `childName` (`ChildProfile.swift:87`) MUST NOT appear in any persisted record, export or FHIR resource; it MAY be rendered on screen and on the PDF cover only.
- CNS-8. The subject-age estimator MUST be off by default (`GazeSessionController.isSubjectAgeEstimationEnabled = false`), enabled only by `SubjectAgeQuickCheckCard` for its own session, and `project.yml` camera usage string MUST disclose "an optional on-device age-range check" when that feature is compiled in.
- CNS-9. The research study feature (consent document, enrollment, sync, norm collection) MUST be hidden in Release unless a `ResearchStudyConfiguration` is present (Info.plist dictionary `ASDResearchStudy` with `ingestURL` (https only), `studyIdentifier`, `irbProtocolId`, `institution`, `principalInvestigator`, `consentVersion`). The fictional defaults at `ResearchKitStudyModels.swift:124-131` MUST be removed; `ResearchKitSyncService` MUST fail closed when the URL is absent or not https.

### CAM - Camera lifecycle (ADR-0002, decision 7)

- CAM-1. Before starting a session the app MUST check `AVCaptureDevice.authorizationStatus(for: .video)`; `.notDetermined` triggers `requestAccess`; `.denied`/`.restricted` shows a view with an "Open Settings" button (`UIApplication.openSettingsURLString`). No session starts in those states.
- CAM-2. `GazeSessionController` MUST implement `session(_:didFailWithError:)`, `sessionWasInterrupted(_:)`, `sessionInterruptionEnded(_:)`; on failure or interruption it MUST set `trackingStage = .unavailable(reason:)`, stop appending frames, and record `interruptionCount` and `interruptedSec` on the summary. A summary with `interruptedSec > 0` MUST have `qualityVerdict` no better than `.degraded`.
- CAM-3. While `isRecording`, `UIApplication.shared.isIdleTimerDisabled` MUST be true and MUST be restored on stop, failure, or scene phase change to `.background`. Moving to `.background` MUST stop the session (frames so far are kept).

### CONC - Concurrency (ADR-0002 does not cover; see work package WP-2.1)

- CONC-1. All per-frame state (`aoiTracker`, `calibrationProfile` copy, `isRecording`, `sessionStartTime`, `clipStartTime`, blink debounce, 1-Euro filters, `lastValid*`) MUST be owned by exactly one isolation domain: `GazeProcessingActor`. The `ARSessionDelegate` MUST only build a `FaceAnchorSnapshot` (`GazeProcessingActor.swift:10`) and hand it to the actor; UI updates come back as `GazeUIState` on the main actor. The duplicate pipeline in `GazeSessionController.session(_:didUpdate anchors:)` (`:695-856`) MUST be deleted, not kept alongside.
- CONC-2. `GazeSessionController` MUST be `@MainActor` and MUST NOT be `@unchecked Sendable`. The `ARSessionDelegate` conformance MUST move to a small `final class ARSessionBridge: NSObject, ARSessionDelegate` that holds the actor reference and is `Sendable`.
- CONC-3. The package MUST compile with `-strict-concurrency=complete` producing zero warnings in `CurvAsdGazeKit`; `swift test --sanitize=thread` MUST pass for the kit.
- CONC-4. Order of events MUST be preserved: frames delivered before `stopSession` MUST be included; frames after MUST be dropped. `stopSession` MUST become `async` (`finalizeSession` on the actor) and a synchronous wrapper MAY exist only under `#if DEBUG` for existing tests.

### PRV - Privacy manifests and disclosures

- PRV-1. Apple's manifest "collected data" means data transmitted off the device. `apps/ios_asd_gaze_app/Resources/PrivacyInfo.xcprivacy` MUST therefore declare, all with `Linked = false` and `Tracking = false` and purpose `NSPrivacyCollectedDataTypePurposeAppFunctionality`: (a) `NSPrivacyCollectedDataTypeHealth` only if the research upload (CNS-9) is compiled in, because diary observations are part of the study bundle; (b) `NSPrivacyCollectedDataTypeOtherDataTypes` (gaze telemetry summaries) only under the same condition. When no upload path is compiled in (the default Release), the `NSPrivacyCollectedDataTypes` array MUST be empty and the App Store Connect privacy label MUST say "Data Not Collected". `NSPrivacyTracking` stays `false`, `NSPrivacyTrackingDomains` stays empty. The user-initiated share sheet (PDF, FHIR file) is not collection and MUST NOT be declared.
- PRV-2. Both manifests MUST list exactly the required-reason API categories the target actually uses. The list MUST be derived with `scripts/check_privacy_api_use.sh` (new; greps the target's sources for `UserDefaults`, `.creationDate`/`.modificationDate`/`NSFileCreationDate`/`contentModificationDateKey`, `systemUptime`/`mach_absolute_time`, `volumeAvailableCapacity`) and the script MUST fail when a used category is missing from the manifest or a declared category is unused. Current state: the kit declares `UserDefaults` and `SystemBootTime` (`domains/.../Resources/PrivacyInfo.xcprivacy:85-100`); the implementer verifies rather than assumes.
- PRV-3. `project.yml` usage strings MUST mention: camera (gaze and facial expression, on device, never stored), optional age-range check (CNS-8), HealthKit read/write (already present), speech, microphone, photos (already present). Motion string stays.

### FHIR - Profile and coding (ADR-0003)

- FHIR-1. The export unit MUST be a FHIR R4 `Bundle` of `type: collection` produced by `FHIRBundleBuilder` (new) containing: one `Patient` (id = participant token, no name, `birthDate` absent, extension for age in months), one `RelatedPerson` (relationship `PRN`/parent, no name unless the caregiver typed one and opted to include it), one `Device` (`deviceName` = app display name, `version` = `MARKETING_VERSION`, `type` text "Mobile gaze engagement observation app"), one `Provenance` per Observation (`agent.who` = Device, `recorded`, `activity` = `http://terminology.hl7.org/CodeSystem/v3-DataOperation#CREATE`, and `entity.what` for the vault file id), and Observations/QuestionnaireResponses with `fullUrl` = `urn:uuid:<uuid>` and relative references resolved inside the bundle.
- FHIR-2. `Observation.status` MUST be `final` when `captureSource == .trueDepth` and `qualityVerdict` is `optimal` or `acceptable`; `preliminary` when `degraded`; no Observation when `rejected` or simulated. `Observation.dataAbsentReason` MUST NOT be used to smuggle placeholders; absent metrics are absent components (PLC-3).
- FHIR-3. Observation codes: panel code stays LOINC `93044-6` with SNOMED `713444003` as a second coding. Components MUST use a local CodeSystem `<FHIRNamespaces.local>/CodeSystem/gaze-engagement` (constant, default `http://curv.org/fhir`, to be replaced per ADR-0005) with codes `social-preference-score`, `eye-region-dwell-ratio`, `mouth-region-dwell-ratio`, `valid-frame-ratio`, `social-smile-intensity-mean`, `smile-reactivity-index`, `flat-affect-score`, `surprise-score-mean`, `composite-engagement`, `clip-score-<stimulusId>`. SNOMED `247540003` (`FHIRInteroperabilityMapper.swift:61-78`) MUST NOT be used as a measurement code; it MAY appear as an `interpretation` only if a clinician-authored rule produces it, which is out of scope.
- FHIR-4. `QuestionnaireResponse.questionnaire` MUST be the canonical `http://loinc.org/q/80628-1`; `item.linkId` MUST be the LOINC item code from `StandardCodingSystems.LOINC.mchatQuestionCodes`; the fallback `"80628-1-\(qId)"` (`:196`) MUST be deleted and `loincCode(forMCHATQuestionId:)` MUST return a non-optional `String` (precondition on 1...20). Total score and risk level MUST be emitted as a separate `Observation` with LOINC `80649-7` (`valueInteger`) and `80650-5` (`valueCodeableConcept`, text = official M-CHAT-R/F category).
- FHIR-5. Mood diary entries MUST map to `Observation` (category `survey`, code local `mood-diary-entry`, `valueCodeableConcept` for mood, `note` for free text, `component` for intensity), milestones to `Observation` (category `survey`, code local `developmental-milestone`, value `CodeableConcept` achieved/not yet), medications to `MedicationStatement` (`medicationCodeableConcept.text` = name, `dosage.text`, `effectivePeriod`). These live in `FHIRCareMapper.swift` (new) with their codes in `FHIRCareCodings.swift` (new) so the package does not touch `ClinicalInteroperabilityStandards.swift`.
- FHIR-6. The OMOP export MUST keep concept ids only where they are real standard concepts; `40769032`, `40769033`, `40769034` (`:236`, `:256`, `:267`) MUST be verified against Athena by the implementer and replaced with `0` (no matching concept) plus `measurement_source_value` if not verifiable. Reference ranges (`:250-251`, `:261-262`) MUST be removed; the app has no validated norms.
- FHIR-7. Every FHIR JSON produced in tests MUST validate against the FHIR R4 JSON schema (`fhir.schema.json`, vendored under `apps/ios_asd_gaze_app/Tests/Fixtures/`, 4.0.1) using the test-only validator in `FHIRSchemaValidationTests.swift` (JSONSchema via `swift-json-schema` is NOT allowed as a dependency; the test MUST use a minimal structural check: resourceType, required fields per resource, reference resolution inside the bundle).

### CI - Build, test and configuration

- CI-1. `.github/workflows/ios.yml` (new) MUST run on `macos-15` (or newer available image) on push/PR touching `apps/ios_asd_gaze_app/**`, `domains/asd_screening/ios/**`, `.github/workflows/ios.yml`: (a) `swift test --scratch-path "$RUNNER_TEMP/kit"` in `domains/asd_screening/ios`; (b) `xcodegen generate --spec apps/ios_asd_gaze_app/project.yml`; (c) `xcodebuild test ... -derivedDataPath "$RUNNER_TEMP/dd" -parallel-testing-enabled NO CODE_SIGNING_ALLOWED=NO` for `AsdGazeCollectorTests`; (d) a Release `build` with `CODE_SIGNING_ALLOWED=NO`; (e) `python3 scripts/check_asd_claims.py`; (f) `bundle exec fastlane validate_metadata` is NOT required (needs Ruby env) but `fastlane/metadata` length checks MUST be re-implemented in the claims script.
- CI-2. Derived data, scratch paths and simulators MUST never be inside the repo; local commands use `/private/tmp/asd-*`.
- CI-3. `project.yml` MUST set `SWIFT_VERSION: "6.0"` once CONC-3 holds; until then it stays 5.9 and `SWIFT_TREAT_WARNINGS_AS_ERRORS: YES` is set for the kit only.
- CI-4. The bundle identifier, team id, Apple ID and review contact MUST be read from environment (`APP_IDENTIFIER`, `APPLE_TEAM_ID`, `APPLE_ID`, `REVIEW_CONTACT_FIRST_NAME`, `REVIEW_CONTACT_LAST_NAME`, `REVIEW_CONTACT_EMAIL`, `REVIEW_CONTACT_PHONE`, `REVIEW_DEMO_USER`, `REVIEW_DEMO_PASSWORD`) with no committed defaults that identify a person. `fastlane/metadata/review_information/` MUST be deleted and listed in `.gitignore`; the `update_metadata` lane MUST pass `app_review_information:` from env.
- CI-5. `fastlane/metadata/en-US/age_suitability_url.txt`, `marketing_url.txt`, `support_url.txt`, `privacy_url.txt`, `privacy_policy_url.txt` MUST point at URLs the publishing organisation controls (ADR-0005); the claims script MUST fail on `astro-fusion.com`.

## Public API changes (kit), fixed here so packages can run in parallel

```swift
// GazeTypes.swift
public enum CaptureSource: String, Codable, Sendable { case trueDepth = "truedepth", simulated = "simulated" }
public enum TrackingStage: Sendable, Equatable {  // existing cases retained
    case unavailable(reason: TrackingUnavailableReason)       // new
}
public enum TrackingUnavailableReason: String, Sendable, Codable {
    case faceTrackingUnsupported, cameraPermissionDenied, sessionFailed, interrupted
}

// GazeSessionController.swift (after WP-2.1 it is @MainActor)
public private(set) var captureSource: CaptureSource?
public var isSubjectAgeEstimationEnabled: Bool            // default false
public func startSession(battery: StimulusAOITracker? = nil, stimulusId: String? = nil) // live only
#if ASD_SIMULATION_ENABLED
public func startSimulatedSession(battery: StimulusAOITracker? = nil, stimulusId: String? = nil)
#endif
public func stopSession() async -> GazeSessionReportSummary

// GazeSessionReportSummary: all stored properties, no defaults; new fields
public let captureSource: CaptureSource
public let firstEyeLatencyMs: Double?
public let eyeDwellRatio: Double?
public let mouthDwellRatio: Double?
public let interruptionCount: Int
public let interruptedSec: Double

// SessionHeader: new required field
public let captureSource: CaptureSource
```

## Dependencies

- XcodeGen (`/opt/homebrew/bin/xcodegen`), Xcode 27.1 locally; CI image must provide an iOS 18+ simulator.
- CareKit 4.1.0 via SwiftPM (network needed on first resolve).
- `scripts/check_asd_claims.py` (new, Python 3.11, no third-party packages).

## Deliberately left out

- No new on-device database, no SQLCipher.
- No first-fixation latency detector; the metric is removed from exports until one exists with tests on recorded telemetry.
- No ML classifier work; the classifier is Debug-only.
- No Android work beyond the metadata claims denylist.
- No localisation of new strings beyond English (hi/ne-NP get the English string with a TODO marker; the claims script covers those locales).
