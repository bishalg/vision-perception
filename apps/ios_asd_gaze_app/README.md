# iOS Pediatric Gaze Collector (`apps/ios_asd_gaze_app`)

> **Application**: Pediatric Social Attention & Oculomotor Gaze Collector  
> **Platform**: iOS / iPadOS 18.0+ (TrueDepth ARKit face tracking; Swift 5.9 language mode with complete strict-concurrency checking)  
> **Architecture**: Model-View-Controller (MVC) + Apple ResearchKit & CareKit + Liquid Glass Design  
> **Package Link**: [`domains/asd_screening/ios`](../../domains/asd_screening/ios) (`CurvAsdGazeKit`)  
> **Full Architecture & Handover Guide**: [`docs/ASD_GAZE_TRACKER_ARCHITECTURE.md`](../../docs/ASD_GAZE_TRACKER_ARCHITECTURE.md)  
> **Roadmap & Specifications**: [`domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md`](../../domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md)  

---

## 1. Overview & Clinical Context

This application coordinates the pediatric eye-gaze data collection and social visual attention screening workflow:
1. **IRB Informed e-Consent & Survey**: Caregiver onboarding and validated M-CHAT-R/F screening.
2. **Interactive 5-Point Bubble Calibration**: Engaging, fast optical axis calibration using floating popped bubbles.
3. **Synchronized Stimuli Playback**: GeoPref split-screen and naturalistic face videos synchronized with 60 Hz TrueDepth telemetry.
4. **CareKit Longitudinal Milestones**: Milestone tracking at 18, 24, and 36 months with continuous trend visualizations.
5. **Zero Raw Video Recording**: Camera frames are processed strictly in volatile memory. Only numerical telemetry is encrypted at rest using Apple CryptoKit (`AES-GCM-256`).

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

One command runs every gate (kit tests, kit ThreadSanitizer, strict-concurrency build, XcodeGen,
app tests on a simulator, unsigned Release build, claims/secrets, privacy-API and Release-symbol checks):

```bash
scripts/ios_test.sh --dry-run          # print the commands only
DERIVED_DATA=/private/tmp/asd-app-dd scripts/ios_test.sh
SIM_NAME="iPhone 16" scripts/ios_test.sh   # if the default simulator is not installed
```

Build output always lives outside the repository (`DERIVED_DATA`, default `/private/tmp/asd-app-dd`).
CI runs the same script from `.github/workflows/ios.yml`.

## 3. Screening Flow & Stimulus Batteries

1. **Onboarding** (`OnboardingFlowView`): caregiver intro → `ChildProfile`
   (age band 12–60 mo, session language, 30 s vs 60 s battery) → consent.
2. **Stimulus battery** (`StimulusPlaylist`): age-matched clinical clip list —
   **Standard 30 s** express battery (4 clips) or **Comprehensive 60 s** battery
   (8 clips). `OnDemandStimulusManager` resolves playback offline-first;
   `StimulusDownloadGateView` blocks the session until clips are cached and
   `StimulusVideoPlayerView` plays them gaze-synced.
3. **Procedural oculomotor stimuli** (`ProceduralOculomotorStimulusView` in
   `CurvAsdGazeKit`): code-drawn smooth-pursuit / saccade targets for
   calibration and oculomotor probes — no video files needed.
4. **3D face-mesh eye tracking**: every frame reads the TrueDepth
   `ARFaceAnchor` (face + per-eye 3D transforms, `lookAtPoint`, 52 FACS
   blendshapes) and `ScreenRaycaster` projects gaze onto screen AOIs at
   60 Hz. Zero raw video is ever stored — only numerical telemetry,
   AES-GCM-256 encrypted (`SessionVaultManager`).
5. **Results & care loop**: biomarker detail views, AI risk-stratification
   matrix, replay heatmaps, doctor PDF report (`DoctorReportGenerator`),
   CareKit milestones/diaries, ResearchKit study sync.

## 4. Directory Layout

```text
apps/ios_asd_gaze_app/
├── README.md                           # This setup guide
├── project.yml                         # XcodeGen project specification
├── Sources/
│   ├── App/
│   │   └── AsdGazeApp.swift            # SwiftUI App entry point & root router
│   ├── Models/
│   │   ├── ChildProfile.swift          # Age band, language, 30/60 s battery
│   │   ├── StimulusPlaylist.swift      # Standard-30 s / Comprehensive-60 s batteries
│   │   ├── OnDemandStimulusManager.swift # Offline-first stimulus resolution
│   │   ├── ResearchKitStudyModels/Store/SyncService.swift
│   │   ├── CarePlanModels.swift        # CareKit tasks, diary, milestones
│   │   ├── CareKitClinicalStore.swift
│   │   ├── DoctorReportGenerator.swift # PDF clinical report
│   │   └── GazeSessionRecord.swift
│   ├── UI/
│   │   ├── AppLocalizer.swift
│   │   └── LiquidGlassTheme.swift      # GlassCard, GlassInput & color tokens
│   └── Views/
│       ├── SplashScreenView.swift
│       ├── OnboardingFlowView.swift
│       ├── MCHATSurveyModalView.swift
│       ├── ResearchStudyConsentView.swift
│       ├── GazePositioningStepView.swift
│       ├── BubbleCalibrationView.swift # Interactive 5-point calibration target
│       ├── MainGazeScannerView.swift
│       ├── ARFaceCameraView.swift      # TrueDepth face-mesh camera feed
│       ├── StimulusScreeningView.swift # Legacy screening container
│       ├── GazeStimulusScreeningView.swift # Playlist-driven screening session
│       ├── StimulusVideoPlayerView.swift
│       ├── StimulusDownloadGateView.swift
│       ├── GazeScreeningWizardView.swift
│       ├── GazeSessionResultView.swift
│       ├── GazeReplayHeatmapFullScreenView.swift
│       ├── BiomarkerDetailViews.swift
│       ├── BiomarkerAffectAndQualityViews.swift
│       ├── BiomarkerRiskStratificationView.swift
│       ├── CaregiverDashboardView.swift
│       └── CareKitDashboardModals.swift
└── Tests/                              # 55 tests: playlist integrity, CareKit,
                                        # ResearchKit sync, report generation
```
