# ADR-0008: Clinician workspace inside the console and one Rust export bridge to hospital systems

Status: Proposed, 2026-10-31 (owner defaults accepted the same day: both regional HL7 v2 /
CSV / Excel hospitals and FHIR R4 / SMART EHRs; the doctor panel is a role in the console)
Spec ids: RBAC-*, KEY-*, MRN-*, CW-*, SIGN-*, NOTE-*, XC-*, CSV-*, XLSX-*, PDF-*, FHX-*,
HL7-*, EXJ-*, INTG-*, DLV-*, MIG-*, CTR-*, UI-* in
`docs/spec/modules/asd-clinician-and-interop/technical.md`

## Context

The ASD product records gaze-engagement sessions on a phone, scores them with the Rust core
and shows them in the Curv Console (`apps/curv_console`, Next.js, thin UI over the Rust
backend `rust/crates/asd-backend`). A clinic pilot needs two things that do not exist yet:

1. A place where a clinician reviews a session, records a judgement and signs it, with the
   record-keeping a hospital expects (notes, a letter, a printable summary, an MRN link).
2. A way to get results into the hospital's own systems. In Nepal and India that usually
   means a hospital information system that takes HL7 v2 files or spreadsheets. In
   Epic/Cerner-style EHRs it means FHIR R4 with SMART authorisation.

The product is an expert support system that plugs into those systems. It is not an EHR,
does not own diagnoses, and must keep ADR-0004's non-diagnostic posture.

What exists (verified at `1f43026`, details G1..G18 in the spec): RBAC with a `clinician`
role but no clinical actions, and `superadmin` allowed everything (G1). A review queue for
QA with a plaintext note column (G2). Free-JSON org settings copied into the append-only
audit log (G3). A job runner that runs each job in one transaction with no blob access
(G5). The iOS FHIR bundle builder (no DiagnosticReport; G6) and a DocumentReference
mapper with a random id and a wrong format code (G7). An unverified LOINC panel code (G8).
No staff display names (G9). WP-B.6 (care events), WP-B.8 (server PDF/FHIR) and WP-C.2
(console live mode, review queue) are being built in parallel.

## Decisions

1. **The doctor panel is the `clinician` role in the existing console.** There is no
   separate app, no new role and no new auth plane. Fourteen RBAC actions are added in one
   matrix (RBAC-1/2). The QA review queue (`review_items`, reviewers) and clinical review
   (`clinical_reviews`, clinicians) are different records with different purposes. The QA
   queue is not reused for sign-off.
2. **The vendor does not read clinical content.** `superadmin` is denied the clinical-content
   actions (RBAC-3), and `admin` (operations) cannot read notes or impressions. There is no
   break-glass path in v1. A hospital that needs one gets it as a separate audited feature
   later.
3. **Electronic sign-off is a hash-bound, step-up, append-only record.** A signature binds
   to `SHA-256(JCS(snapshot))` of exactly what the signer saw (SIGN-4), requires
   re-entering the password and TOTP if enrolled (SIGN-5), and is immutable through DB
   triggers (SIGN-7). Amendment creates a new version and never edits a signed one (SIGN-8).
   A second signature is an org policy (`none | red_flag | always`) and must come from a
   different user (SIGN-6). This mirrors the intent of 21 CFR Part 11 §11.50/11.70
   (signature manifestation, signature/record linking) without claiming compliance.
4. **No patient names are stored, ever (v1).** The hospital EHR is the system of record for
   identity. The only identified datum is an optional hospital MRN per participant
   (`identity.patient_ref_mode = mrn`, default `none`), encrypted, with a blind index for
   exact lookup (MRN-1..6, KEY-4). HL7 `PID-5` is sent empty (or the pseudonymous token, per
   profile); the receiving system matches on `PID-3`. Storing an MRN makes those rows
   identifiable health data. That is why MRN mode is opt-in per org, encrypted, never in
   de-identified output, and audited on every read.
5. **Application-level envelope encryption, not pgcrypto.** AES-256-GCM in Rust with a
   per-org data key wrapped by a key-encryption key from the environment (KMS later), AAD
   bound to table/column/org/row (KEY-1..7). This supersedes `02` ADR-B8's "column-level
   pgcrypto" wording, because pgcrypto puts the key into SQL text, where statement logs and
   `pg_stat_statements` can capture it. The existing plaintext `review_items.note` moves to
   the same scheme (KEY-7).
6. **One Rust crate, `asd-export`, owns every outbound format.** That covers CSV (three
   dialects), XLSX, PDF/A-2b, FHIR R4 and HL7 v2.5.1. The crate is pure (no I/O, no async),
   so the backend uses it now and the iOS app can use it later through FFI (`pdf` feature
   off). If WP-B.8 lands its PDF/FHIR code in a separate crate or inside `asd-backend`,
   WP-I.1 folds it into `asd-export` so each format has one implementation (XC-2). The
   console renders nothing itself and only shows or downloads what the backend produced
   (UI-2), as `CLAUDE.md` requires.
7. **De-identification, consent eligibility and watermarking live in the crate, not in
   callers** (XC-4..6). The backend supplies facts. The crate decides inclusion and
   renders the watermark, so every format gets the same rules. Each artifact that leaves
   the system, including a single printed PDF, gets an `exports` row, an export id
   printed on it, and audit rows for creation, download and delivery (EXJ-6/7). This also
   gives an accounting of disclosures per participant.
8. **Coding: local codes first, standard codes only when verified.** This extends ADR-0003
   D3. Metrics keep the local `gaze-engagement` CodeSystem in FHIR and coding system `L`
   in HL7 OBX-3. Hospitals can map them to their own local codes per integration
   (`code_map`), which is how Nepali and Indian HIS installations usually work. Every
   standard code sits in one table with a `verified` flag. LOINC `93044-6` is unverified
   and not emitted until checked (XC-7, G8). The fixed set of codes needed by
   DocumentReference and M-CHAT is verified in WP-I.3.
9. **C-CDA: no.** It is a US (ONC) document standard. The target EHRs (Epic, Cerner/Oracle)
   accept FHIR R4 DocumentReference with a PDF and structured Observations, and the
   regional hospitals do not consume CDA at all. India's ABDM uses FHIR R4 profiles. A
   C-CDA generator would be a large, rarely exercised template system. If a US partner
   requires it, revisit with a new ADR.
10. **HL7 v2.5.1 ORU^R01 is delivered as files first, MLLP later.** Regional HIS
    integrations commonly accept SFTP or shared-folder drops. MLLP needs a VPN or an
    inbound port at the hospital and ACK handling. Files: WP-I.4 and WP-I.7. MLLP/TLS:
    WP-I.11, backlog.
11. **Delivery is pull-based for webhooks, push for SFTP, and outbox-driven.** Webhook
    bodies never carry clinical data. They carry a signed event and a short-lived download
    URL (DLV-2, Standard Webhooks signature scheme). SFTP uses a server-generated Ed25519
    key and a pinned host key (INTG-5). Auto-delivery on final sign is enqueued in the
    signing transaction (transactional outbox, DLV-6). Network I/O never runs inside a
    DB transaction, which requires a runner change (`run_after`, heartbeat, a job context
    with blob store and keys; G5).
12. **FHIR push before FHIR façade.** System-to-system push via SMART Backend Services
    (WP-I.9), then SMART EHR launch of the console (WP-I.10). WP-B.10 (we act as a FHIR
    server) stays backlog until a partner asks.
13. **Integration settings are their own table with write-only, encrypted secrets**
    (INTG-1/2). They never go into `organizations.settings`, whose snapshots are copied
    into the immutable audit log (G3).
14. **Migrations 0008 (clinical) and 0009 (exports/integrations) only**, 0010 reserved for
    SMART clients. 0006/0007 belong to the in-flight packages; an unused 0007 stays unused
    (MIG-1).

## Alternatives rejected

- **Separate doctor app.** It would duplicate auth, RBAC, session views and deployment, and
  clinicians would get a second login. The owner chose a role.
- **Reuse `review_items` for sign-off.** QA triage (capture quality, labels) and clinical
  judgement have different actors, lifecycles and legal weight. Mixing them would put
  clinical text into a table reviewers can write.
- **Store patient names encrypted behind a policy flag.** It turns the whole database into
  a named-patient record system, which changes data-protection obligations (HIPAA in the
  US, India's DPDP Act 2023, Nepal's Privacy Act 2018) for every deployment. The EHR
  beside the console already shows the name. Rejected for v1 and kept as owner question
  Q1 in case a hospital without an EHR insists.
- **No MRN at all; clinician types it at export time.** This is the least storage, but
  auto-delivery to an HIS after sign-off would be impossible, and so would SMART launch
  patient matching. Rejected. MRN stays opt-in instead.
- **pgcrypto column encryption.** Rejected for key exposure in SQL (D5).
- **Generate exports in TypeScript in the console (SheetJS, pdfmake).** This violates the
  Rust-core rule and would make formats differ between console downloads and automated
  deliveries.
- **Use a FHIR model crate (e.g. `fhir-sdk`).** It is large and would add a second set of
  resource definitions to keep in line with the Swift builder. Hand-built `serde_json`
  values with structural tests match ADR-0003 D8 and the iOS approach.
- **Use an HL7 v2 crate.** Available crates focus on parsing. The ORU^R01 encoder is a few
  hundred lines with golden files, and owning the escaping rules is safer than
  depending on a lightly maintained crate.
- **`ssh2` (libssh2) for SFTP.** It pulls a C library and OpenSSL. `russh`/`russh-sftp`
  (Apache-2.0, pure Rust) is preferred, subject to `cargo deny` in WP-I.7.
- **Webhooks carrying FHIR bundles.** That would push PHI to an endpoint whose security we
  cannot verify, retried by us, and logged by intermediaries. Rejected in favour of thin
  events plus pull.
- **Google Sheets API delivery.** It needs Google OAuth per org and sends PHI to a third
  party. The `csv_sheets` dialect covers manual import.

## Consequences

- `rbac::allows` changes semantics for `superadmin` (RBAC-3). Existing tests that assume
  superadmin can do everything must be checked. Operations staff need a non-clinical
  path to support clinics (they keep sessions and metrics read).
- A new secret `ASD_DATA_KEK` must be provisioned (compose `.env.example`, WP-B.7 Secrets
  Manager/KMS). Losing it loses all clinical text, so backup and rotation runbooks are
  owed by WP-E.6. Key rotation re-encryption is not built in v1.
- `users` gains `display_name` and `professional_registration`, and the console profile
  page must let users set them before they can sign.
- The job runner gains `run_after`, heartbeats and a job context (blob store, keyring,
  config). WP-I.6 owns that change to WP-B.5's runner.
- `BlobStore` gains `presign_get`, `delete` and a large-object put. Both S3 and memory
  implementations change.
- The backend gains an outbound HTTP client with a platform-root TLS verifier and an SSRF
  guard, plus an SFTP client. Both are new licence and advisory surface for `cargo deny`.
- PDF fonts (Noto, SIL OFL-1.1) are data files outside the crate allow-list. The owner
  must ratify OFL for embedded fonts (open question Q3), or a different font is chosen.
- The Swift claims scanner does not see Rust-rendered text. Each encoder carries its own
  test for the mandatory sentence (XC-6).
- iOS `DocumentReference` defects (G7) remain until the app adopts `asd-export` via FFI or
  is patched. That is a follow-up WP-A item, not part of this ADR's packages.
- ADR-0003 D7 (namespace `http://curv.org/fhir`) still governs every local URI here.
  Changing the publisher domain (ADR-0005) changes iOS and server fixtures together.

## Open questions for the owner

- **Q1** Is there any customer without an EHR who needs patient names in the console? If so,
  a new ADR (encrypted display name, per-org opt-in, separate retention).
- **Q2** Retention versus erasure for signed clinical reviews and notes. Medical-record
  retention rules (for example, Indian Medical Council guidance of 3 years for outpatient
  records) may conflict with a parent's erasure request. The proposed default: erasure
  removes sessions and measurements and crypto-shreds notes only after the org's
  `clinical.retention_years`. This needs a decision before WP-B.9 executes erasure on
  clinical rows.
- **Q3** Ratify SIL OFL-1.1 for embedded font data (Noto Sans, Noto Sans Devanagari) in
  `docs/license_policy.md`.
- **Q4** Who may cosign: any other clinician (default), or only users the owner marks as
  senior?
- **Q5** Pilot partner details that fix the first integration profile: HIS vendor, HL7
  version actually accepted, MRN assigning-authority code, SFTP versus shared folder, and
  whether `PID-5` may be empty.
- **Q6** Verification of LOINC `93044-6` (and whether to keep it on iOS) by someone with
  LOINC access. Until then it is not emitted by the server.
