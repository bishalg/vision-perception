# iOS Pediatric Gaze Collector (`apps/ios_asd_gaze_app`)

> **Application**: EarlyGaze Observation Capture. A capture tool that records where a child looks during short clips and delivers each session to the child's clinic. This is not a diagnostic tool. It does not measure, screen for or diagnose any condition; your clinic reviews each session.  
> **Platform**: iOS / iPadOS 18.0+ (TrueDepth ARKit face tracking; Swift 6 language mode with complete strict-concurrency checking)  
> **Architecture**: Model-View-Controller (MVC) + Apple ResearchKit & CareKit + Liquid Glass Design  
> **Package Link**: [`domains/asd_screening/ios`](../../domains/asd_screening/ios) (`CurvAsdGazeKit`)  
> **Full Architecture & Handover Guide**: [`docs/ASD_GAZE_TRACKER_ARCHITECTURE.md`](../../docs/ASD_GAZE_TRACKER_ARCHITECTURE.md)  
> **Roadmap & Specifications**: [`domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md`](../../domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md)  

---

## 1. Overview

This application records short gaze observation sessions and delivers them to the clinic that reviews them:
1. **Consent and onboarding**: caregiver consent, a short child profile (age band, session language), and a permission primer before the camera is used.
2. **Interactive 5-point bubble calibration**: fast optical axis calibration using floating popped bubbles.
3. **Synchronized stimulus playback**: split-screen and naturalistic face clips recorded alongside 60 Hz TrueDepth telemetry.
4. **Delivery to the clinic**: each session is kept on the device or sent to the clinic's console for review. The App Store build shows no verdict, band, flag or report on the device.
5. **Zero raw video recording**: camera frames are processed strictly in volatile memory. Only numerical telemetry is encrypted at rest using Apple CryptoKit (`AES-GCM-256`).
6. **Debug and DemoRelease only**: the caregiver dashboard, daily notes diary, milestone checklists, PDF report and M-CHAT questionnaire exist only in those configurations. They are not part of the App Store build.

### Build configurations

| Configuration | Purpose | Swift flags (see `project.yml`) |
| --- | --- | --- |
| `Debug` | Full UI including the dashboard, simulation and bench tools | `ASD_SIMULATION_ENABLED`, `ASD_RESEARCH_CLASSIFIER`, `ASD_DEMO_TOOLS`, `ASD_BENCH_CAPTURE` |
| `DemoRelease` | Demo build with the result screens | `ASD_APP_STORE_AGE_ASSURANCE`, `ASD_DEMO_TOOLS` |
| `Release` | **App Store build**: tabs Capture / Sessions / Settings, no verdict surfaces ([`store-collection-build.md`](../../docs/spec/modules/asd-gaze-app-store-readiness/store-collection-build.md)) | `ASD_APP_STORE_AGE_ASSURANCE`, `ASD_STORE_BUILD` |
| `StoreSim` | The store UI on a simulator, for App Store screenshots; never archived | `ASD_STORE_BUILD`, `ASD_STORE_SCREENSHOTS`, `ASD_APP_STORE_AGE_ASSURANCE` |

Store screenshots: `apps/ios_asd_gaze_app/scripts/run_store_screenshots.sh` drives the Maestro flows in
`maestro/flows/store/` against `StoreSim`; the Gherkin mirrors are `Tests/bdd/features/store_*.feature`.

### Localisation

`Resources/Localizable.xcstrings` and `Resources/InfoPlist.xcstrings` carry English, Hindi and Nepali (`en`/`hi`/`ne`).
Code looks strings up with `Localizer.tr` (`Sources/UI/AppLocalizer.swift`). `scripts/ios_test.sh` runs
`asd-cli i18n lint` (no hard-coded UI literals) and `asd-cli i18n check` (every key has all three languages).

### Consent and privacy

Views live in `Sources/Views/Consent/` (`ConsentFlowView`, `ConsentReviewView`, `PermissionPrimerView`,
`PrivacyDashboardView`, `BeforeYouBeginView`). Models: `ConsentDocument`, `ConsentRecordStore`,
`FeatureOptIns`, `PermissionPrimer` in `Sources/Models/`.

---

## 2. Running the App

### Option A: Open Directly in Xcode (GUI)
```bash
open apps/ios_asd_gaze_app/AsdGazeCollector.xcodeproj
```
1. Select the `AsdGazeCollector` scheme.
2. Select your connected device (e.g., **Bishal’s iPhone 17 Pro**) or an iOS Simulator.
3. Press **`Cmd + R`** to build and run.

### Option B: Deploy to Connected iPhone (CLI)
```bash
./scripts/run_gaze_app_device.sh   # from the repository root
```
Builds, signs, installs via `xcrun devicectl`, and launches directly on your paired physical device.

### Option C: Run in iPhone Simulator (CLI)
```bash
./scripts/run_gaze_app_sim.sh   # from the repository root
```
Boots the first available iPhone simulator, builds, installs, and launches automatically.

---

### Running the tests

One command runs every gate: Rust core xcframework, kit tests, kit ThreadSanitizer tests, a strict-concurrency
(Swift 6, warnings as errors) kit build, XcodeGen, app tests on a simulator, unsigned Release and DemoRelease builds,
then the static gates: claims/secrets/prose (`scripts/check_asd_claims.py --swift --prose --secrets`),
JSON-schema contract, i18n lint and check, privacy API use, and Release and DemoRelease binary symbols.

```bash
scripts/ios_test.sh --dry-run          # print the commands only
DERIVED_DATA=/private/tmp/asd-app-dd scripts/ios_test.sh
SIM_NAME="iPhone 16" scripts/ios_test.sh   # if the default simulator is not installed
```

Build output always lives outside the repository (`DERIVED_DATA`, default `/private/tmp/asd-app-dd`).
The JSON-schema contract gate is Rust: the script builds `asd-cli` (`cargo build -p asd-cli`, so a Rust toolchain is required) and runs `asd-cli schema check` plus `asd-cli schema validate` against `Tests/Fixtures/expected-session-v1.6.json`. Set `ASD_CARGO_TARGET_DIR` to keep cargo output outside the repo.
CI runs the same script from `.github/workflows/ios.yml`.

## 3. Capture Flow & Stimulus Batteries

1. **Onboarding** (`OnboardingFlowView`, `Sources/Views/Consent/`): caregiver intro → `ChildProfile`
   (age band 12–60 mo, session language, 30 s vs 60 s battery) → consent → permission primer.
2. **Stimulus battery** (`StimulusPlaylist`): age-matched clip list —
   **Standard 30 s** express battery (4 clips) or **Comprehensive 60 s** battery
   (8 clips). `OnDemandStimulusManager` resolves playback offline-first;
   `StimulusDownloadGateView` blocks the session until clips are cached and
   `StimulusVideoPlayerView` plays them gaze-synced.
3. **Procedural oculomotor stimuli** (`ProceduralOculomotorStimulusView` in
   `CurvAsdGazeKit`): code-drawn smooth-pursuit / saccade targets for
   calibration and oculomotor probes — no video files needed.
4. **3D face-mesh eye tracking**: every frame reads the TrueDepth
   `ARFaceAnchor` (face + per-eye 3D transforms, `lookAtPoint`, 52
   blendshapes) and `ScreenRaycaster` projects gaze onto screen AOIs at
   60 Hz. Zero raw video is ever stored — only numerical telemetry,
   AES-GCM-256 encrypted (`SessionVaultManager`).
5. **Sessions and delivery**: the App Store build lists each recorded session under the Sessions tab
   (`StoreSessionsView`) and delivers it to the clinic when the device is enrolled (`ClinicDeliveryGate`).
   Debug and DemoRelease additionally show the session summary, replay heatmaps, and the CareKit
   milestones/diaries.

## 4. Directory Layout

```text
apps/ios_asd_gaze_app/
├── README.md, project.yml              # This guide; XcodeGen spec (configurations above)
├── Resources/                          # Localizable.xcstrings, InfoPlist.xcstrings, PrivacyInfo.xcprivacy
├── maestro/flows/store/                # Maestro flows for the store screenshots
├── scripts/                            # run_store_screenshots.sh, run_maestro_screens.sh
├── fastlane/                           # metadata, screenshots, review_notes.txt, lanes
├── Sources/
│   ├── App/AsdGazeApp.swift            # SwiftUI entry point & root router (store vs full UI)
│   ├── Models/
│   │   ├── StoreBuild.swift, StoreScreenshotFixture.swift   # store-build switch; screenshot fixture
│   │   ├── CaptureEntryGate, AgeAssuranceGate, SubjectAgeCheckController, ClinicDeliveryGate
│   │   │                               # gates in front of capture and delivery
│   │   ├── Capture pipeline: ChildProfile, StimulusPlaylist, OnDemandStimulusManager,
│   │   │   CameraAccessCoordinator, GazeSessionRecord, GazeSessionHistoryStore,
│   │   │   SessionSyncClient, SyncTransport, DeviceRegistrationService, DataDeletionService
│   │   ├── Consent and privacy: ConsentDocument, ConsentRecordStore, FeatureOptIns, PermissionPrimer
│   │   ├── Care and clinical (Debug/DemoRelease): CarePlan*, CareKit*, MoodDiary*, FHIR*,
│   │   │   ResearchKit*, DoctorReportGenerator, ClinicalMilestoneCatalog
│   │   └── Locale and bench: FormatLocale, BenchLogWriter, BlendShapeCSVLogger
│   ├── UI/                             # AppLocalizer (Localizer.tr), LiquidGlassTheme, CaptureOrientationLock
│   └── Views/
│       ├── Consent/                    # ConsentFlowView, ConsentReviewView, PermissionPrimerView,
│       │                               # PrivacyDashboardView, BeforeYouBeginView
│       ├── Store build: StoreSessionsView, SettingsView, NotDiagnosticFooter, CameraDeniedView
│       ├── Capture: OnboardingFlowView, GazePositioningStepView, BubbleCalibrationView,
│       │   ARFaceCameraView, MainGazeScannerView, GazeStimulusScreeningView,
│       │   StimulusVideoPlayerView, StimulusDownloadGateView
│       └── Debug/DemoRelease only: CaregiverDashboardView, MoodDiaryView, MCHAT*View,
│           GazeSessionResultView, PediatricReportView, Engagement*Views, DemoToolsView, BenchCaptureView
└── Tests/                              # app tests per module (store build, consent, capture hygiene,
    │                                   # sync, localisation, CareKit, FHIR, report generation)
    ├── bdd/features/                   # Gherkin; store_*.feature mirror the Maestro store flows
    ├── Fixtures/, Support/
    └── StoreBuildTests, BuildConfigurationTests, ConsentFlowTests, CaptureHygieneTests, ...
```
