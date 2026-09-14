# Validation of a Mobile App for Remote Autism Screening in Toddlers (SenseToKnow)

> **Document ID**: `DOC-RES-ASD-002`  
> **Source Link**: [NEJM AI Article DOI: 10.1056/AIcs2400510](https://ai.nejm.org/doi/10.1056/AIcs2400510)  
> **Journal**: *NEJM AI (New England Journal of Medicine AI)*, Published September 26, 2024  
> **DOI**: [`10.1056/AIcs2400510`](https://doi.org/10.1056/AIcs2400510)  
> **Authors**: Pradeep Raj Krishnappa Babu, J. Matias Di Martino, Rachel Aiello, Brian Eichner, Steven Espinosa, Jennifer Green, Jill Howard, Sam Perochon, Marina Spanos, Saritha Vermeer, Geraldine Dawson, Guillermo Sapiro  
> **Institutions**: Duke Center for Autism and Brain Development, Duke University (Durham, NC); Princeton University (Princeton, NJ); Catholic University of Uruguay; École Normale Supérieure Paris-Saclay  
> **Application Name**: **SenseToKnow** (Mobile iOS App on iPhone and iPad)

---

## 1. Executive Summary & Breakthrough Contribution

Prior objective eye-tracking systems (such as EarliPoint or Tobii laboratory rigs) require dedicated hardware, controlled lighting, chin rests, and trained clinical technicians. This restricts their availability to specialized medical centers.

The **SenseToKnow** study is the first large-scale, prospective clinical validation demonstrating that a **purely software-based mobile application deployed on consumer Apple iPhones and iPads**, administered completely remotely by parents/caregivers in unstructured home environments, achieves **clinical-grade diagnostic accuracy (AUC 0.92, Sensitivity 83.0%, Specificity 93.3%)**.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                      SenseToKnow Remote Mobile Architecture                             │
│                                                                                         │
│  [ Caregiver's Own Consumer Device (iPhone / iPad) ]                                    │
│         │                                                                               │
│         ├── 1. Interactive Media Stimuli: 10–12 short movies (social vs dynamic toys)   │
│         ├── 2. Gamified Bubble-Popping: Target re-centering, motor & touch tracking     │
│         └── 3. Front-Facing TrueDepth / RGB Sensor: Passive video stream capture        │
│                                                                                         │
│  [ Multimodal Computer Vision (CV) Feature Extraction Engine ]                          │
│  ┌─────────────────────────┬─────────────────────────┬───────────────────────────────┐  │
│  │ Gaze Preference & AOI   │ Facial Affect Dynamics  │ Head Movement & Posture       │  │
│  │ • Dwell time on faces   │ • Smile intensity       │ • Angular velocity            │  │
│  │ • Social vs dynamic toys│ • Flat affect duration  │ • Multiscale entropy          │  │
│  │ • Saccade latency       │ • Delayed facial mimicry│ • Response to name head-turns │  │
│  └─────────────────────────┴─────────────────────────┴───────────────────────────────┘  │
│                                            │                                            │
│                                            ▼                                            │
│  [ Gradient Boosted Decision Trees / Ensemble ML Classifier ]                           │
│         │                                                                               │
│         └── AUC = 0.92 | Sensitivity = 83.0% | Specificity = 93.3%                      │
│             PPV = 84.3% | NPV = 92.6% (Consistent across iPhone & iPad)                 │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Cohort Demographics & Study Protocol

| Parameter | Clinical Specification |
| :--- | :--- |
| **Study Setting** | Fully remote, home-based administration by caregivers on their personal devices |
| **Cohort Size** | $N = 620$ toddlers enrolled and evaluable |
| **Age Window** | 16 to 40 months (mean $\approx 22.4$ months) |
| **Autism Diagnoses** | $n = 188$ toddlers independently evaluated and diagnosed with ASD by expert clinicians |
| **Non-Autism Controls**| $n = 432$ toddlers (neurotypical development or non-ASD developmental delay) |
| **Hardware Devices** | Consumer Apple iPhone (e.g., iPhone 11–14 Pro) and Apple iPad (iPad 7th gen+, iPad Pro) |
| **Test Duration** | $\approx 10\text{–}12\text{ minutes}$ total engagement |
| **Reference Standard** | In-person gold-standard clinical evaluation: DSM-5 diagnostic criteria, ADOS-2, and cognitive testing by licensed clinical psychologists |

---

## 3. Digital Phenotyping Battery & Sensory Extraction

SenseToKnow transcends unimodal eye tracking by fusing **four distinct behavioral modalities** recorded simultaneously through the front-facing camera:

### 3.1 Social vs. Non-Social Visual Engagement (Gaze)
- **Visual Stimuli**: Split-screen and full-screen vignettes contrasting lively social interactions (actors singing, dancing, making eye contact) with mechanically dynamic non-social toys (spinning gears, spinning tops, rolling balls).
- **Extracted Gaze Metrics**:
  - Total fixation duration on social vs inanimate Areas of Interest (AOIs).
  - Time-to-first-fixation on eyes vs mouth vs background.
  - Saccadic re-fixation latencies between competing stimuli.

### 3.2 Dynamic Facial Affect & Emotional Expressivity
- Tracks 68 facial landmarks and facial action units (Action Unit 6: Cheek Raiser, Action Unit 12: Lip Corner Puller).
- **Biomarkers**:
  - Smile frequency and smile latency in response to humorous or social video events.
  - Duration of neutral/flat affect during high-salience emotional stimuli.
  - Attenuation of spontaneous emotional mimicry.

### 3.3 Head Pose & Motor Orienting Dynamics
- Extracts 3D head pose orientation $(\text{yaw}, \text{pitch}, \text{roll})$ and translation $(x, y, z)$.
- **Biomarkers**:
  - Head movement multiscale entropy and fractal dimension (identifying atypical postural stiffness or motor instability).
  - Orienting response latency during audio prompts (calling child's name).

### 3.4 Motor & Touch Interaction (Bubble-Popping Minigame)
- Interleaved bubble-popping games serve a dual purpose:
  1. Re-calibrates and re-centers the child's gaze between movies.
  2. Quantifies motor coordination, touch precision, finger velocity, and response latency.

---

## 4. Machine Learning Formulation & Results

### 4.1 Feature Ingestion & Classification Pipeline
- High-dimensional behavioral features are aggregated across all video stimuli.
- Model Architecture: **XGBoost (Extreme Gradient Boosting)** ensemble classifier trained with cross-validation and strict participant-level stratification to prevent data leakage.
- Missing data handling: Native sparsity handling in decision trees allows robust classification even when the child turns away for a segment of the video.

### 4.2 Diagnostic Performance

| Performance Metric | SenseToKnow Result (Remote Home) | Clinical Significance |
| :--- | :--- | :--- |
| **Area Under ROC Curve (AUC)** | **0.92** | Exceptional discriminative power in uncontrolled environments |
| **Sensitivity** | **83.0%** | Accurately identifies 83 out of 100 toddlers with ASD |
| **Specificity** | **93.3%** | Very low false positive rate ($6.7\%$), minimizing caregiver anxiety |
| **Positive Predictive Value (PPV)**| **84.3%** | 84.3% of positive app flags represent true clinical autism |
| **Negative Predictive Value (NPV)**| **92.6%** | High certainty of negative result |
| **Device Parity** | **iPhone vs. iPad: No significant difference** | Proves invariance to screen dimensions and viewing distance |

---

## 5. Sub-Linked Research & Citation Network

The authors build upon a rigorous progression of foundational computer vision and behavioral pediatrics research:

1. **Duke CV/ML Digital Phenotyping Precursors**:
   - Chang Z, Di Martino JM, Aiello R, et al. *Computational Methods to Measure Patterns of Gaze in Toddlers With Autism Spectrum Disorder.* JAMA Pediatr. 2021;175(8):827–836. [`doi:10.1001/jamapediatrics.2021.0530`](https://doi.org/10.1001/jamapediatrics.2021.0530)
   - Krishnappa Babu PR, Di Martino JM, Carpenter KLH, et al. *Complexity analysis of head movements in autistic toddlers.* J Child Psychol Psychiatry. 2023;64(7):1063–1072. [`doi:10.1111/jcpp.13681`](https://doi.org/10.1111/jcpp.13681)
   - Babu PRK, Di Martino JM, Carpenter KLH, et al. *Exploring Complexity of Facial Dynamics in Autism Spectrum Disorder.* IEEE Trans Affect Comput. 2021;14(2):1604–1616. [`doi:10.1109/TAFFC.2021.3113876`](https://doi.org/10.1109/TAFFC.2021.3113876)
   - Dawson G, Campbell K, Hashemi J, et al. *Atypical postural control can be detected via computer vision analysis in toddlers with autism spectrum disorder.* Sci Rep. 2018;8(1):17008. [`doi:10.1038/s41598-018-35215-8`](https://doi.org/10.1038/s41598-018-35215-8)
2. **Pediatric Screening Disparities & Behavioral Markers**:
   - Robins DL, Casagrande K, Barton M, et al. *Validation of the Modified Checklist for Autism in Toddlers, Revised With Follow-up (M-CHAT-R/F).* Pediatrics. 2014;133(1):37–45. [`doi:10.1542/peds.2013-1813`](https://doi.org/10.1542/peds.2013-1813)
   - Guthrie W, Wallis K, Bennett A, et al. *Accuracy of Autism Screening in a Large Pediatric Network.* Pediatrics. 2019;144(4):e20183963. [`doi:10.1542/peds.2018-3963`](https://doi.org/10.1542/peds.2018-3963)
   - Chawarska K, Klin A, Paul R, Volkmar F. *Limited Attentional Bias for Faces in Toddlers With Autism Spectrum Disorders.* Arch Gen Psychiatry. 2010;67(2):178–186. [`doi:10.1001/archgenpsychiatry.2009.194`](https://doi.org/10.1001/archgenpsychiatry.2009.194)
   - Miller M, Iosif AM, Young GS, et al. *Response to Name in Infants Developing Autism Spectrum Disorder: A Prospective Study.* J Pediatr. 2017;183:141–146. [`doi:10.1016/j.jpeds.2016.12.071`](https://doi.org/10.1016/j.jpeds.2016.12.071)
3. **Computer Vision & Facial Mesh Models**:
   - De La Torre F, Chu WS, Xiong X, et al. *IntraFace.* IEEE FG. 2015:1–8. [`doi:10.1109/FG.2015.7163082`](https://doi.org/10.1109/FG.2015.7163082)
   - Chen T, Guestrin C. *XGBoost: A Scalable Tree Boosting System.* ACM KDD. 2016:785–794. [`doi:10.1145/2939672.2939785`](https://doi.org/10.1145/2939672.2939785)

---

## 6. Direct Engineering & App Design Takeaways for `apps/ios_asd_gaze_app`

| Clinical / Technical Principle | SenseToKnow Precedent | `apps/ios_asd_gaze_app` Implementation Architecture |
| :--- | :--- | :--- |
| **Zero Hardware Overhead** | Consumer iPhone / iPad front cameras | Uses Apple **ARKit TrueDepth** + CoreVision; works on iPhone 11+ and iPad Pro without external sensor dongles. |
| **Recalibration & Child Engagement** | Interleaved bubble-popping game | Implement interactive bubble popping (`BubbleCalibrationView.swift`) before and between video blocks to re-anchor gaze. |
| **Multimodal Feature Extraction** | Gaze + Face Affect + Head Pose | `CurvAsdGazeKit` extracts 3D head pose $(\text{yaw}, \text{pitch})$, eye gaze $(x, y)$, and facial blendshapes (`smileLeft`, `smileRight`) in real time. |
| **Privacy-by-Design** | Privacy-conscious parent trust | **Zero raw video recording**: Raw camera buffers exist strictly in volatile RAM; disk persistence stores only numerical mathematical features encrypted with Apple CryptoKit (`AES-GCM-256`). |
| **Device Size Invariance** | Screen normalized coordinate mapping | All gaze tracking calculations normalize coordinates to $[0.0, 1.0]$ viewport space, decoupling UI layouts across iPhone and iPad (`ScreenRaycaster.swift`). |
