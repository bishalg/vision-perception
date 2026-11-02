# DPIA-lite: ASD gaze screening platform

Status: draft for the owner to complete and sign (WP-E.6), 2026-11-01. This is a structured privacy risk assessment, not legal advice. Lawful-basis and jurisdiction fields are **placeholders for the owner**.
Path shorthand: **BE/** = `rust/crates/asd-backend/`, **EXP/** = `rust/crates/asd-export/`, **IOS/** = `apps/ios_asd_gaze_app/Sources/`, **KIT/** = `domains/asd_screening/ios/Sources/CurvAsdGazeKit/`, **CON/** = `apps/curv_console/`.
Related: `threat-model.md`, `data-flow.md`, `retention-policy.md`, `aws-baa-checklist.md`.

## 1. What the system does

A parent or clinic uses an iPhone app to play short clips while the TrueDepth camera measures where the child looks. The app computes gaze metrics on the device and sends a **pseudonymous summary** to the backend, where clinic staff review it and may sign a clinical review. The output is a screening aid. Every human-readable export carries the sentence "This is not a diagnostic tool." (`EXP/src/watermark.rs`, ADR-0004).

Not stored anywhere: video, photos, child name, date of birth, address, phone (`docs/roadmap_2026q4/02_asd_backend_architecture.md` section 1; `docs/spec/adr/ADR-0008-clinician-workspace-and-interop.md` D4). The only identifier a clinic may add is a hospital MRN, opt-in per organisation.

## 2. Purposes

| # | Purpose | Who | Data | Code anchor |
|---|---|---|---|---|
| P1 | Screening result for the family or clinic | parent, clinician | D1, D6 | `BE/src/ingest/mod.rs` |
| P2 | Clinical review and sign-off, referral letters | clinician | D4, D3 (optional) | `BE/src/clinical/`, `BE/src/routes/notes.rs` |
| P3 | Delivery of results to a hospital system | org admin, clinician | D10 (identified) | `BE/src/jobs/deliver.rs`, `BE/src/interop/` |
| P4 | Research datasets and norm baselines | researcher | D6 de-identified | `EXP/src/dataset.rs`, `BE/src/jobs/norm_aggregate.rs` |
| P5 | Quality improvement | org staff | D6 de-identified | `EXP/src/policy.rs` |
| P6 | Security and accountability (audit log) | admin | D9 | `BE/src/audit.rs` |
| P7 | Care diary and reminders | parent | D7 | `BE/src/care/mod.rs` |

## 3. Lawful basis (OWNER DECISION O-2)

Fill in per purpose and per market. The code does not choose a basis; it only records `consent_version` and `consent_at` per participant (`BE/migrations/0001_initial.sql`).

| Purpose | Candidate basis (owner to confirm) | Jurisdiction notes to complete |
|---|---|---|
| P1 | Parental consent | `[owner]` |
| P2, P3 | Provision of health care by the clinic (clinic is controller) or consent | `[owner]` |
| P4, P5 | Explicit consent (research); IRB or ethics approval reference | `[owner: IRB id, institution, PI]` (the app refuses to show research enrolment without a real study configuration, `IOS/Models/ResearchStudyConfiguration.swift`) |
| P6 | Legitimate interest or legal obligation | `[owner]` |
| P7 | Parental consent | `[owner]` |

Roles to fix: who is **controller** and who is **processor** for each customer type (home user, clinic, research site). The vendor operates the system and, by design, cannot read clinical content (`BE/src/auth/rbac.rs`, RBAC-3), which supports a processor position for clinic data. OWNER DECISION O-2b.

Regimes the owner should assess (not assessed here): HIPAA (US, only if US covered entities are customers; see `aws-baa-checklist.md`), GDPR (EU/UK users), India DPDP Act (children's data, verifiable parental consent), Nepal. The pitch materials must not claim HIPAA compliance until a BAA and an external review exist (`docs/roadmap_2026q4/02_asd_backend_architecture.md` ADR-B8).

## 4. Data minimisation (what the design already does)

| Measure | Evidence |
|---|---|
| No names, DOB, address, phone; age is a bracket | `BE/migrations/0001_initial.sql` (`participants`), ADR-0008 D4 |
| Participant token is per session when the family is not enrolled, so sessions cannot be linked | `IOS/Models/GazeSessionRecord.swift` (`participantToken`) |
| On-device processing; no photos or video saved | `apps/ios_asd_gaze_app/project.yml` camera usage string |
| Frame-level data sealed on the device; server cannot read it | `KIT/SessionVaultManager.swift` |
| MRN opt-in (default `none`), encrypted, never in de-identified output | `EXP/src/deid.rs`, ADR-0008 D4 |
| Webhook bodies carry no clinical data | `BE/src/interop/webhook.rs` |
| De-identified exports: month-level dates, per-export pseudonyms, no notes, no signer names | `EXP/src/deid.rs` |
| Logs drop identifiers and clinical fields | `BE/src/telemetry.rs` |
| App privacy label "Data Not Collected" holds only while no research configuration is compiled in | `apps/ios_asd_gaze_app/Resources/PrivacyInfo.xcprivacy`, ADR-0002 D9 |

Open minimisation points: frame files are uploaded although nobody can read them (OWNER DECISION O-13); `sessions.summary` stores the whole on-device record including calibration and tracking-quality detail (`BE/src/ingest/derive.rs`); age bracket plus month-level dates plus measurements can be quasi-identifying in small cohorts (R8 in `threat-model.md`).

## 5. Consent model

### 5.1 On the device

- Capture is gated by onboarding consent: a `ConsentToken` can be minted only after the profile records consent (`IOS/Models/ChildProfile.swift`, ADR-0002 D1).
- Research uploads need a configured study with a real IRB id and an https URL; without it the sync service fails closed (ADR-0002 D5, `IOS/Models/ResearchStudyConfiguration.swift`).
- Revoking research consent purges queued payloads; a "delete all observation data" action empties the vault, history, queues and calibration (`IOS/Models/DataDeletionService.swift`, ADR-0002 D6).

### 5.2 On the server

| Rule | Effect | Where |
|---|---|---|
| Withdrawn participant | Uploads refused with `409 participant-withdrawn`; always excluded from exports | `BE/src/ingest/mod.rs`, `EXP/src/policy.rs` |
| Purpose `research` or `quality_improvement` | De-identified exports only; identified request refused `400 consent-purpose-mismatch` | `EXP/src/policy.rs`, `BE/src/routes/exports.rs` |
| Purpose `research` | A session is included only if `rawPayload.normCollection.consented` is `true`; absent is not consent | `EXP/src/policy.rs`, `BE/src/export_build/record.rs` |
| Purpose `clinical_care`, `patient_request` | Included in either mode, whatever the research flag says | `EXP/src/policy.rs` |
| Norm baselines | Only sessions with the same consent flag and a `norm_candidate` review item | `BE/src/jobs/norm_aggregate.rs` |

### 5.3 Findings on the consent chain

1. **No producer of `normCollection.consented` found in the iOS sources** (searched `apps/ios_asd_gaze_app/Sources` and the kit; only the local `normCollectionOptIn` setting exists). The server fails closed, so research exports and norm candidates will contain no sessions until the app emits the flag. Needs confirmation by the iOS owner.
2. **No client calls the device erasure route** (`POST /v1/participants/{token}/erasure`, `BE/src/routes/care.rs`); no match for "erasure" in the iOS sources. Today only an org owner or admin can file erasure from the console (`CON/src/app/[org]/privacy-actions.ts`). A parent using the app cannot trigger server-side erasure. The on-device delete-all removes local data only.
3. Consent text, version and storage of the signed consent (who consented: parent or guardian) are not stored beyond `consent_version` and `consent_at`. OWNER DECISION O-6.

## 6. Children's data considerations

| Topic | Current state | Owner action |
|---|---|---|
| Who consents | A parent or guardian operates the app; the code does not verify identity or relationship | O-6: define consent and age-of-majority handling per market (DPDP requires verifiable parental consent) |
| Age assurance | Release builds request the App Store declared age range (`IOS/Models/AgeAssuranceGate.swift`); this concerns the app user, not the child | Confirm intent |
| Sensitivity | Health-related observations about a child, with potential developmental labels | Treat as special-category data in every regime |
| No profiling or advertising | No analytics SDK or tracking domains declared (`apps/ios_asd_gaze_app/Resources/PrivacyInfo.xcprivacy`: `NSPrivacyTracking` false) | Keep it so; re-check on every SDK addition |
| Claims | Screening aid only, mandatory not-diagnostic sentence | Keep claims review (ADR-0004) |
| Child reaching adulthood | No rule exists | O-3: retention end for minors |
| Erasure by parent | Possible only through the clinic today (finding 2) | Build the app-side request or document the clinic route |

## 7. Retention

Summarised here, detailed in `retention-policy.md`. Default `retention_months = 24` per organisation; the nightly sweep deletes **only** frame blobs past retention, keeping summaries, clips, reviews and clinical records (`BE/src/jobs/retention.rs`). Export artifacts expire after 72 hours by default (`rust/crates/asd-contract/src/export.rs`). There is no automatic deletion of summaries, care events, participants, users or audit rows. Owner decisions O-3, O-4, O-5.

## 8. Erasure and the clinical hold

The erasure job deletes sessions, clips, reviews, labels, frame blobs, care events, draft letters, MRN mapping and unexpired export artifacts, and leaves a tombstone participant row (`BE/src/jobs/erasure.rs`). A participant with a signed clinical review, any clinical note or a final letter is **held**: those records, the sessions they document and the MRN mapping stay, and the request ends `held`.

**OWNER DECISION O-1 (ADR-0008 Q2, open):** medical-record retention rules (for example three years for outpatient records under Indian Medical Council guidance, quoted in ADR-0008) may outlive an erasure request. Proposed default in the ADR: erase sessions and measurements, and crypto-shred notes only after the org's `clinical.retention_years`. Per-record keys would be needed because the DEK is per org (`BE/src/crypto/keyring.rs`). Until decided, held requests stay open and the parent is not told the full data is gone.

Also outside the job: database backups and snapshots, copies already delivered to hospitals or downloaded by researchers.

## 9. Risks and mitigations

Likelihood (L) and severity (S): Low, Medium, High. Author's assessment for owner review; links point to `threat-model.md`.

| # | Risk to the child or family | L | S | Mitigation in place | Residual action |
|---|---|---|---|---|---|
| 1 | Re-identification of pseudonymous summaries by someone with DB access | Medium | High | Pseudonymous tokens, no names, per-session tokens when not enrolled, RBAC and tenant isolation | Encrypted RDS and private network (WP-B.7); R5 |
| 2 | Disclosure of MRN or clinical notes | Low | High | AES-256-GCM per org, vendor carve-out, audit of MRN reads | KEK custody and rotation (R1) |
| 3 | Account takeover of a clinician | Medium | High | Argon2id, rate limits, TOTP when enrolled, 15-minute JWT | Build TOTP enrolment and enforce it (R2) |
| 4 | Erasure request only partly honoured | High | Medium | Hold with explicit status and reason | Decide O-1; build parent-side request (finding 2) |
| 5 | Research data used without consent | Low | High | Consent filter in the pure export crate, DB check on datasets, withdrawn excluded | Confirm the app emits the consent flag (finding 1) |
| 6 | Export file leaks after delivery | Medium | High | 5-minute links, 72-hour expiry, watermark, audited, vendor cannot download | Recipient agreements (O-15); minimum cell size (R8) |
| 7 | Data in backups outlives policy | Medium | Medium | None yet | O-12 and `aws-baa-checklist.md` |
| 8 | Lost or stolen phone exposes sessions | Low | Medium | Vault encryption, Keychain `ThisDeviceOnly`, complete file protection, no backup | None |
| 9 | Over-collection through frame uploads | Medium | Medium | Frames sealed with device key | Decide O-13 |
| 10 | Secondary use by vendor staff | Low | High | `superadmin` denied clinical content and exports; no break-glass in v1 | O-14 |
| 11 | Undetected misuse | Medium | Medium | Append-only audit log, disclosure accounting per participant | Off-box audit copy (R9) |
| 12 | Data transferred to an unsuitable region | Medium | Medium | None (code default region is `us-east-1`, `BE/src/config.rs`) | O-7 |

## 10. Rights handling

| Right | Supported today | Gap |
|---|---|---|
| Access, portability | `patient_request` purpose exports (CSV, FHIR, XLSX) through the console; `GET /v1/participants/{id}/disclosures` for accounting | No self-service for parents |
| Rectification | Clinical amendment creates a new version, never edits a signed one (ADR-0008 D3) | None for device data |
| Erasure | Console-filed job with clinical hold | See section 8 |
| Withdrawal of consent | Sets `withdrawn_at`; blocks new uploads and exports | App-side trigger missing (finding 2) |
| Objection to research use | Per-session consent flag | Producer missing (finding 1) |

## 11. Owner decisions (consolidated list)

| ID | Decision | Where it bites |
|---|---|---|
| O-1 | Retention versus erasure for signed clinical records (ADR-0008 Q2): hold, delay, or crypto-shred per record | `BE/src/jobs/erasure.rs`, `retention-policy.md` |
| O-2 | Lawful basis per purpose and market; controller or processor roles per customer type (O-2b) | section 3 |
| O-3 | Retention periods: default `retention_months` (24 today), whether summaries ever expire, care events, participants, minors reaching adulthood | `retention-policy.md` |
| O-4 | Export artifact lifetime default (72 h today, range 1 to 168 h) | `rust/crates/asd-contract/src/export.rs` |
| O-5 | Audit log retention and off-box copy | `threat-model.md` R9 |
| O-6 | Consent capture design: who consents, evidence stored, verifiable parental consent, consent text versioning | section 5.3, 6 |
| O-7 | Hosting region and data residency per market | `aws-baa-checklist.md` |
| O-8 | Whether HIPAA applies (US customers) and signing the AWS BAA; H-4 and H-11 in the tracker | `aws-baa-checklist.md` |
| O-9 | KEK custody (KMS design), rotation cadence, backup of the KEK | `threat-model.md` R1 |
| O-10 | Mandatory TOTP for which roles; console idle timeout and session length. TOTP part implemented, owner to confirm (WP-B.16, ADR-0010): required for `owner`, `admin`, `clinician`, `superadmin` after a 14-day grace period from first sight (constants in `asd-contract::auth_policy`). Idle timeout and session length still open | `threat-model.md` R2, R7; `rust/crates/asd-contract/src/auth_policy.rs` |
| O-11 | Privacy contact or DPO, breach notification process and timelines | not covered by code |
| O-12 | Backup retention and how erased data is handled in restores | `retention-policy.md` |
| O-13 | Whether the app should upload `frames.jsonl.enc` at all, given no one can decrypt it | section 4 |
| O-14 | Whether any vendor break-glass access to clinical content is ever allowed (ADR-0008 D2 says none in v1) | `BE/src/auth/rbac.rs` |
| O-15 | Data-processing terms with hospital and research recipients of exports | threat model 4.4 |

## 12. Sign-off

| Role | Name | Decision (accept, accept with conditions, reject) | Date | Signature |
|---|---|---|---|---|
| Owner / controller representative | | | | |
| Privacy contact or DPO | | | | |
| Security reviewer | | | | |
| Clinical lead (for P2 and the hold policy) | | | | |

Conditions attached to approval: `[to be filled in]`. Review again before: first clinical pilot, any new data class, any new recipient or region, and every schema major version.
