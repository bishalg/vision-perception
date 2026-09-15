# Autism Spectrum Disorder (ASD) Eye-Tracking & Digital Phenotyping Research Library

> **Directory**: `docs/research/asd_gaze_biomarkers/`  
> **Status**: Active Clinical & Engineering Knowledge Repository  
> **Target Applications**: `apps/ios_asd_gaze_app` (Host iOS App) & `domains/asd_screening` (Core Biomarker Engine)  
> **Scope**: Pediatric digital biomarkers, oculomotor tracking, social-visual engagement, and machine learning diagnostics.

---

## 1. Master Comparative Benchmark Matrix

The following table summarizes the four foundational publications establishing modern objective digital biomarkers for early ASD detection:

| Attribute | 1. EarliPoint Diagnostic Trial (JAMA 2023) | 2. SenseToKnow Remote App (NEJM AI 2024) | 3. GeoPref & DMN-Visual Link (eLife 2019) | 4. ML Eye-Tracking Review (JBI 2022) |
| :--- | :--- | :--- | :--- | :--- |
| **Document ID** | [`DOC-RES-ASD-001`](01_jama_2023_earlipoint_social_visual_engagement.md) | [`DOC-RES-ASD-002`](02_nejm_ai_2024_sensetoknow_remote_mobile_screening.md) | [`DOC-RES-ASD-003`](03_elife_2019_geopref_default_mode_visual_hypoconnectivity.md) | [`DOC-RES-ASD-004`](04_jbi_2022_machine_learning_eyetracking_systematic_review.md) |
| **Lead Authors** | Warren Jones, Cheryl Klaiman, et al. | P. R. Krishnappa Babu, G. Sapiro, G. Dawson | Michael V. Lombardo, Karen Pierce, et al. | Qiuhong Wei, Tingyu Li, et al. |
| **Institutions** | Marcus Autism Center, Emory, WashU | Duke University & Princeton University | UC San Diego Autism Center of Excellence | Children's Hospital of Chongqing Med Univ |
| **Journal** | *JAMA* 2023; 330(9): 854–865 | *NEJM AI* 2024; Published Sept 2024 | *eLife* 2019; 8: e47427 | *J. Biomed. Inform.* 2022; 137: 104254 |
| **DOI** | [`10.1001/jama.2023.13295`](https://doi.org/10.1001/jama.2023.13295) | [`10.1056/AIcs2400510`](https://doi.org/10.1056/AIcs2400510) | [`10.7554/eLife.47427`](https://doi.org/10.7554/eLife.47427) | [`10.1016/j.jbi.2022.104254`](https://doi.org/10.1016/j.jbi.2022.104254) |
| **Cohort Size** | $N = 475$ evaluable toddlers | $N = 620$ toddlers (188 autistic) | $N = 143$ eye-track; $N = 117$ rs-fMRI | $N = 1,396$ across 24 pooled studies |
| **Target Age** | 16 to 30 months | 16 to 40 months | 12 to 48 months | 1.5 to 20+ years (highest in $<6\text{ yrs}$) |
| **Hardware Platform**| Standalone clinic eye-tracking cart | **Consumer Apple iPhone & iPad** | Laboratory eye-tracker + 3T MRI | EyeLink, Tobii, SMI lab systems |
| **Stimulus Battery**| Brief dynamic social video vignettes | Movies + Gamified Bubble-Popping | Split-screen (Social vs Dynamic Geo) | Social vs Object videos, static faces |
| **Biomarker Focus**| Social Visual Engagement (SVE) Index | Multi-modal: Gaze + Affect + Pose + Touch | 69% GeoPref Subtype Fixation Rule | Tabular fixations, saccades, scanpaths |
| **Accuracy / AUC** | $\text{AUC} \approx 0.85\text{–}0.90$ | $\mathbf{AUC = 0.92}$ | $\text{AUC} \approx 0.88$ (extreme specificity) | $\mathbf{\text{Summary AUC} = 0.88}$ |
| **Sensitivity** | **71.0% – 78.0%** (81.9% in replication) | **83.0%** | High sensitivity when combined | **80.0%** (pooled meta-analysis) |
| **Specificity** | **80.7% – 85.4%** (89.9% in discovery) | **93.3%** | **98.0%** (for $\ge 69\%$ GeoPref subtype) | **81.0%** (pooled meta-analysis) |
| **Key Clinical Merit**| FDA De Novo Class II clearance; ADOS correlation ($R = -0.75$) | Remote caregiver home screening; zero clinical setup required | Proves macroscale neurobiological DMN-visual hypoconnectivity | Evaluates 24 ML models; exposes pitfalls like data leakage |

---

## 2. Research Dossier Directory

| Document | Identifier | Focus & Landmark Discoveries |
| :--- | :--- | :--- |
| **[`01_jama_2023_earlipoint_social_visual_engagement.md`](01_jama_2023_earlipoint_social_visual_engagement.md)** | `DOC-RES-ASD-001` | **EarliPoint System & Social Visual Engagement**: Multisite double-blind trial ($N = 475$ toddlers); normative SVE divergence index correlates with ADOS-2 ($R = -0.75$), Mullen verbal ($R = 0.65$), and Mullen nonverbal ($R = 0.65$). FDA De Novo Class II authorization. |
| **[`02_nejm_ai_2024_sensetoknow_remote_mobile_screening.md`](02_nejm_ai_2024_sensetoknow_remote_mobile_screening.md)** | `DOC-RES-ASD-002` | **SenseToKnow Remote Mobile App**: Unconstrained caregiver home screening ($N = 620$ toddlers) on iPhone and iPad; multimodal fusion of gaze preference, 3D head-pose dynamics, facial affect expressivity, and bubble-popping motor response. $\text{AUC} = 0.92$, $\text{Sens} = 83\%$, $\text{Spec} = 93.3\%$. |
| **[`03_elife_2019_geopref_default_mode_visual_hypoconnectivity.md`](03_elife_2019_geopref_default_mode_visual_hypoconnectivity.md)** | `DOC-RES-ASD-003` | **GeoPref Paradigm & Neuroimaging Grounding**: Natural-sleep resting-state fMRI in toddlers proves that extreme geometric fixation ($\ge 69\%$) is underpinned by marked functional hypoconnectivity between the Default Mode Network (DMN) and visual association cortex. |
| **[`04_jbi_2022_machine_learning_eyetracking_systematic_review.md`](04_jbi_2022_machine_learning_eyetracking_systematic_review.md)** | `DOC-RES-ASD-004` | **Machine Learning Meta-Analysis & Systematic Review**: Synthesis of 24 studies ($N = 1,396$). Pooled accuracy 81%, pooled sensitivity 80%, pooled specificity 81%. Evaluates SVM, Random Forest, CNNs, LSTMs; details pitfalls of small-sample overfitting and data leakage. |
| **[`05_app_design_and_clinical_translation_guide.md`](05_app_design_and_clinical_translation_guide.md)** | `DOC-RES-ASD-005` | **Clinical-to-Code Implementation Blueprint**: Direct mapping of all four papers into `apps/ios_asd_gaze_app` and `domains/asd_screening`. Covers the 16 digital biomarkers, 1-Euro adaptive filtering, 5-point bubble calibration UX, Doctor Report generation, and HIPAA/COPPA zero-video privacy architecture. |
| **[`06_clinical_interoperability_fhir_smart_omop_standards.md`](06_clinical_interoperability_fhir_smart_omop_standards.md)** | `DOC-RES-ASD-006` | **Clinical Interoperability & Big Data Standards**: SMART on FHIR, OAuth 2.0 PKCE, OMOP CDM v5.4 mapping, ePRO/eCOA/ObsRO regulatory taxonomy, and semantic vocabularies (LOINC, SNOMED CT, ICD-10/11, USCDI) for EHR and registry integration. |

---

## 3. Shareable Publication-Grade Clinical PDFs

Two publication-grade PDF documents have been compiled for medical doctors, clinical investigators, and research sharing. They can be re-generated at any time using [`scripts/generate_clinical_and_research_pdfs.py`](../../../scripts/generate_clinical_and_research_pdfs.py):

| Document | File Path / Link | Target Audience & Contents |
| :--- | :--- | :--- |
| **PDF 1: Research Evidence Dossier** | [`ASD_External_Research_Synthesis_and_Evidence_Dossier.pdf`](ASD_External_Research_Synthesis_and_Evidence_Dossier.pdf) | **For Researchers & Clinicians**: 3-page scientific synthesis of JAMA 2023, NEJM AI 2024, eLife 2019, and JBI 2022. Includes full comparative benchmark table, clinical accuracy (AUC/Sens/Spec), DMN neuroimaging findings, and citation bibliography. |
| **PDF 2: Clinical Architecture & Roadmap** | [`ASD_Clinical_System_Architecture_and_Roadmap.pdf`](ASD_Clinical_System_Architecture_and_Roadmap.pdf) | **For Pediatricians & Doctor Reviewers**: 3-page non-technical guide explaining passive infrared tracking, zero-video privacy architecture, current capabilities, **synchronized facial emotion tracking**, **age-based developmental emotion matrix (12–48 mo)**, and 4-phase clinical deployment roadmap. |

> **Automated Re-Generation**:  
> To re-compile both PDFs at any time, run:  
> ```bash
> python3 scripts/generate_clinical_and_research_pdfs.py
> ```  
> Output copies are placed in `docs/research/asd_gaze_biomarkers/` and mirrored to `output/clinical_pdfs/`.

---

## 4. Protocol for Adding Future Research Papers


To ensure this research library remains well-organized, scalable, and directly actionable for ongoing app development, adhere to the following protocol when adding new papers:

### 3.1 File Naming & Numbering Convention
- Format: `XX_<journal>_<year>_<short_slug>.md`
- Examples:
  - `06_nature_2025_infant_gaze_trajectory.md`
  - `07_lancet_psychiatry_2026_digital_biomarker_intervention.md`

### 3.2 Required Document Sections
Every new paper dossier should include:
1. **Header Block**: Title, Document ID (`DOC-RES-ASD-XXX`), URL, Journal, Year, DOI, PMID/PMCID, ClinicalTrials ID, Authors, Institutions.
2. **Executive Summary & Scientific Question**: What problem does this paper address? What is its core innovation?
3. **Cohort & Methodology**: Sample size ($N$), age range, diagnostic criteria (ADOS/DSM-5), control groups, hardware/sensors used.
4. **Key Findings & Diagnostic Accuracy**: Numerical metrics (Sensitivity, Specificity, AUC, PPV, NPV, Effect sizes, Correlations).
5. **Sub-Linked Research & Citation Network**: Key antecedent and descendant publications with clickable DOIs.
6. **Direct Engineering & App Design Takeaways**: A structured table mapping the paper's findings directly to files in `apps/ios_asd_gaze_app` and `domains/asd_screening`.

### 3.3 Registration
After creating a new dossier, update:
1. The **Master Comparative Benchmark Matrix** in this file (`docs/research/asd_gaze_biomarkers/README.md`).
2. The root index in **`docs/research/README.md`**.
