# ADR-0014: Open-source readiness and healthcare-standards alignment

Status: proposed, 2026-11-09 (owner direction: "we are planning to make it all open source soon; follow the
best practices and standards already set for healthcare, and design it using existing standards and
open-source tools as much as possible"). Decisions marked **owner** need the owner's answer before the
package starts; everything else has a recommended default and can proceed.

Related: ADR-0004 (claims), ADR-0005 (secrets and identity), ADR-0008 (clinician workspace and interop),
ADR-0009 (security operations), ADR-0011 (EMR-lite, deployment), ADR-0013 (three audiences, one Rust core),
`docs/security/*`, `docs/roadmap_2026q4/03_asd_admin_panel_spec.md` §3a (console navigation, WP-C.23).

## Context

What already follows a standard (keep, and say so in the public README):

| Area | Standard or tool in use | Where |
| --- | --- | --- |
| Clinical interop | FHIR R4 `Bundle`, `Provenance`, `Observation`; HL7 v2 ORU^R01 file; LOINC and SNOMED codings with a verified code table | `rust/crates/asd-export/{fhir,hl7v2,codes.rs}`, `asd-contract::care` |
| De-identification | HIPAA Safe Harbor style de-id, small-cell suppression (n ≥ 12), pseudonym salts per export | `asd-export::deid`, `asd-core::analysis` |
| Consent | Versioned consent document with SHA-256, re-consent on change; consent policy table drives exports | iOS `ConsentDocument`, `asd-export::policy` |
| Security engineering | Threat model, DPIA-lite, HIPAA safeguards and seven written policies; append-only audit; RBAC with claims; mandatory TOTP; Ed25519 + JCS signed config bundles; CSP with nonces; `cargo-deny` licence and advisory policy | `docs/security/`, `asd-backend`, `rust/deny.toml` |
| Contracts | One Rust source of truth exported as JSON Schema (`schemars`); clients checked against it | `asd-contract`, `docs/spec/schemas/asd/` |
| Privacy by design | Camera frames never stored; encryption at rest on device (CryptoKit) and per-org DEK on the server; retention sweeps; erasure ledger | iOS vault, `asd-backend::jobs` |
| Accessibility | Text always accompanies colour; keyboard-driven review queue; `aria-current`, focus styles | console `globals.css`, components |
| Deployment | One-command clinic server with TLS, encrypted backups, restore drill | `deploy/clinic-server/` |
| Honest claims | Claims denylist and gate over Swift, prose, metadata and review notes | `scripts/check_asd_claims.py` |

What is missing for an open-source release, or home-grown where a standard exists:

- The repository `LICENSE` is proprietary while the Rust workspace declares `Apache-2.0` and the console
  `UNLICENSED`. A committed password sits in git history (ADR-0005 H1: no rewrite yet).
- No `SECURITY.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `CITATION.cff`, `CODEOWNERS`, Dependabot, SBOM,
  release signing or provenance. `cargo-deny` runs, but no `cargo audit`, `npm audit`, secret scanning or CodeQL.
- Login is a home-grown password + TOTP flow; there is no OpenID Connect option for clinics that already run
  an identity provider.
- The audit log is a custom shape (`AuditEntry`); the guardian consent is not exported as a FHIR `Consent`.
- The console UI is hand-rolled CSS with no accessibility test automation; WP-C.23 adds a shell by hand.
- No OpenTelemetry export; `tracing` only.
- SNOMED CT use in a distributed open-source product needs a licence position (LOINC is free with attribution).

## Decisions

### A. Licensing and repository hygiene

| # | Decision | Default / owner |
| --- | --- | --- |
| O-1 | Code licence **Apache-2.0** (patent grant; same as HAPI FHIR, Medplum, Open Health Stack). Documentation **CC-BY-4.0**. Stimulus clips, fonts and any third-party media are **excluded** and listed in `THIRD_PARTY_NOTICES.md` with their own terms (H-8). The repository `LICENSE` file and the console `package.json` change to match the Rust workspace. | **owner** confirms Apache-2.0 (alternative: MPL-2.0 as OpenMRS uses; AGPL-3.0 if the owner wants a network copyleft) |
| O-2 | Publish from a **fresh-history repository** (or run the documented `git filter-repo` once and force-push before the first public push): the password in history (ADR-0005 H1) must not reach the public remote. The rotation is confirmed first. | **owner** picks fresh-history vs rewrite |
| O-3 | Community files at the repo root: `SECURITY.md` (coordinated disclosure, 90-day window, contact mailbox, no bug bounty), `CONTRIBUTING.md` (DCO sign-off, gates to run, spec-before-code rule, claims rule), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `CITATION.cff`, `CODEOWNERS`, issue and PR templates, `GOVERNANCE.md` (single maintainer, decisions by ADR). | default |
| O-4 | Supply chain: Dependabot (cargo, npm, swift, actions); `cargo audit` and `npm audit --omit=dev` in CI; `gitleaks` in CI with the existing `check_asd_claims.py --secrets` kept; CodeQL for JS/TS; CycloneDX SBOMs (`cargo cyclonedx`, `npm sbom`) attached to releases; container images and release archives signed with Sigstore `cosign` keyless; SLSA provenance via GitHub artifact attestations; `cargo-about` and `license-checker` generate `THIRD_PARTY_NOTICES.md`; REUSE lint (`reuse lint`) for SPDX coverage. | default |
| O-5 | Regulatory position statement `docs/regulatory/position.md` for public readers: not a medical device, no diagnostic claim (ADR-0004), what would change if that ever changed (IEC 62304, ISO 14971, FDA/MDR), and the claims gate as the enforcement. Add a GDPR and Nepal Individual Privacy Act 2075 mapping to `docs/security/dpia-lite.md`. | legal review by **owner** |

### B. Healthcare interoperability

| # | Decision | Default / owner |
| --- | --- | --- |
| O-6 | Guardian consent is exported as a **FHIR R4 `Consent`** resource (scope `patient-privacy`, category `IDSCL`/research-excluded, `provision` for clinic review, `sourceAttachment` = consent text hash) inside the session bundle and the case package; the ingest contract carries `consent.{document_version, sha256, language}` (already planned as schema v1.9 in WP-S.6). | default |
| O-7 | Audit log export as **FHIR R4 `AuditEvent`** (IHE ATNA / DICOM audit vocabulary for `type`/`subtype`, `agent`, `entity`), one resource per `AuditEntry`, via `GET /audit?format=fhir` and in the case package. The storage shape stays. | default |
| O-8 | Terminology: keep LOINC (attribution in `THIRD_PARTY_NOTICES.md`). SNOMED CT codes stay only where the clinic's country has a licence or an affiliate licence is obtained; the code table marks each SNOMED code `licence_required: true` and exports can switch them off per org (`codes.snomed_enabled`). | **owner / legal**: SNOMED position for Nepal (not a member) and the US (member) |
| O-9 | Device enrolment and sync stay on the project's own REST contract (documented by JSON Schema and OpenAPI generated from `asd-contract`), because no FHIR resource models gaze sessions. The OpenAPI document is published with the schemas. | default |
| O-10 | Nepal reporting and EMR bridges are roadmap, not commitments: DHIS2 aggregate export (ADX or Web API) and OpenMRS FHIR2 import of the session bundle are evaluated with a partner clinic (ADR-0011 D1 stands: build when a clinic asks). | default |

### C. Identity, security operations, observability

| # | Decision | Default / owner |
| --- | --- | --- |
| O-11 | **OpenID Connect** as an optional login method (`openidconnect` crate, authorization code + PKCE), mapped to the existing roles by claim; local password + TOTP stays for clinics without an IdP. Documented against Keycloak as the reference open-source IdP (`deploy/clinic-server` optional service). Passkeys (WebAuthn) as a later second factor. | default |
| O-12 | OWASP ASVS 4.0 Level 2 self-assessment published at `docs/security/asvs-l2.md`, mapped to the threat model; gaps become work packages. | default |
| O-13 | OpenTelemetry: `tracing-opentelemetry` + OTLP exporter behind a feature flag and env var; redaction layer kept; console uses `@vercel/otel` or the OTel JS SDK only if a clinic asks. | default |

### D. Console UI on an established design system

| # | Decision | Default / owner |
| --- | --- | --- |
| O-14 | Adopt **IBM Carbon Design System** (`@carbon/react`, Apache-2.0) for the console: UI Shell (Header with the five divisions, SideNav, HeaderGlobalBar for the account menu) exactly matches the WP-C.23 layout; `DataTable`, `Form`, `Notification`, `Modal`, `Tabs`; WCAG 2.1 AA tested; theme tokens (white, g10, g90, g100) replace the hand-rolled `--bg/--surface/...` tokens; Devanagari font stays self-hosted. Migration is incremental: shell first (replacing `src/components/shell/` from WP-C.23), then tables and forms, then page by page. | **owner** confirms Carbon (alternatives: NHS design system, MIT, public-facing patterns and NHS branding; GOV.UK Design System; Material Web; or keep hand-rolled tokens and add only Radix primitives) |
| O-15 | Accessibility automation: `@axe-core/playwright` on every e2e page visit, failing on WCAG 2.1 AA violations; target **WCAG 2.2 AA** documented in `docs/accessibility.md` with the manual checks (keyboard, screen reader, zoom 200 %, contrast). | default |

## Work packages

| WP | Scope | Tier | Depends on |
| --- | --- | --- | --- |
| WP-O.1 | O-1, O-3: licence files, `THIRD_PARTY_NOTICES.md` generation, community files, templates, `GOVERNANCE.md`; README "standards we follow" section | impl (Sonnet) | owner O-1 |
| WP-O.2 | O-4: Dependabot, `cargo audit`, `npm audit`, gitleaks, CodeQL, CycloneDX SBOM, cosign signing and attestations in the release workflow, REUSE lint | impl (Sonnet) | — |
| WP-O.3 | O-2: history rewrite or fresh-history publish runbook, rotation check, public-remote checklist | owner + impl | owner O-2 |
| WP-O.4 | O-6, O-7: FHIR `Consent` and `AuditEvent` in `asd-export` + backend endpoints, contract v1.9 `consent`, goldens | senior (Opus) | WP-S.6 contract |
| WP-O.5 | O-8, O-9: terminology licence flags, OpenAPI from `asd-contract`, published with the schemas | impl (Sonnet) | owner O-8 |
| WP-O.6 | O-14 spike: Carbon UI Shell replacing `src/components/shell/`, one data table page migrated, bundle size and CSP check, decision record | senior (Opus) | WP-C.23, owner O-14 |
| WP-O.7 | O-15: axe in e2e, `docs/accessibility.md`, fixes | impl (Sonnet) | WP-C.23 |
| WP-O.8 | O-11: OIDC login option + Keycloak reference compose | senior (Opus) | — |
| WP-O.9 | O-12, O-13, O-5: ASVS L2 self-assessment, OTel exporter, regulatory position statement, DPIA mapping | impl (Sonnet) + owner review | — |

Order: WP-O.2 and WP-O.7 can start now; WP-O.1 and WP-O.3 after O-1/O-2; WP-O.6 after WP-C.23 lands; the
rest in phase order. None of this blocks the App Store submission.

## Consequences

- Public readers see a project that names its standards and proves them with gates, instead of claiming them.
- A design-system migration touches every console page over time; WP-C.23 keeps the shell swappable so the
  first step is cheap.
- OIDC and FHIR exports add surface area; both are optional per deployment and covered by the existing RBAC
  and audit.
- Licence and history decisions gate the public push; nothing else does.
