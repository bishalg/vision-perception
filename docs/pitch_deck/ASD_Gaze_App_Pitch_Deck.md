# Curv ASD Gaze Screening App — Pitch Deck

> **For clinicians, caregivers, investors, and partners**  
> **Version 1.0 — September 2026**  
> **Contact**: Bishal Ghimire · bishal@curv.engineering

---

## 1. The Problem: A Silent Epidemic

| Statistic | Source |
|-----------|--------|
| **1 in 36** children diagnosed with ASD (USA, 2023) | CDC ADDM Network |
| **Average age of diagnosis: 4.5 years** — critical intervention window missed | Zwaigenbaum et al., 2019 |
| **< 20%** of toddlers receive standardized developmental screening | AAP Policy Statement 2020 |
| **$2.4M** lifetime societal cost per undiagnosed individual | Buescher et al., 2014 |

**Bottom line**: We identify autism too late, with tools that are subjective, clinic-bound, and inaccessible to most families.

---

## 2. Our Solution: Objective, At-Home, Early

**Curv ASD Gaze Screening** turns the iPhone's TrueDepth camera into a clinical-grade eye tracker that measures *how a toddler looks at the world* — in 30–60 seconds, at home, with zero raw video recorded.

| What We Measure (16 Digital Biomarkers) | Clinical Meaning |
|------------------------------------------|------------------|
| **Social Preference Index** (GeoPref) | Does the child prefer faces over patterns? |
| **Eye Dwell Ratio** | How much time on eyes vs mouth? |
| **First-Eye Latency** | Speed of orienting to social stimuli |
| **Smile Contagion / Reactivity** | Shared positive affect |
| **Flat Affect Duration** | Reduced emotional expressivity |
| **Saccadic Latency / Velocity** | Oculomotor control |
| **Joint Attention Accuracy** | Following gaze/pointing |
| **M-CHAT-R/F Risk Score** (20-item caregiver survey) | Standardized behavioral screen |

**Output**: A single-page, clinician-ready PDF report with FHIR-interoperable data — ready for Epic, Cerner, or any EHR.

---

## 3. Why Now? Convergence of Three Forces

```
┌─────────────────────────────────────────────────────────────────┐
│  TRUE DEPTH HARDWARE       │  INTEROPERABILITY FOCUS            │
│  • 60 Hz 3D face mesh      │  • FHIR R4 & SMART on FHIR         │
│  • Sub-mm gaze tracking    │    paediatric clinic workflows     │
│  • On-device, no cloud     │  • Zero-raw-video local encryption │
├────────────────────────────┼────────────────────────────────────┤
│  LITERATURE-BACKED         │  MARKET PULL                       │
│  • GeoPref & Social Dwell  │  • 4M births/year in US            │
│  • Fixation dynamics       │  • Telehealth reimbursement (CPT)  │
│  • 6 engagement indices    │  • Employer/payer ASD mandates     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. Product Walkthrough (60 Seconds)

### Step 1: Onboarding (2 min)
- Caregiver creates child profile (name, age bracket, home language)
- **Progressive Apple-CDN download** of age-matched stimulus videos — happens once, shows live MB progress
- Privacy consent: **Zero raw video ever stored** — only encrypted numerical telemetry

### Step 2: Calibration (15 sec)
- 5-point bubble-pop game maps child's gaze to screen coordinates
- OneEuro filter + blink rejection = clinical-grade signal quality

### Step 3: Screening (30 sec / 60 sec)
| Battery | Clips | Age Range | Use Case |
|---------|-------|-----------|----------|
| **Express 30s** | 4 GeoPref/social | 18–36 mo | Well-child visit triage |
| **Comprehensive 60s** | 8 clinical clips | 12–60 mo | Full screening + research |

Clips: GeoPref split-screen, peekaboo, name calling, rhyme play, slapstick humor — **dynamic AOIs** track dwell on eyes, mouth, objects in real time.

### Step 4: M-CHAT-R/F Survey (2 min)
- 20 standardized caregiver questions on iPhone
- Instant scoring: **Low / Medium (Follow-Up flagged) / High** risk
- Medium risk auto-surfaces **M-CHAT Follow-Up interview** banner per protocol

### Step 5: Results & Actions
- **Pediatrician PDF** (1 page): biomarkers + M-CHAT + CareKit adherence
- **Caregiver Dashboard**: 7-day trend, mood diary, medication log, milestones
- **Export**: FHIR R4 Observation + QuestionnaireResponse + DocumentReference
- **Sync**: SMART on FHIR to Epic/Cerner with PKCE-secure OAuth

---

## 5. Clinical Evidence & Differentiation

| Feature | Curv ASD Gaze | EarliPoint (FDA) | Paper M-CHAT | ADOS-2 (Gold Std) |
|---------|---------------|------------------|--------------|-------------------|
| **Setting** | Home (iPhone) | Clinic (tablet) | Paper/portal | Specialist clinic |
| **Time** | 30–60 sec | 12 min | 5–10 min | 45–60 min |
| **Objectivity** | 6 engagement indices | 12 biomarkers | Binary caregiver report | Clinician judgment |
| **Age Range** | 12–60 months | 16–30 months | 16–30 months | 12+ months |
| **Cost/Session** | ~$0 (marginal) | ~$300 | ~$50 | ~$2,000 |
| **EHR Integration** | FHIR R4 + SMART | Proprietary | Manual entry | Manual entry |
| **Intended Use** | Observation diary / Research export | De Novo cleared | N/A | N/A |

**Unique moat**: Only solution combining **TrueDepth 3D gaze + validated stimuli + FHIR interoperability + zero-video privacy** on consumer hardware.

---

## 6. Data Privacy & Ethics — Built In, Not Bolted On

| Principle | Implementation |
|-----------|----------------|
| **Zero Raw Video** | Frames processed in volatile RAM; only 60 Hz numerical telemetry encrypted (AES-GCM-256, Keychain) |
| **De-Identified by Default** | Study token (`child-<uuid>`); no name/DOB/PHI in research vault |
| **Caregiver Control** | One-tap consent withdrawal; local delete; no forced cloud sync |
| **Consent Architecture** | COPPA-aligned consent design; ResearchKit flow ready for institutional IRB |
| **Audit Trail** | Cryptographic session hash (`payloadSha256`) + immutable local log |

---

## 7. Business Model & Go-to-Market

### Revenue Streams
| Stream | Model | Year 1 Target |
|--------|-------|---------------|
| **B2B2C: Health Systems** | Per-screening license + EHR integration fee | 5 pilot sites |
| **B2B: Pharma / CRO** | Digital biomarker endpoint for trials | 2 partnerships |
| **B2C: Direct-to-Caregiver** | Freemium (screening free; longitudinal dashboard $9.99/mo) | 10k families |
| **Data Licensing (opt-in)** | De-identified cohort analytics for OHDSI/OMOP researchers | Year 2 |

### GTM Sequence
1. **IRB Pilots** (Q4 2026): Duke, Stanford, CHOP — 200 families, clinical validation
2. **EHR Pilots** (Q1 2027): Epic Community Connect + Cerner — SMART on FHIR integration
3. **Payer Conversations** (Q2 2027): CPT 96116/96127 reimbursement pathway
4. **National Launch** (H2 2027): App Store + provider directory

---

## 8. Traction & Milestones

| Milestone | Status | Evidence |
|-----------|--------|----------|
| Core perception engine (C++20, OpenCV) | ✅ Shipped | `core/`, 100% test coverage |
| iOS TrueDepth gaze pipeline (ARKit, 60 Hz) | ✅ Shipped | `domains/asd_screening/ios/` |
| Stimulus library (GeoPref, Face Scanning, Oculomotor) | ✅ Shipped | 8 clinical clips, dynamic AOIs |
| Onboarding + progressive CDN download | ✅ Shipped | 4-step flow, English with Hindi & Nepali localization in progress |
| M-CHAT-R/F engine + Follow-Up flag | ✅ Shipped | 20 items, official polarity |
| CareKit + ResearchKit longitudinal tracking | ✅ Shipped | Tasks, diary, milestones, sync |
| **FHIR R4 + SMART on FHIR (PKCE)** | ✅ **Just completed** | Observation, QuestionnaireResponse, DocumentReference |
| Doctor PDF report generator | ✅ Shipped | 1-page clinical summary |
| **Test Suite** | ✅ **72/72 passing** | Unit + integration + instrument validation |

---

## 9. Team & Advisors

| Role | Name | Background |
|------|------|------------|
| Founder / CTO | **Bishal Ghimire** | C++ perception systems, OpenCV, ARKit, FHIR |
| Clinical Advisor | **TBD** | Pediatric neurodevelopment |
| Health Systems Advisor | **TBD** | EHR integration & health system pilots |

---

## 10. The Ask

| Round | Amount | Use of Funds | Valuation Cap |
|-------|--------|--------------|---------------|
| **Pre-Seed** | **$750K** | EHR integration pilots, clinical research partnerships, 2 senior hires (iOS, Full-Stack) | **$8M SAFE** |

**Runway**: 18 months to Series A milestones:
- 3 health system sandbox pilots signed
- 5,000 screened families across pilot sites
- $500K ARR

### Aspirational Milestones (2027+, Subject to Partner Validation)
- Multi-site clinical accuracy study with academic partner
- FDA Pre-Sub / De Novo submission package
- SOC 2 Type II audit

---

## 11. Appendix: Technical Deep Dive (For Technical Reviewers)

### Architecture
```
┌────────────────────────────────────────────────────────────┐
│  iOS App (Swift 6, SwiftUI)                                │
│  ├── GazeSessionController  →  ARFaceAnchor @ 60 Hz        │
│  ├── ScreenRaycaster        →  3D gaze → screen AOIs       │
│  ├── GazeProcessingActor    →  OneEuro filter, I-VT fix/sac │
│  ├── StimulusAOITracker     →  Dynamic AOI dwell scoring   │
│  ├── BiomarkerClassificationEngine → ANE inference         │
│  ├── SessionVaultManager    →  AES-GCM-256 encrypted store │
│  ├── CarePlanStore          →  CareKit tasks/diary/meds    │
│  ├── ResearchKitStudyStore  →  ORKTaskResult + consent     │
│  ├── SmartOnFhirAuthService →  OAuth 2.0 PKCE + Keychain   │
│  ├── FHIRInteroperabilityMapper → R4 Observation/QR/DocRef │
│  └── ClinicalInteroperabilityStandards → LOINC/SNOMED/ICD  │
├────────────────────────────────────────────────────────────┤
│  C++ Core (CMake, C++20)                                   │
│  ├── Steger sub-pixel ridge extraction                     │
│  ├── Temporal: I-VT, I-DT, DTW, 1-Euro filter             │
│  └── Evidence schema (immutable, content-addressable)      │
└────────────────────────────────────────────────────────────┘
```

### Key Repositories
- **Monorepo**: `github.com/bishalg/vision-perception` (private)
- **Mobile Overlay Kit** (shared iOS/Android): `apps/mobile_overlay/`
- **Palm Domain** (separate product line): `domains/palm/`

### Compliance & Privacy Checklist
- [x] Zero raw video storage
- [x] AES-GCM-256 + Keychain encryption
- [x] Cryptographic session audit trail (`payloadSha256`)
- [x] FHIR R4 + USCDI v3/v4/v5 alignment
- [x] SMART on FHIR PKCE (no client secret)
- [x] COPPA-aligned consent flow architecture
- [ ] FDA Q-Sub / pre-submission meeting (Aspirational 2027)
- [ ] SOC 2 Type II (Aspirational 2027)

---

## 12. Contact & Next Steps

**Bishal Ghimire**  
Founder & CTO, Curv  
📧 bishal@curv.engineering  
📱 +1 (xxx) xxx-xxxx  
🔗 github.com/bishalg/vision-perception (private — access on request)

**Next step**: 30-min demo + Q&A → IRB pilot design → LOI for pilot site

---

*Curv: Helper, not oracle. The app proposes, the clinician decides, the child benefits.*