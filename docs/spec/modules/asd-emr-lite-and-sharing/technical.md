# ASD EMR-lite record, share-a-case and clinic deployment - Technical Spec

Decision record: `docs/spec/adr/ADR-0011-emr-lite-share-a-case-and-deployment.md`.
Vision: `docs/product/VISION.md`. Work packages: `work-packages.md` in this folder;
acceptance scenarios: `acceptance.md`. Written 2026-11-02 against `dev` at `fe0ed56`.

Builds on `asd-rust-core-and-backend/technical.md` (AUTH-*, ING-*, OBS-*, DEP-1..3,
KEY-*) and `asd-clinician-and-interop/technical.md` (RBAC-*, KEY-*, MRN-*, CW-*, SIGN-*,
NOTE-*, XC-*, FHX-*, EXJ-*, CTR-*, UI-*). Ids here: **REC** (record), **SHR** (share a
case), **SYNC** (offline-first), **DPL** (deployment), **HIP** (HIPAA-safeguard gaps),
**TRS** (TOTP reset), **AND** (Android bindings and spike). Every MUST is testable; the
proving test or command is named in `work-packages.md` per package.

## Purpose and scope

In: the record data model (encounters, attachments, follow-ups, search, participant
creation and continuity), the case package format and share links, the device sync
hardening and field harness, the one-server deployment with backups and restore drills,
the audit and session-policy gaps against HIPAA technical safeguards, the org-admin TOTP
reset, Android bindings and the capture spike. Out: identity storage (`local_record`,
wave 3, REC-9 only sketches the contract), hospital integrations (deferred), PDF/A fonts
(WP-I.5), letters (WP-C.12 re-scoped), the legal and administrative pack (WP-E.8).

## Verified current state this spec builds on

- `participants` (migration `0001_initial.sql:94-107`): `id, org_id, token, age_bracket_at_enrol,
  language, consent_version, consent_at, mode, created_at, withdrawn_at` (+ `erased_at`,
  0017); unique `(org_id, token)`. Rows are created only by ingest
  (`asd-backend/src/ingest/mod.rs:101-104`, upsert) and by nothing else; the only
  participant routes are `GET /orgs/{id}/participants` (`routes/participants.rs`),
  `GET /participants/{id}` and `/timeline` (`routes/participant_detail.rs`).
- The device token: `GazeSessionRecord.participantToken(enrollment:sessionId:)`
  (`apps/ios_asd_gaze_app/Sources/Models/GazeSessionRecord.swift:215-219`) returns the
  research `participantId` when enrolled, else `child-<sessionId.prefix(8)>`: a new
  participant per session in clinic mode (G1).
- RBAC: 33 actions, 7 roles, `CLINICAL_CONTENT_ACTIONS` denied to `superadmin`
  (`auth/rbac.rs:24-64, 128-140, 149`). `admin` holds no clinical-content action.
- Encryption: `crypto/{aead,keyring,blind_index,rotate}.rs`, AAD `table:column:org:row`;
  encrypted columns listed in ADR-0011 context. Blob store trait `blob/mod.rs` (`put`,
  `put_object_large`, `presign_get`, `delete`; `MAX_BLOB_BYTES` 50 MB).
- Exports: `asd-contract::export::{ExportFormat, ExportRequest, Export, Disclosure}`,
  job `jobs/export.rs` (snapshot tx → render/upload → record tx), `exports`,
  `export_items`, `export_deliveries`; download `routes/exports.rs:466` audits
  `export.download` when the URL is issued; the proxy `file()` handler (482-530) writes no
  audit row (G2). `asd-export`: `deid.rs` (per-export salted HMAC pseudonyms `P-`/`S-`,
  `YYYY-MM` dates, strips MRN/notes/impressions/signers/device id), `policy.rs`
  (purposes; research and QI de-identified only), `watermark.rs`, `fhir/{session,clinical}.rs`,
  `pdf/{summary,diary,letter}.rs` (pdf-writer, built-in fonts), `dataset.rs` (jsonl/parquet
  rows, WP-B.9), `tabular/csv.rs` (ZIP with per-file sha256 manifest).
- Audit: `audit_log` append-only (0002 triggers and revoked grants), writer
  `audit.rs:157-179` inside the handler transaction; audited reads today:
  `session.raw_read`, `participant.care_summary_read`, `patient_ref.read/search`,
  `export.download` (URL issue). Not audited: `GET /sessions/{id}`, `/clips`,
  `/report.pdf`, `/fhir`, `GET /participants/{id}`, `/timeline`,
  `GET /clinical-reviews/{id}`, `GET /notes/{id}`, notes list, worklist, lists,
  `/participants/{id}/disclosures`, the served export file (G2).
- Auth: `auth/password.rs` (Argon2id), `auth/jwt.rs` (900 s), `auth/refresh.rs` (30 d,
  families), `auth/totp.rs`, `auth/totp_policy.rs` (ADR-0010), `clinical/reauth.rs`
  (step-up password + TOTP, 10/min per user), `RateLimiter` in `auth/mod.rs:219`.
  `admin reset-totp` in `admin.rs` is the only TOTP recovery (G3).
- Console cookies (`apps/curv_console/src/lib/auth/cookies.ts:11,25-40`): `httpOnly`,
  `sameSite=lax`, `secure` in production, access cookie = token lifetime, refresh/role/org
  cookies 7 days; no idle timeout (G4). All routes dynamic since WP-C.16 (`connection()`,
  per-request nonce).
- iOS sync (`ResearchKitSyncService.swift`, `SessionSyncClient.swift`, `SyncTransport.swift`):
  queue files under `Application Support/ResearchStudyQueue/` with `.syncmeta` retry
  state (RKS:46-49, 187-209, 613-628), stages `post` → `uploadBlobs` with per-blob `done`
  (RKS:65-115, 742-780), backoff `[1,2,4,8,60]` s and `maxAttempts = 6` duplicated from
  `asd-sync/src/retry.rs:6-20` (RKS:35-43), dead-letter after the sixth transient failure
  (RKS:564-567), quarantine for other 4xx (RKS:553-557), 401/403 re-register once
  (SSC:358-379), every 409 = `Duplicate` = delivered (`asd-sync/src/response.rs:64-72`,
  SSC:391-392, RKS:548-552). Flush triggers: reconnect (RKS:305-315), registration
  (RKS:404-406), pull-to-refresh, debug buttons. Not: enqueue (RKS:356-391), launch
  (`App/AsdGazeApp.swift:63`), foreground, timer. `isOnline` starts `true` (RKS:165). Blob
  PUT = `URLSession.shared.data(for:)` with the whole file in memory, 60 s
  (`SyncTransport.swift:22-28`, SSC:416-425). Test doubles: `Tests/Support/SyncTestDoubles.swift`
  (`StubSyncTransport`, `MockURLProtocol`). `asd-ffi` exposes `sync_backoff_schedule`,
  `sync_next_retry_delay`, `sync_max_attempts`, `sync_classify_response`
  (`asd-ffi/src/lib.rs:536-651`) (G5).
- Backend blobs (`routes/blobs.rs:105-186`): proxy PUT under the device bearer, 50 MB,
  content-type check, whole body stored then key set, re-PUT allowed, no checksum (G6).
- Deployment: `rust/Dockerfile` (distroless, `asd-backend` + `asd-cli`),
  `rust/docker-compose.yml` (postgres, minio, minio-init, migrate, api on `8000:8000`,
  worker, seed, tunnel profiles; owner DB role; no TLS; no console; no backup) (G7);
  `apps/curv_console` has no Dockerfile and `next.config.ts` sets no `output` (G8); no
  `pg_dump`/restore anywhere (G9); `aws_infra/` is the flood CDK (G10).
- Android: `asd-ffi/uniffi.toml` `[bindings.kotlin] package_name = "org.curv.asd.core"`;
  `asd-ffi/Cargo.toml:15` crate-type `["lib","staticlib"]`; `scripts/build_android_aar.sh`
  builds the C++ `curv_capi` (G11).

## 1. Record: participants, encounters, attachments, follow-ups, search (REC)

- **REC-1** `POST /v1/orgs/{id}/participants` (`ParticipantCreate`, new action for
  `owner, admin, clinician`) creates a participant with a server-minted token
  `child-<uuid v4 without dashes, first 20 hex>` (never user-supplied), optional
  `age_bracket_at_enrol`, `language`, `mode = clinic`, `consent_version`/`consent_at`
  (the consent the clinic recorded on paper or in the app). Response is `api_v1::Participant`
  plus `qr_payload = "asd://participant/<token>?org=<org_id>"`. Ingest keeps its upsert
  (ING-*), so a device that posts a session for that token links it to this row.
- **REC-2** Participant search: `POST /v1/orgs/{id}/participants/search` body
  `ParticipantSearchRequest { token_prefix?, mrn?, age_bracket?, last_seen_from?,
  last_seen_to?, has_open_follow_up?, cursor?, limit? }` (`ParticipantRead`; `mrn` needs
  `PatientRefRead` and reuses the blind index, MRN-4 rule: body, never query string).
  Result rows = `Participant` + `sessions_n`, `last_session_at`, `open_encounters_n`,
  `next_follow_up_at`. Identity fields (REC-9) are not searchable before wave 3.
- **REC-3** `encounters(id uuid, org_id, participant_id, kind text CHECK IN
  ('screening_visit','follow_up_visit','phone_call','home_visit','other'), occurred_at
  timestamptz, clinician_id uuid NULL REFERENCES users, reason text NULL (≤ 200, clear:
  it is a category-like short text, no clinical judgement), summary_ct bytea NULL,
  summary_key_id, status text CHECK IN ('open','closed') DEFAULT 'open', follow_up_due_at
  timestamptz NULL, follow_up_done_at timestamptz NULL, row_version int, created_by,
  created_at, updated_at)`. `summary_ct` is the clinician's text (≤ 8 000 chars,
  KEY-1 with AAD `encounters:summary:org:id`). Index `(org_id, participant_id,
  occurred_at desc)` and `(org_id, follow_up_due_at) WHERE follow_up_done_at IS NULL`.
  `sessions.encounter_id uuid NULL` and `clinical_notes.encounter_id uuid NULL` (both
  `REFERENCES encounters ON DELETE SET NULL`). Migration **0024**.
- **REC-4** Routes: `POST /v1/participants/{id}/encounters` (`EncounterWrite`: `owner,
  admin, clinician`; `summary` only with `ClinicalNoteWrite`, else `400
  encounter-summary-forbidden`), `GET /v1/participants/{id}/encounters` (`ParticipantRead`;
  `summary` returned only with `ClinicalContentRead`, otherwise `has_summary: bool`),
  `GET /v1/encounters/{id}`, `PATCH /v1/encounters/{id}` (`If-Match: "<row_version>"`,
  `412 stale-version` as SIGN-3; `closed` encounters accept only `follow_up_done_at` and
  reopening by `owner`/`clinician`), `POST /v1/encounters/{id}/sessions {session_id}`
  and `DELETE …/sessions/{session_id}` link and unlink a session of the same participant
  (`EncounterWrite`), `GET /v1/orgs/{id}/follow-ups?due_before=&assigned=` lists open
  follow-ups (`ParticipantRead`). Every write audits one row (AUTH-5) with ids and
  `has_summary`, never the text.
- **REC-5** Timeline (CW-3) gains entry kinds `encounter` (header: kind, date, clinician
  display name, status; never the summary) and `attachment` (kind, name, size). Session
  entries carry `encounter_id`.
- **REC-6** `attachments(id uuid, org_id, participant_id, encounter_id NULL, kind text
  CHECK IN ('paper_form','referral','report','photo_of_document','other'), file_name
  text (≤ 120, sanitised to `[A-Za-z0-9._-]`), media_type text CHECK IN
  ('application/pdf','image/jpeg','image/png'), byte_size int CHECK (byte_size BETWEEN 1
  AND 10485760), sha256 bytea, blob_key text, dek_key_id, uploaded_by, created_at,
  deleted_at NULL)`. Bytes are encrypted with the org DEK (KEY-1, AAD
  `attachments:blob:org:id`) **before** `BlobStore::put` under
  `attachments/{org_id}/{participant_id}/{attachment_id}`; the server never stores
  plaintext bytes. Migration 0024.
- **REC-7** Attachment routes: `POST /v1/participants/{id}/attachments` (metadata,
  `AttachmentWrite`: `owner, admin, clinician`) answers `201` with `upload: {method:
  PUT, url}` (proxy mode: `PUT /v1/attachments/{id}/content` under the user bearer;
  presigned mode is not offered because the bytes must pass through the encryptor);
  `PUT …/content` streams ≤ 10 MiB, verifies `Content-Digest: sha-256=<base64>` when
  present (`400 digest-mismatch`), encrypts, stores, sets `sha256`; `GET
  /v1/attachments/{id}` (`ClinicalContentRead`; attachments are clinical content) decrypts
  and streams with `Cache-Control: no-store` and audits `attachment.read`; `DELETE`
  (`AttachmentWrite`) sets `deleted_at` and deletes the blob (audited). Erasure (WP-B.9
  job) deletes attachments and blobs of the participant; the hold rule (O-1) applies as
  for notes.
- **REC-8** RBAC additions (one matrix change, RBAC-1 style): `ParticipantCreate`,
  `EncounterWrite`, `AttachmentWrite`, `CaseShare` (`owner, clinician`), `ShareLinkManage`
  (`owner, clinician`), `AuditExport` (`owner, admin`), `UserTotpReset` (`owner, admin`).
  `superadmin` is denied `AttachmentWrite`, `CaseShare`, `ShareLinkManage` and attachment
  reads (they join `CLINICAL_CONTENT_ACTIONS`); it keeps `ParticipantCreate` and
  `EncounterWrite` without summaries, as `admin` does.
- **REC-9** Contract (`asd-contract/src/emr.rs`, registered in `schema.rs`, `$defs` in
  `api-v1.json`): `Encounter`, `EncounterKind`, `EncounterStatus`, `CreateEncounterRequest`,
  `UpdateEncounterRequest`, `EncounterPage`, `LinkSessionRequest`, `FollowUp`,
  `FollowUpPage`, `Attachment`, `AttachmentKind`, `AttachmentMediaType`,
  `CreateAttachmentRequest`, `AttachmentUpload`, `ParticipantSearchRequest`,
  `ParticipantSearchRow`, `ParticipantSearchPage`, `CreateParticipantRequest`,
  `CreatedParticipant` (with `qr_payload`), `ParticipantIdentity { given_name,
  family_name, sex, birth_year_month, guardian_phone }` (type only; no route stores it
  before wave 3), `CaseShareRequest`, `ShareLink`, `CreateShareLinkRequest`,
  `ShareLinkPage`, `ShareLinkOpenRequest`, `ShareLinkOpenResponse`. Problem slugs
  (CTR-2): `encounter-closed`, `encounter-summary-forbidden`, `attachment-too-large`,
  `attachment-type-not-allowed`, `digest-mismatch`, `share-link-expired`,
  `share-link-exhausted`, `share-passcode-invalid`, `case-consent-missing`,
  `identity-disabled`, `totp-reset-forbidden`. All DTO fields that older consoles do not
  send are `#[serde(default)]`.
- **REC-10** Nothing in the record is pre-filled from the attention flag or the score
  (SIGN-2 extended): `reason`, `summary`, `kind` are the staff member's input; the API
  refuses no value but the console offers no "suggest" button and the spec forbids one.
- **REC-11** Device continuity: the app mints the participant token once per child
  profile (`ChildProfile.participantToken`, generated `child-<uuid>` on first use,
  persisted with the profile) and uses it for every session of that profile; a research
  enrolment token still takes precedence. `GazeSessionRecord.participantToken(...)` takes
  the profile token and never derives it from the session id. Existing history rows keep
  their old tokens. (Wave 3, REC-12: scanning the console QR.)
- **REC-12** (wave 3) In clinic mode the capture flow offers "Scan participant" (camera
  QR of REC-1's payload; validated: scheme `asd`, host `participant`, token matches
  `^child-[0-9a-f]{20}$`, `org` equals the device's org or the scan is refused) and
  "Recent on this device"; the chosen token is the session's `participantId`.

## 2. Share a case (SHR)

- **SHR-1** `asd-export::case_package::build(input: &CasePackageInput) -> Result<Vec<u8>,
  ExportError>` renders a ZIP (deflate) with exactly these entries, in this order:
  `manifest.json`, `README.txt`, `fhir/bundle.json`, `report/case-summary.pdf` (only with
  the `pdf` feature; otherwise omitted and listed as `absent` in the manifest),
  `data/sessions.jsonl`, `data/clips.csv`, and in identified mode only `notes/notes.json`
  and `attachments/<attachment_id>-<file_name>`. No other entries, no directories entries,
  fixed timestamps (1980-01-01) so the bytes are deterministic for identical input.
- **SHR-2** `manifest.json` is `CasePackageManifest` (`asd-export/src/case_package/manifest.rs`,
  `schemars`, exported by `asd-cli schema export` as `docs/spec/schemas/asd/case-package-v1.json`
  and checked by `asd-cli schema check`): `schema: "case-package/1.0"`, `package_id` (=
  export id), `created_at`, `generator {crate, version}`, `org {name, timezone}`, `mode ∈
  {deidentified, identified}`, `purpose`, `recipient {label}` (identified: the free text;
  de-identified: `"de-identified recipient"`), `consent {reference, recorded_at,
  recorded_by_display}` (identified only), `participant` (de-identified: `{pseudonym}`;
  identified: `{token, mrn?: {authority, value}}`), `sessions_n`, `date_range {from, to}`
  (`YYYY-MM` in de-identified mode), `files[] {path, media_type, bytes, sha256}` for every
  other entry, `watermark` (the XC-6 line), `not_diagnostic: true`.
- **SHR-3** De-identified mode applies `deid.rs` to every file with **one** salt per
  package, so `P-…`/`S-…` pseudonyms match across `fhir/bundle.json`, `data/sessions.jsonl`
  and `data/clips.csv`; a test greps the whole archive for the token, MRN, session ids,
  signer names, device id, impression and note text and finds none. Identified mode uses
  the participant token and MRN (MRN-6 still forbids the MRN to callers without
  `PatientRefRead`: the backend omits it, the crate renders what it is given).
- **SHR-4** `fhir/bundle.json` is the existing clinical bundle encoder (FHX-*) extended
  to several sessions of one participant: one `Patient`, one `Organization`, one
  `Practitioner` per signer, one `Device` per device, per session one `Observation` set
  and one `DiagnosticReport` where a signed review exists, `Provenance` per signature;
  `Bundle.type = collection` for the package (ADR-0003 D1; the transaction form stays
  for server push). Unverified codes stay dropped (XC-7).
- **SHR-5** `report/case-summary.pdf` is the existing summary renderer applied to the
  case: cover (org, mode, purpose, export id, watermark), a per-session page in date
  order (the clinical summary page as NOTE-5 renders it, `PRELIMINARY - NOT SIGNED` banner
  when unsigned), encounter list (dates, kinds, clinician, summary text in identified
  mode only), both mandatory sentences on every page (XC-6). PDF/A-2b and Devanagari
  arrive with WP-I.5 and change nothing in the package contract.
- **SHR-6** `README.txt` (English; Nepali line added with WP-I.5 fonts is not needed for
  plain text, so a Nepali paragraph is included now) states: what the package is, the
  mode, the two mandatory sentences, the export id and recipient label, how to verify
  files (`sha256sum -c` lines generated from the manifest), that the sender's clinic
  remains the controller and that the recipient must not re-identify de-identified data.
- **SHR-7** Backend: `ExportFormat::case_package` (`asd-contract::export`), request
  fields `case: CaseShareRequest { participant_id, session_ids?: [] (default all),
  recipient: String (10..200, encrypted at rest in `exports.case_recipient_ct`),
  consent_reference: Option<String> (≤ 200, clear: it is an identifier such as a form
  number), consent_recorded_at: Option<ts>, include_attachments: bool }`. Rules: one
  participant per package; identified needs `CaseShare` + `ExportIdentified`, `purpose ∈
  {clinical_care, patient_request}`, and `consent_reference` + `consent_recorded_at`, else
  `400 case-consent-missing`; de-identified needs `CaseShare` + `ExportDeidentified` and
  follows `policy.rs` (research purpose → only `normCollection.consented` sessions, XC-5);
  withdrawn participants refuse (`409 participant-withdrawn`). The job reuses `jobs/export.rs`
  phases; `export_items` gets one row per included session; `exports.excluded` counts
  the rest. Migration **0025** adds `exports.case_recipient_ct`, `case_consent_reference`,
  `case_consent_recorded_at`, `case_participant_id`.
- **SHR-8** Share links: `share_links(id uuid, org_id, export_id, passcode_hash text
  (Argon2id, AUTH-1 params), created_by, created_at, expires_at (≤ created_at + 7 d),
  max_downloads int (1..5), downloads int DEFAULT 0, opens int DEFAULT 0, revoked_at
  NULL, last_opened_at, last_opened_ip inet)`, migration 0025. `POST
  /v1/exports/{id}/share-link` (`ShareLinkManage`, same org, export `ready`, format
  `case_package`) generates a 12-character passcode from `[A-HJ-NP-Z2-9]` (no ambiguous
  glyphs), returns `{link: {id, url, expires_at, max_downloads}, passcode}` **once**;
  the passcode is never stored or returned again. `GET /v1/exports/{id}/share-links`,
  `DELETE /v1/share-links/{id}` (revoke). Audit `share_link.create/revoke` with ids.
- **SHR-9** Public routes (no bearer; the first such routes serving participant data):
  `POST /v1/share/{link_id}/open {passcode}` → `200 {export_id, file_name, bytes,
  expires_at, downloads_left, download_token}` or `404` (unknown, revoked or expired:
  one answer for all, `share-link-expired` only when the link existed and expired) or `401
  share-passcode-invalid`; `GET /v1/share/{link_id}/file?t=<download_token>` streams the
  artifact (`Content-Disposition: attachment`, `Cache-Control: no-store`), increments
  `downloads` atomically (`UPDATE … WHERE downloads < max_downloads RETURNING`, else `410
  share-link-exhausted`). `download_token` = HMAC over `(link_id, passcode_hash, exp 5 min)`
  as EXJ-5's proxy token. Rate limits: 10 opens per link per hour and 30 per IP per hour
  (`429` + `Retry-After`), through the existing `RateLimiter`. Audit `share_link.open`,
  `share_link.open_failed`, `share_link.download` with `ip` (the first time an IP of a
  non-user is in `audit_log`; it is the disclosure record the Privacy Act and 164.528
  ask for). Expired or revoked links delete nothing: the artifact's own expiry (EXJ-4)
  governs. When the export expires, `open` answers `410 export-expired`.
- **SHR-10** Console: `/[org]/participants/[id]` gains a "Share case" action (dialog:
  mode, purpose, sessions, recipient, consent reference + date, attachments toggle;
  submits `ExportRequest` with `case`; shows the export on the existing exports page) and,
  on a ready case export, "Create share link" (shows the URL and the passcode once, with
  copy buttons and the sentence "send the passcode by another channel"). Public page
  `/share/[id]` (outside `[org]`, no session required, same CSP): passcode form (POST
  server action, UI-3 rules), then the download button and the expiry; errors map the
  slugs. The console holds no format or policy logic (UI-2).
- **SHR-11** Erasure (EXJ-8) also revokes share links of exports that include the
  participant and deletes unexpired artifacts; delivered downloads cannot be recalled
  (stated in the dialog).

## 3. Offline-first capture and sync (SYNC)

- **SYNC-1** Capture never depends on the network: starting, calibrating, recording,
  scoring and showing results work with the radio off; the only network-touching step
  before capture is the optional config-bundle refresh, which fails closed to the cached
  or bundled configuration (CFG-*, verified).
- **SYNC-2** Flush triggers: `enqueueSession` schedules a flush immediately; the app
  flushes at launch (after `installAtLaunch`), on `scenePhase == .active`, on reconnect
  (existing), after registration (existing), and a timer flushes when the earliest
  `nextAttemptAt` passes while online. `isOnline` starts as `nil`/unknown and the first
  `NWPathMonitor` update decides; a device that launches online therefore flushes within
  5 s of launch.
- **SYNC-3** Backoff and attempt budget come from Rust: `syncBackoffSchedule()`,
  `syncNextRetryDelay(attempt, retryAfter)`, `syncMaxAttempts()` (`asd-ffi`); the Swift
  constants in `ResearchKitSyncService.swift:35-43` are deleted. A parity test asserts the
  delays the service uses equal the FFI values.
- **SYNC-4** Dead-letter is not final: on each reconnect and at launch, dead-lettered
  items are moved back to the queue once with `attemptCount = 0` and a `deadLetterCycles`
  counter in `.syncmeta`; after 3 cycles the item stays dead-lettered and the clinic UI
  shows "needs attention" with a one-tap retry (existing `retryDeadLetterItems`).
  Quarantine stays manual.
- **SYNC-5** Blob uploads use a background `URLSession` (`URLSessionConfiguration.background`
  with a fixed identifier, `isDiscretionary = false`, `sessionSendsLaunchEvents = true`)
  and `uploadTask(with:fromFile:)` over the queue's encrypted file; completion is handled
  by the app delegate's `handleEventsForBackgroundURLSession` and marks the blob `done`
  in `.syncmeta`. The POST (`/v1/sessions`, 4 MB max) stays a foreground data task. A
  relaunch while an upload is in flight resumes it without re-sending the POST.
- **SYNC-6** `Content-Digest: sha-256=:<base64>:` (RFC 9530) is sent on every blob PUT;
  the backend (`routes/blobs.rs`) verifies it when present and answers `400 digest-mismatch`
  on mismatch; absent headers are accepted (older apps). The attachments PUT (REC-7) uses
  the same code path.
- **SYNC-7** `asd-sync::classify_response` on `409` reads the problem body: slug
  `idempotency-key-reused` or `participant-withdrawn` → `Fatal` (quarantine, item kept
  for inspection, never deleted); body `{duplicate: true}` or no problem body → `Duplicate`
  (delivered). The FFI re-exports the behaviour unchanged (`sync_classify_response`
  takes status, headers and body already; if the body is not passed today, the signature
  gains it in a backward-compatible way by a new function and the old one is kept for one
  release).
- **SYNC-8** Clinic mode shows a sync status line on the dashboard: `n waiting`, `n
  uploading`, `n need attention`, `last synced <time>`; parent mode keeps the existing
  "stuck" affordance. No participant tokens or ids in the status text.
- **SYNC-9** Field harness, iOS: `Tests/Support/FaultInjectingTransport.swift` (a
  `SyncTransport` that scripts per-request outcomes: `drop` (no response, transport
  error after `latency`), `latency(ms)`, `status(code, retryAfter)`, `cutBody(afterBytes)`
  for PUTs, `flapOnline(pattern)` driving the service's online state) and
  `Tests/SyncFieldScenarioTests.swift`: (a) 40 sessions enqueued offline, network flaps
  every 2 s for 2 minutes with 30 % drops and 1–3 s latency → the stub server has exactly
  40 session POSTs accepted (duplicates answered `200 duplicate:true` are allowed but
  counted), 40 blobs complete, 0 dead-letters, 0 quarantines; (b) a blob cut mid-body is
  completed on the next flush without a second POST; (c) a new service instance over the
  same directory (relaunch) finishes what the old one started; (d) `409
  participant-withdrawn` → quarantined, not deleted; (e) `429 Retry-After: 30` → no
  attempt before 30 s. Runs in `scripts/ios_test.sh` (app tests).
- **SYNC-10** Field harness, stack: `scripts/field_sync_test.sh` starts toxiproxy
  (`ghcr.io/shopify/toxiproxy`) in front of the compose API, applies `latency 2000 ±
  1500 ms`, `bandwidth 64 KB/s`, `timeout` on 20 % of connections and `slicer`, then
  posts 50 `asd-cli synth` sessions (each 3 times, concurrently, with the real headers)
  and 50 proxy-mode blob PUTs with `Content-Digest`; passes when `sessions` has exactly 50
  rows, every `frames_blob_key` is set, `audit_log` has 50 `session.ingest`, and no 5xx
  was logged. Not in CI by default (`ASD_FIELD_TEST=1` enables it in `rust.yml` as a
  manual job).

## 4. Deployment: one server, one command (DPL)

- **DPL-1** `deploy/clinic-server/` is the production stack: `docker-compose.yml`
  (services `caddy`, `api`, `worker`, `migrate`, `postgres`, `minio`, `minio-init`,
  `console`, `backup` (profile `backup`, run by a host timer), `tunnel` (profile)),
  `Caddyfile`, `install.sh`, `upgrade.sh`, `backup.sh`, `restore.sh`, `restore-drill.sh`,
  `status.sh`, `.env.example`, `README.md`, `systemd/asd-backup.{service,timer}`,
  `systemd/asd-drill.{service,timer}`, `test/compose_smoke.sh`. It pins image tags by
  `ASD_VERSION` in `.env`. `rust/docker-compose.yml` is unchanged and stays dev/demo.
- **DPL-2** `install.sh` (bash, Debian/Ubuntu, Docker Engine ≥ 25 with the compose
  plugin; refuses to run as a non-root without `sudo`; idempotent): asks for `DOMAIN`,
  `TLS_MODE ∈ {letsencrypt, internal, tunnel}`, `ADMIN_EMAIL`, `ORG_NAME`; generates
  `POSTGRES_PASSWORD`, `ASD_APP_DB_PASSWORD`, MinIO credentials, `ASD_JWT_SIGNING_KEY`,
  `ASD_BUNDLE_SIGNING_KEY` (both via `asd-cli bundle keygen` in the image),
  `ASD_DATA_KEK` (`openssl rand -base64 32`), an `age` key pair for backups (public key
  into `.env`, private key printed on the key card only), writes `.env` with mode `0600`;
  checks that the data directory is on an encrypted filesystem (`lsblk -o TYPE,FSTYPE`
  shows `crypt`, or `/proc/mounts` for a LUKS mapper) and stops with instructions unless
  `--allow-unencrypted-disk`; starts the stack; waits for `/health/ready` through Caddy;
  runs `admin create-org` and `admin create-user --role owner` with a generated password;
  installs the two systemd timers; prints the **key card** once (KEK, age private key,
  owner password, bundle public key) with the instruction to store it off the server, and
  then exits. Re-running with an existing `.env` changes nothing and prints the status.
- **DPL-3** TLS: Caddy terminates for `https://<DOMAIN>` (console) and
  `https://api.<DOMAIN>` (API, device plane), HTTP→HTTPS redirect, TLS ≥ 1.2, HSTS left to
  the apps (OBS-4, C.16). `letsencrypt`: HTTP-01 on 80/443 (DNS-01 documented as a Caddy
  build variant, not shipped). `internal`: Caddy's local CA; `install.sh` prints the root
  certificate path and `README.md` explains the iPad profile install and that
  `url_rules` still needs a DNS name (an `/etc/hosts`-style name on the clinic router or
  Pi-hole; documented). `tunnel`: the cloudflared service with the token from `.env`.
  Postgres and MinIO bind only on the compose network (no `ports:`); the API's
  `ASD_METRICS_BIND_ADDR` listens on the compose network only; `ASD_TRUST_PROXY_HEADERS=true`
  with Caddy's `X-Forwarded-For` (F11); `ASD_CONSOLE_ORIGIN=https://<DOMAIN>`.
- **DPL-4** Least privilege: `api` and `worker` connect as `asd_app` (threat-model F10);
  `migrate` connects as the owner; `install.sh` sets the `asd_app` password after the
  first migration (`ALTER ROLE asd_app PASSWORD`). The smoke test (DPL-9) runs the
  console login, ingest and a clinical note as `asd_app` to prove the grants suffice; any
  missing grant is a migration in WP-B.18's report, not a fallback to the owner role.
- **DPL-5** Console image: `apps/curv_console/Dockerfile` (multi-stage, `node:22-alpine`,
  `npm ci`, `npm run build` with `output: "standalone"`, runtime image copies
  `.next/standalone` + `static` + `public`, non-root user, `CURV_API_MODE=live`,
  `CURV_API_BASE_URL=http://api:8000`, `CURV_COOKIE_SECURE=true`, `NODE_ENV=production`).
  The CSP `prod` Playwright project (C.16) is the proof that the standalone build still
  carries the nonce logic; `postbuild` must still pass.
- **DPL-6** `backup.sh` (runs in the `backup` service image = the backend image plus
  `postgresql-client`, `mc` and `age`, built from `deploy/clinic-server/backup.Dockerfile`):
  `pg_dump -Fc` of `asd` (owner role), `mc mirror` of the MinIO bucket to a temp dir,
  `asd-backend admin erasure-ledger` output appended to `erasure-ledger.jsonl` (B.22),
  `manifest.json` (time, `ASD_VERSION`, row counts of `participants`, `sessions`,
  `audit_log`, max `audit_log.ts`, bucket object count, sha256 of each part), all
  tar'd and encrypted with `age -r <public key>` to
  `/var/lib/asd/backups/asd-<UTC timestamp>.tar.age`; local retention 14 daily + 8 weekly
  (`BACKUP_KEEP_DAILY`, `BACKUP_KEEP_WEEKLY`); optional `rclone copy` to `BACKUP_REMOTE`
  when set (encrypted bytes only). `.env` (secrets) is **not** in the backup; the key
  card is the backup of the keys. The timer runs nightly at 02:30 local; a failed backup
  writes `/var/lib/asd/backups/LAST_FAILED` and `status.sh` shows it.
- **DPL-7** `restore.sh <archive>` (needs the age private key on stdin or a file):
  decrypts, verifies the manifest hashes, stops `api`/`worker`, `pg_restore --clean
  --if-exists` into `asd`, `mc mirror` back into the bucket, runs `migrate` (the backup
  may predate the current version), replays the erasure ledger
  (`asd-backend admin requeue-erasure --ledger erasure-ledger.jsonl`: every participant
  in the ledger whose restored row is not `erased_at IS NOT NULL` gets a new erasure job;
  B.22), starts the stack, prints the row counts versus the manifest. The runbook states
  that data written after the backup is lost and that erasures after it are replayed.
- **DPL-8** `restore-drill.sh` (monthly timer, 03:30 on the first Sunday): restores the
  newest archive into a scratch compose project `asd-drill` (separate volumes, no
  published ports, no Caddy), compares `participants`/`sessions`/`audit_log` counts and
  `max(audit_log.ts)` with the manifest, runs `asd-backend admin verify` (SELECT 1 per
  table, decrypt one `org_data_keys` row with the KEK to prove the key card matches),
  tears the project down, writes `/var/lib/asd/drills/<date>.{PASS|FAIL}` and the log.
  `status.sh` shows the last drill result and age; older than 45 days is a warning.
- **DPL-9** `test/compose_smoke.sh` (CI job `deploy.yml`, ubuntu runner): builds both
  images, runs `install.sh --non-interactive --tls internal --allow-unencrypted-disk`
  with a throwaway domain in `/etc/hosts`, waits for ready, logs in to the console
  through Caddy (`curl` with the cookie jar), posts one synthetic session through the
  device plane, writes a clinical note, runs `backup.sh`, `restore-drill.sh`
  (expects PASS), `upgrade.sh` to the same version (no-op path), and `status.sh`.
  Runs on every change under `deploy/`, `rust/Dockerfile`, `apps/curv_console/Dockerfile`.
- **DPL-10** `upgrade.sh <version>`: pulls the pinned images, runs `backup.sh` first
  (refuses to continue if it fails), `migrate`, restarts `api`/`worker`/`console`, waits
  for ready, prints the version from `/health`. The ADR-0009 note "run
  `encrypt-review-notes` with the previous release" style instructions live in
  `deploy/clinic-server/README.md` "Upgrading" per version.
- **DPL-11** `docs/roadmap_2026q4/02 §2` ADR-B7 and `asd-rust-core-and-backend` DEP-3
  (AWS) are superseded by DPL-1..10; `aws_infra/` is flood-only and gets no `asd/` app
  (ADR-0011 D8). `README.md` of the deploy folder says in one paragraph how to run the
  same stack on a cloud VM (EC2/Hetzner) and that region choice is O-7/O-8.

## 5. HIPAA technical-safeguard gaps (HIP)

- **HIP-1** Read audits: every response that contains clinical content or participant
  detail writes one `audit_log` row before the body is sent, in a short transaction of
  its own (`audit::record_read(pool, …)`); failure to write the row fails the request
  (`500`), never "best effort". Actions and targets: `session.read` (`GET /sessions/{id}`,
  `/clips`, `/report.pdf`, `/fhir`, `/clinical-summary.pdf`; one row per request with
  `after: {"part": "detail"|"clips"|"report"|"fhir"}`), `participant.read` (`GET
  /participants/{id}`), `participant.timeline_read`, `clinical_review.read`, `note.read`
  (`GET /notes/{id}` and one row per list page with the ids read), `disclosures.read`,
  `attachment.read` (REC-7), `export.file_served` (the proxy `file()` handler and the
  share-link download). List endpoints (sessions, participants, worklist) are **not**
  audited per row (they carry no clinical text); the worklist is audited once per page
  as `worklist.read` because it shows dispositions.
- **HIP-2** Automatic logoff (console): idle timeout `CURV_IDLE_TIMEOUT_MIN` (constant
  30; owner O-10) and absolute session length `CURV_SESSION_MAX_HOURS` (constant 12)
  enforced in `proxy.ts` from two cookies set at login (`curv_last_seen`, `curv_login_at`,
  `httpOnly`, same flags as the auth cookies): a request past either limit clears the auth
  cookies and redirects to `/login?reason=idle|expired`; `curv_last_seen` is refreshed on
  every document request. The refresh cookie's `max-age` becomes `min(backend TTL,
  CURV_SESSION_MAX_HOURS)` (the 7-day comment in `cookies.ts:11` is corrected to the real
  30-day backend TTL). The backend is unchanged; a 15-minute access token already limits
  API use after the cookies are gone.
- **HIP-3** Audit export: `POST /v1/orgs/{id}/exports` with `format = audit_csv`
  (`AuditExport`; filters `from`, `to`, `actor_id`, `action_prefix`) renders the org's
  audit rows through `asd-export::tabular` (canonical CSV, formula-escaped) as a normal
  export job with the usual TTL and download audit; it never contains `before`/`after`
  payloads of clinical actions beyond ids and hashes (they never did). The backup (DPL-6)
  is the off-box copy; O-5 decides retention.
- **HIP-4** Blob integrity: SYNC-6.
- **HIP-5** Attachments at rest: REC-6/7 (DEK before blob store).
- **HIP-6** Transmission: DPL-3 (TLS in the stack, nothing published on host ports but
  Caddy).
- **HIP-7** Contingency: DPL-6..8 and B.22; the drill is scheduled, not optional.
- **HIP-8** Share links: SHR-8/9 (passcode, expiry, limits, audit with IP).
- **HIP-9** The public share routes and the console `/share/[id]` page send `Cache-Control:
  no-store`, `X-Robots-Tag: noindex`, and the same CSP; the console's CSP violation
  listener (C.16) runs on the page in e2e.

## 6. Org-admin TOTP reset (TRS)

- **TRS-1** `POST /v1/orgs/{id}/users/{uid}/totp/reset` (`UserTotpReset`: `owner`,
  `admin`; an `admin` calling it on an `owner` gets `403 totp-reset-forbidden`; a user
  cannot reset themselves, `DELETE /users/me/totp` is their path), body `{password,
  totp_code?}` = the caller's step-up (SIGN-5 rules through `clinical/reauth.rs`;
  `401 reauthentication-failed`, audited, no change), plus `reason` (10..200 chars,
  stored in the audit `after`).
- **TRS-2** Effect, one transaction: `users.totp_secret_ct`, `totp_pending_ct` and the
  pending expiry are cleared; `last_totp_step` reset; every refresh-token family of the
  target is revoked (the target's next request after its 15-minute token expires must log
  in again); `totp_required_by` is **not** touched (ADR-0010 trap 15: no new grace); audit
  `user.totp_reset_by_admin` with `target_id`, the reason and nothing else. The shared
  function `auth::totp::reset_for_user(tx, user_id)` is used by this route and by
  `admin reset-totp` so the two cannot drift.
- **TRS-3** `UserSummary`/`UserProfile` already expose `totp_enrolled`; the console users
  page gains a "Reset two-step sign-in" action for `owner`/`admin` with the step-up
  dialog (UI-3) and the sentence that the user will have to enrol again (wave 2, part of
  WP-C.18's console files or a one-day follow-up if C.18 is late).

## 7. Android (AND)

- **AND-1** `asd-ffi` crate-type gains `cdylib`; `scripts/build_asd_android.sh` builds
  `libasd_ffi.so` for `arm64-v8a`, `armeabi-v7a`, `x86_64` with `cargo ndk` (NDK r27+,
  `ANDROID_NDK_HOME`), generates Kotlin bindings with the crate's `uniffi-bindgen`
  (`--language kotlin`, library mode), and writes `build/asd/android/{jniLibs/<abi>/,
  kotlin/org/curv/asd/core/}` (git-ignored, like the Apple output).
- **AND-2** `apps/android_asd_core/` is a Gradle library module (Kotlin, `minSdk 26`,
  AGP current LTS, dependency `net.java.dev.jna:jna:5.x@aar`) that packages the bindings
  and `jniLibs` into an AAR `org.curv.asd:core`. Its JVM unit test (`x86_64` host
  library, `jna.library.path`) calls `coreVersion()`, `contractInfo`, `scoreSessionJson`
  on `apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.8.json` and the golden
  vectors `asd-core` ships, and compares bytes after canonicalisation (FFI-4 for Android).
  `scripts/build_asd_android.sh --test` runs it.
- **AND-3** CI: `rust.yml` gains an `android-bindings` job (ubuntu, `android-actions/setup-android`,
  NDK) that runs AND-2's test; it is required for `asd-ffi` changes.
- **AND-4** Capture spike (WP-A.14; not a product package): `apps/android_asd_gaze_spike/`
  (throw-away app, excluded from release gates) with two capture back-ends (ARCore
  Augmented Faces; MediaPipe Face Landmarker with iris landmarks) writing the Rust
  raw-log format (`asd-core::gaze::raw_log`, via the AND-2 bindings) for the 13-point
  grid and a 60 s free-viewing clip on two devices from the owner's list (one ~USD 120
  class, one ~USD 250 class). Output: `docs/research/android_gaze_spike/<date>.md` with,
  per device and back-end, `asd-cli gaze replay --check` accuracy in degrees (median,
  p90), valid-frame ratio, frame rate, CPU/thermal note, and a go/no-go line against the
  iPad numbers from WP-G.14. The spike writes no product code into `domains/` or
  `rust/`; if a Rust change is needed, it is a separate package.

## Public Rust API sketch (load-bearing names only)

```rust
// asd-contract/src/emr.rs
pub struct Encounter { pub id: Uuid, pub participant_id: Uuid, pub kind: EncounterKind, pub occurred_at: String,
    pub clinician_id: Option<Uuid>, pub reason: Option<String>, pub summary: Option<String>, pub has_summary: bool,
    pub status: EncounterStatus, pub follow_up_due_at: Option<String>, pub follow_up_done_at: Option<String>,
    pub session_ids: Vec<String>, pub row_version: i32, pub created_at: String }
pub struct CaseShareRequest { pub participant_id: Uuid, pub session_ids: Vec<String>, pub recipient: String,
    pub consent_reference: Option<String>, pub consent_recorded_at: Option<String>, pub include_attachments: bool }
pub struct ShareLink { pub id: Uuid, pub export_id: Uuid, pub url: String, pub expires_at: String,
    pub max_downloads: i32, pub downloads: i32, pub opens: i32, pub revoked_at: Option<String> }

// asd-export/src/case_package/mod.rs
pub struct CasePackageInput<'a> { pub record: &'a ExportRecord, pub sessions: &'a [GazeSessionRecord],
    pub encounters: &'a [EncounterText], pub notes: Option<&'a [NoteText]>, pub attachments: &'a [AttachmentBytes],
    pub mode: Mode, pub purpose: Purpose, pub recipient_label: &'a str, pub consent: Option<ConsentRef>,
    pub salt: [u8; 32], pub export_id: Uuid, pub created_at: DateTime<Utc> }
pub fn build(input: &CasePackageInput) -> Result<Vec<u8>, ExportError>;
pub fn manifest_schema() -> schemars::Schema;            // exported as case-package-v1.json

// asd-backend/src/audit.rs
pub async fn record_read(pool: &PgPool, actor: &Actor, org: Uuid, action: &'static str,
    target_type: &'static str, target_id: &str, after: Option<Value>) -> AppResult<()>;

// asd-backend/src/auth/totp.rs
pub async fn reset_for_user(tx: &mut PgConnection, user_id: Uuid) -> AppResult<()>;   // route + admin CLI

// asd-sync/src/response.rs
pub fn classify_response_with_body(status: u16, retry_after: Option<&str>, body: &[u8]) -> Outcome;
```
