# ASD Clinician Workspace and Interoperability Bridge - Technical Spec

Status: Proposed (2026-10-31). Snapshot reviewed: `dev` at `1f43026`.
Decision record: `docs/spec/adr/ADR-0008-clinician-workspace-and-interop.md`.
Builds on `docs/spec/modules/asd-rust-core-and-backend/` (ids `API-*`, `AUTH-*`, `ING-*`,
`CFG-*`, `OBS-*`), ADR-0003 (FHIR coding, ids `FHIR-*`), ADR-0004 (claims), ADR-0006
(Rust core). Work packages: `work-packages.md` (WP-C.9..C.14, WP-I.1..I.11).

## Purpose and scope

The product is an expert support system that plugs into the hospital's existing systems;
it is not an EHR and does not replace one. Two workstreams:

1. **Clinician workspace** (WP-C.9..C.14): the existing `clinician` role gets a worklist of
   sessions awaiting clinician review, a participant timeline, session review with
   electronic sign-off (optional second signature), encrypted clinical notes, a referral or
   summary letter, a printable clinical summary, and an optional hospital MRN mapping.
2. **Interoperability and export bridge** (WP-I.1..I.11): one Rust crate `asd-export`
   producing CSV (canonical, Excel, Google Sheets), XLSX, PDF/A-2b, FHIR R4 and HL7 v2.5.1
   ORU^R01; export jobs with filters, identified and de-identified modes, consent checks,
   audit and a watermark per export; delivery by signed download URL, signed webhook and
   SFTP drop; per-org integration settings; later FHIR push (SMART Backend Services), SMART
   EHR launch and HL7 MLLP.

Out of scope (deliberately): C-CDA (ADR-0008 D9), storing patient names (D4), diagnosis
coding (ICD/SNOMED findings) entered by us, direct Google Sheets API delivery, a template
engine for letters, cross-export stable research pseudonyms, small-cell suppression,
being a FHIR server (WP-B.10 stays backlog), HL7 inbound (ADT/ORM) messages, MLLP until
WP-I.11, iOS adopting `asd-export` through FFI (future WP-A item).

## Verified current state this spec builds on

| # | Finding | Evidence |
| --- | --- | --- |
| G1 | `rbac::allows` returns `true` for `superadmin` on every action. The vendor's superadmin would therefore read clinical notes and MRNs unless clinical-content actions are carved out. | `rust/crates/asd-backend/src/auth/rbac.rs:allows` |
| G2 | `review_items.note` (free text, migration 0003) is stored in clear. `02` ADR-B8 requires column-level encryption for free-text notes. | `migrations/0003_review_manual.sql`; `routes/review.rs:221` |
| G3 | `organizations.settings` is free JSON, merge-patched, and its full before/after snapshot is written to the append-only `audit_log`. Any secret placed there becomes permanent in the audit log. | `routes/orgs.rs:35,118-123`; `migrations/0002_auth_grants.sql` |
| G4 | `BlobStore` has `presign_put`, `put`, `get`, `ready`; no `presign_get`, no `delete`. `put` refuses bodies over 50 MB. | `src/blob/mod.rs` |
| G5 | The job runner executes each handler inside one `REPEATABLE READ` transaction together with the success update; handlers receive only `(&mut tx, &Job)` (no blob store, config or keys); visibility timeout 15 min; `MAX_ATTEMPTS = 3`; no `run_after` (no delayed retry). Network I/O in a handler would hold a transaction open. | `src/jobs/runner.rs` |
| G6 | iOS `FHIRBundleBuilder` emits `Patient`, `RelatedPerson`, `Device`, gaze `Observation`, `Provenance`, M-CHAT resources. It emits no `DiagnosticReport`. Resource ids are RFC 4122 v5 UUIDs over `http://curv.org/fhir/<name>`. | `apps/ios_asd_gaze_app/Sources/Models/FHIRBundleBuilder.swift` |
| G7 | `DoctorReportInput.toFHIRDocumentReference` uses a random id (`UUID().uuidString.prefix(8)`), the IHE format code `urn:ihe:iti:xfdf:2008` (that is XFDF, not PDF), the US Core category system, and a bare `Patient/<id>` reference. | `DoctorReportGenerator.swift:490-568` |
| G8 | LOINC `93044-6` "Gaze tracking assessment panel" is used as the gaze `Observation.code`. It is not verified against the LOINC release. ADR-0003 kept it. Sending it to a hospital in OBR-4 or `DiagnosticReport.code` makes an unverified claim. | `ClinicalInteroperabilityStandards.swift:67` |
| G9 | `users` has no display name and no professional registration number, so a signature block cannot be rendered. | `migrations/0001_initial.sql` (`users`) |
| G10 | The iOS doctor report is US Letter. Nepal and India use A4. | `DoctorReportGenerator.swift:pageWidth` |
| G11 | Organisations have no time zone. HL7 `TS` values and printed reports need local time (Nepal is UTC+05:45, India UTC+05:30). | `organizations` table |
| G12 | The backend has no general HTTP client (only the AWS SDK's). `Cargo.toml` notes that `webpki-roots` (CDLA-Permissive-2.0) is not on the licence allow-list. | `asd-backend/Cargo.toml`, `rust/deny.toml` |
| G13 | Console: one `types.ts`, `live.ts`, `mock.ts`, `messages/en.ts` and one nav in `[org]/layout.tsx`. Every console package edits them. The console uses `npm` (`package-lock.json`, `console.yml`); `04 §5` says `pnpm`. | `apps/curv_console/src/lib/api/*`, `.github/workflows/console.yml` |
| G14 | `.gitignore` ignores `*.pdf` and `*.zip`. | `.gitignore:25,228` |
| G15 | `participants` has `consent_version`, `consent_at`, `withdrawn_at` but no research-consent flag. The only research-consent signal is per session: `summary->'normCollection'->>'consented'`. | `0001_initial.sql`; `ingest/auto_review.rs` (ING-3) |
| G16 | The claims scanner checks Swift sources (`--swift`) only. Strings rendered by Rust into PDFs, HL7 NTE or FHIR narrative are not scanned. | `scripts/check_asd_claims.py` |
| G17 | `care_events` exists since 0001; WP-B.6 (in flight) adds the routes and possibly migration 0006. WP-B.8 (in flight) adds server PDF/FHIR, possibly a report crate and migration 0007. WP-C.2 (in flight) edits the console API layer and nav. | `04 §0`; worktrees under `.claude/worktrees/` |
| G18 | iOS already has a device-side SMART client (`SmartOnFhirAuthService`, patient-scoped write scopes). Server-side push is a different flow (backend services, system scopes). | `SmartOnFhirAuthService.swift:345-357` |

## 1. RBAC additions (RBAC)

- **RBAC-1** `auth/rbac.rs` MUST gain these actions (each with a `perm::` marker and
  exactly one `MATRIX` row): `WorklistRead`, `ClinicalReviewWrite`, `ClinicalSign`,
  `ClinicalCosign`, `ClinicalContentRead`, `ClinicalNoteWrite`, `LetterWrite`,
  `PatientRefRead`, `PatientRefWrite`, `ExportRead`, `ExportDeidentified`,
  `ExportIdentified`, `IntegrationRead`, `IntegrationManage`.
- **RBAC-2** Matrix (org roles; `-` = denied):

  | Action | owner | admin | clinician | researcher | reviewer | viewer | superadmin |
  | --- | --- | --- | --- | --- | --- | --- | --- |
  | WorklistRead | Y | Y | Y | - | - | - | Y |
  | ClinicalReviewWrite | - | - | Y | - | - | - | - |
  | ClinicalSign | - | - | Y | - | - | - | - |
  | ClinicalCosign | - | - | Y | - | - | - | - |
  | ClinicalContentRead | Y | - | Y | - | - | - | - |
  | ClinicalNoteWrite | - | - | Y | - | - | - | - |
  | LetterWrite | - | - | Y | - | - | - | - |
  | PatientRefRead | Y | Y | Y | - | - | - | - |
  | PatientRefWrite | Y | Y | Y | - | - | - | - |
  | ExportRead | Y | Y | Y | Y | - | - | Y |
  | ExportDeidentified | Y | Y | Y | Y | - | - | - |
  | ExportIdentified | Y | - | Y | - | - | - | - |
  | IntegrationRead | Y | Y | - | - | - | - | Y |
  | IntegrationManage | Y | Y | - | - | - | - | - |

- **RBAC-3** `allows()` MUST NOT grant `superadmin` the actions in a new constant
  `CLINICAL_CONTENT_ACTIONS` = {`ClinicalReviewWrite`, `ClinicalSign`, `ClinicalCosign`,
  `ClinicalContentRead`, `ClinicalNoteWrite`, `LetterWrite`, `PatientRefRead`,
  `PatientRefWrite`, `ExportDeidentified`, `ExportIdentified`, `IntegrationManage`} (G1).
  The vendor operates the system; it does not read clinical content and does not take
  copies of a clinic's data, even de-identified. There is no break-glass path in v1.
- **RBAC-4** Worklist and session reads show sessions only; the clinician impression,
  notes, letters and MRN come only from endpoints that require `ClinicalContentRead` or
  `PatientRefRead`. A role without them gets the same JSON with those fields absent (not
  `null`, not masked strings).
- **RBAC-5** The existing rule holds: another org's resource answers 404; a forbidden role
  answers 403 (AUTH-4). The RBAC unit test `every_action_has_exactly_one_row` stays green
  and a new test asserts RBAC-3 for every action in `CLINICAL_CONTENT_ACTIONS`.

## 2. Encryption at rest and keys (KEY)

- **KEY-1** Free text and identifiers in the clinical tables (impression, recommendations,
  note bodies, letter bodies and recipients, MRN) MUST be encrypted in the application with
  AES-256-GCM before they reach SQL. They are stored as `bytea` with a `key_id`. `pgcrypto`
  is not used (ADR-0008 D5: keys would appear in SQL text and server logs).
- **KEY-2** Envelope scheme: one random 256-bit data key (DEK) per org, stored wrapped
  (AES-256-GCM key wrap) by the key-encryption key `ASD_DATA_KEK` (32 bytes, base64, from
  env/Secrets Manager; KMS in WP-B.7) in `org_data_keys`. A missing `ASD_DATA_KEK` MUST make
  every clinical-content endpoint answer `503 problem clinical-crypto-unavailable` and MUST
  NOT stop the rest of the service from starting.
- **KEY-3** Ciphertext layout: `version(1) || nonce(12) || ciphertext || tag(16)`, nonce
  random per encryption. The AAD MUST be `"{table}:{column}:{org_id}:{row_id}"`, so a
  ciphertext copied to another row or org fails to decrypt (test).
- **KEY-4** Exact-match lookup of an MRN uses a blind index:
  `HMAC-SHA256(org_index_key, normalize(assigning_authority) || 0x1F || normalize(mrn))`,
  `org_index_key = HKDF-SHA256(DEK, info = "mrn-bidx-v1")`. `normalize` = trim, NFKC,
  upper-case, remove spaces and hyphens.
- **KEY-5** Key rotation: a new DEK gets a new `key_id`; old rows keep decrypting with
  their `key_id`; a re-encryption job is out of scope for v1 (listed in ADR consequences).
  Deleting an org's DEK rows (org offboarding) MUST render its clinical content
  unrecoverable (crypto-shredding); this is a documented operator action, not an API.
- **KEY-6** The crypto module (`asd-backend/src/crypto/`) MUST be the only code that calls
  the AEAD. Plaintext MUST NOT be logged; the redaction layer (OBS-1) gains the field names
  `impression`, `recommendations`, `body`, `recipient`, `mrn`, `password`.
- **KEY-7** G2 follow-up: `review_items.note` MUST be migrated to the same scheme
  (`note_ct`, `note_key_id`) in migration 0008. A migration cannot encrypt (the key is not in
  SQL), so: the API writes only `note_ct` from 0008 on; `asd-backend admin
  encrypt-review-notes` encrypts existing rows and sets `note = NULL` (idempotent); a later
  migration drops `note` once every deployment has run the command (follow-up, not 0008).

## 3. Patient reference (MRN) (MRN)

- **MRN-1** No patient name, date of birth, phone, address or photo is ever stored
  (ADR-0008 D4). The only identified datum is the hospital MRN.
- **MRN-2** Per-org setting `identity.patient_ref_mode ∈ {none, mrn}`, default `none`.
  With `none`, every MRN endpoint answers `409 problem patient-ref-disabled`. Changing it
  needs `OrgUpdate` and is audited (existing route).
- **MRN-3** `patient_refs` holds at most one MRN per participant and per
  `(org_id, assigning_authority, mrn_bidx)` (unique). `assigning_authority` is a short
  plain-text code configured by the org (e.g. `TUTH`, `AIIMS-DEL`); it identifies the
  hospital, not the patient.
- **MRN-4** Lookup by MRN is `POST` with the MRN in the body, never in a URL or query
  string (URLs reach logs; OBS-1).
- **MRN-5** Every MRN read (decrypt) and write writes one audit row with
  `target_type = "patient_ref"` and no MRN value in `before`/`after` (only `{"set": true}`).
- **MRN-6** De-identified exports and every response to a caller without `PatientRefRead`
  never contain the MRN.

## 4. Worklist, review and sign-off (CW, SIGN)

- **CW-1** `GET /v1/orgs/{id}/worklist` (`WorklistRead`) lists the org's sessions whose
  clinical review is not `final`, newest first, cursor over `(received_at, id)`, filters
  `state`, `flag`, `assigned=me|unassigned|any`, `from`, `to`. Item = `SessionSummary` +
  `clinical_state`, `assigned_to`, `age_hours`, `cosign_required`, `has_patient_ref`.
- **CW-2** Worklist scope is computed, not inserted at ingest (no change to WP-B.3 code).
  Org setting `clinical.worklist_scope ∈ {all, flagged}` (default `all` for `clinic`
  orgs, `flagged` for others); `flagged` = `attention_flag ∈ {red, yellow, unreliable}`.
  `synthetic` sessions appear only when `ASD_ALLOW_SYNTHETIC=true`.
- **CW-3** `GET /v1/participants/{id}` (`ParticipantRead`) returns the participant
  (`api_v1::Participant` fields) plus `has_patient_ref`; `GET /v1/participants/{id}/timeline`
  returns a merged, newest-first, cursor-paged stream of `session`, `clinical_review`
  (state changes and signatures), `note` (header only: author, time, kind), `letter`
  (header only) and `care_event` (WP-B.6 table, kind + time + payload summary) entries.
  Note and letter bodies are never in the timeline.
- **SIGN-1** States of `clinical_reviews.state`: `open` (row created on claim) →
  `in_progress` (first draft save) → `awaiting_cosign` (signed, second signature required)
  → `final`; `final|awaiting_cosign` → `amending` (amend started) → back through signing.
  One review row per session (unique).
- **SIGN-2** Draft content: `disposition` ∈ {`no_concern_noted`, `monitor`,
  `refer_for_evaluation`, `recapture_requested`, `not_interpretable`} (required to sign),
  `impression` (encrypted, ≤ 8 000 chars), `recommendations` (encrypted, ≤ 8 000 chars).
  These are the clinician's statements. The system MUST NOT pre-fill `disposition` or
  `impression` from the attention flag.
- **SIGN-3** Concurrency: `PATCH /v1/clinical-reviews/{id}` requires `If-Match: "<row_version>"`;
  a mismatch answers `412 problem stale-version`.
- **SIGN-4** Content hash: `content_hash = SHA-256(JCS(ClinicalReviewSnapshot))`, where the
  snapshot (type in `asd-contract::clinical`) holds `session_id`, `org_id`,
  `bundle_version`, `schema_version`, the stored session measurements shown on the review
  page (the `SessionDetail` measurement fields and every clip `MetricRow`), `disposition`,
  `impression`, `recommendations`, `version_no`. The server returns it with the draft;
  `POST /v1/clinical-reviews/{id}/sign` MUST carry the same hash, else `409 problem
  content-changed`. This proves the signer saw what is signed.
- **SIGN-5** Signing requires step-up re-authentication in the same request: the user's
  password (Argon2id verify, AUTH-1 rate limit) and, when the user has TOTP, a current code.
  Body also carries `meaning_ack: true` (the UI shows the meaning statement,
  `"I have reviewed this observation record and approve this review."`). Wrong password →
  `401 problem reauthentication-failed`, audited, no state change.
- **SIGN-6** Second signature: org setting `clinical.cosign_policy ∈ {none, red_flag,
  always}` (default `none`). When required, the first signature moves the review to
  `awaiting_cosign`; `POST /v1/clinical-reviews/{id}/cosign` (same step-up, same hash) by a
  different user with `ClinicalCosign` moves it to `final`. The same user cosigning →
  `409 problem cosigner-must-differ` (handler check and a DB `CHECK`/trigger).
- **SIGN-7** Immutability: a signed version (`clinical_review_versions.status = 'signed'`)
  MUST NOT change; the only allowed update is `signed → superseded`. `clinical_signatures`
  is append-only. Both enforced by triggers, as for `audit_log` (AUTH-5).
- **SIGN-8** Amendment: `POST /v1/clinical-reviews/{id}/amend {reason}` (reason 10..500
  chars) copies the signed content into a new draft version (`version_no + 1`); the earlier
  version becomes `superseded` only when the new one is signed. Amending while
  `awaiting_cosign` withdraws the pending cosign (audited).
- **SIGN-9** Each of claim, draft save, sign, cosign, amend writes exactly one audit row in
  the same transaction (AUTH-5) with `content_hash` (hex) and `version_no`, never the text.
- **SIGN-10** When a review reaches `final` (including a signed amendment), the same
  transaction MUST call `interop::on_review_finalized(tx, review_id)`. WP-C.10 creates it as a
  no-op; WP-I.7 fills it (transactional outbox: enqueue auto-deliveries, DLV-6).

## 5. Notes and letters (NOTE)

- **NOTE-1** Clinical notes are per participant, optionally linked to a session,
  append-only. A correction is a new note with `corrects_id`; the corrected note is shown
  struck through, never edited or deleted (trigger).
- **NOTE-2** `GET /v1/participants/{id}/notes` and `GET /v1/notes/{id}` need
  `ClinicalContentRead`; `POST /v1/participants/{id}/notes` needs `ClinicalNoteWrite`.
  Body ≤ 20 000 chars, encrypted (KEY-1).
- **NOTE-3** Referral or summary letter: `POST /v1/participants/{id}/letters`
  (`LetterWrite`) creates a draft with `kind ∈ {referral, summary}`, `session_ids[]`,
  `recipient` (free text, encrypted), `body` (free text, encrypted). `PATCH` while draft
  (If-Match), `POST /v1/letters/{id}/finalize` with step-up (SIGN-5 rules, own hash over the
  letter snapshot) freezes it. Finalized letters are immutable (trigger).
- **NOTE-4** `GET /v1/letters/{id}/pdf` renders through `asd-export` (PDF-*) and creates an
  `exports` row (EXJ-7). No letter template engine: fixed layout = org letterhead (org name,
  `settings.letterhead.address_lines[]`), date, recipient, body, linked session summary
  table, signature block (display name, registration, signed time), mandatory sentences
  (PDF-5).
- **NOTE-5** `GET /v1/sessions/{id}/clinical-summary.pdf` (`SessionRead`; impression and
  MRN included only with `ClinicalContentRead`/`PatientRefRead`) renders the printable
  clinical summary through `asd-export` and creates an `exports` row. Console "print" opens
  this PDF; the console never renders its own print layout.
- **NOTE-6** `users` gains `display_name` (≤ 120) and `professional_registration`
  (≤ 60, e.g. "NMC 12345") (G9), editable by the user and by `UserManage`. These are staff
  data and are stored in clear. Signing (SIGN-5) requires `display_name` to be set
  (`409 problem signer-profile-incomplete`).
- **NOTE-7** Org settings added: `timezone` (IANA, default `UTC`; G11), `paper ∈ {a4,
  letter}` (default `a4`; G10), `letterhead.address_lines` (≤ 4 lines, ≤ 80 chars).

## 6. The `asd-export` crate (XC)

- **XC-1** One crate `rust/crates/asd-export`, Apache-2.0, `std` only, no tokio, sqlx,
  reqwest or file system access: every encoder is `fn(&Input, &Options) -> Result<Vec<u8>>`.
  Depends on `asd-core`, `asd-contract`. Features: `pdf` (font data + PDF engine), `xlsx`;
  both on in the backend. It MUST be buildable for `aarch64-apple-ios` without the `pdf`
  feature (future FFI use).
- **XC-2** If WP-B.8 created a separate report crate (e.g. `asd-report`) or put PDF/FHIR
  rendering in `asd-backend`, WP-I.1 MUST move that code into `asd-export` (`git mv`,
  tests kept, B.8's endpoints re-pointed) so there is exactly one implementation of each
  format. B.8's golden tests MUST stay green unchanged.
- **XC-3** Canonical input: `ExportRecord` (versioned, `record_version = "1"`) = session
  facts (`SessionDetail` fields), clips with `MetricRow`s, clinical review summary (state,
  disposition, signed_at, signers' display names and registrations, impression and
  recommendations only when present in the input), patient reference (token, optional MRN
  + assigning authority), org facts (name, timezone, paper). The backend builds it; the
  crate never queries anything.
- **XC-4** De-identification (`deid::apply(records, salt) -> records`), applied by the crate,
  not the caller, whenever `mode = deidentified`:
  MRN removed; participant token replaced by `P-` + first 10 base32 chars of
  `HMAC-SHA256(per_export_salt, participant_id)`; session id replaced by `S-` + the same
  construction over `session_id`; timestamps reduced to `YYYY-MM` plus
  `days_since_first_session` per participant; impression, recommendations, note and letter
  text removed; signer names and registrations removed (signed flag and disposition kept);
  device id removed; `age_bracket_at_enrol` kept. The salt is random per export and never
  stored after the job finishes.
- **XC-5** Consent policy (`policy::eligibility(purpose, mode, facts) -> Include |
  Exclude(reason)`), pure and table-tested. Rules: `withdrawn_at` set → exclude always;
  `purpose = research` → include a session only when its `normCollection.consented` is
  `true` (G15) and mode is `deidentified`; `purpose = clinical_care | patient_request` →
  identified or de-identified allowed; `purpose = quality_improvement` → de-identified only.
  Identified mode with `purpose = research` is rejected at request time (400).
- **XC-6** Watermark: `Watermark { export_id, requested_by_display, requested_at_utc, mode,
  purpose, org_name }`, rendered per format (CSV-3, XLSX-2, PDF-4, FHX-7, HL7-6). Every
  human-readable artifact also carries the ADR-0004 sentence "This is not a diagnostic
  tool." (G16: a Rust test asserts it per format, because the Swift claims scanner does
  not see these strings).
- **XC-7** Code tables: every external code (LOINC, SNOMED CT, HL7 terminology, IHE format
  codes, UCUM) used by any encoder lives in `asd-export/src/codes.rs` with a
  `verified: bool` and a source note. An encoder MUST NOT emit a code with
  `verified = false` unless the org's integration profile opts in. LOINC `93044-6` starts
  `verified = false` (G8).
- **XC-8** Determinism: same input + same options + same `generated_at` → identical bytes
  for CSV, XLSX (fixed zip timestamps), FHIR and HL7; identical extracted text and
  structure for PDF. Golden tests rely on this.

### 6.1 CSV (CSV)

- **CSV-1** Three dialects from one writer: `csv` (RFC 4180, UTF-8 without BOM, LF,
  RFC 3339 UTC timestamps), `csv_excel` (UTF-8 with BOM, CRLF, `YYYY-MM-DD HH:MM:SS` in the
  org time zone plus a `utc_offset` column), `csv_sheets` (UTF-8 without BOM, LF,
  `YYYY-MM-DD HH:MM:SS` in the org time zone plus `utc_offset`; booleans `TRUE`/`FALSE`).
  Decimal separator is always `.`, no thousands separator, absent values are empty cells.
- **CSV-2** Formula-injection guard in every dialect: a text cell starting with `=`, `+`,
  `-`, `@`, TAB or CR is prefixed with `'`. Numeric cells are written by the number
  formatter, never as text, so negative numbers are not prefixed.
- **CSV-3** Every row carries `export_id` as its first column. A multi-table CSV export is a
  ZIP of `sessions.csv`, `clips.csv`, `metric_rows.csv`, `reviews.csv`, `manifest.json`
  (watermark, column dictionary, counts, SHA-256 of each file).
- **CSV-4** Column names are stable snake_case, documented in `manifest.json` and in
  `asd-export/src/tabular/columns.rs` (one source for CSV and XLSX).

### 6.2 XLSX (XLSX)

- **XLSX-1** Built with `rust_xlsxwriter` (MIT OR Apache-2.0). Sheets: `README` (watermark,
  sentence XC-6, column dictionary), `Sessions`, `Clips`, `MetricRows`, `Reviews`. Dates are
  Excel date cells in the org time zone; text cells are written with `write_string`, never
  parsed as formulas.
- **XLSX-2** Watermark also in document properties (title, subject, comments) and in each
  sheet's printed header/footer.
- **XLSX-3** Row cap 200 000 per sheet; above it the request is refused (400) with a hint to
  narrow filters.

### 6.3 PDF/A (PDF)

- **PDF-1** Clinical summary and letters are PDF/A-2b, generated in Rust (engine chosen in
  WP-I.5; `krilla` (MIT OR Apache-2.0) is the default candidate because it validates PDF/A
  and embeds subset fonts; if WP-B.8 picked another engine that cannot produce PDF/A,
  WP-I.5 replaces it and keeps B.8's tests).
- **PDF-2** Fonts embedded and subset; Latin and Devanagari coverage (Noto Sans,
  Noto Sans Devanagari, SIL OFL-1.1, pending owner ratification of OFL for font data,
  ADR-0008 open question Q3). Clinician free text in Devanagari MUST render with correct
  shaping (test extracts the text back).
- **PDF-3** Paper from org setting (A4 default). Times in the org time zone with offset.
- **PDF-4** Footer on every page: `Export {export_id} · {requested_by} · {requested_at} ·
  page n/m`; XMP metadata carries `export_id`, mode, purpose. Unsigned reviews print a
  banner `PRELIMINARY - NOT SIGNED`; `awaiting_cosign` prints `AWAITING SECOND SIGNATURE`;
  synthetic sessions print the ADR-0001 simulated-data banner text.
- **PDF-5** Mandatory sentences (ADR-0004): "This is not a diagnostic tool." and "Part of
  Open Spectrum Tracking". The clinician's disposition is labelled as the clinician's
  statement.
- **PDF-6** Clinical summary sections: header (org name, letterhead), participant (token;
  MRN + authority when included), session facts, quality, per-clip expected-vs-recorded
  table (same rows as the console Clips tab, CON-19), clinician review (disposition,
  impression, recommendations), signatures (name, registration, role sign/cosign, time,
  first 12 hex chars of `content_hash`), mandatory sentences.

### 6.4 FHIR R4 (FHX)

- **FHX-1** The clinical report bundle extends, and does not fork, the session bundle that
  WP-B.8 builds for parity with iOS (`GET /v1/sessions/{id}/fhir` stays parity-exact).
  Resource ids keep the iOS v5 UUID scheme over `http://curv.org/fhir/<name>`, so the same
  session yields the same `Observation` id on device and server.
- **FHX-2** Contents (`Bundle.type = collection` for download; `transaction` for push,
  WP-I.9): `Patient`, `Organization` (the org), `Practitioner` per signer, `Device`, the gaze
  `Observation` (identical to the parity bundle), `DiagnosticReport`, `DocumentReference`
  (the PDF/A summary), `Provenance` per signature.
- **FHX-3** `Patient`: de-identified → `id` and identifier = export pseudonym (XC-4),
  system `http://curv.org/fhir/NamingSystem/export-pseudonym`; identified → identifier
  `{system: <integration profile mrn_system>, value: MRN, type: v2-0203 MR}` plus the token
  identifier. Never `name`, `birthDate`, `telecom`, `address`.
- **FHX-4** `DiagnosticReport.status`: no signature → `preliminary`; `awaiting_cosign` →
  `preliminary`; `final` → `final`; signed amendment → `amended`. `code`: local
  `http://curv.org/fhir/CodeSystem/gaze-engagement#gaze-observation-review` (LOINC added only
  when `verified`, XC-7). `subject` → Patient, `result` → the gaze Observation,
  `resultsInterpreter` → signer Practitioners, `performer` → Organization,
  `conclusion` = impression (identified only), `conclusionCode` = disposition from local
  CodeSystem `.../CodeSystem/clinical-disposition`. `presentedForm` is absent: the PDF lives
  only in the `DocumentReference`, whose `context.related` points at the DiagnosticReport.
  Narrative `text.div` carries the XC-6 sentence.
- **FHX-5** `DocumentReference`: deterministic id (v5 over
  `DocumentReference/{export_id}`), `status = current`, `docStatus` = `preliminary|final|amended`
  (as FHX-4), `type` LOINC per document kind from `codes.rs` (clinical summary and referral
  codes verified in WP-I.3), `content.attachment` = `contentType application/pdf`,
  `data` (download bundle) or `url` (push), `size`, `hash` (SHA-1 base64 as R4 requires),
  `format` = IHE PDF format code from `codes.rs` (fixing G7).
- **FHX-6** `Practitioner`: identifier `http://curv.org/fhir/NamingSystem/console-user` =
  user id; `name.text` = display name; registration as a second identifier with system
  from the integration profile when configured. De-identified mode omits Practitioners.
- **FHX-7** Watermark: `Bundle.identifier` = export id
  (`http://curv.org/fhir/NamingSystem/export`), `Bundle.meta.tag` with mode and purpose
  codes from `.../CodeSystem/export-mode`.
- **FHX-8** Validation in tests is structural (resource types, required elements, every
  reference resolves inside the bundle, no forbidden Patient elements), as ADR-0003 D8.
  An optional CI job may run the HL7 FHIR validator jar; it is not a gate.

### 6.5 HL7 v2.5.1 ORU^R01 (HL7)

- **HL7-1** Hand-written encoder (no crate): segments `MSH`, `PID`, `PV1` (only when the
  profile sets `send_pv1`, class `O`), `ORC`, `OBR`, `OBX`*, `NTE`*. Segment separator CR,
  field `|`, components `^`, repetition `~`, escape `\`, subcomponent `&`; the five
  delimiters inside data are escaped as `\F\ \S\ \R\ \E\ \T\`; CR/LF in text become `\.br\`.
  `MSH-18 = UNICODE UTF-8`.
- **HL7-2** `MSH-3/4/5/6` from the integration profile (sending app/facility, receiving
  app/facility); `MSH-7` and every `TS` in the org time zone with offset
  (`YYYYMMDDHHMMSS+HHMM`, e.g. `+0545`); `MSH-9 = ORU^R01^ORU_R01`; `MSH-10` = delivery id;
  `MSH-11 = P` (or `T` for test destinations); `MSH-12 = 2.5.1`.
- **HL7-3** `PID-3` = MRN `^^^{assigning_authority}^MR` (identified only; HL7 deliveries
  without an MRN are refused at request time). `PID-5` per profile `pid5_mode ∈ {empty,
  token}`, default `empty` (no names are stored, MRN-1). No DOB, sex, address.
- **HL7-4** `OBR-4` = local service code `ASD-GAZE^Gaze engagement observation review^L`
  (alternate triplet LOINC only when verified, XC-7); `OBR-7` = session start; `OBR-22` =
  sign time; `OBR-25` and every `OBX-11` = `P` (not final), `F` (final), `C` (signed
  amendment); `OBR-32` = first signer (`id&display name`).
- **HL7-5** `OBX`: one `NM` per measured metric (same metric set and order as the FHIR
  Observation components; absent metrics absent), `OBX-3` = local code from the
  gaze-engagement CodeSystem with coding system `L`, overridable per profile by `code_map`
  (hospital local codes); `OBX-6` = `1^ratio^UCUM` or `1^score^UCUM`; one `CWE` OBX for the
  disposition; one `TX` OBX per paragraph of impression (identified only); optionally one
  `ED` OBX with the PDF/A summary base64 (`^AP^PDF^Base64^...`) when the profile sets
  `embed_pdf`.
- **HL7-6** `NTE` segments: watermark line and the XC-6 sentence.
- **HL7-7** One message per file for file delivery; file name
  `{prefix}{export_id}_{session_id_hash}.hl7`. Batch envelopes (FHS/BHS) and MLLP framing
  are WP-I.11.

## 7. Export jobs (EXJ)

- **EXJ-1** `POST /v1/orgs/{id}/exports` body `ExportRequest { format, mode, purpose,
  filters, destination_ids[], reason }`; `format ∈ {csv, csv_excel, csv_sheets, xlsx,
  pdf_summary, fhir_bundle, fhir_ndjson, hl7v2}`; `mode ∈ {identified, deidentified}`;
  `purpose ∈ {clinical_care, research, quality_improvement, patient_request}`;
  `reason` 10..500 chars (shown in the audit); filters = the session-list filters
  (`flag`, `quality`, `from`, `to`, `bundle_version`, `reviewed`) plus `participant_ids[]`,
  `session_ids[]`, `clinical_state[]`. Answers `202` with the `Export` resource.
- **EXJ-2** Authorisation: `ExportDeidentified` or `ExportIdentified` by mode; identified
  needs `identity.patient_ref_mode = mrn` only for formats that need an MRN (`hl7v2`);
  impression text is included only for callers with `ClinicalContentRead`.
- **EXJ-3** The job (`jobs.kind = 'export'`) reads one consistent snapshot, applies XC-5
  per session, writes `export_items(export_id, participant_id, session_id)` for included
  rows, counts exclusions by reason into `exports.excluded`, renders with `asd-export`,
  uploads the artifact to `exports/{org_id}/{export_id}/{file}`, stores `sha256`,
  `byte_size`, `row_count`. Rendering and upload happen outside any open DB transaction;
  the job is idempotent by `export_id` (deterministic blob key, overwrite allowed).
- **EXJ-4** Artifacts expire after `exports.expires_at` (default 72 h, org setting
  `exports.ttl_hours` 1..168). A sweep deletes the blob and sets `status = expired`.
  `export_items` stay (accounting of disclosures).
- **EXJ-5** `POST /v1/exports/{id}/download-url` (`ExportRead`, same org; identified exports
  only for the requester or a caller with `ExportIdentified`) audits `export.download` and
  returns `{url, expires_at}`: a pre-signed S3 GET valid 5 minutes in `presigned` blob mode,
  or a relative `/v1/exports/{id}/file?t=<token>` in `proxy` mode where `t` is an HMAC over
  `(export_id, user_id, exp)` valid 5 minutes. Expired export → `410 problem export-expired`.
- **EXJ-6** Every export writes `export.create` (request, counts) and each download and
  delivery writes its own audit row. `GET /v1/participants/{id}/disclosures`
  (`ClinicalContentRead`) lists exports and deliveries that included the participant.
- **EXJ-7** Single-document renders (NOTE-4, NOTE-5) also create an `exports` row
  (`kind = 'document'`, synchronous, artifact streamed and not stored) so each printed
  document has an export id and an audit row.
- **EXJ-8** Erasure (WP-B.9) MUST delete unexpired artifacts whose `export_items` include
  the participant; delivered copies cannot be recalled (stated in the UI).
- **EXJ-9** Limits: 200 000 session rows per export; artifact up to 500 MB through a new
  `BlobStore::put_object_large` (streaming/multipart in S3) because `put` caps at 50 MB (G4);
  a running export heartbeats `jobs.heartbeat_at` so the 15-minute reclaim does not run it
  twice (G5).

## 8. Delivery and integration settings (DLV, INTG)

- **INTG-1** `integration_destinations(id, org_id, kind, name, config jsonb, secret_ct,
  secret_key_id, secret_fingerprint, status, environment ∈ {test, production}, created_by,
  created_at, updated_at, last_test_at, last_test_ok)`, `kind ∈ {webhook, sftp,
  fhir_server, hl7_mllp}` (`fhir_server` and `hl7_mllp` accepted only after WP-I.9/I.11).
  Integration settings MUST NOT live in `organizations.settings` (G3).
- **INTG-2** Secrets are write-only: `PUT /v1/integrations/{id}/secret` stores them
  encrypted (KEY-1 with AAD over the destination row); reads return `has_secret` and a
  fingerprint (first 8 hex of SHA-256), never the value. Audit rows carry the fingerprint
  only.
- **INTG-3** `config` per kind: webhook `{url, events[]}`; sftp `{host, port, username,
  remote_dir, host_key_sha256, file_prefix}`; profiles: `hl7 {sending_app,
  sending_facility, receiving_app, receiving_facility, assigning_authority, pid5_mode,
  send_pv1, embed_pdf, code_map{local_code: hospital_code}}`, `fhir {mrn_system,
  registration_system, document_kind_codes{}}`, `format`, `mode`, `trigger ∈ {manual,
  on_final_sign}`.
- **INTG-4** `POST /v1/integrations/{id}/test` sends a synthetic payload (no participant
  data; a fixed test record with token `TEST-0000`) and stores `last_test_at`,
  `last_test_ok`, and a short error class (no remote response bodies).
- **INTG-5** SFTP auth is by an Ed25519 key pair generated by the server per destination
  (`POST /v1/integrations/{id}/ssh-key` returns the OpenSSH public key for hospital IT);
  password auth is not offered. `host_key_sha256` is mandatory and pinned; a mismatch fails
  the delivery with `host-key-mismatch` and never falls back to trust-on-first-use.
- **DLV-1** Each (export, destination) pair is one `export_deliveries` row and one job
  (`kind = 'export_deliver'`) with retry backoff 1 min, 5 min, 30 min, 2 h, 12 h
  (`jobs.run_after`, new), then `failed`. Network I/O happens outside DB transactions (G5).
- **DLV-2** Webhook: `POST` JSON event `{type: "export.ready" | "review.finalized", id,
  org_id, export_id, format, mode, created_at, download: {url, expires_at}}`. The body
  never contains clinical data or MRN; receivers pull through the short-lived URL (EXJ-5
  rules, 15 minutes for webhooks). Signed per the Standard Webhooks scheme: headers
  `webhook-id`, `webhook-timestamp`, `webhook-signature: v1,<base64 HMAC-SHA256(secret,
  "{id}.{timestamp}.{body}")>`; secret `whsec_` + 32 random bytes.
- **DLV-3** Outbound HTTP guard: `https` only; DNS resolved once and the connection pinned to
  the resolved address; refuse loopback, RFC 1918, link-local (incl. `169.254.169.254`),
  CGNAT `100.64.0.0/10`, IPv6 ULA/link-local/loopback, multicast; 10 s connect, 30 s
  total, no redirects followed, response body read ≤ 64 KiB and discarded. TLS roots from
  the platform store (G12).
- **DLV-4** SFTP: upload to `{remote_dir}/.{name}.part`, then rename to `{name}`; the
  artifact SHA-256 is written as `{name}.sha256` beside it.
- **DLV-5** Delivery status `pending → sending → delivered | failed`, `attempts`,
  `last_error_class` (enum, no free remote text), `remote_ref` (e.g. HTTP status), visible in
  `GET /v1/integrations/{id}/deliveries` and on the export.
- **DLV-6** Auto-delivery: `on_review_finalized` (SIGN-10) creates, in the signing
  transaction, one `exports` row (identified, `purpose = clinical_care`, the destination's
  format) and its delivery rows for each `production` destination with
  `trigger = on_final_sign`. A failed delivery never blocks or undoes the signature.
- **DLV-7** FHIR push (WP-I.9): SMART Backend Services (`client_credentials`,
  `private_key_jwt` with ES384 or RS384, JWKS URL served by us), `transaction` bundle with
  conditional create (`ifNoneExist` on Observation/DiagnosticReport identifiers); then
  SMART EHR launch (WP-I.10) of the console with patient context resolved by MRN blind
  index (KEY-4). Both relate to WP-B.10 (FHIR façade), which stays backlog: push first,
  façade only when a partner asks.

## 9. Migrations (MIG)

- **MIG-1** Numbers: `0006` WP-B.6, `0007` WP-B.8 (in flight). If WP-B.8 ships no
  migration, `0007` stays unused forever (sqlx would otherwise apply a later-added 0007 out
  of order on databases already at 0008).
- **MIG-2** `0008_clinical.sql` (WP-C.9): `org_data_keys`, `patient_refs`,
  `clinical_reviews`, `clinical_review_versions`, `clinical_signatures`, `clinical_notes`,
  `referral_letters`; `users.display_name`, `users.professional_registration`;
  `review_items.note_ct`, `note_key_id` (KEY-7; the plaintext column is emptied by the
  back-fill command and dropped by a later migration);
  immutability triggers (SIGN-7, NOTE-1, NOTE-3); indexes `clinical_reviews(org_id, state)`,
  `clinical_notes(participant_id, created_at)`, unique `patient_refs(org_id,
  assigning_authority, mrn_bidx)`.
- **MIG-3** `0009_exports.sql` (WP-I.6): `exports`, `export_items`,
  `integration_destinations`, `export_deliveries`; `jobs.run_after timestamptz`,
  `jobs.heartbeat_at timestamptz`; index `export_items(participant_id)`; claim index
  replaced by `(run_after, created_at) WHERE status = 'queued'`.
- **MIG-4** `0010` is reserved for WP-I.9 (`smart_clients`: our client id per FHIR
  server, key id, token endpoint).

## 10. Contract (CTR)

- **CTR-1** New request/response types live in new files `asd-contract/src/clinical.rs`
  (WP-C.9) and `asd-contract/src/export.rs` (WP-I.6), registered in `schema.rs` and exported
  into `api-v1.json` (`$defs`). `api_v1.rs` is not edited by these packages except
  `Participant` gaining `has_patient_ref: Option<bool>` (C.11).
- **CTR-2** New problem slugs added to `problem.rs` and `$defs/ProblemSlug`:
  `clinical-crypto-unavailable`, `patient-ref-disabled`, `stale-version`,
  `content-changed`, `reauthentication-failed`, `cosigner-must-differ`,
  `signer-profile-incomplete`, `export-expired`, `export-too-large`,
  `consent-purpose-mismatch`, `host-key-mismatch` (used in delivery status only).
- **CTR-3** The console's zod schemas for the new endpoints are checked against
  `api-v1.json` by the existing `contract.test.ts` mechanism (WP-C.8).

## 11. Console (UI)

- **UI-1** Routes: `/[org]/worklist`, `/[org]/participants/[id]` (timeline, MRN panel,
  notes, letters, disclosures), `/[org]/sessions/[id]` gains a `Clinical review` tab,
  `/[org]/letters/[id]`, `/[org]/exports`, `/[org]/settings/integrations`,
  `/[org]/settings/clinical` (worklist scope, cosign policy, patient-ref mode, timezone,
  paper, letterhead). Nav entries shown by role (from the token's role; the backend still
  enforces).
- **UI-2** The console holds no format, de-identification, consent, hash or watermark
  logic (CLAUDE.md). Downloads go through a route handler that asks the backend for a
  download URL and redirects (presigned) or streams (proxy), as `api/report/[id]` does.
- **UI-3** Sign and cosign dialogs collect the password (and TOTP) in a server action,
  send them once, and never store them; the dialog shows the meaning statement and the
  `content_hash` prefix.
- **UI-4** No MRN, note text or impression in URLs, query strings, page titles or
  client-side logs; MRN search is a POST form.
- **UI-5** New API code goes in new modules (`src/lib/api/clinical.ts`,
  `src/lib/api/exports.ts`, `src/lib/api/mock/clinical.ts`, `.../mock/exports.ts`) with their
  own zod schemas; `types.ts`, `live.ts`, `mock.ts`, `index.ts`, `en.ts` and the nav get only
  the minimal re-export/merge lines (G13).

## Public Rust API sketch (load-bearing names only)

```rust
// asd-export
pub mod model;     // ExportRecord, ClinicalReviewSummary, PatientRef, OrgFacts, Watermark
pub mod deid;      // apply(records: &[ExportRecord], salt: &[u8; 32]) -> Vec<ExportRecord>
pub mod policy;    // Purpose, Mode, ConsentFacts, eligibility(..) -> Eligibility
pub mod codes;     // Code { system, code, display, verified }, tables
pub mod tabular;   // columns, csv::write(records, Dialect, &Watermark), xlsx::write(..)
pub mod fhir;      // session_bundle(..) [from WP-B.8], clinical_report_bundle(.., BundleKind)
pub mod hl7v2;     // oru_r01(record, &Hl7Profile, &Watermark, control_id) -> Vec<u8>
#[cfg(feature = "pdf")] pub mod pdf; // clinical_summary(..), letter(..)

// asd-contract
pub mod clinical;  // ClinicalReviewSnapshot, content_hash(&snapshot) -> [u8; 32], DTOs
pub mod export;    // ExportRequest, Export, Delivery, IntegrationDestination DTOs

// asd-backend
pub mod crypto;    // Keyring, encrypt(org, aad, &[u8]), decrypt, mrn_blind_index
pub mod interop;   // on_review_finalized(tx, review_id) (stub in C.10, filled in I.7)
```
