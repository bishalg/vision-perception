# Curv ASD Gaze Screening — One-Page Executive Summary

## The Hook
**Mobile autism observation diary and structured gaze engagement analysis on a parent's iPhone — 30 seconds, at home, zero video recorded, structured clinician export.**

---

## Problem
- **1 in 36** kids has autism (CDC 2023)
- **Average diagnosis: 4.5 years** — misses the 0–3 neuroplasticity window
- **Current tools**: subjective questionnaires (M-CHAT), clinic-only eye trackers ($20K+), or specialist evals ($2K+, 6-month waitlists)
- **Result**: 80% of toddlers never get screened; lifetime cost per missed case: **$2.4M**

---

## Solution
**Curv ASD Gaze Observation App** — uses the iPhone's TrueDepth camera (already in 500M+ devices) for structured gaze engagement capture.

| What It Does | How |
|--------------|-----|
| **Measures 6 engagement indices** in 30–60 sec | Social preference, eye dwell, saccadic latency, smile contagion, flat affect, joint attention |
| **Runs M-CHAT-R/F** (20-item caregiver survey) | Caregiver observation capture + Follow-Up scoring per protocol |
| **Generates clinician PDF + FHIR data** | 1-page report + Observation/QuestionnaireResponse/DocumentReference for Epic/Cerner |
| **Privacy-first** | Zero raw video — only encrypted numerical telemetry (AES-GCM-256, Keychain) |
| **At home, on consumer hardware** | No clinic visit, no $20K device, no specialist needed to administer |

---

## Validation & Differentiation
| | Curv | EarliPoint (FDA) | Paper M-CHAT | ADOS-2 |
|---|------|------------------|--------------|--------|
| **Setting** | Home (iPhone) | Clinic tablet | Paper/portal | Specialist |
| **Time** | 30–60 sec | 12 min | 5–10 min | 45–60 min |
| **Objectivity** | 6 continuous engagement indices | 12 biomarkers | Binary report | Clinician judgment |
| **Cost/Session** | ~$0 marginal | ~$300 | ~$50 | ~$2,000 |
| **EHR Integration** | **FHIR R4 + SMART** | Proprietary | Manual | Manual |

**Only solution combining**: TrueDepth 3D gaze + validated stimuli + FHIR interoperability + zero-video privacy on consumer hardware.

---

## Traction (Code Complete, Ready for Clinical Validation)
- ✅ **Core engine**: C++20 perception core, 100% test coverage
- ✅ **iOS pipeline**: ARKit 60 Hz gaze + OneEuro filter + dynamic AOIs
- ✅ **Stimuli**: GeoPref, Face Scanning, Oculomotor — 8 clinical clips
- ✅ **M-CHAT-R/F engine**: 20 items, official polarity, Follow-Up flag
- ✅ **CareKit + ResearchKit**: Longitudinal tracking, consent, sync
- ✅ **FHIR R4 + SMART on FHIR (PKCE)**: Observation, QuestionnaireResponse, DocumentReference
- ✅ **Doctor PDF generator**: 1-page clinical summary
- ✅ **72/72 tests passing** (unit + instrument validation)

---

## Business Model
| Stream | Model | Year 1 Target |
|--------|-------|---------------|
| Health Systems (B2B2C) | Per-screening license + EHR integration | 5 pilot sites |
| Pharma/CRO (B2B) | Digital biomarker endpoint for trials | 2 partnerships |
| Direct-to-Caregiver (B2C) | Freemium: screening free, dashboard $9.99/mo | 10k families |

---

## The Ask & Roadmap
**$750K Pre-Seed (SAFE, $8M cap)** → 18 months to Series A milestones:
- 3 health system contracts (Epic/Cerner sandbox pilots)
- 5,000 screened families across pilot sites
- $500K ARR

### Aspirational Milestones (2027+, Subject to Partner Validation)
- Formal clinical validation study with research partners
- FDA pre-sub / De Novo submission preparation
- Peer-reviewed multi-site diagnostic accuracy publication

---

## Team
- **Bishal Ghimire** (Founder/CTO): 10+ years C++ perception, OpenCV, ARKit, FHIR — built the entire stack
- **Advisory Focus**: Pediatric neurodevelopment and digital health interoperability
- **Technical Moat**: TrueDepth 3D gaze + validated stimuli + FHIR + zero-video privacy on consumer hardware

---

## Contact
**Bishal Ghimire** · bishal@curv.engineering · +1 (xxx) xxx-xxxx  
Private repo access on request: `github.com/bishalg/vision-perception`

---

*Curv: Helper, not oracle. The app proposes, the clinician decides, the child benefits.*