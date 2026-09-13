# Pediatric Social Attention & Autism Gaze Literature Review

> **Package**: `domains/asd_screening`  
> **Document**: `domains/asd_screening/docs/LITERATURE.md`  
> **Scientific Scope**: Literature review, clinical validation benchmarks, and the 12-Phase SaMD roadmap

---

## 1. Landmark Clinical Literature & Findings

Eye-tracking research over the past two decades has established that differences in **social attention allocation** represent one of the earliest quantifiable behavioral markers of autism spectrum phenotypes:

### 1.1 Klin & Jones (Yale Child Study Center / Marcus Autism Center)
- **Klin et al. (2002)**: *Visual Fixation Patterns During Viewing of Naturalistic Social Situations as Predictors of Social Competence in Individuals with Autism*. Demonstrated that individuals with autism demonstrate significantly reduced visual fixation on the eye region of human faces and disproportionate fixation on mouths, bodies, and inanimate objects.
- **Jones & Klin (2013)**: *Attention to eyes is present but in decline in 2–6-month-old infants later diagnosed with autism* (Nature). Longitudinal tracking demonstrated that infants later diagnosed with ASD exhibit typical eye contact at 2 months, followed by a steep developmental decline between 2 and 24 months.

### 1.2 Pierce et al. (UC San Diego Autism Center of Excellence)
- **Pierce et al. (2011, 2016)**: *Preference for Geometric Patterns in Early Autism (The GeoPref Test)*. Developed the split-screen preferential looking paradigm. When presented with concurrent social activities and dynamic geometric patterns, 40%+ of toddlers with ASD spent $> 70\%$ of their visual time fixated on geometric shapes, whereas neurotypical toddlers overwhelmingly preferred social activities.

### 1.3 Duke University: SenseToKnow & Tablet-Based Phenotyping
- **Egger, Dawson, Sapiro et al. (2018–2023)**: *Automatic Computer Vision-Based Measurement of Social Visual Attention*. Validated that commodity consumer cameras (iPad/iPhone) without specialized infrared hardware can achieve high discriminative sensitivity ($\ge 85\%$) in pediatric primary care clinics by analyzing head movement, gaze orienting, and affective dynamics during brief 3-minute video stimuli.

### 1.4 The FDA-Cleared EarliPoint Evaluation
- **EarliPoint System (FDA 510(k) Cleared, 2023)**: The first FDA-cleared diagnostic aid using eye tracking. Tracks visual engagement in toddlers aged 16–30 months while watching dynamic social vignettes, quantifying social disability and verbal/non-verbal cognitive ability.

---

## 2. The 12-Phase Medical & Engineering Roadmap

| Phase | Phase Name | Primary Objective & Deliverable |
| :--- | :--- | :--- |
| **Phase 0** | **Intended Use & Regulatory Scoping** | Establish non-diagnostic SaMD positioning, target cohort (18–48 mo), and clinical advisory board. |
| **Phase 1** | **Requirements & Standards** | Software requirements (SRS), data dictionary, ISO 14971 risk management register. |
| **Phase 2** | **Study Protocol & Stimuli Design** | GeoPref, Face Scanning, and Joint Attention video stimuli with normalized dynamic AOIs. |
| **Phase 3** | **Offline iOS Data Collection MVP** | SwiftUI + ARKit TrueDepth 60Hz logger with animated 5-point bubble calibration. |
| **Phase 4** | **Data Ingestion & Preprocessing Pipeline** | Python data cleaning, I-VT event segmentation, and dataset versioning (DVC). |
| **Phase 5** | **Exploratory Data Analysis & Features** | Feature catalog reliability, ICC stability, and developmental age-normalization curves. |
| **Phase 6** | **Baseline Explainable ML Model** | ElasticNet & LightGBM trained with participant-level grouped cross-validation and SHAP. |
| **Phase 7** | **Clinical Validation & Feasibility** | Multi-site holdout validation against clinical ADOS-2 and M-CHAT-R/F scores. |
| **Phase 8** | **Offline CoreML Model Deployment** | Quantized `.mlpackage` executing on Apple Neural Engine (ANE) with $< 2\text{ms}$ latency. |
| **Phase 9** | **ResearchKit & CareKit Expansion** | Digital e-Consent (`ORKConsentReviewStep`) and longitudinal tracking (`OCKCarePlanActivity`). |
| **Phase 10** | **MLOps & Enterprise Scalability** | Automated data quality monitoring, drift detection, and secure HIPAA cloud backend. |
| **Phase 11** | **Clinical & Regulatory Dossier** | Complete SaMD Class II dossier, usability validation (IEC 62366), and clinical evaluation report. |
| **Phase 12** | **Post-Market Surveillance & Maintenance** | Post-market quality tracking, continuous learning, and multi-site research consortium. |

---

## 3. Immediate 90-Day Execution Plan

- **Days 0–30 (Foundation)**: Complete C++20 `AsdBiomarkerPack`, stimuli specifications, and research ingestion tool. Establish IRB e-consent template.
- **Days 31–60 (Client & Calibration)**: Implement the custom `ORKActiveStep` in Swift featuring the animated 5-point bubble calibration on iPad.
- **Days 61–90 (Pilot Ingestion)**: Collect 20–30 de-identified pilot calibration sessions; verify zero frame drops and feature calculation parity between C++ and Python.
