# App Store Optimization (ASO) Architecture SSOT — Vision Perception

**Document Owner:** Vision Perception & Mobile Machine Learning Core  
**Standard Version:** 2026.1  
**Scope:** `apps/ios_palm_app`, `apps/android_palm_app`, `apps/ios_asd_gaze_app`

---

## 1. Core Principles & Mathematical Budgets

Metadata for computer vision and medical/developmental screening apps is strictly governed by platform indexing algorithms and compliance policies.

### Strict Budget Ceilings
1. **Apple App Store (iOS)**:
   - `Title`: Strictly **≤ 30 characters**.
   - `Subtitle`: Strictly **≤ 30 characters**.
   - `Keywords`: Strictly **≤ 100 characters**, comma-separated, **zero spaces**, zero repetition from Title or Subtitle.
2. **Google Play Store (Android)**:
   - `Title`: Strictly **≤ 30 characters**.
   - `Short Description`: Strictly **≤ 80 characters**.
   - `Full Description`: Strictly **≤ 4,000 characters** with 2–3% natural keyword density.
3. **Medical Disclaimer Invariant**:
   - For `EarlyGaze` (Autism/ADHD), all product page descriptions must explicitly state:
     > *"EarlyGaze is an observational developmental screening aid and monitoring reference. It is NOT a diagnostic medical device and does NOT diagnose Autism Spectrum Disorder (ASD), ADHD, or any medical condition."*
4. **Appeeky ASO Skills**:
   - 34 specialized agent skills installed in `.agents/skills/` (including `aso-router`, `metadata-optimization`, `android-aso`, `rating-prompt-strategy`).

---

## 2. Active Application Catalog & Fastlane Metadata

| Application | Platform | App Store Title (≤30) | Subtitle (≤30) | Primary Target Keywords |
| :--- | :--- | :--- | :--- | :--- |
| **PalmReader** | iOS & Android | `PalmReader: Hand Line Scanner` (29) | `Samudrika Shastra & Palmistry` (29) | `palmistry,samudrika,lifeline,heartline,headline,mount,fortune,destiny,future,kundli,horoscope` (93/100) |
| **EarlyGaze** | iOS & Android | `EarlyGaze: Autism & ADHD Test` (29) | `At-Home Pediatric Gaze Track` (28) | `mchat,pediatric,toddler,development,screening,biomarker,saccade,telehealth,sensory,behavior,focus` (97/100) |

---

## 3. Directory Layout

```
vision-perception/
├── .agents/
│   └── skills/                     ← 34 Appeeky ASO Agent Skills
├── apps/
│   ├── ios_palm_app/fastlane/metadata/
│   │   └── en-US/                  ← name.txt, subtitle.txt, keywords.txt, description.txt
│   ├── android_palm_app/fastlane/metadata/
│   │   └── android/en-US/          ← title.txt, short_description.txt, full_description.txt
│   └── ios_asd_gaze_app/fastlane/metadata/
│       ├── en-US/                  ← iOS metadata
│       └── android/en-US/          ← Android metadata
└── docs/
    ├── architecture/APP_STORE_OPTIMIZATION_ASO_SSOT.md
    └── qa/ASO_AND_STORE_LISTING_AUDIT_QA_CHECKLIST.md
```
