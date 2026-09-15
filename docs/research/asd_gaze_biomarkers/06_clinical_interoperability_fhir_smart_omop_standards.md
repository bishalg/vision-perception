# Clinical Interoperability, SMART on FHIR, OMOP CDM & Global Health Data Standards for Pediatric Digital Phenotyping

> **Document ID**: `DOC-RES-ASD-006`  
> **Target Applications**: `apps/ios_asd_gaze_app` (ResearchKit & CareKit Modules) & `domains/asd_screening`  
> **Subject**: Health Interoperability Frameworks, SMART on FHIR, OMOP Common Data Model, ePRO/eCOA Taxonomy, and Semantic Coding Standards (LOINC, SNOMED CT, ICD-10/11, USCDI)  
> **Scope**: Bridging Mobile Oculomotor Telemetry & Caregiver Logs into Electronic Health Records (EHR) and Global Research Registries

---

## 1. Executive Summary & The Interoperability Imperative

When developing mobile health applications for pediatric neurodevelopment (Autism Spectrum Disorder and ADHD), capturing high-fidelity sensor telemetry is only the first step. For an app to transition from an isolated consumer novelty into a **clinically actionable medical tool** and a **globally recognized research instrument**, its data must be seamlessly understood by:

1. **Hospital Electronic Health Records (EHRs)** such as Epic Systems, Oracle Cerner, and MEDITECH so that pediatricians can review eye-tracking biomarkers directly within their clinical workflow.
2. **Academic & Global Research Networks** such as the Observational Health Data Sciences and Informatics (OHDSI) consortium, clinical trial registries, and National Institutes of Health (NIH) data repositories.
3. **Regulatory Bodies (FDA, EMA)** requiring standard audit trails, 21 CFR Part 11 compliance, and Real-World Evidence (RWE) validation.

To achieve this, the platform must implement a layered interoperability architecture:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                     Global Healthcare Interoperability Stack for Mobile ASD/ADHD                │
├────────────────────────────────┬────────────────────────────────┬───────────────────────────────┤
│ Layer                          │ Standard Framework             │ Role in `apps/ios_asd_gaze_app`│
├────────────────────────────────┼────────────────────────────────┼───────────────────────────────┤
│ 1. Application Launch & Auth   │ SMART on FHIR, OAuth 2.0, OIDC │ Secure EHR login & clinician  │
│                                │ PKCE (RFC 7636)                │ app launch from Epic / Cerner │
├────────────────────────────────┼────────────────────────────────┼───────────────────────────────┤
│ 2. Data Exchange Envelope      │ HL7 FHIR Release 4 (R4)        │ Standard JSON representations │
│                                │ USCDI v3 / v4 Baselines        │ of Patients, Observations,    │
│                                │                                │ Consents & Questionnaires     │
├────────────────────────────────┼────────────────────────────────┼───────────────────────────────┤
│ 3. Semantic Vocabularies       │ • LOINC                        │ Codes M-CHAT-R/F & gaze tests │
│    (Clinical Dictionaries)     │ • SNOMED CT                    │ Codes behaviors (stimming, etc)│
│                                │ • ICD-10-CM / ICD-11           │ Diagnostic tags (ASD, ADHD)   │
├────────────────────────────────┼────────────────────────────────┼───────────────────────────────┤
│ 4. Clinical Outcome Taxonomy   │ ePRO, eCOA, ObsRO, PerfO       │ Regulatory categorization of  │
│                                │ Digital Biomarker Definitions  │ diaries, surveys & gaze data  │
├────────────────────────────────┼────────────────────────────────┼───────────────────────────────┤
│ 5. Big Data & Aggregation      │ OMOP Common Data Model (CDM)   │ Harmonizes multi-site trials  │
│                                │ RWD ➔ RWE Pipeline (FDA DHT)   │ for epidemiology & analytics  │
└────────────────────────────────┴────────────────────────────────┴───────────────────────────────┘
```

---

## 2. Integration & Connectivity: SMART on FHIR, OAuth 2.0 & OIDC

### 2.1 What is SMART on FHIR?
**SMART** (*Substitutable Medical Applications, Reusable Technologies*) is an open, standards-based API specification that sits on top of **HL7 FHIR** (*Fast Healthcare Interoperability Resources*).

- **Analogy**: If FHIR is the vocabulary and grammar of healthcare data, SMART is the **identity badge, security guard, and application container**.
- **Clinical Utility**: It allows our iOS application to run either as a standalone mobile app connected to a hospital patient portal (Patient-Facing App) or as an embedded tab inside Epic Hyperspace or Cerner PowerChart (Clinician-Facing App). A pediatrician can "order" or review an eye-tracking screening without manually retyping patient names or copying numbers between screens.

### 2.2 Security Architecture: OAuth 2.0 with PKCE and OpenID Connect (OIDC)
Native mobile applications cannot securely conceal client secrets (as binaries can be reverse-engineered). Therefore, SMART on FHIR mandates the **OAuth 2.0 Authorization Code Flow with PKCE (Proof Key for Code Exchange, RFC 7636)**:

```
┌──────────────┐                 ┌──────────────┐                 ┌──────────────┐
│ iOS ASD App  │                 │ EHR Auth Svr │                 │ FHIR Gateway │
│ (ResearchKit)│                 │(Epic/Cerner) │                 │ (EHR Record) │
└──────┬───────┘                 └──────┬───────┘                 └──────┬───────┘
       │ 1. Discovery: /.well-known/smart-configuration                  │
       │────────────────────────────────────────────────────────────────►│
       │                                                                 │
       │ 2. Authorize Request (client_id, code_challenge, scopes)        │
       │────────────────────────────────►│                               │
       │ 3. Clinician / Caregiver logs in & approves access              │
       │ 4. Auth Code Redirect (via custom URI scheme or Universal Link) │
       │◄────────────────────────────────│                               │
       │                                                                 │
       │ 5. Token Exchange: POST /token (code + code_verifier)           │
       │────────────────────────────────►│                               │
       │ 6. Returns Access Token + Refresh Token + OIDC id_token         │
       │◄────────────────────────────────│                               │
       │                                                                 │
       │ 7. GET /Observation?patient=12345 (Bearer Access Token)         │
       │────────────────────────────────────────────────────────────────►│
       │ 8. Returns FHIR Patient Record & Clinical Observations          │
       │◄────────────────────────────────────────────────────────────────│
```

#### Standard SMART Scopes for the ASD App:
- `launch/patient`: Grants context for the specific toddler currently selected in the EHR.
- `patient/Patient.read`: Retrieves demographic baseline (DOB, sex, legal guardian).
- `patient/Observation.read`, `patient/Observation.write`: Reads prior developmental screeners; writes back the 16 objective digital biomarkers.
- `patient/QuestionnaireResponse.write`: Writes completed M-CHAT-R/F survey responses.
- `patient/DocumentReference.write`: Uploads the final pediatrician PDF summary generated by `DoctorReportGenerator.swift`.
- `offline_access`: Enables background token refresh without forcing caregivers to log in repeatedly.

---

## 3. Patient Tracking Taxonomy: ePRO, eCOA, and Digital Biomarkers

Regulatory agencies (FDA CDRH, CDER) and clinical research organizations classify mobile health data into strict methodological categories:

```
                       ┌─────────────────────────────────────────────────────────┐
                       │          Clinical Outcome Assessments (eCOA)            │
                       └────────────────────────────┬────────────────────────────┘
                                                    │
         ┌──────────────────────────┬───────────────┴──────────────┬──────────────────────────┐
         ▼                          ▼                              ▼                          ▼
┌──────────────────┐      ┌──────────────────┐           ┌──────────────────┐       ┌──────────────────┐
│      ePRO        │      │      ObsRO       │           │      ClinRO      │       │      PerfO       │
│ Patient-Reported │      │ Observer-Reported│           │ Clinician-Reported│      │ Performance-Based│
└──────────────────┘      └──────────────────┘           └──────────────────┘       └──────────────────┘
• Direct child     • Caregiver diary  • ADOS-2 scores             • Bubble-pop touch
  input (older       logs (mood, sleep, • Pediatrician clinical     precision & motor
  adolescents)       stimming episodes)   diagnostic report         reaction speed
                          • M-CHAT-R/F survey
```

| Term | Full Definition | Representation in `apps/ios_asd_gaze_app` | Regulatory Significance |
| :--- | :--- | :--- | :--- |
| **ePRO** | *Electronic Patient-Reported Outcome*: Measurement based on a report that comes directly from the patient without interpretation by a clinician. | Directly reported symptom scores in older verbal children or adolescent ADHD tracking modules. | High patient-centric validity; required for primary endpoints in subjective clinical trials. |
| **eCOA** | *Electronic Clinical Outcome Assessment*: The overarching umbrella covering ePRO, ObsRO, ClinRO, and PerfO. | The complete CareKit + ResearchKit tracking session bundle. | Must comply with 21 CFR Part 11 (audit logging, electronic signatures, data provenance). |
| **ObsRO** | *Observer-Reported Outcome*: Assessment based on observation by someone other than the patient (e.g., parent/caregiver). | **Caregiver Diary check-ins** (`CarePlanStore` / `CareKitClinicalStore`) and the **20-question M-CHAT-R/F screener**. | Standard of care for toddlers (12–36 months) who cannot self-report. |
| **PerfO** | *Performance Outcome*: Measurement based on a task performed by the patient according to instructions. | **The Gamified 5-Point Bubble-Popping minigame**: Measures finger tap velocity, motor latency, and spatial accuracy. | Objective behavioral performance; resistant to caregiver reporting bias. |
| **Digital Biomarker** | Objective, quantified physiological and behavioral data collected by digital sensing technologies. | **TrueDepth 60 Hz Oculomotor Stream**: Social Visual Engagement index, GeoPref ratio, saccadic latency, smile contagion, flat affect duration. | Evaluated under FDA Digital Health Technologies (DHT) framework for exploratory or secondary clinical trial endpoints. |

---

## 4. Semantic Vocabularies: Global Clinical Coding Standards

To ensure universal semantic interoperability, data fields cannot use proprietary keys (e.g., `"smile_score"` or `"stimming"`). They must be bound to globally standardized medical ontology concept identifiers:

### 4.1 LOINC (Logical Observation Identifiers Names and Codes)
Used for **standardized clinical observations, laboratory tests, and survey instruments**:

| Clinical Entity / Observation | LOINC Code | Long Formal Name | Code System URI |
| :--- | :--- | :--- | :--- |
| **M-CHAT-R Panel** | `80628-1` | *Modified Checklist for Autism in Toddlers, Revised [M-CHAT-R]* | `http://loinc.org` |
| **M-CHAT-R Question 1 (Gaze follow)** | `80629-9` | *If you point at something across the room, does your child look at it?* | `http://loinc.org` |
| **M-CHAT-R Question 7 (Show interest)**| `80635-6` | *Does your child point with one finger to show you something interesting?* | `http://loinc.org` |
| **M-CHAT-R Question 10 (Response to name)**| `80638-0` | *Does your child respond when you call his or her name?* | `http://loinc.org` |
| **M-CHAT-R Total Score** | `80649-7` | *M-CHAT-R total score [Score]* | `http://loinc.org` |
| **M-CHAT-R Risk Level** | `80650-5` | *M-CHAT-R risk level [Interpretation]* | `http://loinc.org` |
| **Visual Gaze Tracking Observation** | `93044-6` | *Gaze tracking assessment panel* | `http://loinc.org` |
| **Visual Reaction Time (Saccades)** | `8692-2` | *Reaction time - visual stimulus* | `http://loinc.org` |
| **Behavioral Affect Assessment** | `72514-3` | *Pain and behavioral observation score* | `http://loinc.org` |

### 4.2 SNOMED CT (Systematized Nomenclature of Medicine - Clinical Terms)
Used for **clinical findings, symptoms, behaviors, and anatomical/sensor procedures**:

| Clinical Finding / Behavior | SNOMED CT Code | Fully Specified Name (FSN) | Category |
| :--- | :--- | :--- | :--- |
| **Autism Spectrum Disorder** | `35919005` | *Autism spectrum disorder (disorder)* | Disorder |
| **ADHD** | `406506008` | *Attention deficit hyperactivity disorder (disorder)* | Disorder |
| **Abnormal Eye Contact** | `247540003` | *Abnormal eye contact (finding)* | Finding |
| **Poor Eye Contact** | `247541004` | *Poor eye contact (finding)* | Finding |
| **Impaired Joint Visual Attention** | `708151006` | *Impaired joint attention (finding)* | Finding |
| **Stereotypic Movement / Stimming** | `247547000` | *Motor stereotypy (finding)* | Finding |
| **Repetitive Behavior** | `247546009` | *Repetitive behavior (finding)* | Finding |
| **Flat Affect** | `247528003` | *Flat affect (finding)* | Finding |
| **Lack of Social Smile** | `247535008` | *Lack of social smile (finding)* | Finding |
| **Delayed Language Milestone** | `229721007` | *Language development delayed (finding)* | Finding |
| **Eye Tracking Procedure** | `713444003` | *Eye tracking examination (procedure)* | Procedure |

### 4.3 ICD-10-CM & ICD-11 (Diagnostic Classifications)
Used for official clinical diagnoses, billing, reimbursement, and national registries:

| Diagnosis Description | ICD-10-CM Code | ICD-11 Code | Clinical Notes |
| :--- | :--- | :--- | :--- |
| **Autistic Disorder / ASD** | `F84.0` | `6A02` | Subtypes in ICD-11 qualify language and intellectual status (`6A02.0` to `6A02.5`). |
| **Pervasive Developmental Disorder, Unspecified** | `F84.9` | `6A02.Z` | Common provisional diagnostic code prior to full multidisciplinary confirmation. |
| **ADHD, Combined Presentation** | `F90.2` | `6A05.2` | Both inattention and hyperactivity-impulsivity criteria met. |
| **ADHD, Predominantly Inattentive** | `F90.0` | `6A05.0` | Inattentive subtype; prominent oculomotor scanning disorganization. |
| **Expressive Language Disorder** | `F80.1` | `6A01.0` | High comorbidity with ASD-GeoPref subtype (eLife 2019). |

### 4.4 USCDI (United States Core Data for Interoperability)
The **USCDI** is a standardized set of health data classes established by the Office of the National Coordinator for Health Information Technology (ONC / ASTP). To be certified under ONC Health IT rules, healthcare systems must support exchanging these data classes:
- **USCDI v3/v4 Data Classes relevant to our app**:
  - `Patient Demographics`: Age, birth sex, race, ethnicity.
  - `Assessment and Plan of Treatment`: CareKit developmental goals and milestone tracking.
  - `Clinical Tests`: The 16 digital biomarkers categorized under Clinical Tests / Objective Diagnostics.
  - `Health Concerns & Problems`: SNOMED CT and ICD-10 codes for suspected developmental delay.
  - `Caregiver / Proxy`: Documenting caregiver consent and proxy report status.

---

## 5. Research & Big Data: OMOP Common Data Model & RWD/RWE Pipeline

### 5.1 Why OMOP CDM?
While FHIR is designed for **transactional exchange** (moving one patient's chart from an app to a hospital), it is poorly suited for **large-scale population epidemiology and machine learning**. 

The **OMOP CDM (Common Data Model)**, maintained by the OHDSI consortium:
- Formats heterogeneous multi-site electronic health records, clinical trials, and mobile device data into a **single unified relational schema**.
- Enables researchers worldwide to run identical analytics, machine learning classifiers, and epidemiological studies without sharing raw patient identifiers.

```
┌───────────────────────────┐                       ┌───────────────────────────┐
│   `apps/ios_asd_gaze_app` │                       │    OMOP CDM v5.4 Tables   │
│   (Local Mobile Data)     │                       │    (Research Data Lake)   │
├───────────────────────────┤                       ├───────────────────────────┤
│ ResearchStudyEnrollment   │──────────────────────►│ PERSON                    │
│ GazeSessionRecord         │──────────────────────►│ MEASUREMENT               │
│ CareKitCaregiverDiary     │──────────────────────►│ OBSERVATION               │
│ TrueDepth Sensor Spec     │──────────────────────►│ DEVICE_EXPOSURE           │
│ MedicationItem Log        │──────────────────────►│ DRUG_EXPOSURE             │
│ Diagnostic ICD-10 Tag     │──────────────────────►│ CONDITION_OCCURRENCE      │
└───────────────────────────┘                       └───────────────────────────┘
```

### 5.2 Table-by-Table Mapping for ASD Telemetry

#### 1. `MEASUREMENT` Table (Digital Biomarkers)
Every completed eye-tracking screening outputs rows in `MEASUREMENT`:
- `person_id`: Anonymized study participant integer.
- `measurement_concept_id`: Standard Concept ID mapped from LOINC (e.g., Concept `37048700` for Eye Tracking Observation).
- `measurement_date`, `measurement_datetime`: Session timestamp.
- `value_as_number`: Continuous metric value (e.g., `0.72` for Social Preference Score, `340.0` for First Eye Latency in milliseconds).
- `unit_concept_id`: Percent (`8554`), Milliseconds (`8555`), or Ratio (`8523`).
- `range_low`, `range_high`: Age-bracketed normative peer distribution boundaries.

#### 2. `OBSERVATION` Table (ePRO / ObsRO Surveys)
M-CHAT-R/F responses and caregiver evening diary logs populate `OBSERVATION`:
- `observation_concept_id`: M-CHAT-R question Concept ID mapped from LOINC `80629-9`.
- `value_as_concept_id`: Standard Concept `4188539` ("Yes") or `4188540` ("No").
- `qualifier_concept_id`: Pass or Fail flag.

#### 3. `DRUG_EXPOSURE` Table (Medication Adherence)
Logged medication doses from CareKit populate `DRUG_EXPOSURE`:
- Standard RxNorm codes for ADHD/ASD medications (e.g., RxNorm `316049` for Methylphenidate, `310798` for Guanfacine).
- Quantifies adherence rates alongside behavioral gaze tracking stability.

### 5.3 Turning Real-World Data (RWD) into Real-World Evidence (RWE)
Per FDA guidance (*Digital Health Technologies for Remote Data Acquisition in Clinical Investigations*, Dec 2023):
1. **Verification**: Proving the hardware (TrueDepth camera + ARKit raycaster) measures 3D eye gaze accurately against optical bench truth.
2. **Analytical Validation**: Proving the algorithmic biomarker calculation (`ScreenRaycaster.swift` + `1-Euro filter`) performs reliably across diverse home environments (lighting variations, head tilt, viewing distances 40–55 cm).
3. **Clinical Validation**: Demonstrating that mobile digital biomarkers correlate statistically with clinical outcome endpoints (ADOS-2, Mullen Scales, Vineland Adaptive Behavior Scales).
4. **Data Integrity (21 CFR Part 11)**: Cryptographic session hashing (`payloadSha256`), secure timestamping, and immutable audit logs ensure RWD collected from home iPhones/iPads can support formal FDA regulatory submissions.

---

## 6. FHIR R4 Resource Mapping: Generating Interoperable Payloads

To illustrate concrete interoperability, here is how our application's data translates into standard FHIR Release 4 JSON resources:

### 6.1 Digital Biomarker as a FHIR `Observation` Resource
```json
{
  "resourceType": "Observation",
  "id": "asd-biomarker-sve-session-12345",
  "status": "final",
  "category": [
    {
      "coding": [
        {
          "system": "http://terminology.hl7.org/CodeSystem/observation-category",
          "code": "exam",
          "display": "Exam"
        }
      ]
    }
  ],
  "code": {
    "coding": [
      {
        "system": "http://loinc.org",
        "code": "93044-6",
        "display": "Gaze tracking assessment panel"
      },
      {
        "system": "http://snomed.info/sct",
        "code": "713444003",
        "display": "Eye tracking examination (procedure)"
      }
    ],
    "text": "Pediatric Social Visual Engagement Gaze Screening"
  },
  "subject": {
    "reference": "Patient/STUDY-ASD-8A2F1B",
    "display": "Anonymous Study Participant"
  },
  "effectiveDateTime": "2026-09-15T09:30:00Z",
  "component": [
    {
      "code": {
        "coding": [
          {
            "system": "http://loinc.org",
            "code": "8692-2",
            "display": "First eye fixation latency"
          }
        ]
      },
      "valueQuantity": {
        "value": 310.0,
        "unit": "ms",
        "system": "http://unitsofmeasure.org",
        "code": "ms"
      }
    },
    {
      "code": {
        "text": "Geometric Fixation Ratio (Pierce GeoPref Rule)"
      },
      "valueQuantity": {
        "value": 0.28,
        "unit": "ratio",
        "system": "http://unitsofmeasure.org",
        "code": "1"
      },
      "interpretation": [
        {
          "coding": [
            {
              "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
              "code": "N",
              "display": "Normal"
            }
          ]
        }
      ]
    },
    {
      "code": {
        "coding": [
          {
            "system": "http://snomed.info/sct",
            "code": "247528003",
            "display": "Flat affect score"
          }
        ]
      },
      "valueQuantity": {
        "value": 0.15,
        "unit": "score",
        "system": "http://unitsofmeasure.org",
        "code": "1"
      }
    }
  ],
  "device": {
    "display": "Apple TrueDepth 3D Optical Sensor (CurvAsdGazeKit v1.4)"
  }
}
```

### 6.2 Standardized M-CHAT-R/F as a FHIR `QuestionnaireResponse`
```json
{
  "resourceType": "QuestionnaireResponse",
  "id": "mchat-rf-response-9988",
  "status": "completed",
  "questionnaire": "http://loinc.org/80628-1",
  "subject": {
    "reference": "Patient/STUDY-ASD-8A2F1B"
  },
  "authored": "2026-09-15T09:15:00Z",
  "item": [
    {
      "linkId": "1",
      "text": "If you point at something across the room, does your child look at it?",
      "answer": [{ "valueBoolean": true }]
    },
    {
      "linkId": "7",
      "text": "Does your child point with one finger to show you something interesting?",
      "answer": [{ "valueBoolean": true }]
    },
    {
      "linkId": "10",
      "text": "Does your child respond when you call his/her name?",
      "answer": [{ "valueBoolean": true }]
    }
  ]
}
```

---

## 7. Actionable Architectural Roadmap for `apps/ios_asd_gaze_app`

To make our current codebase fully compatible with these standards, we execute a targeted 3-stage integration:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                      Implementation Plan in `apps/ios_asd_gaze_app`                             │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ STAGE 1: Semantic Metadata Decoration (`ResearchKitStudyModels.swift`)                          │
│ • Annotate `MCHATQuestion` with standard LOINC codes (`80629-9` to `80648-9`).                  │
│ • Annotate `ORKActiveStepResult` with standard LOINC/SNOMED CT concept identifiers.             │
│ • Annotate `CarePlanTask` & CareKit entries with SNOMED CT behavioral finding tags.             │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ STAGE 2: FHIR R4 Bundle Serializer (`FHIRInteroperabilityMapper.swift`)                         │
│ • Leverage existing `apple/FHIRModels` Swift package already resolved in project.               │
│ • Add `.toFHIRObservation()` on `GazeSessionRecord`.                                            │
│ • Add `.toFHIRQuestionnaireResponse()` on `MCHATScoreResult`.                                   │
│ • Add `.toFHIRDocumentReference()` for generated Doctor Report PDF payloads.                   │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│ STAGE 3: SMART on FHIR Auth & EHR Ingestion Gateway (`SmartOnFhirAuthService.swift`)           │
│ • Implement OAuth 2.0 PKCE client targeting standard SMART App Launch endpoints.                │
│ • Store bearer tokens securely in iOS Keychain via Apple CryptoKit.                            │
│ • Provide "Connect Hospital EHR" button in CareKit Settings for direct Epic/Cerner sync.       │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Testing & Validation via Logica Sandbox (SMART on FHIR)

Before deploying against production hospital EHR environments (such as Epic Community Connect or Oracle Health / Cerner Millennium), the mobile client must be validated against an interactive SMART on FHIR sandbox.

### 8.1 Setting Up the Logica Sandbox (Formerly SMART Sandbox)
1. **Tenant Provisioning**:
   - Provision a cloud FHIR R4 sandbox instance at [Logica Health](https://sandbox.logicahealth.org/).
   - Populate synthetic pediatric patient personas (e.g., toddler profiles aged 16–36 months with baseline developmental milestones).
2. **Client Registration (OAuth 2.0 Dynamic or Pre-Registered)**:
   - **Client Type**: Public Native App (`iOS / Swift`).
   - **Redirect URI**: Custom URL scheme (e.g., `asd-gaze-app://oauth-callback`).
   - **Token Endpoint Auth Method**: `none` (PKCE `code_challenge_method = S256` required).
   - **Allowed Scopes**:
     ```text
     openid profile launch/patient patient/Observation.read patient/Observation.write patient/QuestionnaireResponse.write
     ```
3. **Validating Both SMART Launch Workflows**:
   - **EHR Launch**: Clinician launches the ASD screening tool from inside the EHR chart. The app receives a `launch` token parameter and exchanges it for access + `patient` context ID.
   - **Standalone Patient Launch**: Caregiver launches the app at home, taps "Connect Pediatric Clinic", selects the hospital provider, logs in via the patient portal, and authorizes data sharing.
4. **End-to-End Observation Ingestion**:
   - Once authorized, the app transmits the multi-component digital biomarker `Observation` payload (generated by `FHIRInteroperabilityMapper.makeFHIRObservation`).
   - Verify that the Logica FHIR server returns `HTTP 201 Created` with a permanent server resource ID, and inspect the rendered observation in the simulated EHR chart.

---

## 9. Future-Proofing for USCDI v5 (U.S. Core Data for Interoperability)

The Office of the National Coordinator for Health Information Technology (ONC) has published **USCDI Version 5 (v5)**, expanding on USCDI v4 to standardize critical clinical and social determinants of health data classes.

### 9.1 Key USCDI v5 Additions Relevant to ASD/ADHD Tracking

| USCDI v5 Data Class | USCDI v5 Data Element | Application Architecture Mapping |
| :--- | :--- | :--- |
| **Sex Parameters for Clinical Use (SPCU)** | *Sex Parameter for Clinical Use* | Distinguishes anatomical/biological sex parameters used in normative gaze modeling from administrative gender identity. |
| **Observations** | *Observation* (General Category) | Expands clinical observation classes beyond routine vitals to encompass continuous digital health technology (DHT) telemetry. |
| **Orders** | *Medical Order Status & Detail* | Enables digital prescriptions: pediatricians can order an "At-Home ASD Screen Battery" with a tracked order ID linked to the session. |
| **Laboratory / Clinical Tests** | *Test Values & Result Status* | Formalizes digital biomarker output as certified clinical test results alongside genetic and metabolic panels. |

### 9.2 Structural Alignment in `apps/ios_asd_gaze_app`
1. **Sex Parameter Distinction**:
   `ChildProfile.swift` maps administrative demographic sex cleanly while providing an SPCU field (`biologicalSexForNormativeBaseline`) for developmental gaze comparison against normative distributions.
2. **Prescription Order Linkage (`basedOn`)**:
   Our FHIR `Observation` payload generator already supports referencing an EHR `ServiceRequest` or `DeviceRequest` in `basedOn: [ { "reference": "ServiceRequest/ORD-987" } ]`, ensuring full alignment when USCDI v5 certified EHRs issue digital eye-tracking screening orders.

