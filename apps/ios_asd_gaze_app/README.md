# iOS Pediatric Gaze Collector (`apps/ios_asd_gaze_app`)

> **Application**: Pediatric Social Attention & Oculomotor Gaze Collector  
> **Platform**: iOS / iPadOS 17.0+ (TrueDepth ARKit 6, Swift 6 Concurrency)  
> **Architecture**: Model-View-Controller (MVC) + Apple ResearchKit & CareKit + Liquid Glass Design  
> **Package Link**: [`domains/asd_screening/ios`](../../domains/asd_screening/ios) (`CurvAsdGazeKit`)  
> **Full Specifications**: [`domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md`](../../domains/asd_screening/docs/IOS_APP_DESIGN_AND_ROADMAP.md)  

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
./scripts/run_gaze_app_device.sh
```
Builds, signs, installs via `xcrun devicectl`, and launches directly on your paired physical device.

### Option C: Run in iPhone Simulator (CLI)
```bash
./scripts/run_gaze_app_sim.sh
```
Boots the first available iPhone simulator, builds, installs, and launches automatically.

---

## 3. Directory Layout

```text
apps/ios_asd_gaze_app/
├── README.md                           # This setup guide
├── project.yml                         # XcodeGen project specification
└── Sources/
    ├── App/
    │   └── AsdGazeApp.swift            # SwiftUI App entry point & root router
    ├── UI/
    │   └── LiquidGlassTheme.swift      # GlassCard, GlassInput & color tokens
    └── Views/
        ├── MainGazeScannerView.swift   # Palm-scanner style side-by-side live scanner
        ├── GazeSessionResultView.swift # Post-capture analysis & clinical evidence report
        ├── CaregiverDashboardView.swift# Longitudinal milestones & history
        ├── BubbleCalibrationView.swift # Interactive 5-point calibration target
        └── StimulusScreeningView.swift # Synchronized video player + real-time gaze HUD
```
