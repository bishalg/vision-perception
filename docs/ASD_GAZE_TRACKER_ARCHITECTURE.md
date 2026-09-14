# ASD Pediatric Gaze Screening & Oculomotor Biomarker System
## High-Level Architecture, Working Principles, Modes & Data Flow Handover

> **Target Application**: `apps/ios_asd_gaze_app` (Host iOS App)  
> **Core Sensory Package**: `domains/asd_screening/ios` (`CurvAsdGazeKit`)  
> **Clinical C++ Engine**: `domains/asd_screening` (`AsdBiomarkerPack`)  
> **Target Population**: Toddlers aged 18–48 months & Caregivers  
> **Design Aesthetic**: Liquid Glass (iOS Native, Apple ResearchKit + CareKit)  

---

## 1. Executive Summary & Working Principle

The **ASD Pediatric Gaze Screening App** is an objective digital biomarker collection and social-visual attention screening tool. It quantifies early developmental markers of Autism Spectrum Disorder (ASD)—specifically **preferential visual attention (GeoPref paradigm)**, **oculomotor saccade latencies**, and **concomitant facial affect reactivity**.

### Core Technical Working Principle

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TrueDepth Sensor Array (60 Hz)                       │
│  - Infrared Dot Projector (30,000 dots)                                 │
│  - Flood Illuminator (ambient light invariant)                          │
│  - Bionic Front Camera (AVFoundation / ARKit 6)                        │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ 60 FPS Face Anchor
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│              ScreenRaycaster (CurvAsdGazeKit / Swift 6)                 │
│  - Projects 3D optical axes (lookAtPoint, leftEye, rightEye)            │
│    onto physical device screen plane (Z = 0)                            │
│  - Converts physical millimeters to normalized viewport (0.0 to 1.0)    │
│  - Applies 1-Euro Adaptive Low-Pass Filter (eliminates jitter)          │
│  - Applies Affine Calibration Correction (from 5-point bubble target)  │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ Filtered Screen Gaze (x, y)
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│              Stimulus Synchronizer & Dynamic AOI Evaluator              │
│  - Sub-16.6ms frame synchronization (CADisplayLink / AVPlayer)          │
│  - GeoPref Battery: Social Cartoon vs Dynamic Geometric Patterns        │
│  - Real-time Area-of-Interest (AOI) Bounding Box Hit Testing            │
│  - Facial Affect Extraction (Smile Intensity, Flat Affect, Surprise)   │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ High-Frequency Telemetry Stream
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│               Privacy-by-Design & Security Engine                       │
│  ★ ZERO RAW VIDEO RECORDING: Camera frames exist only in volatile RAM    │
│  - Purely numerical time-series encrypted at rest (CryptoKit AES-GCM)   │
│  - Fully de-identified tokens (random UUIDv4; no child PII stored)      │
│  - Generates 16 Clinical Biomarkers & Caregiver Report Summary          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Privacy-by-Design (Critical Medical Principle)

1. **Zero Raw Video or Facial Image Recording**:
   - The device's front camera is processed **strictly in-memory** by ARKit and CoreVision.
   - **No video files (`.mp4`, `.mov`) or raw pixel buffers (`CVPixelBuffer`) are ever written to disk storage or transmitted over the network.**
2. **Strictly Numerical Telemetry**:
   - Only mathematical time-series data is recorded:
     $$\{ t_i, x_i, y_i, \text{yaw}_i, \text{pitch}_i, \text{distance}_i, \text{AOIs}_i, \text{smile}_i \}$$
3. **Hardware-Bound Encryption at Rest**:
   - Stored session telemetry is encrypted with Apple **CryptoKit (`AES-GCM-256`)** using keys backed by the device's Secure Enclave.
4. **COPPA / HIPAA De-identification**:
   - Child names or birthdays are never bundled with raw gaze samples. Participants are referenced solely by randomized UUID hashes.

---

## 3. Supported Modes & Views

### A. Viewfinder Display Modes (`ScannerDisplayMode`)

The primary scanning interface (`MainGazeScannerView`) supports 3 responsive display layouts toggled via the top layout menu:

#### 1. Side-by-Side (Split View) — *Recommended for Clinical Observation*
Split 58% / 40% view allowing clinician/caregiver to observe the child's live face alignment alongside the synchronized stimuli.

```text
┌─────────────────────────────────────────────────────────────────────────┐
│  ● Gaze Active   [ Calibrated ✓ ]   [ Focus: Social ]   ⏱ 01:24   [Menu]│
├─────────────────────────────────────┬───────────────────────────────────┤
│ STIMULUS PLAYER (58% Width)         │ TRUE-DEPTH FACE HUD (40% Width)   │
│ ┌─────────────────┬───────────────┐ │ ┌───────────────────────────────┐ │
│ │  SOCIAL SCENE   │ DYNAMIC GEO   │ │ │  [Video Feed / Depth Mesh]    │ │
│ │                 │               │ │ │                               │ │
│ │       ☼         │      ◆        │ │ │             (• •)             │ │
│ │     Cartoon     │   Pulsing     │ │ │               ⌣               │ │
│ │     Actor       │  Kaleidoscope │ │ │      Crosshair: (x, y)        │ │
│ │                 │               │ │ └───────────────────────────────┘ │
│ └─────────────────┴───────────────┘ │ Distance: 48 cm   Quality: OPTIMAL│
├─────────────────────────────────────┴───────────────────────────────────┤
│ [ Pause / Record ]               [ Calibrate ]              [ Complete ]│
└─────────────────────────────────────────────────────────────────────────┘
```

#### 2. Picture-in-Picture (Floating PiP) — *Maximizes Stimulus Engagement*
Full-screen stimulus video with a small floating 140×190pt face alignment HUD pinned to the bottom-right corner.

```text
┌─────────────────────────────────────────────────────────────────────────┐
│ FULLSCREEN CLINICAL STIMULUS (Social vs Dynamic Geometric)              │
│                                                                         │
│                                              ┌────────────────────────┐ │
│                                              │ FLOATING TRUE-DEPTH HUD│ │
│                                              │      (• •)  ⌣          │ │
│                                              │ Dist: 48cm  Gaze: OK   │ │
│                                              └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

#### 3. Full Screen (Stimulus Only) — *Distraction-Free Child Immersion*
Hides the camera HUD entirely so toddlers are not distracted by seeing their own face. Telemetry collection continues at 60 Hz in the background.

---

### B. Hardware vs Simulation Modes

1. **TrueDepth Hardware Mode** (Default on physical devices with TrueDepth):
   - Uses `ARKit.ARFaceTrackingConfiguration` at 60 Hz.
   - Extracts 3D ocular transforms, head pose, eye openness, and facial action blendshapes.
2. **Deterministic Mathematical Simulation Mode**:
   - For Xcode Simulator or devices without TrueDepth hardware.
   - Synthesizes physiological saccades (200–400ms fixations, 30ms saccadic jumps, drift noise, and blink cycles) to allow 100% testable end-to-end verification.

---

## 4. End-to-End Application Flow

```text
               ┌──────────────────────────────────────────┐
               │ 1. Caregiver Dashboard (Milestones View) │
               │   - Review prior developmental trends    │
               │   - Check scheduled 18/24/36m milestones │
               └────────────────────┬─────────────────────┘
                                    │ Tap "Start Screening"
                                    ▼
               ┌──────────────────────────────────────────┐
               │ 2. 5-Point Bubble Calibration View       │
               │   - Playful animated popping bubbles     │
               │   - Locks 300ms fixations at 5 anchors   │
               │   - Computes per-child affine correction │
               └────────────────────┬─────────────────────┘
                                    │ Calibration Verified (<1.5° error)
                                    ▼
               ┌──────────────────────────────────────────┐
               │ 3. Main Gaze Scanner (Stimulus Session)  │
               │   - 3-Stage Tracking (Search -> Face ->  │
               │     Gaze Active)                         │
               │   - GeoPref & Naturalistic Stimulus Loop │
               │   - Live Quality Gate (Distance 40-55cm, │
               │     Yaw < 25°, Pitch < 20°)              │
               └────────────────────┬─────────────────────┘
                                    │ Session Completes (60–120s)
                                    ▼
               ┌──────────────────────────────────────────┐
               │ 4. Gaze Session Result & Biomarkers      │
               │   - Social Preference Index (e.g. 74%)   │
               │   - Eye vs Mouth Dwell Ratios            │
               │   - First Saccade Latency (ms)           │
               │   - Affect Reactivity & Quality Verdict  │
               └────────────────────┬─────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
      ┌───────────────────────────┐   ┌───────────────────────────┐
      │ Pediatrician PDF Export   │   │ CareKit Longitudinal Store│
      │ Single-page clinical card │   │ Encrypted local database  │
      │ for well-child pediatric  │   │ with FHIR milestone sync  │
      │ checkups.                 │   │ capabilities.             │
      └───────────────────────────┘   └───────────────────────────┘
```

---

## 5. What Data is Collected?

### A. Frame-by-Frame Telemetry (`GazeFrame` at 60 Hz)

| Telemetry Field | Type | Unit / Range | Description |
| :--- | :--- | :--- | :--- |
| `timestamp` | `Double` | Unix Seconds | Frame presentation time linked with sub-16.6ms display link. |
| `gazeX`, `gazeY` | `Double` | $[0.0, 1.0]$ | Calibrated gaze coordinate normalized to screen width/height. |
| `rawGazeX`, `rawGazeY`| `Double` | $[0.0, 1.0]$ | Uncalibrated 1-Euro filtered screen intersection point. |
| `distanceCm` | `Double` | cm (Typ: 40–55) | Metric distance between TrueDepth sensor and infant face. |
| `headYawDeg` | `Double` | Degrees ($\pm 25^\circ$) | Left/Right head turn angle. |
| `headPitchDeg` | `Double` | Degrees ($\pm 20^\circ$) | Up/Down head tilt angle. |
| `headRollDeg` | `Double` | Degrees ($\pm 15^\circ$) | Head tilt toward shoulder. |
| `leftEyeOpenness` | `Double` | $[0.0, 1.0]$ | Eyelid openness ratio ($0.0 = \text{blink}$). |
| `rightEyeOpenness`| `Double` | $[0.0, 1.0]$ | Eyelid openness ratio. |
| `activeAOIs` | `[String]`| Identifiers | Active Areas-of-Interest fixated (e.g. `Social_Actor`, `Dynamic_Geo`). |
| `facialAffect` | Struct | Metrics | Smile intensity, flat affect score, surprise index. |
| `qualityValid` | `Bool` | Boolean | True if head pose, distance, and eye visibility pass Quality Gate. |

---

### B. Session Clinical Biomarkers (`GazeSessionReportSummary`)

| Biomarker | Clinical Meaning & ASD Relevance |
| :--- | :--- |
| **`socialPreferenceScore`** | Percentage of valid fixation time spent on social scene vs geometric patterns ($>65\%$ is typical; $<50\%$ warrants pediatric evaluation). |
| **`socialDwellSec`** | Cumulative seconds spent fixating social actors. |
| **`geometricDwellSec`** | Cumulative seconds spent fixating high-contrast geometric animations. |
| **`eyeDwellRatio`** | Ratio of gaze fixated on the actor's eye region (decreased eye fixations are classic early ASD biomarkers). |
| **`mouthDwellRatio`** | Ratio of gaze fixated on the actor's mouth region. |
| **`firstEyeLatencyMs`** | Saccadic latency (ms) to make the initial shift from screen center to the actor's eyes upon face onset. |
| **`socialSmileIntensityMean`** | Expressive smiling response intensity measured during key humorous stimulus events. |
| **`smileReactivityIndex`** | Temporal contingency between child smile onset and social character smiling (shared positive affect). |
| **`flatAffectScore`** | Proportion of session characterized by prolonged facial affective hypomimia. |
| **`qualityVerdict`** | Session validity rating: `.optimal`, `.acceptable`, or `.rejected` (e.g. child was too far or turned away). |

---

## 6. Mathematical Foundations

### 1. 3D-to-2D Physical Screen Raycasting (`ScreenRaycaster`)

The front-facing TrueDepth camera origin $(0, 0, 0)$ is situated above the physical OLED display plane. For an iPhone/iPad:
$$\mathbf{P}_{\text{screen}} = \mathbf{P}_{\text{eye}} + t \cdot \hat{\mathbf{v}}_{\text{gaze}}$$
The screen lies at $Z = 0$ in device coordinates. The raycast intersection solves for parameter $t$:
$$t = \frac{- P_{\text{eye}, z}}{\hat{v}_{\text{gaze}, z}}$$
Physical coordinates $(X_{\text{mm}}, Y_{\text{mm}})$ are converted into points via device PPI ($\approx 460\text{ ppi}$ on modern Super Retina OLED) and clamped to the viewport boundaries.

### 2. 1-Euro Adaptive Low-Pass Filter

To resolve the trade-off between **steady fixation precision** and **low-latency saccadic responsiveness**, two adaptive 1-Euro filters are maintained:
$$f_c = f_{c,\min} + \beta \cdot |\dot{x}|$$
$$\alpha = \frac{1}{1 + \frac{\tau}{T}}, \quad \tau = \frac{1}{2 \pi f_c}$$
- At low gaze velocity ($|\dot{x}| \approx 0$), cutoff frequency $f_c \to 1.2\text{ Hz}$, eliminating micro-tremor and sensor noise.
- At high velocity during a saccade, $f_c$ scales up dynamically, eliminating perceived latency.

### 3. Affine Calibration Correction (`BubbleCalibrationView`)

To rectify infant anatomical variations (angle $\kappa$ between pupillary optical axis and foveal visual axis):
$$\begin{bmatrix} x_{\text{cal}} \\ y_{\text{cal}} \end{bmatrix} = \begin{bmatrix} a_{11} & a_{12} \\ a_{21} & a_{22} \end{bmatrix} \begin{bmatrix} x_{\text{raw}} \\ y_{\text{raw}} \end{bmatrix} + \begin{bmatrix} t_x \\ t_y \end{bmatrix}$$
Fitted via least-squares across the 5 bubble target anchors.

---

## 7. App Tab Architecture & Directory Layout

```text
Sources/
├── App/
│   └── AsdGazeApp.swift                # App entry, TabView (Dashboard, Scanner, Calibrate, Settings)
├── UI/
│   └── LiquidGlassTheme.swift          # Liquid Glass visual tokens (.ultraThinMaterial, GlassCard)
├── Models/
│   ├── CarePlanModels.swift            # CareKit milestone tasks & medication store
│   ├── CareKitClinicalStore.swift      # Local CareKit clinical database
│   ├── ResearchKitStudyModels.swift    # Research study & consent schemas
│   └── ResearchKitSyncService.swift    # Offline-first secure synchronization queue
└── Views/
    ├── MainGazeScannerView.swift       # Live split-screen scanner (Side-by-Side, PiP, Full Screen)
    ├── GazeSessionResultView.swift     # Biomarker summary report & clinical graphs
    ├── CaregiverDashboardView.swift    # Longitudinal developmental milestones & trends
    ├── BubbleCalibrationView.swift     # Interactive 5-point popping bubble calibration
    ├── MCHATSurveyModalView.swift      # Validated 20-question M-CHAT-R/F survey
    ├── ResearchStudyConsentView.swift  # IRB informed e-Consent workflow
    └── SettingsView.swift              # Quality gate thresholds & simulation toggles
```

---

---

## 8. Core Architecture Pillars: Concurrency, Volatile Batching & CoreML ANE

### A. Swift 6 Concurrency & Actor Isolation (`GazeProcessingActor`)
- **Challenge**: At 60 Hz, ARKit delivers `ARFaceAnchor` updates via `ARSessionDelegate` while the stimulus video (`AVPlayer`) renders concurrently. Heavy math (3D screen raycasting, 1-Euro adaptive low-pass filtering, affine matrix transformations, and AOI bounding box queries) would cause the Main Thread to drop frames if executed synchronously.
- **Solution**:
  - All heavy geometric raycasting, filtering, and telemetry sample construction are isolated in `GazeProcessingActor` and dedicated high-priority queue (`arDelegateQueue`, `.userInteractive`).
  - Only consolidated, immutable `GazeUIState` snapshots are dispatched to `@MainActor` at 60 Hz to update the TrueDepth reticle and HUD badges.
  - The Main Thread maintains zero contention, guaranteeing zero dropped stimulus frames during clinical observation.

### B. High-Frequency Disk Writes & CryptoKit: Volatile RAM Batching
- **Challenge**: A 2-minute clinical screening session produces 7,200 telemetry points. Attempting to encrypt and write each sample to disk at 60 Hz causes thermal throttling, I/O stutter, and UI thread freezing.
- **Solution**:
  - Telemetry is accumulated strictly in an in-memory volatile RAM buffer (`[GazeFrame]`) during the active session (< 2 MB footprint).
  - Absolutely **zero disk writes** or cryptographic seals occur during active tracking.
  - Upon observation completion, `SessionVaultManager.saveSessionBatchAsync` executes a single atomic batch write: encrypting the 7,200 frames with Apple CryptoKit (`AES-GCM-256`) in the background off the main thread.

### C. CoreML Biomarker Classification Engine on Apple Neural Engine (ANE)
- **Challenge**: Raw statistical dwell times must be translated into actionable clinical risk stratification for pediatricians and ResearchKit/CareKit records.
- **Solution**:
  - `CurvAsdGazeKit/Inference/BiomarkerClassificationEngine.swift` synthesizes multi-modal biomarkers (`socialPreferenceScore`, `eyeDwellRatio`, `firstEyeLatencyMs`, `flatAffectScore`, `smileReactivityIndex`).
  - Targets CoreML with `MLModelConfiguration.computeUnits = .all`, prioritizing the Apple Neural Engine (ANE) for sub-2ms on-device inference.
  - Incorporates a deterministic clinical baseline fallback (calibrated against Pierce et al. GeoPref & Jones & Klin saccade latency literature) for test environments and uncompiled deployments.
  - Produces structured `BiomarkerClassificationResult` (`.typical`, `.monitor`, `.elevatedRisk`) embedded in ResearchKit/CareKit schemas and rendered in `GazeSessionResultView`.

---

## 9. Summary for Future Engineers & Researchers

- **To run on physical hardware**: Open `AsdGazeCollector.xcodeproj`, choose your paired iPhone (e.g. iPhone 17 Pro), and run.
- **To test on simulator**: Switch to **Simulation Mode** in the ellipsis menu `(...)` or Settings; it synthesizes physiologically realistic gaze samples and saccades.
- **To add new stimuli**: Update `StimulusAOITracker.swift` with the time-coded bounding boxes of new video segments.
- **To export data**: Use the Share button on `GazeSessionResultView` for encrypted JSON telemetry or pediatrician PDF reports.

---

## 9. Clinical Literature & Empirical Foundation

The algorithms, biomarkers, and UX in this application are directly derived from four landmark clinical studies documented in [`docs/research/asd_gaze_biomarkers/`](research/asd_gaze_biomarkers/README.md):
1. **EarliPoint System (JAMA 2023)**: [`docs/research/asd_gaze_biomarkers/01_jama_2023_earlipoint_social_visual_engagement.md`](research/asd_gaze_biomarkers/01_jama_2023_earlipoint_social_visual_engagement.md) — FDA De Novo Class II authorization, Social Visual Engagement (SVE) divergence index, normative toddler distribution.
2. **SenseToKnow (NEJM AI 2024)**: [`docs/research/asd_gaze_biomarkers/02_nejm_ai_2024_sensetoknow_remote_mobile_screening.md`](research/asd_gaze_biomarkers/02_nejm_ai_2024_sensetoknow_remote_mobile_screening.md) — Remote mobile app on iPhone/iPad (AUC 0.92, Sens 83.0%, Spec 93.3%); multimodal gaze, head pose, affect, and bubble-popping motor response.
3. **GeoPref & DMN-Visual Link (eLife 2019)**: [`docs/research/asd_gaze_biomarkers/03_elife_2019_geopref_default_mode_visual_hypoconnectivity.md`](research/asd_gaze_biomarkers/03_elife_2019_geopref_default_mode_visual_hypoconnectivity.md) — 69% Geometric preference subtype and functional hypoconnectivity between Default Mode Network and visual cortex.
4. **Machine Learning Meta-Analysis (JBI 2022)**: [`docs/research/asd_gaze_biomarkers/04_jbi_2022_machine_learning_eyetracking_systematic_review.md`](research/asd_gaze_biomarkers/04_jbi_2022_machine_learning_eyetracking_systematic_review.md) — 24 pooled studies ($N = 1,396$), algorithmic comparison, and feature engineering standards.
5. **Engineering & Translation Blueprint**: [`docs/research/asd_gaze_biomarkers/05_app_design_and_clinical_translation_guide.md`](research/asd_gaze_biomarkers/05_app_design_and_clinical_translation_guide.md) — Complete clinical-to-code mapping for our 16 digital biomarkers and Doctor Report PDF generation.

