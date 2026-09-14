# Clinical-to-Code Engineering & App Design Blueprint

> **Document ID**: `DOC-RES-ASD-005`  
> **Target Application**: `apps/ios_asd_gaze_app` (Host iOS Application)  
> **Sensory Engine**: `domains/asd_screening/ios` (`CurvAsdGazeKit`)  
> **Clinical C++ Pack**: `domains/asd_screening` (`AsdBiomarkerPack`)  
> **Synthesized Research Base**: JAMA (2023), NEJM AI (2024), eLife (2019), JBI (2022)

---

## 1. Executive Synthesis & Architectural Vision

The four landmark clinical publications form a unified theoretical and empirical foundation for developing our native iOS Autism Spectrum Disorder (ASD) digital screening application:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        Four-Pillar Scientific Foundation                               │
├────────────────────────────┬────────────────────────────┬──────────────────────────────┤
│ Scientific Pillar          │ Landmark Paper             │ Direct Application to Code   │
├────────────────────────────┼────────────────────────────┼──────────────────────────────┤
│ 1. Transactional Gaze      │ Jones et al. (JAMA 2023)   │ Sub-16.6ms gaze sampling;    │
│    Biomarker & FDA Path    │ EarliPoint FDA Clearance   │ SVE normative divergence     │
│                            │                            │ index in DoctorReportGen     │
├────────────────────────────┼────────────────────────────┼──────────────────────────────┤
│ 2. Unconstrained Mobile    │ Babu et al. (NEJM AI 2024) │ Front-camera TrueDepth only; │
│    Multi-modal Phenotyping │ SenseToKnow (AUC 0.92)     │ Fuses Gaze + Affect + Pose   │
│                            │                            │ Bubble-pop recalibration     │
├────────────────────────────┼────────────────────────────┼──────────────────────────────┤
│ 3. GeoPref & Macroscale    │ Lombardo et al.(eLife 2019)│ Split-screen GeoPref test;   │
│    Neurobiological Anchor  │ GeoPref & DMN-Visual Link  │ 69% Geometric Fixation Rule; │
│                            │                            │ High-specificity subtype flag│
├────────────────────────────┼────────────────────────────┼──────────────────────────────┤
│ 4. Algorithmic Rigor &     │ Wei et al. (JBI 2022)      │ Age-bracketed toddler norms; │
│    Defensive ML Validation │ Meta-Analysis (24 studies) │ Zero-leakage CV; lightweight │
│                            │                            │ CoreML decision trees        │
└────────────────────────────┴────────────────────────────┴──────────────────────────────┘
```

---

## 2. Sensory Pipeline & Codebase Mapping

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               `apps/ios_asd_gaze_app` Telemetry & Inference Pipeline                   │
│                                                                                        │
│   [ AVFoundation / ARKit 6 TrueDepth Camera (60 FPS) ]                                 │
│   • Infrared Dot Projector (ambient light invariant)                                   │
│   • TrueDepth Face Anchor: lookAtPoint, leftEye, rightEye, 3D Head Pose                │
│                                      │                                                 │
│                                      ▼                                                 │
│   [ ScreenRaycaster.swift (`CurvAsdGazeKit`) ]                                         │
│   • Projects 3D optical ray onto physical screen plane (Z = 0)                         │
│   • Applies 1-Euro Adaptive Low-Pass Filter (eliminates micro-tremor)                  │
│   • Affine calibration matrix applied from 5-point bubble target                       │
│   • Normalizes screen gaze coordinates to [0.0, 1.0] viewport space                    │
│                                      │                                                 │
│                                      ▼                                                 │
│   [ AOIEvaluator.swift & AffectExtractor.swift ]                                      │
│   • Real-time Area-of-Interest (AOI) bounding box hit testing (Social vs Geo)          │
│   • ARFaceAnchor blendshape telemetry: Smile intensity, surprise, flat affect          │
│   • Distance gating: Rejects frames if distance < 35 cm or > 65 cm                     │
│                                      │                                                 │
│                                      ▼                                                 │
│   [ AsdBiomarkerPack (C++20 Clinical Engine) ]                                         │
│   • Computes 16 Core Digital Biomarkers:                                               │
│     - GeoPref Ratio & Fixation Dwell Time (eLife 2019 / Pierce)                        │
│     - Social Visual Engagement (SVE) Divergence Index (JAMA 2023 / Jones)              │
│     - Saccade Latency & Fixation Count (JBI 2022 / Wei)                                │
│     - Facial Expressivity & Affect Mimicry Score (NEJM AI 2024 / Sapiro)               │
│                                      │                                                 │
│                                      ▼                                                 │
│   [ GazeSessionRecord.swift & CareKitClinicalStore.swift ]                             │
│   • Privacy Engine: ZERO raw video recording. Volatile RAM processing only.           │
│   • Hardware-bound encryption at rest via CryptoKit (AES-GCM-256).                     │
│   • UUIDv4 de-identified participant tokens (HIPAA / COPPA compliant).                 │
│                                      │                                                 │
│                                      ▼                                                 │
│   [ DoctorReportGenerator.swift ]                                                      │
│   • Multi-page clinical PDF report for pediatricians & developmental specialists       │
│   • Dimensional percentiles for Social Disability, Verbal, and Nonverbal ability       │
│   • Visual radar chart & scanpath density heatmap                                      │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. UI/UX & Mobile Ergonomics for Toddlers & Caregivers

### 3.1 Onboarding & Environmental Verification (Caregiver UX)
Drawing from the remote administration lessons in SenseToKnow (NEJM AI 2024):
1. **Device Placement Guidance**:
   - Audio-visual guide instructing the caregiver to seat the toddler comfortably on their lap or high chair.
   - Live distance feedback gauge: Ideal viewing distance is **$45\text{–}55\text{ cm}$**. The app inhibits test start if the child's face is outside this envelope.
2. **Ambient Lighting Check**:
   - While TrueDepth IR sensors work in darkness, child attention requires well-diffused room lighting without harsh backlighting (e.g., windows behind the child).

### 3.2 Dynamic Viewfinder Layouts (`ScannerDisplayMode`)
Following clinical observation requirements (EarliPoint / JAMA 2023):
- **Side-by-Side Split View ($58\% / 40\%$)**:
  - Stimulus player on the left, live face alignment HUD on the right. Allows the clinician or caregiver to observe the child's face alignment and eye tracking crosshair in real time.
- **Picture-in-Picture (Floating PiP)**:
  - Full-screen stimulus with a small floating ($140 \times 190\text{ pt}$) face alignment window in the lower corner. Maximizes visual immersion for the child.

### 3.3 The Gamified Bubble Calibration & Recentering Loop
- Rather than clinical calibration dots (which toddlers ignore), implement **interactive floating bubble popping**:
  - A colorful bubble appears at 5 key screen locations (center, top-left, top-right, bottom-left, bottom-right).
  - Toddler reaches out and touches the bubble to pop it (triggering sound and particle burst).
  - During the fixate-and-touch event, the app captures ground-truth gaze coordinates to solve the affine calibration matrix.
  - Re-anchors attention between video blocks, eliminating calibration drift identified in the JBI 2022 meta-analysis.

---

## 4. The 16 Clinical Biomarkers Specification

| Index | Biomarker Key | Scientific Source | Formulation & Units | Clinical Interpretation |
| :--- | :--- | :--- | :--- | :--- |
| **BM-01** | `geo_fixation_percentage` | eLife 2019 / Pierce | $T_{\text{geo}} / (T_{\text{geo}} + T_{\text{social}}) \times 100\ [\%]$ | Extreme geometric preference ($\ge 69\%$) signifies high-risk ASD subtype ($98\%$ specificity). |
| **BM-02** | `social_fixation_percentage`| eLife 2019 / Jones | $T_{\text{social}} / T_{\text{valid}}\ [\%]$ | Normative toddlers spend $>70\%$ attending to social scenes. |
| **BM-03** | `social_visual_engagement` | JAMA 2023 / Jones | $\|\mathbf{gaze}(t) - \boldsymbol{\mu}_{\text{typical}}(t)\|_{\boldsymbol{\Sigma}^{-1}}$ | Accrual of divergence from shared social attention; predicts ADOS-2 scores ($R = -0.75$). |
| **BM-04** | `time_to_first_fixation_face`| NEJM AI 2024 / Babu | Milliseconds $[0\text{–}5000\text{ ms}]$ | Latency before first fixating on human face; delayed in ASD toddlers. |
| **BM-05** | `eye_vs_mouth_ratio` | JBI 2022 / Wei | $T_{\text{eyes}} / T_{\text{mouth}}\ [\text{ratio}]$ | Atypical fixation shift away from eyes toward mouth or hairline. |
| **BM-06** | `saccade_mean_latency` | JBI 2022 / Wei | Milliseconds $[\text{ms}]$ | Saccadic reaction time between competing visual stimuli. |
| **BM-07** | `saccade_peak_velocity` | JBI 2022 / Wei | Degrees per second $[^{\circ}/\text{s}]$ | Oculomotor ballistic speed; altered cerebellar-brainstem oculomotor tuning. |
| **BM-08** | `fixation_count_total` | JBI 2022 / Wei | Count per minute $[n/\text{min}]$ | Visual scanning strategy density. |
| **BM-09** | `mean_fixation_duration` | JBI 2022 / Wei | Milliseconds $[\text{ms}]$ | Prolonged "sticky" fixation vs hyper-fragmented scanning. |
| **BM-10** | `smile_mimicry_rate` | NEJM AI 2024 / Babu | Percentage of scenes $[0\text{–}100\%]$ | Frequency of spontaneous smiling when on-screen actor smiles. |
| **BM-11** | `flat_affect_duration` | NEJM AI 2024 / Babu | Seconds $[0\text{–}60\text{ s}]$ | Total duration of unexpressive/neutral face during humorous vignettes. |
| **BM-12** | `head_pose_entropy` | NEJM AI 2024 / Babu | Multiscale Sample Entropy | Postural instability and movement complexity. |
| **BM-13** | `response_to_name_latency` | NEJM AI 2024 / Miller | Milliseconds $[0\text{–}8000\text{ ms}]$ | Time required for head turn when child's name is voiced by the app. |
| **BM-14** | `touch_reaction_time` | NEJM AI 2024 / Sapiro | Milliseconds $[\text{ms}]$ | Motor latency in popping target bubbles. |
| **BM-15** | `valid_sample_ratio` | JAMA 2023 / Jones | $N_{\text{valid\_frames}} / N_{\text{total\_frames}}\ [\%]$ | Quality gate indicator; requires $\ge 75\%$ for valid clinical diagnostic assessment. |
| **BM-16** | `overall_composite_risk_score`| NEJM AI / JBI 2022 | Continuous score $[0.0\text{–}1.0]$ | Machine learning ensemble prediction (XGBoost/SVM); flags Low/Moderate/High risk. |

---

## 5. Doctor Report Generator & Clinical Export Engine

The `DoctorReportGenerator.swift` module formats the extracted session telemetry into an authoritative, multi-page clinical document suitable for inclusion in electronic health records (EHR):

1. **Header & De-Identification**:
   - Session Hash (UUIDv4), Age in Months, Device Type, Lens Calibration status.
   - Strict notice: *Screening Aid Only — Not a Standalone Diagnostic Tool*.
2. **Executive Summary & Radar Chart**:
   - Overall Risk Category: **Low Risk**, **Moderate / Monitor**, or **High Risk / Specialist Referral Advised**.
   - 4-Axis Clinical Radar Chart:
     - **Social Visual Engagement (SVE)** percentile.
     - **GeoPref Subtype Index** (with explicit $\ge 69\%$ alert if triggered).
     - **Facial Affect & Mimicry** percentile.
     - **Oculomotor Saccade Dynamics** percentile.
3. **Biological & Empirical Reference Section**:
   - Embeds citations to JAMA 2023, NEJM AI 2024, and eLife 2019 so that reviewing pediatricians and developmental psychologists understand the neurobiological meaning behind the scores.
4. **CareKit Integration**:
   - Automatically stores completed screenings as clinical assessment tasks in `CareKitClinicalStore.swift`, supporting longitudinal tracking across well-child pediatric checkups (18, 24, 30, and 36 months).

---

## 6. Regulatory & Privacy Architecture (HIPAA / COPPA / FDA SaMD)

1. **Zero Raw Video Recording Principle**:
   - Camera pixel buffers (`CVPixelBuffer`) are processed frame-by-frame in volatile RAM and immediately discarded.
   - No `.mp4` or `.mov` files are created, stored, or transmitted.
2. **Encrypted Telemetry Storage**:
   - Only numerical biomarker vectors are persisted to disk.
   - Encrypted with **Apple CryptoKit (`AES-GCM-256`)** using keys backed by the device's Secure Enclave.
3. **FDA SaMD Considerations**:
   - Adheres to FDA Class II Software as a Medical Device (SaMD) principles modeled after EarliPoint (`DEN220050`).
   - Clear clinical disclaimers, audit trails, and strict exclusion of uncalibrated or low-quality sessions ($<75\%$ valid tracking).
