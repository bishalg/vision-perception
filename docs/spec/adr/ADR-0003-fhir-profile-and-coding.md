# ADR-0003: FHIR R4 Bundle profile and coding strategy for parent observations

Status: Proposed, 2026-10-22
Spec ids: FHIR-1..7, PLC-3

## Context

`FHIRInteroperabilityMapper` (`apps/ios_asd_gaze_app/Sources/Models/FHIRInteroperabilityMapper.swift`)
emits a single loose `Observation` and a `QuestionnaireResponse`. Problems found:

- Bare `Patient/<participantId>` subject (`:172-175`) with no Patient resource, and
  the id is sometimes the child's name (ADR-0002).
- `status: final` unconditionally (`:153`), no `Provenance`, no `Device` resource,
  no caregiver `RelatedPerson`; nothing says the data came from a phone app.
- SNOMED CT `247540003` "Abnormal eye contact (finding)" is used as the code of a
  numeric ratio component (`:61-78`). A finding concept is not a measurement.
- First-look latency, a constant, is emitted as LOINC `8692-2` with an OMOP
  reference range (`:20-38`, `:243-253`).
- `questionnaire` is `http://loinc.org/80628-1` built by string concatenation
  (`:214`); invented `80628-1-<qId>` linkIds exist as a fallback (`:196`) although
  all 20 LOINC item codes are present in `ClinicalInteroperabilityStandards.swift:80-106`.
- Mood diary, milestones and medication logs, which are the core Open Spectrum
  Tracking observations, are not exported at all.
- OMOP concept ids `40769032/33/34` are unverified.

The Open Spectrum Tracking spec (`spec/modules/behavioral-pattern-detector/technical.md`)
asks for FHIR-aligned Observations with SNOMED/LOINC codes, `status`,
`effectiveDateTime` and `Patient/{id}` subjects; its `spec/fhir/` folder is empty, so
this ADR is the first concrete FHIR contract in the mission and is written to be
copied there.

## Decisions

1. Export unit is a `Bundle` of `type: collection` with `urn:uuid:` fullUrls and
   internal references. Contents: `Patient` (token id, age-in-months extension, no
   name or birth date), `RelatedPerson` (parent, optional name), `Device` (the app,
   with version), one `Provenance` per observation resource (agent = Device,
   activity CREATE, entity = vault file id), then the observations.
2. `Observation.status` is derived from capture source and quality: `final` for
   live optimal/acceptable, `preliminary` for degraded, nothing for rejected or
   simulated. Provenance, not status, carries "how it was recorded".
3. Codes: the panel keeps LOINC `93044-6` plus SNOMED `713444003` (procedure).
   Every numeric component uses a local CodeSystem under `FHIRNamespaces.local`
   (`<base>/CodeSystem/gaze-engagement`), because no standard code for "eye-region
   dwell ratio from a phone app" exists and pretending otherwise misleads a
   receiving system. SNOMED findings are never used as measurement codes.
4. Absent metrics are absent components; no `dataAbsentReason`, no defaults, no
   reference ranges until the project has validated norms.
5. M-CHAT-R/F: `QuestionnaireResponse.questionnaire` = canonical
   `http://loinc.org/q/80628-1`; items use the LOINC item codes as linkIds; total
   score and category go in a separate Observation (`80649-7`, `80650-5`).
6. Diary, milestones and medications map to `Observation` (category `survey`) and
   `MedicationStatement`, in a separate mapper with its own code file so the
   package can be built in parallel with the gaze mapper.
7. The namespace base is a single constant, defaulting to the existing
   `http://curv.org/fhir` (already used at `:108`), to be changed once ADR-0005 fixes
   the publishing organisation's domain. Changing it later is a one-line edit plus
   fixture updates.
8. Validation in tests is structural (resource type, required elements, reference
   resolution) with no new dependency; a full validator is a later CI concern.

## Alternatives rejected

- Use SNOMED/LOINC for everything by picking "closest" concepts. Rejected: the
  existing code shows where that leads (findings as numbers, invented codes).
- `Bundle.type = transaction`. Rejected: the app does not target a specific server;
  a collection is portable and can be transformed by whoever receives it.
- Emit `Composition`/document bundle. Deferred: useful for the PDF-equivalent but
  not required for review; can be added without changing the resources above.
- Keep OMOP export. Kept but minimised; it is used by the research pipeline only.

## Consequences

- `GazeSessionRecord.toFHIRObservation` is replaced by `FHIRBundleBuilder.bundle(for:)`;
  call sites in result view, PDF/export sheets and tests change (WP-3.1).
- `ClinicalInteroperabilityStandards.loincCode(forMCHATQuestionId:)` becomes
  non-optional; one test (`ClinicalInteroperabilityTests.testMCHATQuestionLOINCCodes`)
  already asserts the 20 codes and stays valid.
- Open Spectrum Tracking can adopt `FHIRBundleBuilder`'s shape as `spec/fhir/`
  content; the local CodeSystem will need to move to the mission's domain.
