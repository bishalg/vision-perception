# ASD Clinician Workspace and Interoperability Bridge - Work Packages

Companion to `technical.md` (spec ids), `acceptance.md`, `qa-validation.md` and ADR-0008.
The live status table stays `docs/roadmap_2026q4/04_asd_phased_work_packages.md §0`; this
file holds the detail. Inside a wave, packages share no files except the one-line
registration points named per package (`routes/mod.rs`, `Cargo.lock`), which the main
session merges. Repo-wide gates run in the main session after integration.

Tier legend: `S` = `senior-implementer` (Opus); `I` = `implementer` (Sonnet).
Every package: write the listed tests first, then the code; report what was skipped.

Gates (named once, required by every package that touches the area):
- Rust: `scripts/rust_test.sh` (fmt, clippy `-D warnings`, `cargo test --workspace` with
  Postgres, `cargo deny check`, `cargo sqlx prepare --check --workspace`,
  `asd-cli schema check`). After any query change: `cargo sqlx prepare --workspace`.
- Console: `cd apps/curv_console && npm run lint && npm run typecheck && npm test && npm run build`
  (the console uses npm, not pnpm; `04 §5` is out of date on this).

## Wave 0: prerequisites (in flight, not part of this plan)

WP-B.6 (care events, migration 0006), WP-B.8 (server PDF + FHIR, maybe a report crate,
maybe migration 0007), WP-C.2 (console review queue, devices, live mode) must be merged to
`dev` before the packages that name them start. Nothing below edits their files while they
are in flight.

## Wave 1 (parallel; needs B.6 merged for C.9, B.8 merged for I.1)

### WP-C.9 Clinical data foundation (backend) - `S`

Goal: RBAC-1..5, KEY-1..7, MRN-1..3 storage, MIG-2, CTR-1 (`clinical.rs`), CTR-2 (clinical
slugs), NOTE-6, NOTE-7 settings validation, SIGN-4 hash function. Everything later packages
need so they never touch `rbac.rs`, the migration or the crypto module again.

Files owned:
- `rust/crates/asd-backend/migrations/0008_clinical.sql`
- `rust/crates/asd-backend/src/crypto/{mod,aead,keyring,blind_index}.rs` (new)
- `rust/crates/asd-backend/src/auth/rbac.rs` (all 14 actions for C.* and I.*, the
  `CLINICAL_CONTENT_ACTIONS` exclusion)
- `rust/crates/asd-backend/src/config.rs` (`ASD_DATA_KEK`, optional), `rust/.env.example`
- `rust/crates/asd-backend/src/app.rs` (`AppState.keyring: Option<Arc<Keyring>>`)
- `rust/crates/asd-backend/src/telemetry.rs` (KEY-6 field names)
- `rust/crates/asd-backend/src/routes/orgs.rs` (validate the NOTE-7, MRN-2, CW-2, SIGN-6
  setting keys and value ranges only)
- `rust/crates/asd-backend/src/routes/users.rs` (`display_name`,
  `professional_registration` on create/update/self)
- `rust/crates/asd-backend/src/routes/review.rs` + `src/admin.rs`
  (`encrypt-review-notes` back-fill, KEY-7; `note` written encrypted)
- `rust/crates/asd-contract/src/clinical.rs` (new: `ClinicalReviewSnapshot`,
  `content_hash`, all clinical DTOs used by C.10–C.12), `src/lib.rs`, `src/schema.rs`
  (register), `src/problem.rs` (clinical slugs), `src/api_v1.rs` (`OrgUser` +
  `display_name`, `professional_registration`; `UpdateUserRequest` same)
- `docs/spec/schemas/asd/api-v1.json` (regenerated)
- Tests: `asd-backend/tests/{crypto,rbac_clinical,migrations_0008,review_note_encryption}.rs`,
  `asd-contract/tests/clinical_hash.rs`

Must not touch: `ingest/**`, `jobs/**`, `routes/sessions.rs`, console.

Tests first:
- `crypto.rs`: round-trip; AAD swap (row id, org id, column) fails; tampered tag fails;
  wrong `key_id` fails; missing KEK → `Keyring` absent and the clinical endpoints'
  extractor answers 503 (use a dummy route in the test).
- Blind index: normalisation table (`" 12-34 "` = `"1234"`), different orgs give different
  indexes for the same MRN.
- `rbac_clinical.rs`: the RBAC-2 table row by row; RBAC-3 for every
  `CLINICAL_CONTENT_ACTIONS` member; the existing `every_action_has_exactly_one_row`.
- `migrations_0008.rs`: triggers refuse UPDATE of a signed version except
  `signed → superseded`, refuse UPDATE/DELETE on `clinical_signatures`,
  `clinical_notes`, finalized `referral_letters`; `CHECK` refuses a cosign by the signer.
- `review_note_encryption.rs`: manual enqueue stores `note_ct` and leaves `note` NULL;
  the back-fill command encrypts existing rows, nulls `note`, and is idempotent (second run
  changes nothing); a review note written before 0008 decrypts after the back-fill.
- `clinical_hash.rs`: JCS hash is stable across key order and float formatting
  (`1.0` vs `1`), changes when any snapshot field changes.

Depends on: B.6 merged (migration order). Parallel with: I.1.

### WP-I.1 `asd-export` crate foundation - `S`

Goal: XC-1..8 skeleton; fold B.8's PDF/FHIR code into the crate (XC-2); `ExportRecord`
model; `deid`; `policy`; `codes` (with `verified`); `Watermark`; all module files present as
stubs (`//! owner: WP-I.n`) so Wave 2 packages only fill their own files; all external
dependencies of later format packages declared once.

Files owned:
- `rust/Cargo.toml` (workspace member), `rust/crates/asd-export/**` (new):
  `Cargo.toml` (deps: `asd-core`, `asd-contract`, `serde`, `serde_json`, `sha2`, `hmac`,
  `base32` or own encoder, `zip` (no default features, deflate only), optional
  `rust_xlsxwriter` (feature `xlsx`), optional `krilla` + font assets (feature `pdf`) unless
  B.8 already chose an engine), `src/{lib,model,deid,policy,codes,watermark}.rs`,
  stubs `src/tabular/{mod,columns,csv,xlsx}.rs`, `src/fhir/{mod,clinical}.rs`,
  `src/hl7v2/{mod,escape,segments}.rs`, `src/pdf/{mod,summary,letter}.rs`
- If B.8 created `rust/crates/asd-report/` or `asd-backend/src/report/`: `git mv` into
  `asd-export/src/{pdf,fhir}/`, update B.8's call sites in `asd-backend/src/routes/*`
  (only the `use` lines) and `asd-backend/Cargo.toml`; remove the old crate from the
  workspace.
- `rust/deny.toml` only if a new licence exception is needed (report it, do not widen
  silently).
- Tests: `asd-export/tests/{deid,policy,codes,model_roundtrip,watermark}.rs`; B.8's tests
  moved unchanged.

Must not touch: `asd-backend` beyond the B.8 call-site `use` lines; contract files.

Tests first:
- `deid.rs`: every XC-4 transformation; the same participant within one export maps to one
  pseudonym; two exports (different salts) give different pseudonyms; no field named in
  the de-id list survives (walk the serialized JSON).
- `policy.rs`: full table of purpose × mode × {withdrawn, consented, not consented}.
- `codes.rs`: every code has a non-empty source note; `93044-6` is `verified = false`; an
  encoder helper refuses an unverified code without the opt-in.
- `model_roundtrip.rs`: `ExportRecord` built from the iOS golden session fixture
  (`asd-core/tests/fixtures/expected-session-v1.6.json`) serialises deterministically.
- `cargo build -p asd-export --no-default-features --target aarch64-apple-ios` succeeds
  (or `cargo check` when the target is not installed; record which).

Depends on: B.8 merged. Parallel with: C.9.

## Wave 2 (parallel)

### WP-C.10 Worklist, clinical review, sign/cosign/amend (backend) - `S`

Goal: CW-1, CW-2, SIGN-1..10, RBAC-4 for review content.
Files owned: `asd-backend/src/routes/{worklist,clinical_reviews}.rs` (new),
`asd-backend/src/clinical/{mod,review,sign,reauth}.rs` (new),
`asd-backend/src/interop/mod.rs` (new, `on_review_finalized` no-op stub),
`asd-backend/src/routes/mod.rs` (two `merge` lines), `asd-backend/src/lib.rs`
(`pub mod clinical; pub mod interop;`),
`asd-backend/tests/{worklist,clinical_review,signing}.rs`.
Must not touch: `rbac.rs`, `0008`, `crypto/**`, `asd-contract` (DTOs came with C.9; if one
is missing, report it rather than editing `clinical.rs` in parallel with C.11).
Tests first: worklist scope `all` vs `flagged`; synthetic hidden unless allowed; another
org → 404; state machine table (every allowed and refused transition); `If-Match`
mismatch → 412; sign with stale hash → 409 `content-changed`; wrong password → 401 and an
audit row, no state change; TOTP required when enrolled; cosign policy `red_flag` only on
red sessions; same-user cosign → 409; amend while `awaiting_cosign` withdraws the cosign;
signed version bytes unchanged after amend; `on_review_finalized` called exactly once per
finalisation (test double through a counter in the stub); 20 concurrent sign attempts on
one draft → exactly one signature (row lock `FOR UPDATE`).
Depends on: C.9. Parallel with: C.11, I.2, I.3, I.4, I.5.

### WP-C.11 Participant detail, timeline, MRN, clinical notes (backend) - `I`

Goal: CW-3, MRN-2..6, NOTE-1, NOTE-2.
Files owned: `asd-backend/src/routes/{participant_detail,patient_refs,notes}.rs` (new),
`asd-backend/src/read/timeline.rs` (new), `asd-backend/src/routes/mod.rs` (three `merge`
lines), `asd-contract/src/api_v1.rs` (only `Participant.has_patient_ref`),
`asd-backend/src/routes/participants.rs` (fill `has_patient_ref`),
`asd-backend/tests/{timeline,patient_refs,notes}.rs`.
Must not touch: `crypto/**` (call its API), `rbac.rs`, `0008`, `clinical.rs`.
Tests first: MRN endpoints 409 when mode `none`; put/get/delete round-trip with audit rows
that contain no MRN; search by MRN is POST and finds across normalisation variants;
duplicate MRN in the same authority → 409; reviewer/viewer get participant without
`has_patient_ref` details and 403 on MRN read; notes append-only (correction creates a new
row, original unchanged); note body never in timeline or logs (`log_redaction` style
capture); timeline merges the five kinds in time order with a stable cursor across pages
(ties on equal timestamps).
Depends on: C.9, B.6. Parallel with: C.10, I.2–I.5.

### WP-I.2 Tabular encoders: CSV (3 dialects) and XLSX - `I`

Goal: CSV-1..4, XLSX-1..3, XC-6 for tabular, XC-8.
Files owned: `asd-export/src/tabular/{mod,columns,csv,xlsx}.rs`,
`asd-export/tests/{csv,xlsx}.rs`, `asd-export/tests/golden/tabular/*.csv` (CSV goldens are
text; XLSX is asserted by reading it back with `calamine` as a dev-dependency, MIT, so no
binary golden is committed).
Tests first: formula-injection table (`=1+1`, `+x`, `-x` text vs `-1.5` number, `@x`,
leading TAB/CR); BOM present only for `csv_excel`; CRLF only for `csv_excel`; org time zone
`Asia/Kathmandu` renders `+05:45` offset column; absent values empty; `export_id` first
column on every row; ZIP manifest hashes match file bytes; XLSX dates are date cells, text
cells never formulas, README sheet contains the watermark and the XC-6 sentence; 200 001
rows → `ExportTooLarge`; byte-identical output for identical input.
Depends on: I.1. Parallel with: C.10, C.11, I.3, I.4, I.5.

### WP-I.3 FHIR R4 clinical report bundle - `S`

Goal: FHX-1..8, XC-7 verification of the document and M-CHAT codes, parity with iOS ids.
Files owned: `asd-export/src/fhir/clinical.rs` (+ `fhir/mod.rs` additions; B.8's session
bundle code is reused, not copied), `asd-export/src/codes.rs` (verification flags and
sources for the codes this package uses), `asd-export/tests/{fhir_clinical,fhir_parity}.rs`,
`asd-export/tests/golden/fhir/*.json`.
Tests first: v5 UUID function reproduces every `fullUrl` in
`apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json` from its inputs; the gaze
Observation inside the clinical bundle equals the parity bundle's Observation
byte-for-byte after JCS; DiagnosticReport status table (FHX-4); de-identified bundle has no
Practitioner, no MRN, no `conclusion`; identified bundle Patient has MR identifier and no
`name`/`birthDate`/`telecom`/`address`; every reference resolves; DocumentReference id is
deterministic and its `format` is the verified IHE PDF code; `attachment.hash` is SHA-1
base64 of the PDF bytes; narrative contains the XC-6 sentence; `transaction` variant uses
`PUT`/conditional `ifNoneExist` on identifiers.
Depends on: I.1 (and B.8's session bundle). Parallel with: C.10, C.11, I.2, I.4, I.5.

### WP-I.4 HL7 v2.5.1 ORU^R01 encoder - `I`

Goal: HL7-1..7.
Files owned: `asd-export/src/hl7v2/{mod,escape,segments}.rs`, `asd-export/tests/hl7v2.rs`,
`asd-export/tests/golden/hl7v2/*.hl7` (text, CR separators; add a `.gitattributes` line
`rust/crates/asd-export/tests/golden/hl7v2/*.hl7 -text` so git does not rewrite CRs).
Tests first: escape table for all five delimiters, backslash, CR/LF → `\.br\`, non-ASCII
Devanagari passes through as UTF-8; MSH field count and `MSH-2 = ^~\&`; `TS` with `+0545`
and `+0530`; `OBX-11`/`OBR-25` `P`/`F`/`C` by review state; absent metrics produce no OBX;
`code_map` overrides OBX-3; `pid5_mode` empty vs token; refuses to encode without MRN;
`embed_pdf` adds one ED OBX with base64 that decodes to the input bytes; NTE carries
watermark and XC-6 sentence; golden messages for: unsigned, final, amended, de-identified
refused (HL7 is identified-only).
Depends on: I.1. Parallel with: C.10, C.11, I.2, I.3, I.5.

### WP-I.5 PDF/A clinical summary and letter renderer - `S`

Goal: PDF-1..6, NOTE-4 layout, XC-6 for PDF.
Files owned: `asd-export/src/pdf/{mod,summary,letter,layout,fonts}.rs`,
`asd-export/assets/fonts/` (Noto Sans Regular/Bold, Noto Sans Devanagari Regular, each with
its `OFL.txt`; total < 1.5 MB; blocked on owner Q3, use the same files B.8 uses if it already
embeds a font), `asd-export/tests/pdf.rs`, `NOTICE.md` (font credit line).
Tests first: PDF/A-2b identification in XMP (`pdfaid:part=2`, `conformance=B`), every font
embedded (walk font dictionaries), no transparency groups or encryption; text extraction
(dev-dependency `lopdf` or `pdf-extract`, MIT/Apache) finds: watermark footer on every
page, both mandatory sentences, `PRELIMINARY - NOT SIGNED` for unsigned input, signer name
and registration for signed input, the Devanagari test string `नेपाल परीक्षण` round-trips;
A4 vs Letter media box; multi-page clip table repeats its header; identical extracted text
for identical input. No PDF bytes are committed (`*.pdf` is git-ignored, G14).
Optional (not a gate): veraPDF in a manual CI job.
Depends on: I.1 (B.8 engine choice). Parallel with: C.10, C.11, I.2, I.3, I.4.

## Wave 3 (parallel)

### WP-C.12 Letters and clinical summary endpoints (backend) - `I`

Goal: NOTE-3, NOTE-4, NOTE-5, EXJ-7 (document exports).
Files owned: `asd-backend/src/routes/{letters,clinical_summary}.rs` (new),
`asd-backend/src/export_build/record.rs` (new: builds `ExportRecord` from the DB; I.6 reuses
it), `asd-backend/src/routes/mod.rs` (two lines), `asd-backend/tests/{letters,clinical_summary}.rs`.
Note: EXJ-7 needs the `exports` table. If WP-I.6's migration 0009 is not merged yet, C.12
waits for it or ships with the document-export row behind a function I.6 fills. Preferred
order: merge I.6's migration first (it is the first commit of I.6).
Tests first: draft/patch/finalize with step-up and hash; finalized letter immutable;
PDF endpoints return `application/pdf` with `Cache-Control: no-store`; impression and MRN
absent from the PDF text for a caller without `ClinicalContentRead`/`PatientRefRead`;
each render creates one `exports` row (`kind = document`) and one audit row; the export id
printed in the footer equals that row's id.
Depends on: C.10, C.11, I.5, I.6 (migration). Parallel with: I.6 (code), C.13.

### WP-I.6 Export jobs, artifacts, download URLs (backend) - `S`

Goal: EXJ-1..9, MIG-3, CTR-1 (`export.rs`), CTR-2 (export slugs); runner changes (G5).
Files owned: `asd-backend/migrations/0009_exports.sql`,
`asd-backend/src/jobs/{runner,mod,export}.rs` (`run_after`, heartbeat, `JobContext {pool,
blobs, keyring, config}`, export handler with phase split: snapshot tx → render/upload →
record tx), `asd-backend/src/blob/{mod,memory,s3}.rs` (`presign_get`, `delete`,
`put_object_large`), `asd-backend/src/routes/exports.rs`,
`asd-backend/src/export_build/{mod,filters}.rs` (reuses C.12's `record.rs` if merged;
otherwise creates it and C.12 rebases), `asd-backend/src/main.rs` (worker gets the
context), `asd-backend/src/routes/mod.rs` (one line), `asd-contract/src/export.rs`,
`asd-contract/src/{lib,schema,problem}.rs`, `docs/spec/schemas/asd/api-v1.json`,
`asd-backend/tests/{exports,export_jobs,runner_run_after,blobs_get}.rs`.
Must not touch: `asd-export` (consume it; format gaps go back to I.2–I.5 owners).
Tests first: request validation (identified + research → 400 `consent-purpose-mismatch`;
identified without `ExportIdentified` → 403; HL7 without MRN mode → 409); withdrawn
participant excluded and counted; research purpose includes only `normCollection.consented`
sessions; de-identified CSV contains no token, MRN or session id from the DB; job is
idempotent (run twice → one artifact, same sha256); artifact SHA-256 matches; heartbeat
prevents reclaim of a long job (simulate with a short visibility timeout); `run_after` in
the future is not claimed; download-url audited, 5-minute expiry, other org 404, expired
410; proxy-mode token rejects tampering and expiry; sweep expires artifacts and keeps
`export_items`; norm-aggregate jobs still pass `jobs_norms.rs` unchanged.
Depends on: I.1, I.2 (first format), C.9. FHIR/HL7/PDF formats plug into the dispatch as
I.3/I.4/I.5 merge (each is one match arm + one test, done by the main session or a small
follow-up). Parallel with: C.12 (after migration commit), C.13.

### WP-C.13 Console: worklist and clinical review tab - `I`

Goal: UI-1 (worklist, review tab, clinical settings page), UI-3, UI-4, UI-5 for these
screens; mock data for every new endpoint.
Files owned: `apps/curv_console/src/app/[org]/worklist/**`,
`apps/curv_console/src/app/[org]/settings/clinical/**`,
`apps/curv_console/src/components/clinical/{ReviewPanel,SignDialog,CosignDialog,AmendDialog,WorklistTable}.tsx`,
`apps/curv_console/src/lib/api/clinical.ts`, `src/lib/api/mock/clinical.ts`,
`src/messages/clinical.ts`; minimal lines in `src/lib/api/{index,types,live,mock}.ts`,
`src/messages/en.ts`, `src/app/[org]/layout.tsx` (nav), `src/components/SessionTabs.tsx`
(one tab), `src/app/[org]/sessions/[id]/page.tsx`;
tests `apps/curv_console/tests/{clinical-api,worklist,sign-dialog}.test.ts(x)`.
Tests first: zod schemas accept the `api-v1.json` examples for every clinical DTO
(contract test); sign dialog never puts the password in component state after submit and
posts it only through the server action; content-hash prefix shown; 409 `content-changed`
shows a reload prompt; role-based nav (viewer sees no Worklist); no MRN or impression in
any `href` (render and scan).
Depends on: C.2 merged, C.10 (contract types; can start against mocks once C.9's DTOs are
merged). Parallel with: C.12, I.6.

## Wave 4 (parallel)

### WP-I.7 Delivery channels and integration settings (backend) - `S`

Goal: INTG-1..5, DLV-1..6; fill `interop::on_review_finalized`.
Files owned: `asd-backend/src/interop/{mod,outbox,webhook,sftp,netguard,ssh_keys}.rs`,
`asd-backend/src/routes/integrations.rs`, `asd-backend/src/jobs/deliver.rs` (+ one match
arm in `runner.rs`), `asd-backend/Cargo.toml` (`reqwest` with `rustls` and
`rustls-platform-verifier` or native roots, no `webpki-roots`; `russh`, `russh-sftp`,
`ssh-key`), `asd-backend/src/routes/mod.rs` (one line), `asd-contract/src/export.rs`
(integration DTOs, if C.9/I.6 did not add them), `asd-backend/tests/{integrations,webhook,sftp,netguard,auto_delivery}.rs`.
Tests first: netguard table (loopback, 10/8, 172.16/12, 192.168/16, 169.254.169.254,
100.64/10, `::1`, `fc00::/7`, `fe80::/10`, multicast; DNS that resolves to a private
address; redirect not followed); webhook signature verifies with a reference
implementation of the Standard Webhooks scheme (vectors in the test); webhook body has no
MRN, token or measurements; secret write-only (GET shows fingerprint only; audit rows hold
fingerprint only); SFTP against an in-process `russh` server: `.part` then rename, `.sha256`
file, host-key mismatch fails without upload; backoff schedule via `run_after`;
`on_review_finalized` creates one export + one delivery per production `on_final_sign`
destination, none for `test` destinations, and a delivery failure leaves the signature
`final`; integration test endpoint uses `TEST-0000` and stores no remote body.
Depends on: I.6, C.10. Parallel with: C.14, I.8 (once routes exist).

### WP-C.14 Console: participant workspace, notes, letters, print - `I`

Goal: UI-1 (participant page: timeline, MRN panel, notes, letters, disclosures; letter
editor), UI-2 print and download, UI-4.
Files owned: `apps/curv_console/src/app/[org]/participants/[id]/**`,
`apps/curv_console/src/app/[org]/letters/[id]/**`,
`apps/curv_console/src/app/api/documents/[kind]/[id]/route.ts` (PDF proxy for summary and
letters, pattern of `api/report/[id]`),
`apps/curv_console/src/components/clinical/{Timeline,PatientRefPanel,NotesPanel,LetterEditor}.tsx`,
additions to `src/lib/api/clinical.ts` and `mock/clinical.ts` (C.13 must be merged first),
tests `apps/curv_console/tests/{timeline,patient-ref,notes,letters}.test.tsx`.
Tests first: MRN entered only through a POST form and never echoed into the URL; note
correction flow shows original struck through; timeline paging; letter finalize uses the
step-up dialog from C.13; print button opens the PDF route with `no-store`.
Depends on: C.11, C.12, C.13. Coordinates with WP-C.3 (participants list stays C.3; the
participant detail page is C.14). Parallel with: I.7, I.8.

### WP-I.8 Console: exports and integrations - `I`

Goal: UI-1 (`/[org]/exports`, `/[org]/settings/integrations`), UI-2 downloads.
Files owned: `apps/curv_console/src/app/[org]/exports/**`,
`apps/curv_console/src/app/[org]/settings/integrations/**`,
`apps/curv_console/src/app/api/exports/[id]/download/route.ts`,
`apps/curv_console/src/components/exports/**`, `src/lib/api/exports.ts`,
`src/lib/api/mock/exports.ts`, `src/messages/exports.ts`; minimal lines in the shared files
listed in C.13 (C.13 merged first). Tests
`apps/curv_console/tests/{exports,integrations}.test.tsx`.
Tests first: export form requires reason and purpose and disables `identified` for
research; identified option hidden for roles without `ExportIdentified`; download route
requests a URL and redirects without logging it; secret fields are write-only (never
pre-filled, response fingerprint shown); SSH public key displayed for copy; delivery log
shows error class only.
Depends on: I.6, I.7 (routes), C.13 merged (shared files). Parallel with: C.14.

## Later (Phase 4 / backlog)

### WP-I.9 FHIR push via SMART Backend Services - `S`
DLV-7 first half, MIG-4. Files: `asd-backend/src/interop/fhir_push.rs`,
`migrations/0010_smart_clients.sql`, `routes/integrations.rs` (kind `fhir_server`), JWKS
route `/.well-known/jwks.json`. Test against the SMART Health IT sandbox or a HAPI FHIR
container in CI. Depends on I.3, I.7. Relates to WP-B.10 (façade stays backlog).

### WP-I.10 SMART EHR launch of the console - `S`
DLV-7 second half. Console launch route, OAuth with PKCE, patient context → participant by
MRN blind index (KEY-4), session in the console. Depends on I.9, C.11, C.14.

### WP-I.11 HL7 MLLP/TLS sender and ACK handling - `S`
HL7 batch (FHS/BHS) and MLLP framing (`0x0B … 0x1C 0x0D`), TLS, AA/AE/AR ACK mapping into
delivery status. Depends on I.4, I.7. Only when a partner needs real-time HL7.

## Parallelisation waves (summary)

| Wave | Packages in parallel | Tier | Shared registration points |
| --- | --- | --- | --- |
| 0 | WP-B.6, WP-B.8, WP-C.2 (in flight, other agents) | - | - |
| 1 | WP-C.9, WP-I.1 | S, S | `rust/Cargo.toml` members (I.1), `Cargo.lock` |
| 2 | WP-C.10, WP-C.11, WP-I.2, WP-I.3, WP-I.4, WP-I.5 | S, I, I, S, I, S | `routes/mod.rs` (C.10, C.11), `Cargo.lock` |
| 3 | WP-I.6 (migration commit first), WP-C.12, WP-C.13 | S, I, I | `routes/mod.rs`, `api-v1.json` regenerated by main session |
| 4 | WP-I.7, WP-C.14, WP-I.8 | S, I, I | `routes/mod.rs`; console shared files serialised C.13 → C.14 → I.8 |
| later | WP-I.9, WP-I.10, WP-I.11 | S, S, S | - |

Integration order inside each wave: contract changes first (`asd-contract`, then
`asd-cli schema export` once in the main session), then backend, then console.

## Traps for implementers

1. **Superadmin bypass.** `rbac::allows` short-circuits on `Role::Superadmin`. RBAC-3 must
   be implemented inside `allows` (or `authorize`), not per handler, or a later route will
   forget it. Check every existing test that uses a superadmin principal for a clinical
   action.
2. **Org settings leak into the audit log.** `PATCH /orgs/{id}` writes the full settings
   before/after into `audit_log`, which cannot be edited or deleted. Never put a secret,
   MRN or free text in `organizations.settings`.
3. **The job runner holds one transaction per job.** Doing S3 uploads, SFTP or HTTP inside
   `run_handler`'s transaction holds locks and snapshot for minutes. WP-I.6 must split
   handlers into phases and make each phase idempotent. The handler also gets no blob
   store today (`execute(pool, job)`), so add `JobContext`.
4. **15-minute reclaim.** A long export is reclaimed and run twice unless it heartbeats.
   Make export and delivery handlers idempotent anyway (deterministic blob keys, delivery
   row state checks).
5. **Migration numbers.** 0006/0007 belong to in-flight packages. If 0007 is never used,
   leave the gap; do not let anyone add 0007 later. Rebase before generating
   `rust/.sqlx/`, and regenerate it after the rebase.
6. **`api-v1.json` and `Cargo.lock` are regenerated, not merged.** Two packages
   regenerating them in parallel conflict. The main session regenerates once after
   integrating a wave.
7. **JCS for hashes.** Use `asd-contract`'s `jcs.rs` (RFC 8785 number formatting with
   `ryu-js`) for `content_hash`. `serde_json::to_vec` differs on floats and key order, so
   a hash computed that way breaks signature verification across versions.
8. **Signed-content hash includes data the clinician saw.** If the session's clip rows
   could change after ingest (they cannot today), the hash would drift. Keep it that way:
   re-scoring a session must create new rows, never update the scored rows a signature
   covers.
9. **AEAD AAD must include the row id**, so `INSERT ... RETURNING id` first and encrypt
   second, or generate the UUID in Rust before the insert. Do not encrypt with an AAD that
   lacks the id "for now".
10. **HL7 escaping order.** Escape `\` first, then `|`, `^`, `&`, `~`; convert line breaks
    last. Golden files use CR (`\r`) segment terminators; editors and git may turn them
    into LF (add the `.gitattributes` `-text` line).
11. **Time zones.** Nepal is `+05:45`. Never format HL7 `TS` or printed times from UTC with
    a hard-coded offset; use `chrono-tz` (MIT/Apache) with the org's IANA zone. Check that
    `chrono-tz` is acceptable to `cargo deny` before relying on it.
12. **CSV formula injection vs numbers.** Only text cells get the `'` prefix. A negative
    number written as text (`"-1.5"`) would be "fixed" into `'-1.5` and break analysis.
    Route numbers through the numeric formatter.
13. **Excel BOM and Sheets.** Google Sheets shows a BOM as junk in cell A1 on some import
    paths, and Excel needs the BOM to read UTF-8. That is why the two dialects exist. Do
    not "simplify" them into one.
14. **FHIR v5 UUIDs.** iOS hashes `"http://curv.org/fhir/" + name` with SHA-1 in the URL
    namespace and lower-cases the result. Any difference in the name strings
    (`Observation/gaze/{sessionId}`) breaks id parity silently. Test against the iOS
    fixture.
15. **DocumentReference.attachment.hash is SHA-1** (base64) in R4, not SHA-256. Use SHA-256
    for our own manifests and SHA-1 only there.
16. **PDF/A and fonts.** PDF/A forbids non-embedded fonts and needs an output intent (sRGB
    ICC profile, which must also be redistributable). Devanagari needs shaping. Prove
    both with the text-extraction test before building layout. `*.pdf` is git-ignored, so
    never commit golden PDFs.
17. **`BlobStore::put` caps at 50 MB.** XLSX or NDJSON exports can exceed it. Add
    `put_object_large` (S3 multipart) instead of raising the global cap, which also guards
    device uploads.
18. **Outbound TLS roots.** `webpki-roots` (CDLA-Permissive-2.0) fails `cargo deny`. Use
    the platform verifier or native roots. The runtime image (`distroless/cc-debian12`)
    ships CA certificates.
19. **SSRF via DNS rebinding.** Resolve once, check every resolved address, connect to the
    checked address (custom resolver in `reqwest`), and disable redirects. Checking the
    hostname string alone is not enough.
20. **Webhook retries leak nothing.** Store and log only an error class. Never store the
    remote response body or the URL with query parameters in `jobs.error` (it is shown in
    the console and logged at `warn`).
21. **Console shared files.** `types.ts`, `live.ts`, `mock.ts`, `index.ts`, `en.ts`,
    `layout.tsx` and `SessionTabs.tsx` are edited by C.2 (in flight), C.13, C.14 and I.8.
    Keep new code in new modules and serialise the one-line edits (C.13 → C.14 → I.8).
22. **The console uses npm.** `package-lock.json` and `console.yml` use npm. Running
    `pnpm install` in `apps/curv_console` creates a second lockfile.
23. **Claims scanner blind spot.** Rust-rendered strings ("diagnosis", "screening") are
    not scanned by `check_asd_claims.py`. Keep all user-visible export strings in
    `asd-export/src/text.rs` and add a Rust test with the same denylist terms as the
    scanner.
24. **Review notes back-fill.** KEY-7 changes WP-B.3's `review.rs` write path. Keep the
    contract (`CreateReviewRequest.note`) unchanged; only storage changes. SQL cannot
    encrypt (no key in the database), so the back-fill is a CLI command, and the
    plaintext column is dropped by a later migration after every deployment has run it.
