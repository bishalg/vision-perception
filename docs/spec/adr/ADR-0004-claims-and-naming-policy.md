# ADR-0004: Non-diagnostic claims and naming policy (App Store 1.4.1)

Status: Proposed, 2026-10-22
Spec ids: CLM-1..6

## Context

App Store Review Guideline 1.4.1 rejects medical apps that could provide inaccurate
data or information, and requires that diagnostic-sounding features be backed by
regulatory clearance. The current listing and UI claim far more than the software
does:

- Name `EarlyGaze: Autism & ADHD Test` (`fastlane/metadata/en-US/name.txt`; Android
  title identical; Hindi and Nepali names say "Autism Screening").
- Release notes: "the first FDA-grade at-home autism screening tool on iPhone",
  "IRB Protocol #2026-088, COPPA/HIPAA aligned", "16 continuous digital biomarkers"
  (`release_notes.txt:3`, `:15-17`, `:7`).
- Keywords include `biomarker`, `screening`; promotional text "objective social
  visual biomarkers".
- In-app: "ensuring high diagnostic reliability" (`BiomarkerRiskStratificationView.swift:241`),
  "calibrated Bayesian risk probability score" (`:253`), a risk percentage with a
  "confidence" and the category "Elevated Risk - Specialist Evaluation Recommended"
  (`BiomarkerClassificationEngine.swift:7-11`) produced by a hand-weighted heuristic
  that consumes placeholder inputs (ADR-0001). M-CHAT categories are shown as bare
  "High Risk" chips (`ResearchKitStudyModels.swift:204-208` via `rawValue` in six views).
- The ethics protocol in the repo itself prescribes cautious, non-diagnostic copy
  (`domains/asd_screening/docs/ETHICS_PROTOCOL.md` section 4) which the UI ignores.

Meanwhile the kit already contains an appropriately worded interpretation layer,
`AttentionFlag` ("This is a screening signal, not a diagnosis."), that the UI does
not use as its primary surface.

## Decisions

1. The product is an observation diary with engagement metrics. Naming rule: the
   name, subtitle and keywords never contain "test", "screening", "autism", "ADHD",
   "diagnosis" or any condition name. The human maintainer picks the name (H2);
   until then the display name is "EarlyGaze Observation Diary".
2. A machine-checked denylist (`scripts/check_asd_claims.py`) is the single source
   of truth for forbidden terms across metadata (all locales, iOS and Android) and
   user-facing strings in Swift sources. It runs in CI and fails the build. Terms
   that are the official vocabulary of a licensed instrument are allowed only on
   lines marked `// claims-ok: M-CHAT-R/F official category`.
3. The ML "risk stratification" and its views, CareKit outcome fields and ResearchKit
   JSON block are Debug-only (`ASD_RESEARCH_CLASSIFIER`). The only interpretation a
   parent sees in Release is `AttentionFlag`.
4. M-CHAT-R/F results keep the instrument's official categories but are always
   presented as "M-CHAT-R/F: <category>" with the official next step, and the raw
   enum values are unchanged because they are persisted.
5. Mandatory sentences on every result surface, the PDF and the store description:
   "This is not a diagnostic tool." and "Part of Open Spectrum Tracking".
6. Primary category Health & Fitness unless the maintainer chooses Medical (H5).
   Either way the review notes describe an observation diary and name a real
   support contact of the organisation.
7. Words like "biomarker", "clinical", "ANE", "FACS", "Bayesian" are removed from
   parent-facing copy. Type names (`BiomarkerClassificationResult`, view names) may
   keep their identifiers; renaming identifiers is Phase 4 and optional.

## Alternatives rejected

- Keep the risk score but add a disclaimer. Rejected: a percentage with a colour
  and "specialist evaluation recommended" is medical advice regardless of footnotes,
  and the number is not validated.
- Rename "risk" to "attention index". Rejected: the heuristic's inputs include
  placeholders; relabelling does not make it a measurement.
- Rely on manual copy review. Rejected: three locales, two stores and ~25 views;
  the denylist script is cheap and catches regressions.

## Consequences

- Store metadata is rewritten in all locales (WP-1.5) and screenshots must be retaken
  after the UI changes (not in scope for agents; listed for the human).
- Several views shrink; `BiomarkerRiskStratificationView` is compiled out in Release.
- `CareKitClinicalStore.recordScreeningOutcome` loses two parameters in Release
  builds; the Debug variant keeps them under the flag.
