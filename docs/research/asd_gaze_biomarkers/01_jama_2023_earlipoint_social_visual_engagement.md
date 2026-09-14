# Eye-Tracking–Based Measurement of Social Visual Engagement Compared With Expert Clinical Diagnosis of Autism

> **Document ID**: `DOC-RES-ASD-001`  
> **Source Link**: [JAMA Article 2808996](https://jamanetwork.com/journals/jama/fullarticle/2808996)  
> **Journal**: *JAMA (Journal of the American Medical Association)*, 2023; 330(9): 854–865  
> **DOI**: [`10.1001/jama.2023.13295`](https://doi.org/10.1001/jama.2023.13295)  
> **Clinical Trial**: ClinicalTrials.gov Identifier [NCT03469986](https://clinicaltrials.gov/study/NCT03469986)  
> **PMID**: `37668621` | **PMCID**: `PMC10481242`  
> **Companion Papers**:  
> • *JAMA Network Open* 2023; 6(9): e2330145 (DOI: `10.1001/jamanetworkopen.2023.30145`, PMCID: `PMC10481232`)  
> • *JAMA Editorial* 2023; 330(9): 815–816 (DOI: `10.1001/jama.2023.3092`, PMCID: `PMC10773262`)  
> **Commercialized System**: **EarliPoint System** (EarliTec Diagnostics; FDA 510(k) De Novo Class II authorization)

---

## 1. Executive Summary & Clinical Context

In the United States and globally, early diagnosis of Autism Spectrum Disorder (ASD) is severely constrained by specialized clinical bottlenecks. Toddlers showing developmental concerns routinely wait over 12 months for comprehensive diagnostic evaluations, with even longer delays observed in low-income, minority, and rural populations.

This landmark multisite study evaluates whether **automated, objective eye-tracking measurements of Social Visual Engagement (SVE)** during brief video viewing can reliably differentiate young children with ASD from non-autistic peers, as well as predict standardized clinical assessment scores.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                   EarliPoint Diagnostic Paradigm (Jones et al.)                  │
│                                                                                  │
│   Toddler watches standardized video scenes (social vignettes, child-directed    │
│   speech / "motherese", peer play, joint attention opportunities)                 │
│                                      │                                           │
│                 Eye-tracking capture at millisecond resolution                   │
│                                      │                                           │
│                                      ▼                                           │
│   Quantify dynamic gaze coordinates relative to normative reference distribution │
│   (Thousands of discrete coordinate points sampled per second)                   │
│                                      │                                           │
│                                      ▼                                           │
│   ┌───────────────────────────┬───────────────────────────┬──────────────────┐   │
│   │ 1. Diagnostic Index (SVE) │ 2. Social Disability (ADOS)│ 3. Mullen Scales │   │
│   │    Sens: 71.0% - 78.0%    │    R = -0.75              │    Verbal: 0.65  │   │
│   │    Spec: 80.7% - 85.4%    │    (68.6% variance)       │    Non-V: 0.65   │   │
│   └───────────────────────────┴───────────────────────────┴──────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Study Design & Cohort Demographics

| Parameter | Specification |
| :--- | :--- |
| **Study Type** | Multisite, prospective, double-blind diagnostic trial |
| **Enrollment Period** | April 2018 through May 2019 |
| **Clinical Sites** | 6 US Academic Specialty Centers (e.g., Marcus Autism Center, Children's Healthcare of Atlanta, Washington University in St. Louis) |
| **Target Population** | Toddlers aged 16 to 30 months referred for suspected developmental delay / ASD |
| **Total Enrolled** | $N = 499$ children |
| **Evaluable Dataset** | $N = 475$ ($95.2\%$ completion rate; 24 excluded due to calibration/cooperation) |
| **Mean Age** | $24.1 \pm 4.4$ months |
| **Demographics** | Asian: 8.0%, Black: 7.8%, White: 74.1%, Hispanic: 14.3%, Other: 9.3% |
| **Clinical Diagnoses** | **ASD**: $n = 221$ ($46.5\%$) \| **Non-ASD**: $n = 254$ ($53.5\%$) |
| **Certain Subgroup** | $n = 335$ children where expert clinician diagnostic certainty was definitive |
| **Reference Standard** | Best-practice comprehensive clinical evaluation (blinded expert clinicians using DSM-5, ADOS-2, and Mullen Scales of Early Learning) |

---

## 3. Technology & Quantitative Biomarker Formulation

### 3.1 Stimulus Battery
The child is seated in front of a calibrated display playing brief ($10\text{–}60\text{ s}$), professionally produced video vignettes depicting:
1. **Dynamic Social Scenes**: Actors speaking infant-directed speech ("motherese") facing directly toward the camera.
2. **Interactive Play**: Peer interactions, cooperative games, turn-taking.
3. **Competing Inanimate Objects**: Visually salient toys, background objects, geometric elements in the periphery.

### 3.2 Eye-Tracking & Telemetry
- Standalone hardware capturing binocular gaze coordinates $(x_i, y_i)$ at high frame rates ($>60\text{ Hz}$).
- Automatically derives timestamped coordinates mapped onto video frame areas of interest (AOIs):
  - **Eyes & Face AOI**: Center of social communicative cues.
  - **Mouth AOI**: Speech and phoneme articulation tracking.
  - **Body / Gesture AOI**: Pointing and functional actions.
  - **Object / Scene AOI**: Non-social elements.

### 3.3 The "Transactional Biomarker" Formulation
Rather than relying on simple total gaze time, the system compares the child's gaze stream against an empirical normative distribution of neurotypical toddlers:
$$\text{Divergence Metric}(t) = \left\| \mathbf{gaze}_{\text{child}}(t) - \boldsymbol{\mu}_{\text{typical}}(t) \right\|_{\boldsymbol{\Sigma}_{\text{typical}}^{-1}}$$
- Accrual of atypical visual experience over time quantifies repeated divergence from shared human social attention.
- Because social learning requires joint engagement, persistent divergence directly predicts developmental divergence in language and social-communication networks.

---

## 4. Key Findings & Diagnostic Performance

### 4.1 Primary Diagnostic Accuracy

| Metric | All Children ($N = 475$) | Clinically Certain Subgroup ($n = 335$) | Primary Care Replication ($n = 370$) |
| :--- | :--- | :--- | :--- |
| **Sensitivity** | **71.0%** (95% CI: 64.7%–76.6%) | **78.0%** (95% CI: 70.7%–83.9%) | **80.6% – 81.9%** |
| **Specificity** | **80.7%** (95% CI: 75.4%–85.1%) | **85.4%** (95% CI: 79.5%–89.8%) | **82.3% – 89.9%** |
| **Positive Predictive Value (PPV)**| 76.2% | 83.1% | 84.1% |
| **Negative Predictive Value (NPV)**| 76.1% | 80.8% | 81.2% |
| **AUC** | $\approx \mathbf{0.85 - 0.90}$ | $\mathbf{0.90}$ | $\mathbf{0.89}$ |

### 4.2 Dimensional Correlation with Gold-Standard Clinical Instruments

Measurements of Social Visual Engagement were not merely binary classifiers; they exhibited strong dimensional correlation with continuous clinical metrics:

1. **Autism Diagnostic Observation Schedule (ADOS-2)**:
   - Correlation: $r = -0.75$ ($95\%\text{ CI: } -0.79 \text{ to } -0.71$)
   - Accounted for **$68.6\%$ of variance** in ADOS-2 total social affect and restricted/repetitive behavior scores ($R_{\text{adj}}^2 = 0.69$).
2. **Mullen Scales of Early Learning (MSEL) - Verbal Ability**:
   - Correlation: $r = 0.65$ ($95\%\text{ CI: } 0.59 \text{ to } 0.70$)
   - Accounted for **$63.4\%$ of variance** in Mullen verbal age-equivalent scores ($R_{\text{adj}}^2 = 0.63$).
3. **Mullen Scales of Early Learning (MSEL) - Nonverbal Cognitive Ability**:
   - Correlation: $r = 0.65$ ($95\%\text{ CI: } 0.59 \text{ to } 0.70$)
   - Accounted for **$49.0\%$ of variance** in Mullen nonverbal age-equivalent scores ($R_{\text{adj}}^2 = 0.49$).

---

## 5. Regulatory Status: FDA De Novo Clearance

- In 2023, the U.S. FDA granted **De Novo Class II authorization** to the **EarliPoint System** (`DEN220050` / `K231189`) as an aid in autism diagnosis and assessment for children aged 16 to 30 months.
- **Indication for Use**: Objective aid to clinical diagnosis and symptom quantification; explicitly **not** a standalone diagnostic or a screening test for general population without clinical concern.
- Operates under CLIA-waived or point-of-care clinic settings, returning automated algorithmic scores within 30 minutes.

---

## 6. Sub-Linked Research & Citation Network

The following key foundational and derivative publications form the empirical substrate of this trial:

1. **Prevalence & Diagnostic Delays**:
   - Maenner MJ, Warren Z, Williams AR, et al. *Prevalence and characteristics of autism spectrum disorder among children aged 8 years—ADDM Network, 2020.* MMWR Surveill Summ. 2023;72(2):1-14. [`doi:10.15585/mmwr.ss7202a1`](https://doi.org/10.15585/mmwr.ss7202a1)
   - Shaw KA, McArthur D, Hughes MM, et al. *Progress and disparities in early identification of autism spectrum disorder: ADDM Network, 2002-2016.* J Am Acad Child Adolesc Psychiatry. 2022;61(7):905-914. [`doi:10.1016/j.jaac.2021.11.019`](https://doi.org/10.1016/j.jaac.2021.11.019)
   - Mandell DS, Novak MM, Zubritsky CD. *Factors associated with age of diagnosis among children with autism spectrum disorders.* Pediatrics. 2005;116(6):1480-1486. [`doi:10.1542/peds.2005-0185`](https://doi.org/10.1542/peds.2005-0185)
2. **Early Oculomotor & Attention Trajectories**:
   - Jones W, Klin A. *Attention to eyes is present but in decline in 2-6-month-old infants later diagnosed with autism.* Nature. 2013;504(7480):427-431. [`doi:10.1038/nature12715`](https://doi.org/10.1038/nature12715)
   - Klin A, Lin DJ, Gorrindo P, Ramsay G, Jones W. *Two-year-olds with autism orient to non-social contingencies rather than biological motion.* Nature. 2009;459(7244):257-261. [`doi:10.1038/nature07868`](https://doi.org/10.1038/nature07868)
   - Chawarska K, Macari S, Shic F. *Decreased spontaneous attention to social scenes in 6-month-old infants later diagnosed with autism spectrum disorders.* Biol Psychiatry. 2013;74(3):195-203. [`doi:10.1016/j.biopsych.2012.11.022`](https://doi.org/10.1016/j.biopsych.2012.11.022)
3. **Companion Replication & Clinical Validity**:
   - Jones W, Klaiman C, Richardson C, et al. *Development and Replication of Objective Measurements of Social Visual Engagement to Aid in Early Diagnosis and Assessment of Autism.* JAMA Netw Open. 2023;6(9):e2330145. [`doi:10.1001/jamanetworkopen.2023.30145`](https://doi.org/10.1001/jamanetworkopen.2023.30145)
   - Lord C, Brugha TS, Charman T, et al. *Autism spectrum disorder.* Nat Rev Dis Primers. 2020;6(1):5. [`doi:10.1038/s41572-019-0138-4`](https://doi.org/10.1038/s41572-019-0138-4)

---

## 7. Direct Engineering & App Design Takeaways for `apps/ios_asd_gaze_app`

| Clinical Requirement | EarliPoint Benchmark | `apps/ios_asd_gaze_app` Implementation Architecture |
| :--- | :--- | :--- |
| **Sampling Frequency** | High-speed dedicated hardware ($>60\text{ Hz}$) | `AVFoundation` + TrueDepth IR Dot Projector running locked at **60 FPS** via `CADisplayLink`. |
| **Gaze Accuracy & Filtering** | Hardware corneal reflection | TrueDepth `ARFaceAnchor` 3D optical axis raycasting + **1-Euro Adaptive Low-Pass Filter** (`ScreenRaycaster.swift`) to suppress tremor without phase delay. |
| **Stimulus Synchronization** | Sub-frame timestamp indexing | Millisecond synchronization between `AVPlayer` playback head and telemetry timestamps ($\Delta t < 16.6\text{ ms}$). |
| **Dimensional Reporting** | Generates percentile indices for Social Disability, Verbal, and Nonverbal ability | `DoctorReportGenerator.swift` produces multi-dimensional PDF reports mapping SVE index against normative percentile thresholds. |
| **Operational Feasibility** | Minimal operator burden; test completion in $<15\text{ mins}$ | On-device 5-point calibration, automated valid-sample ratio quality gating, and instant PDF report generation. |
