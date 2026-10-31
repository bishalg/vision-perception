# Specs (vision-perception)

Spec-driven development index for this repository, following the conventions of the
Open Spectrum Tracking monorepo (`docs/SPEC.md`, `spec/architecture/repo-standards.md`
there): specs come before implementation, a module contract is four files
(`technical.md`, `acceptance.md`, `qa-validation.md`, `non-technical.md`), and
hard-to-reverse decisions are recorded as ADRs.

This folder was created on 2026-10-22; earlier design documents in `docs/` and
`domains/asd_screening/docs/` remain authoritative for what they describe and are
referenced from the module specs rather than duplicated.

## Module specs

| Module | Technical | Acceptance | QA Validation | Non-Technical | Work packages |
| --- | --- | --- | --- | --- | --- |
| ASD gaze app, App Store readiness | [technical.md](modules/asd-gaze-app-store-readiness/technical.md) | [acceptance.md](modules/asd-gaze-app-store-readiness/acceptance.md) | [qa-validation.md](modules/asd-gaze-app-store-readiness/qa-validation.md) | [non-technical.md](modules/asd-gaze-app-store-readiness/non-technical.md) | [work-packages.md](modules/asd-gaze-app-store-readiness/work-packages.md) |
| ASD Rust core and backend | [technical.md](modules/asd-rust-core-and-backend/technical.md) | [acceptance.md](modules/asd-rust-core-and-backend/acceptance.md) | [qa-validation.md](modules/asd-rust-core-and-backend/qa-validation.md) | [non-technical.md](modules/asd-rust-core-and-backend/non-technical.md) | [work-packages.md](modules/asd-rust-core-and-backend/work-packages.md) |
| ASD gaze-tracking robustness | [technical.md](modules/asd-gaze-robustness/technical.md) | [acceptance.md](modules/asd-gaze-robustness/acceptance.md) | (tests per WP) | — | [work-packages.md](modules/asd-gaze-robustness/work-packages.md) |
| ASD clinician workspace and interop | [technical.md](modules/asd-clinician-and-interop/technical.md) | [acceptance.md](modules/asd-clinician-and-interop/acceptance.md) | [qa-validation.md](modules/asd-clinician-and-interop/qa-validation.md) | [non-technical.md](modules/asd-clinician-and-interop/non-technical.md) | [work-packages.md](modules/asd-clinician-and-interop/work-packages.md) |

## Architecture decision records

| ADR | Title | Status |
| --- | --- | --- |
| [ADR-0001](adr/ADR-0001-simulation-mode-policy.md) | Simulation mode is a build-time capability with mandatory data provenance | Proposed |
| [ADR-0002](adr/ADR-0002-consent-and-data-retention.md) | Consent-gated capture, on-device retention and deletion model | Proposed |
| [ADR-0003](adr/ADR-0003-fhir-profile-and-coding.md) | FHIR R4 Bundle profile and coding strategy for parent observations | Proposed |
| [ADR-0004](adr/ADR-0004-claims-and-naming-policy.md) | Non-diagnostic claims and naming policy (App Store 1.4.1) | Proposed |
| [ADR-0005](adr/ADR-0005-org-account-and-bundle-id.md) | Publishing organisation, bundle identifier and review contact | Accepted (H4 resolved 2026-10-29: `com.open-spectrum.eye-gaze`, App Name "Eye Gaze") |
| [ADR-0006](adr/ADR-0006-rust-core-and-backend.md) | Rust is the single implementation of ASD core logic, contracts and backend | Proposed (owner decision in `CLAUDE.md`, 2026-10-30) |
| [ADR-0007](adr/ADR-0007-gaze-tracking-robustness.md) | Gaze-tracking maths in Rust, per-session accuracy in degrees, bench-decided calibration model | Proposed (2026-10-31) |
| [ADR-0008](adr/ADR-0008-clinician-workspace-and-interop.md) | Clinician workspace inside the console and one Rust export bridge to hospital systems | Proposed (2026-10-31) |

## Rules

- A spec change precedes the code change that implements it.
- MUST statements carry stable identifiers (for example `SIM-3`) so tests and
  work packages can cite them.
- Every MUST statement in a module spec is testable; `qa-validation.md` names the
  test or command that proves it.
- ADR status moves from Proposed to Accepted only by a human maintainer.
